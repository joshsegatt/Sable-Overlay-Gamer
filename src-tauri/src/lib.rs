// sable Tauri backend — IPC bridge between React UI and sable-service
//
// All commands go through the service via named pipe for maximum isolation.
// The Tauri process itself runs without elevated privileges.
// Elevated operations are performed by sable-service.exe (separate process).

use sable_core::{AppSettings, OverlayConfig, ServiceRequest, ServiceResponse};
use std::process::Child;
use std::sync::Mutex;
use tauri::{Manager, State};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegSetValueExW,
};

const PIPE_NAME: &str = r"\\.\pipe\sable";

// ─── App State ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub service_available: Mutex<bool>,
    pub overlay_process: Mutex<Option<Child>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(AppSettings::default()),
            service_available: Mutex::new(false),
            overlay_process: Mutex::new(None),
        }
    }
}

// ─── Tauri Entry Point ────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            // Apply start_minimized: hide the window immediately if the user saved that preference.
            let settings_path = get_app_data_dir().join("config").join("settings.json");
            let start_minimized = std::fs::read_to_string(&settings_path)
                .ok()
                .and_then(|s| serde_json::from_str::<sable_core::AppSettings>(&s).ok())
                .map(|s| s.start_minimized)
                .unwrap_or(false);
            if start_minimized {
                if let Some(win) = app.get_webview_window("main") {
                    win.hide().ok();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_ping,
            cmd_get_telemetry,
            cmd_get_system_info,
            cmd_get_games,
            cmd_get_presets,
            cmd_apply_preset,
            cmd_rollback_preset,
            cmd_get_benchmark_sessions,
            cmd_start_benchmark,
            cmd_stop_benchmark,
            cmd_get_bottleneck_report,
            cmd_get_overlay_config,
            cmd_set_overlay_config,
            cmd_get_settings,
            cmd_save_settings,
            cmd_launch_service,
            cmd_set_overlay_visible,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sable application");
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
async fn cmd_ping(state: State<'_, AppState>) -> Result<bool, String> {
    match call_service(ServiceRequest::Ping) {
        Ok(ServiceResponse::Pong) => {
            *state.service_available.lock().unwrap() = true;
            Ok(true)
        }
        _ => {
            *state.service_available.lock().unwrap() = false;
            Ok(false)
        }
    }
}

#[tauri::command]
async fn cmd_get_telemetry() -> Result<serde_json::Value, String> {
    service_call_json(ServiceRequest::GetTelemetry)
}

#[tauri::command]
async fn cmd_get_system_info() -> Result<serde_json::Value, String> {
    service_call_json(ServiceRequest::GetSystemInfo)
}

#[tauri::command]
async fn cmd_get_games() -> Result<serde_json::Value, String> {
    service_call_json(ServiceRequest::GetGames)
}

#[tauri::command]
async fn cmd_get_presets() -> Result<serde_json::Value, String> {
    service_call_json(ServiceRequest::GetPresets)
}

#[tauri::command]
async fn cmd_apply_preset(preset_id: String) -> Result<serde_json::Value, String> {
    let id = parse_uuid(&preset_id)?;
    service_call_json(ServiceRequest::ApplyPreset { preset_id: id })
}

#[tauri::command]
async fn cmd_rollback_preset(preset_id: String) -> Result<serde_json::Value, String> {
    let id = parse_uuid(&preset_id)?;
    service_call_json(ServiceRequest::RollbackPreset { preset_id: id })
}

#[tauri::command]
async fn cmd_get_benchmark_sessions() -> Result<serde_json::Value, String> {
    service_call_json(ServiceRequest::GetBenchmarkSessions)
}

#[tauri::command]
async fn cmd_start_benchmark(game_id: String) -> Result<serde_json::Value, String> {
    let id = parse_uuid(&game_id)?;
    service_call_json(ServiceRequest::StartBenchmark { game_id: id })
}

#[tauri::command]
async fn cmd_stop_benchmark() -> Result<serde_json::Value, String> {
    service_call_json(ServiceRequest::StopBenchmark)
}

#[tauri::command]
async fn cmd_get_bottleneck_report(session_id: String) -> Result<serde_json::Value, String> {
    let id = parse_uuid(&session_id)?;
    service_call_json(ServiceRequest::GetBottleneckReport { session_id: id })
}

#[tauri::command]
async fn cmd_get_overlay_config() -> Result<serde_json::Value, String> {
    service_call_json(ServiceRequest::GetOverlayConfig)
}

#[tauri::command]
async fn cmd_set_overlay_config(config: serde_json::Value) -> Result<serde_json::Value, String> {
    let cfg: OverlayConfig =
        serde_json::from_value(config).map_err(|e| format!("Invalid config: {e}"))?;
    service_call_json(ServiceRequest::SetOverlayConfig(cfg))
}

#[tauri::command]
async fn cmd_get_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap().clone();
    serde_json::to_value(settings).map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_save_settings(
    settings: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let new_settings: AppSettings =
        serde_json::from_value(settings).map_err(|e| format!("Invalid settings: {e}"))?;
    // Apply launch_on_startup registry change before persisting
    if let Err(e) = set_launch_on_startup(new_settings.launch_on_startup) {
        tracing::warn!("Failed to set launch_on_startup: {e}");
    }
    *state.settings.lock().unwrap() = new_settings.clone();
    save_settings_to_disk(&new_settings).map_err(|e| e.to_string())
}

// ─── IPC Helpers ──────────────────────────────────────────────────────────────

fn service_call_json(req: ServiceRequest) -> Result<serde_json::Value, String> {
    match call_service(req) {
        Ok(response) => {
            if let ServiceResponse::Error(msg) = &response {
                return Err(msg.clone());
            }
            serde_json::to_value(response).map_err(|e| e.to_string())
        }
        Err(_) => {
            // Service offline — return a typed rejection so the frontend safeInvoke
            // falls back to its typed empty value instead of casting {offline:true}.
            Err("Service offline — start sable-service.exe with administrator privileges".to_string())
        }
    }
}

fn call_service(req: ServiceRequest) -> anyhow::Result<ServiceResponse> {
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Storage::FileSystem::*;

    let pipe_name: Vec<u16> = format!("{}\0", PIPE_NAME).encode_utf16().collect();

    let pipe = unsafe {
        CreateFileW(
            windows::core::PCWSTR(pipe_name.as_ptr()),
            GENERIC_READ.0 | 0x40000000u32,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
        .map_err(|e| anyhow::anyhow!("Pipe connect failed: {e}"))?
    };

    if pipe.is_invalid() {
        anyhow::bail!("sable-service is not running");
    }

    let encoded = bincode::serialize(&req)?;
    let len = (encoded.len() as u32).to_le_bytes();

    unsafe {
        WriteFile(pipe, Some(&len), None, None)?;
        WriteFile(pipe, Some(&encoded), None, None)?;
    }

    let mut len_buf = [0u8; 4];
    let mut bytes_read = 0u32;
    unsafe {
        ReadFile(pipe, Some(&mut len_buf), Some(&mut bytes_read), None)?;
    }

    let msg_len = u32::from_le_bytes(len_buf) as usize;
    anyhow::ensure!(msg_len > 0 && msg_len <= 1_048_576, "Invalid message length");

    let mut msg_buf = vec![0u8; msg_len];
    let mut bytes_read2 = 0u32;
    unsafe {
        ReadFile(pipe, Some(&mut msg_buf), Some(&mut bytes_read2), None)?;
        let _ = windows::Win32::Foundation::CloseHandle(pipe).ok();
    }

    let response: ServiceResponse = bincode::deserialize(&msg_buf)?;
    Ok(response)
}

fn parse_uuid(s: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(s).map_err(|e| format!("Invalid UUID '{s}': {e}"))
}

fn save_settings_to_disk(settings: &AppSettings) -> anyhow::Result<()> {
    let dir = get_app_data_dir().join("config");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("settings.json");
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, content)?;
    Ok(())
}

fn get_app_data_dir() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("APPDATA") {
        std::path::PathBuf::from(p).join("Sable")
    } else {
        std::path::PathBuf::from(r"C:\Users\Default\AppData\Roaming\Sable")
    }
}

/// Write or delete the Sable entry in HKCU Run to control startup with Windows.
fn set_launch_on_startup(enabled: bool) -> anyhow::Result<()> {
    use windows::core::PCWSTR;

    let run_key: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "Sable\0".encode_utf16().collect();
    let mut hkey = HKEY::default();

    unsafe {
        let open_err = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(run_key.as_ptr()),
            Some(0),
            KEY_WRITE,
            &mut hkey,
        );
        if open_err != windows::Win32::Foundation::ERROR_SUCCESS {
            anyhow::bail!("Failed to open Run registry key: {:?}", open_err);
        }

        if enabled {
            let exe_path = std::env::current_exe()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let exe_wide: Vec<u16> = exe_path.encode_utf16().chain(Some(0)).collect();
            let _ = RegSetValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    exe_wide.as_ptr() as *const u8,
                    exe_wide.len() * 2,
                )),
            );
        } else {
            // Ignore error if the value doesn't exist
            let _ = RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr()));
        }
        let _ = RegCloseKey(hkey).ok();
    }
    Ok(())
}

// ─── Overlay & Service Management ────────────────────────────────────────────

/// Spawn the overlay binary alongside our exe (enabled=true) or kill it (enabled=false).
#[tauri::command]
async fn cmd_set_overlay_visible(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut proc = state.overlay_process.lock().unwrap();
    if enabled {
            if proc.is_none() {
                let exe_dir = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_default();
                let overlay_exe = exe_dir.join("sable-overlay.exe");
                if !overlay_exe.exists() {
                    return Err("sable-overlay.exe not found next to app binary".to_string());
                }
                match std::process::Command::new(&overlay_exe).spawn() {
                    Ok(child) => { *proc = Some(child); }
                    Err(e) => return Err(format!("Failed to spawn overlay: {e}")),
                }
            }
    } else if let Some(mut child) = proc.take() {
        let _ = child.kill();
    }
    Ok(())
}

/// Request elevation and launch sable-service.exe using ShellExecuteW runas.
#[tauri::command]
async fn cmd_launch_service() -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "Cannot determine exe directory".to_string())?
        .to_path_buf();

    let service_path = exe_dir.join("sable-service.exe");
    if !service_path.exists() {
        return Err("sable-service.exe not found next to app binary".to_string());
    }

    let path_wide: Vec<u16> = service_path
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let verb: Vec<u16> = "runas\0".encode_utf16().collect();

    unsafe {
        let result = ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        // HINSTANCE values > 32 indicate success
        if result.0 as usize <= 32 {
            return Err(format!("ShellExecuteW failed with code {}", result.0 as usize));
        }
    }
    Ok(())
}
