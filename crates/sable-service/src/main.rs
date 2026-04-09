// sable-service: Windows telemetry aggregation service.
//
// Runs as a Windows Service (or elevated standalone process for dev).
// Aggregates GPU, CPU, and ETW frame data on a fixed cadence.
// Exposes data via named pipe \\.\pipe\sable to the Tauri app.
//
// Design principles:
//   - Minimal allocations in the hot telemetry loop
//   - Named pipe with ONE concurrent client (the Tauri app)
//   - Fixed 1s polling for telemetry (configurable)
//   - All preset/game operations are request-response via the same pipe

#![allow(unused_imports, unused_variables, dead_code, unused_must_use, unreachable_code, unused_mut, unused_assignments)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use sable_core::{
    AppSettings, BenchmarkMetricsSet, BenchmarkSession, BottleneckDiagnosis, BottleneckKind,
    CpuMetrics, FrameMetrics, GameEntry, GpuMetrics, OverlayConfig, Preset, ServiceRequest,
    ServiceResponse, SystemInfo, TelemetrySnapshot,
};
use sable_etw::EtwSession;
use sable_games::detect_all_games;
use sable_gpu::{get_gpu_info, get_gpu_metrics};
use sable_presets::{bundled_presets, PresetEngine};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn};
use uuid::Uuid;
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER,
    PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
    PdhGetFormattedCounterArrayW, PdhGetFormattedCounterValue, PdhOpenQueryW,
};
use windows::Win32::System::Pipes::*;
use windows::Win32::System::Registry::{
    HKEY, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE,
};
use windows::core::PCWSTR;

const PIPE_NAME: &str = r"\\.\pipe\sable";
const TELEMETRY_INTERVAL_MS: u64 = 1000;

// ─── PDH CPU State ────────────────────────────────────────────────────────────

struct PdhCpuState {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

// PDH handles are HANDLE-based integers; access synchronized via Mutex<ServiceState>.
unsafe impl Send for PdhCpuState {}

struct PdhGpuState {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}
unsafe impl Send for PdhGpuState {}

// ─── Shared Service State ─────────────────────────────────────────────────────

struct ServiceState {
    settings: AppSettings,
    games: Vec<GameEntry>,
    presets: Vec<Preset>,
    preset_engine: PresetEngine,
    etw: EtwSession,
    pdh_cpu: Option<PdhCpuState>,
    pdh_gpu: Option<PdhGpuState>,
    benchmark_active: bool,
    benchmark_start_time: Option<Instant>,
    benchmark_start_snapshot: Option<BenchmarkMetricsSet>,
    benchmark_game_id: Option<Uuid>,
    benchmark_game_name: Option<String>,
    sessions: Vec<BenchmarkSession>,
    overlay_config: OverlayConfig,
    last_telemetry: TelemetrySnapshot,
}

impl ServiceState {
    fn new() -> Self {
        let games = detect_all_games();
        let presets = bundled_presets();
        let mut etw = EtwSession::new();
        if let Err(e) = etw.start() {
            warn!("ETW session could not start: {e} — frame metrics unavailable");
        }

        let pdh_cpu = init_pdh_cpu();
        if pdh_cpu.is_none() {
            warn!("PDH CPU counter unavailable — CPU usage will not be reported");
        }

        let pdh_gpu = init_pdh_gpu();
        if pdh_gpu.is_none() {
            warn!("PDH GPU counter unavailable — GPU utilization will not be reported");
        }

        let sessions = load_sessions_from_disk();
        info!("Loaded {} benchmark sessions from disk", sessions.len());

        Self {
            settings: load_settings_from_disk(),
            games,
            presets,
            preset_engine: PresetEngine::new(),
            etw,
            pdh_cpu,
            pdh_gpu,
            benchmark_active: false,
            benchmark_start_time: None,
            benchmark_start_snapshot: None,
            benchmark_game_id: None,
            benchmark_game_name: None,
            sessions,
            overlay_config: OverlayConfig::default(),
            last_telemetry: TelemetrySnapshot::default(),
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("sable=debug,warn")
        .init();

    info!("Sable service starting...");

    let state = Arc::new(Mutex::new(ServiceState::new()));

    // Background telemetry polling thread
    {
        let state = Arc::clone(&state);
        std::thread::Builder::new()
            .name("telemetry-poller".to_string())
            .spawn(move || {
                loop {
                    // Read the configured interval from state without holding the lock during sleep.
                    let interval_ms = {
                        let st = state.lock().unwrap_or_else(|p| p.into_inner());
                        st.settings.telemetry_interval_ms.max(100) as u64
                    };
                    std::thread::sleep(Duration::from_millis(interval_ms));

                    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
                    // PDH must be collected each tick to compute the CPU rate over the interval.
                    let cpu_pct = pdh_collect_and_read(&st.pdh_cpu);
                    let gpu_pct = pdh_collect_gpu_utilization(&st.pdh_gpu);
                    st.last_telemetry = collect_telemetry(&st.etw, cpu_pct, gpu_pct);
                }
            })?;
    }

    // Named pipe server — 4 concurrent instances: Tauri app + overlay + dev tools
    info!("Named pipe server ready: {PIPE_NAME}");
    for _ in 0..4 {
        let state = Arc::clone(&state);
        std::thread::Builder::new()
            .name("pipe-server".into())
            .spawn(move || {
                loop {
                    match serve_one_client(&state) {
                        Ok(()) => info!("Client disconnected"),
                        Err(e) => warn!("Pipe error: {e}"),
                    }
                }
            })?;
    }
    // Main thread keeps the process alive — worker threads handle all clients
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn serve_one_client(state: &Arc<Mutex<ServiceState>>) -> Result<()> {
    let pipe_name: Vec<u16> = format!("{}\0", PIPE_NAME).encode_utf16().collect();

    // Build a DACL that allows SYSTEM, Administrators and Authenticated Users.
    // This prevents sandboxed/low-integrity processes from sending arbitrary
    // ApplyPreset commands to the elevated service.
    let mut maybe_sa: Option<windows::Win32::Security::SECURITY_ATTRIBUTES> = None;
    let mut sd_ptr: windows::Win32::Security::PSECURITY_DESCRIPTOR =
        windows::Win32::Security::PSECURITY_DESCRIPTOR::default();
    let sddl: Vec<u16> =
        "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;AU)\0".encode_utf16().collect();
    unsafe {
        use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION};
        let ok = ConvertStringSecurityDescriptorToSecurityDescriptorW(
            windows::core::PCWSTR(sddl.as_ptr()),
            SDDL_REVISION,
            &mut sd_ptr,
            None,
        ).is_ok();
        if ok && !sd_ptr.is_invalid() {
            maybe_sa = Some(windows::Win32::Security::SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd_ptr.0,
                bInheritHandle: false.into(),
            });
        }
    }
    let sa_ptr: Option<*const windows::Win32::Security::SECURITY_ATTRIBUTES> =
        maybe_sa.as_ref().map(|sa| sa as *const _);

    let pipe = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(pipe_name.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            255,   // max instances — allow Tauri + overlay + debug clients simultaneously
            65536, // out buffer
            65536, // in buffer
            0,     // default timeout
            sa_ptr,
        )
    };

    // Free the security descriptor after the pipe is created
    if !sd_ptr.is_invalid() {
        unsafe { let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(sd_ptr.0))); };
    }

    if pipe.is_invalid() {
        anyhow::bail!("CreateNamedPipe failed");
    }

    // Block until a client connects
    unsafe { ConnectNamedPipe(pipe, None).ok() };

    // Message loop for this client
    loop {
        let mut len_buf = [0u8; 4];
        if read_exact_from_pipe(pipe, &mut len_buf).is_err() {
            break;
        }
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        if msg_len == 0 || msg_len > 1_048_576 {
            break;
        }

        let mut msg_buf = vec![0u8; msg_len];
        if read_exact_from_pipe(pipe, &mut msg_buf).is_err() {
            break;
        }

        let request: ServiceRequest = match bincode::deserialize(&msg_buf) {
            Ok(r) => r,
            Err(e) => {
                warn!("Deserialize error: {e}");
                break;
            }
        };

        let response = handle_request(request, state);
        let encoded = bincode::serialize(&response).unwrap_or_default();
        let len = (encoded.len() as u32).to_le_bytes();

        if write_to_pipe(pipe, &len).is_err() || write_to_pipe(pipe, &encoded).is_err() {
            break;
        }
    }

    unsafe { windows::Win32::Foundation::CloseHandle(pipe) };
    Ok(())
}

fn handle_request(req: ServiceRequest, state: &Arc<Mutex<ServiceState>>) -> ServiceResponse {
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());

    match req {
        ServiceRequest::Ping => ServiceResponse::Pong,

        ServiceRequest::GetTelemetry => {
            ServiceResponse::Telemetry(st.last_telemetry.clone())
        }

        ServiceRequest::GetSystemInfo => {
            let gpu_info = get_gpu_info().ok();
            ServiceResponse::SystemInfo(build_system_info(gpu_info))
        }

        ServiceRequest::GetGames => ServiceResponse::Games(st.games.clone()),

        ServiceRequest::GetPresets => ServiceResponse::Presets(st.presets.clone()),

        ServiceRequest::ApplyPreset { preset_id } => {
            if let Some(preset) = st.presets.iter().find(|p| p.id == preset_id).cloned() {
                match st.preset_engine.apply(&preset) {
                    Ok(()) => {
                        // Mark preset as applied
                        if let Some(p) = st.presets.iter_mut().find(|p| p.id == preset_id) {
                            p.is_applied = true;
                        }
                        ServiceResponse::PresetApplied { preset_id }
                    }
                    Err(e) => ServiceResponse::Error(format!("Apply failed: {e}")),
                }
            } else {
                ServiceResponse::Error(format!("Preset not found: {preset_id}"))
            }
        }

        ServiceRequest::RollbackPreset { preset_id } => {
            match st.preset_engine.rollback(preset_id) {
                Ok(()) => {
                    if let Some(p) = st.presets.iter_mut().find(|p| p.id == preset_id) {
                        p.is_applied = false;
                    }
                    ServiceResponse::PresetRolledBack { preset_id }
                }
                Err(e) => ServiceResponse::Error(format!("Rollback failed: {e}")),
            }
        }

        ServiceRequest::SetOverlayConfig(config) => {
            st.overlay_config = config;
            ServiceResponse::Pong
        }

        ServiceRequest::GetOverlayConfig => {
            ServiceResponse::OverlayConfig(st.overlay_config.clone())
        }

        ServiceRequest::GetBenchmarkSessions => {
            ServiceResponse::BenchmarkSessions(st.sessions.clone())
        }

        ServiceRequest::StartBenchmark { game_id } => {
            let game_name = st.games.iter()
                .find(|g| g.id == game_id)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown game".to_string());
            st.benchmark_active = true;
            st.benchmark_start_time = Some(Instant::now());
            st.benchmark_start_snapshot = Some(metrics_set_from_telemetry(&st.last_telemetry));
            st.benchmark_game_id = Some(game_id);
            st.benchmark_game_name = Some(game_name);
            info!("Benchmark started for game {game_id}");
            ServiceResponse::BenchmarkStarted
        }

        ServiceRequest::StopBenchmark => {
            if !st.benchmark_active {
                return ServiceResponse::BenchmarkStopped;
            }
            let duration_secs = st.benchmark_start_time
                .take()
                .map(|t| t.elapsed().as_secs() as u32)
                .unwrap_or(0);
            let after = metrics_set_from_telemetry(&st.last_telemetry);
            let session = BenchmarkSession {
                id: Uuid::new_v4(),
                game_id: st.benchmark_game_id.take().unwrap_or(Uuid::nil()),
                game_name: st.benchmark_game_name.take().unwrap_or_default(),
                timestamp: chrono::Utc::now(),
                duration_secs,
                preset_applied: None,
                before: st.benchmark_start_snapshot.take(),
                after: Some(after),
                label: None,
                frametime_history: st.last_telemetry.frames.frametime_history.clone(),
            };
            if let Err(e) = save_session_to_disk(&session) {
                warn!("Failed to persist benchmark session: {e}");
            }
            info!("Benchmark stopped — {}s session saved", duration_secs);
            st.sessions.push(session);
            st.benchmark_active = false;
            ServiceResponse::BenchmarkStopped
        }

        ServiceRequest::GetBottleneckReport { session_id } => {
            // Prefer session data for historical analysis; fall back to live telemetry
            let snap = if let Some(session) = st.sessions.iter().find(|s| s.id == session_id) {
                let metrics = session.after.as_ref().or(session.before.as_ref());
                metrics
                    .map(benchmark_metrics_to_snapshot)
                    .unwrap_or_else(|| st.last_telemetry.clone())
            } else {
                st.last_telemetry.clone()
            };
            let diagnoses = diagnose_bottlenecks(&snap);
            ServiceResponse::BottleneckReport(diagnoses)
        }
    }
}

// ─── Telemetry Collection ─────────────────────────────────────────────────────

fn collect_telemetry(etw: &EtwSession, cpu_pct: Option<f32>, gpu_pct: Option<f32>) -> TelemetrySnapshot {
    let mut gpu = get_gpu_metrics();
    // PDH-sourced GPU utilization fills in when vendor SDK returns nothing
    if gpu.gpu_usage_pct.is_none() {
        gpu.gpu_usage_pct = gpu_pct;
    }
    let frames = etw.get_active_metrics();
    let (ram_used, ram_total) = get_ram_usage();
    let cpu = get_cpu_metrics(cpu_pct);

    TelemetrySnapshot {
        timestamp: Some(chrono::Utc::now()),
        gpu,
        cpu,
        frames,
        ram_used_mb: Some(ram_used),
        ram_total_mb: Some(ram_total),
    }
}

fn get_ram_usage() -> (u64, u64) {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::SystemInformation::GlobalMemoryStatusEx;
        use windows::Win32::System::SystemInformation::MEMORYSTATUSEX;

        let mut ms = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if GlobalMemoryStatusEx(&mut ms).is_ok() {
            let total = ms.ullTotalPhys / (1024 * 1024);
            let available = ms.ullAvailPhys / (1024 * 1024);
            return (total - available, total);
        }
    }
    (0, 0)
}

fn get_cpu_metrics(usage_pct: Option<f32>) -> sable_core::CpuMetrics {
    sable_core::CpuMetrics {
        usage_pct,
        temp_c: None, // WMI thermal — V1.1
        frequency_mhz: None,
        core_count: get_cpu_core_count(),
        logical_count: get_cpu_logical_count(),
        name: get_cpu_name(),
    }
}

fn get_cpu_core_count() -> Option<u32> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
        let mut si = SYSTEM_INFO::default();
        GetSystemInfo(&mut si);
        return Some(si.dwNumberOfProcessors);
    }
    None
}

fn get_cpu_logical_count() -> Option<u32> {
    // Use GetLogicalProcessorInformation to count physical cores (RelationProcessorCore)
    // and logical processors (sum of bits in ProcessorMask for each core entry).
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::SystemInformation::*;
        // First call: get required buffer size
        let mut buf_len: u32 = 0;
        let _ = GetLogicalProcessorInformation(
            None,
            &mut buf_len,
        );
        if buf_len == 0 {
            return get_cpu_core_count(); // fallback
        }
        let entry_size = std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
        let count = buf_len as usize / entry_size;
        let mut buf: Vec<SYSTEM_LOGICAL_PROCESSOR_INFORMATION> = vec![
            SYSTEM_LOGICAL_PROCESSOR_INFORMATION::default(); count
        ];
        let mut buf_len2 = buf_len;
        if GetLogicalProcessorInformation(Some(buf.as_mut_ptr()), &mut buf_len2).is_err() {
            return get_cpu_core_count(); // fallback
        }
        let logical_count: u32 = buf.iter()
            .filter(|e| e.Relationship == RelationProcessorCore)
            .map(|e| e.ProcessorMask.count_ones())
            .sum();
        return if logical_count > 0 { Some(logical_count) } else { get_cpu_core_count() };
    }
    None
}

fn get_cpu_name() -> Option<String> {
    use windows::Win32::System::Registry::*;
    let subkey: Vec<u16> =
        "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0\0"
            .encode_utf16()
            .collect();
    let value_name: Vec<u16> = "ProcessorNameString\0".encode_utf16().collect();
    unsafe {
        let mut hkey = HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u16; 256];
        let mut size = (buf.len() * 2) as u32;
        let q = RegQueryValueExW(
            hkey,
            PCWSTR(value_name.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);
        if q != ERROR_SUCCESS {
            return None;
        }
        let chars = (size as usize / 2).saturating_sub(1); // strip null terminator
        let name = String::from_utf16_lossy(&buf[..chars]).trim().to_string();
        if name.is_empty() { None } else { Some(name) }
    }
}

// ─── Bottleneck Detection ─────────────────────────────────────────────────────

fn diagnose_bottlenecks(snap: &TelemetrySnapshot) -> Vec<BottleneckDiagnosis> {
    let mut diagnoses = Vec::new();

    let gpu_usage = snap.gpu.gpu_usage_pct.unwrap_or(0.0);
    let cpu_usage = snap.cpu.usage_pct.unwrap_or(0.0);
    let gpu_temp = snap.gpu.gpu_temp_c.unwrap_or(0.0);
    let vram_used = snap.gpu.vram_used_mb.unwrap_or(0);
    let vram_total = snap.gpu.vram_total_mb.unwrap_or(1).max(1);
    let fps = snap.frames.fps_avg.unwrap_or(0.0);
    let frametime_stddev = snap.frames.frametime_stddev_ms.unwrap_or(0.0);

    // GPU-bound
    if gpu_usage > 95.0 && cpu_usage < 70.0 {
        diagnoses.push(BottleneckDiagnosis {
            kind: BottleneckKind::GpuBound,
            title: "GPU Bottleneck Detected".to_string(),
            cause: "GPU is running at full capacity while CPU has headroom. The rendering workload exceeds what your GPU can process at this resolution and quality settings.".to_string(),
            recommendation: "Consider lowering resolution scale, shadow quality, or ambient occlusion. Enabling DLSS/FSR/XeSS upscaling may significantly increase GPU headroom.".to_string(),
            confidence: 88,
        });
    }

    // CPU-bound
    if cpu_usage > 90.0 && gpu_usage < 80.0 {
        diagnoses.push(BottleneckDiagnosis {
            kind: BottleneckKind::CpuBound,
            title: "CPU Bottleneck Detected".to_string(),
            cause: "CPU is near saturation while GPU has spare capacity. Likely caused by game logic threads, AI simulation, or excessive draw calls.".to_string(),
            recommendation: "Lower NPC density, simulation quality, or view distance. Enabling the High Performance power plan (if not active) may help. Background process throttling can free additional CPU headroom.".to_string(),
            confidence: 82,
        });
    }

    // VRAM pressure
    let vram_pct = (vram_used as f32 / vram_total as f32) * 100.0;
    if vram_pct > 85.0 {
        diagnoses.push(BottleneckDiagnosis {
            kind: BottleneckKind::VramPressure,
            title: "VRAM Pressure".to_string(),
            cause: format!("VRAM is {}% utilized ({} MB / {} MB). High VRAM usage causes texture streaming and can introduce stutters when assets are evicted.", vram_pct.round(), vram_used, vram_total),
            recommendation: "Lower texture quality settings, disable 4K textures if present, or reduce shadow resolution. Using DLSS Quality mode or FSR Quality mode reduces VRAM requirements.".to_string(),
            confidence: 90,
        });
    }

    // Thermal throttle — NVIDIA (>83°C), AMD (>90°C)
    if gpu_temp > 83.0 || snap.gpu.is_thermal_limited == Some(true) {
        diagnoses.push(BottleneckDiagnosis {
            kind: BottleneckKind::ThermalThrottle,
            title: "GPU Thermal Throttling".to_string(),
            cause: format!("GPU temperature is {gpu_temp:.0}°C, which may be causing thermal throttling. The GPU reduces clock speeds to prevent damage, directly lowering performance."),
            recommendation: "Clean GPU heatsink and case fans. Check thermal paste age (>3 years). Improve airflow. Setting a more aggressive fan curve in your GPU vendor software may help.".to_string(),
            confidence: if snap.gpu.is_thermal_limited == Some(true) { 96 } else { 70 },
        });
    }

    // Power limit hit
    if snap.gpu.is_power_limited == Some(true) {
        diagnoses.push(BottleneckDiagnosis {
            kind: BottleneckKind::PowerLimit,
            title: "GPU Power Limit Reached".to_string(),
            cause: "The GPU has hit its configured power limit and is throttling clock speeds. This is common on factory-clocked cards under sustained load.".to_string(),
            recommendation: "Increasing the power limit in your GPU software (MSI Afterburner, NVIDIA Control Panel) may improve sustained performance. Effect varies by card.".to_string(),
            confidence: 92,
        });
    }

    // Low GPU usage anomaly
    if gpu_usage < 30.0 && fps < 90.0 && cpu_usage < 70.0 {
        diagnoses.push(BottleneckDiagnosis {
            kind: BottleneckKind::LowGpuAnomaly,
            title: "Low GPU Utilization Anomaly".to_string(),
            cause: "GPU, CPU, and FPS are all lower than expected simultaneously. This pattern often indicates a power plan issue, driver problem, or API overhead bottleneck.".to_string(),
            recommendation: "Check Windows Power Plan (switch to High Performance). Verify GPU drivers are up to date. Check for GPU/CPU throttling in vendor software. Ensure no external frame limiter is active.".to_string(),
            confidence: 60,
        });
    }

    if diagnoses.is_empty() {
        diagnoses.push(BottleneckDiagnosis {
            kind: BottleneckKind::None,
            title: "No Significant Bottleneck Detected".to_string(),
            cause: "GPU and CPU appear to be well-balanced for the current workload. Performance is likely close to optimal for your hardware configuration.".to_string(),
            recommendation: "Continue monitoring. If you are not hitting your target frame rate, check game quality settings and resolution.".to_string(),
            confidence: 75,
        });
    }

    diagnoses
}

// ─── System Info Builder ──────────────────────────────────────────────────────

fn build_system_info(gpu_info: Option<sable_core::GpuInfo>) -> SystemInfo {
    let (_, ram_total) = get_ram_usage();
    let hags = sable_presets::get_hags_state_pub().ok();
    let (plan_name, plan_guid) = get_current_power_plan_info();
    SystemInfo {
        gpu: gpu_info,
        cpu_name: get_cpu_name(),
        cpu_cores: get_cpu_core_count(),
        cpu_threads: get_cpu_logical_count(),
        ram_total_mb: Some(ram_total),
        os_version: get_os_version(),
        hags_enabled: hags,
        power_plan_name: plan_name,
        power_plan_guid: plan_guid,
    }
}

fn get_os_version() -> Option<String> {
    // Read from HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion
    use windows::Win32::System::Registry::*;
    let subkey: Vec<u16> =
        "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
            .encode_utf16()
            .collect();
    let read_reg_str = |hkey: HKEY, name: &str| -> Option<String> {
        let val: Vec<u16> = format!("{name}\0").encode_utf16().collect();
        let mut buf = vec![0u16; 256];
        let mut size = (buf.len() * 2) as u32;
        let q = unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(val.as_ptr()),
                None,
                None,
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut size),
            )
        };
        if q != ERROR_SUCCESS {
            return None;
        }
        let chars = (size as usize / 2).saturating_sub(1);
        let s = String::from_utf16_lossy(&buf[..chars]).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    };
    unsafe {
        let mut hkey = HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return Some("Windows".to_string());
        }
        let product = read_reg_str(hkey, "ProductName")
            .unwrap_or_else(|| "Windows".to_string());
        let display_ver = read_reg_str(hkey, "DisplayVersion");
        let _ = RegCloseKey(hkey);
        match display_ver {
            Some(v) => Some(format!("{product} {v}")),
            None => Some(product),
        }
    }
}

// ─── Pipe I/O ─────────────────────────────────────────────────────────────────

fn read_exact_from_pipe(pipe: HANDLE, buf: &mut [u8]) -> Result<()> {
    use windows::Win32::Storage::FileSystem::ReadFile;

    let mut total_read = 0usize;
    while total_read < buf.len() {
        let mut bytes_read = 0u32;
        unsafe {
            ReadFile(
                pipe,
                Some(&mut buf[total_read..]),
                Some(&mut bytes_read),
                None,
            )
            .map_err(|e| anyhow::anyhow!("ReadFile failed: {e}"))?;
        }
        if bytes_read == 0 {
            anyhow::bail!("Pipe closed unexpectedly");
        }
        total_read += bytes_read as usize;
    }
    Ok(())
}

fn write_to_pipe(pipe: HANDLE, buf: &[u8]) -> Result<()> {
    use windows::Win32::Storage::FileSystem::WriteFile;

    let mut total_written = 0usize;
    while total_written < buf.len() {
        let mut bytes_written = 0u32;
        unsafe {
            WriteFile(
                pipe,
                Some(&buf[total_written..]),
                Some(&mut bytes_written),
                None,
            )
            .map_err(|e| anyhow::anyhow!("WriteFile failed: {e}"))?;
        }
        total_written += bytes_written as usize;
    }
    Ok(())
}

// ─── PDH CPU Helpers ──────────────────────────────────────────────────────────

fn init_pdh_cpu() -> Option<PdhCpuState> {
    unsafe {
        let mut query = PDH_HQUERY::default();
        let status = PdhOpenQueryW(PCWSTR::null(), 0, &mut query);
        if status != 0 {
            warn!("PdhOpenQueryW failed: 0x{status:x}");
            return None;
        }
        let counter_path: Vec<u16> =
            "\\Processor(_Total)\\% Processor Time\0"
                .encode_utf16()
                .collect();
        let mut counter = PDH_HCOUNTER::default();
        let status =
            PdhAddEnglishCounterW(query, PCWSTR(counter_path.as_ptr()), 0, &mut counter);
        if status != 0 {
            warn!("PdhAddEnglishCounterW failed: 0x{status:x}");
            PdhCloseQuery(query);
            return None;
        }
        // Prime: first collection establishes the baseline; data is valid on the next call.
        PdhCollectQueryData(query);
        info!("PDH CPU counter initialized");
        Some(PdhCpuState { query, counter })
    }
}

fn pdh_collect_and_read(pdh: &Option<PdhCpuState>) -> Option<f32> {
    let state = pdh.as_ref()?;
    unsafe {
        let status = PdhCollectQueryData(state.query);
        if status != 0 {
            return None;
        }
        let mut fmt = std::mem::zeroed::<PDH_FMT_COUNTERVALUE>();
        let status = PdhGetFormattedCounterValue(
            state.counter,
            PDH_FMT_DOUBLE,
            None,
            &mut fmt,
        );
        if status != 0 || fmt.CStatus != 0 {
            return None;
        }
        let pct = (fmt.Anonymous.doubleValue as f32).clamp(0.0, 100.0);
        Some(pct)
    }
}

// ─── Benchmark Helpers ────────────────────────────────────────────────────────

fn metrics_set_from_telemetry(snap: &TelemetrySnapshot) -> BenchmarkMetricsSet {
    BenchmarkMetricsSet {
        fps_avg: snap.frames.fps_avg.unwrap_or(0.0),
        fps_1pct_low: snap.frames.fps_1_percent_low.unwrap_or(0.0),
        frametime_avg_ms: snap.frames.frametime_avg_ms.unwrap_or(0.0),
        frametime_p99_ms: snap.frames.frametime_p99_ms.unwrap_or(0.0),
        gpu_usage_pct: snap.gpu.gpu_usage_pct.unwrap_or(0.0),
        cpu_usage_pct: snap.cpu.usage_pct.unwrap_or(0.0),
        vram_used_mb: snap.gpu.vram_used_mb.unwrap_or(0),
        gpu_temp_c: snap.gpu.gpu_temp_c.unwrap_or(0.0),
    }
}

// ─── Session Persistence ──────────────────────────────────────────────────────

fn get_app_data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("Sable")
}

fn get_sessions_dir() -> PathBuf {
    get_app_data_dir().join("sessions")
}

fn save_session_to_disk(session: &BenchmarkSession) -> anyhow::Result<()> {
    let dir = get_sessions_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", session.id));
    let json = serde_json::to_string_pretty(session)?;
    std::fs::write(path, json)?;
    Ok(())
}

fn load_sessions_from_disk() -> Vec<BenchmarkSession> {
    let dir = get_sessions_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    match serde_json::from_str::<BenchmarkSession>(&data) {
                        Ok(s) => sessions.push(s),
                        Err(e) => warn!("Failed to parse session {:?}: {e}", path),
                    }
                }
            }
        }
    }
    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    sessions
}

// ─── Power Plan Info ──────────────────────────────────────────────────────────

fn get_current_power_plan_info() -> (Option<String>, Option<String>) {
    // Delegate to the sable-presets crate which already implements this
    if let Ok(guid) = sable_presets::get_current_power_plan_guid() {
        let name = sable_presets::get_current_power_plan_name().ok();
        (name, Some(guid))
    } else {
        (None, None)
    }
}

// ─── PDH GPU Engine Helpers ───────────────────────────────────────────────────

fn init_pdh_gpu() -> Option<PdhGpuState> {
    unsafe {
        let mut query = PDH_HQUERY::default();
        let status = PdhOpenQueryW(PCWSTR::null(), 0, &mut query);
        if status != 0 {
            warn!("PdhOpenQueryW (GPU) failed: 0x{status:x}");
            return None;
        }
        // Wildcard instance — returns one entry per process/engine/adapter combination.
        // We filter later for "engtype_3D" to isolate the 3D engine utilization.
        let counter_path: Vec<u16> =
            "\\GPU Engine(*)\\Utilization Percentage\0"
                .encode_utf16()
                .collect();
        let mut counter = PDH_HCOUNTER::default();
        let status =
            PdhAddEnglishCounterW(query, PCWSTR(counter_path.as_ptr()), 0, &mut counter);
        if status != 0 {
            warn!("PdhAddEnglishCounterW (GPU) failed: 0x{status:x} — GPU utilization unavailable");
            PdhCloseQuery(query);
            return None;
        }
        // Prime: first collection establishes the rate baseline.
        PdhCollectQueryData(query);
        info!("PDH GPU engine counter initialized");
        Some(PdhGpuState { query, counter })
    }
}

/// Returns the peak 3D-engine utilization across all processes and adapters (0–100%).
fn pdh_collect_gpu_utilization(state: &Option<PdhGpuState>) -> Option<f32> {
    let s = state.as_ref()?;
    unsafe {
        let status = PdhCollectQueryData(s.query);
        if status != 0 {
            return None;
        }

        // First call: query required buffer size.
        let mut buf_size: u32 = 0;
        let mut item_count: u32 = 0;
        PdhGetFormattedCounterArrayW(
            s.counter,
            PDH_FMT_DOUBLE,
            &mut buf_size,
            &mut item_count,
            None,
        );
        if buf_size == 0 || item_count == 0 {
            return None;
        }

        // Allocate a flat byte buffer; PDH writes the structs + name strings into it.
        let alloc_size = (buf_size as usize + 128).max(2048);
        let layout = std::alloc::Layout::from_size_align(alloc_size, 8).ok()?;
        let raw = std::alloc::alloc_zeroed(layout);
        if raw.is_null() {
            return None;
        }

        // Second call: fill buffer with PDH_FMT_COUNTERVALUE_ITEM_W array.
        let mut final_count: u32 = item_count;
        let status = PdhGetFormattedCounterArrayW(
            s.counter,
            PDH_FMT_DOUBLE,
            &mut buf_size,
            &mut final_count,
            Some(raw as *mut PDH_FMT_COUNTERVALUE_ITEM_W),
        );

        let result = if status == 0 && final_count > 0 {
            let items = std::slice::from_raw_parts(
                raw as *const PDH_FMT_COUNTERVALUE_ITEM_W,
                final_count as usize,
            );
            let mut max_pct: f32 = 0.0;
            let mut found = false;
            for item in items {
                // szName.0 is *mut u16; read as a null-terminated wide string.
                let name_ptr = item.szName.0 as *const u16;
                if name_ptr.is_null() {
                    continue;
                }
                let mut len = 0usize;
                while *name_ptr.add(len) != 0 {
                    len += 1;
                    if len > 512 { break; } // safety cap
                }
                let name = String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, len));
                if name.contains("engtype_3D") && item.FmtValue.CStatus == 0 {
                    let pct = item.FmtValue.Anonymous.doubleValue as f32;
                    if pct > max_pct {
                        max_pct = pct;
                    }
                    found = true;
                }
            }
            if found { Some(max_pct.clamp(0.0, 100.0)) } else { None }
        } else {
            None
        };

        std::alloc::dealloc(raw, layout);
        result
    }
}

// ─── Settings Persistence ─────────────────────────────────────────────────────

fn get_settings_path() -> PathBuf {
    get_app_data_dir().join("config").join("settings.json")
}

fn load_settings_from_disk() -> AppSettings {
    let path = get_settings_path();
    if !path.exists() {
        return AppSettings::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
            warn!("Failed to parse settings.json: {e} — using defaults");
            AppSettings::default()
        }),
        Err(e) => {
            warn!("Failed to read settings.json: {e} — using defaults");
            AppSettings::default()
        }
    }
}

// ─── Benchmark → Telemetry Conversion ────────────────────────────────────────

/// Convert a BenchmarkMetricsSet snapshot into a TelemetrySnapshot for bottleneck analysis.
fn benchmark_metrics_to_snapshot(m: &BenchmarkMetricsSet) -> TelemetrySnapshot {
    TelemetrySnapshot {
        timestamp: None,
        gpu: GpuMetrics {
            gpu_usage_pct: Some(m.gpu_usage_pct),
            gpu_temp_c: Some(m.gpu_temp_c),
            vram_used_mb: Some(m.vram_used_mb),
            ..Default::default()
        },
        cpu: CpuMetrics {
            usage_pct: Some(m.cpu_usage_pct),
            ..Default::default()
        },
        frames: FrameMetrics {
            fps_avg: Some(m.fps_avg),
            fps_1_percent_low: Some(m.fps_1pct_low),
            frametime_avg_ms: Some(m.frametime_avg_ms),
            frametime_p99_ms: Some(m.frametime_p99_ms),
            ..Default::default()
        },
        ram_used_mb: None,
        ram_total_mb: None,
    }
}
