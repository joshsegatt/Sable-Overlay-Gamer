// sable-etw: PresentMon-style ETW frame time consumer
//
// Frame events are PresentMon IDs, not "any DXGI opcode 1".
// StartTrace / EnableTraceEx2 / OpenTrace failures are errors.

mod present_ids;

use anyhow::{bail, Context, Result};
use present_ids::{is_d3d9_present_start, is_dxgi_present_start, DXGI_PRESENT_TEST};
use sable_core::FrameMetrics;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tracing::{info, warn};
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::Diagnostics::Etw::*;

const RING_BUFFER_CAPACITY: usize = 3600;

const NOISE_EXES: &[&str] = &[
    "dwm.exe",
    "explorer.exe",
    "applicationframehost.exe",
    "searchhost.exe",
    "textinputhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "runtimebroker.exe",
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "discord.exe",
    "spotify.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "eadesktop.exe",
    "battle.net.exe",
    "agent.exe",
    "sable.exe",
    "sable-service.exe",
    "sable-overlay.exe",
];

#[derive(Debug)]
pub struct FrameBuffer {
    frames: VecDeque<f32>,
    capacity: usize,
    last_present_ts: Option<Instant>,
    last_present_qpc: Option<i64>,
}

impl FrameBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity),
            capacity,
            last_present_ts: None,
            last_present_qpc: None,
        }
    }

    pub fn record_present(&mut self, now: Instant) -> Option<f32> {
        let last = self.last_present_ts.replace(now)?;
        let frametime_ms = now.duration_since(last).as_secs_f32() * 1000.0;
        self.push_frame(frametime_ms);
        Some(frametime_ms)
    }

    pub fn record_present_qpc(&mut self, ts_100ns: i64) -> Option<f32> {
        let last = self.last_present_qpc.replace(ts_100ns)?;
        let delta = ts_100ns.saturating_sub(last);
        let frametime_ms = (delta as f32 / 10_000.0).clamp(0.1, 2000.0);
        self.push_frame(frametime_ms);
        Some(frametime_ms)
    }

    fn push_frame(&mut self, frametime_ms: f32) {
        if self.frames.len() == self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(frametime_ms);
    }

    pub fn compute_metrics(&self) -> FrameMetrics {
        if self.frames.is_empty() {
            return FrameMetrics::default();
        }
        let mut sorted: Vec<f32> = self.frames.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = sorted.len() as f32;
        let avg_frametime = sorted.iter().sum::<f32>() / count;
        let fps_avg = if avg_frametime > 0.0 {
            1000.0 / avg_frametime
        } else {
            0.0
        };
        let p99_idx = ((sorted.len() as f32 * 0.99) as usize).min(sorted.len() - 1);
        let p99_frametime = sorted[p99_idx];
        let fps_1pct_low = if p99_frametime > 0.0 {
            1000.0 / p99_frametime
        } else {
            0.0
        };
        let p999_idx = ((sorted.len() as f32 * 0.999) as usize).min(sorted.len() - 1);
        let p999_frametime = sorted[p999_idx];
        let fps_01pct_low = if p999_frametime > 0.0 {
            1000.0 / p999_frametime
        } else {
            0.0
        };
        let variance = sorted.iter().map(|f| (f - avg_frametime).powi(2)).sum::<f32>() / count;
        let history_len = 360.min(self.frames.len());
        let history: Vec<f32> = self
            .frames
            .iter()
            .rev()
            .take(history_len)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        FrameMetrics {
            fps_avg: Some(fps_avg),
            fps_1_percent_low: Some(fps_1pct_low),
            fps_0_1_percent_low: Some(fps_01pct_low),
            frametime_avg_ms: Some(avg_frametime),
            frametime_p99_ms: Some(p99_frametime),
            frametime_stddev_ms: Some(variance.sqrt()),
            frametime_history: history,
            target_process: None,
        }
    }
}

pub struct EtwSession {
    pub buffers: Arc<Mutex<HashMap<u32, FrameBuffer>>>,
    is_running: Arc<Mutex<bool>>,
}

impl EtwSession {
    pub fn new() -> Self {
        Self {
            buffers: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn start(&self) -> Result<()> {
        let mut running = self.is_running.lock().unwrap_or_else(|p| p.into_inner());
        if *running {
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            let buffers = Arc::clone(&self.buffers);
            std::thread::Builder::new()
                .name("sable-etw-consumer".to_string())
                .spawn(move || {
                    if let Err(e) = run_etw_consumer(buffers) {
                        warn!("ETW consumer exited with error: {e}");
                    }
                })
                .context("Failed to spawn ETW consumer thread")?;
        }
        *running = true;
        info!("ETW session started");
        Ok(())
    }

    pub fn stop(&self) {
        let mut running = self.is_running.lock().unwrap_or_else(|p| p.into_inner());
        *running = false;
        #[cfg(target_os = "windows")]
        {
            let session_name: Vec<u16> = "SableEtwSession\0".encode_utf16().collect();
            let props_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + 256;
            let mut props_buf = vec![0u8; props_size];
            let props = props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            unsafe {
                (*props).Wnode.BufferSize = props_size as u32;
                (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
                (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
                (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
                let _ = ControlTraceW(
                    CONTROLTRACE_HANDLE::default(),
                    PCWSTR(session_name.as_ptr()),
                    props,
                    EVENT_TRACE_CONTROL_STOP,
                );
            }
        }
    }

    pub fn get_metrics_for_process(&self, pid: u32) -> FrameMetrics {
        let mut buffers = self.buffers.lock().unwrap_or_else(|p| p.into_inner());
        evict_dead(&mut buffers);
        let mut m = buffers
            .get(&pid)
            .map(|b| b.compute_metrics())
            .unwrap_or_default();
        m.target_process = process_image_name(pid);
        m
    }

    pub fn get_active_metrics(&self) -> FrameMetrics {
        let mut buffers = self.buffers.lock().unwrap_or_else(|p| p.into_inner());
        evict_dead(&mut buffers);
        let catalog = catalog_exe_names();
        let fg = foreground_pid();
        let mut scored: Vec<(u32, i64, String, bool, bool)> = Vec::new();
        for (pid, buf) in buffers.iter() {
            if buf.frames.is_empty() {
                continue;
            }
            let name = process_image_name(*pid).unwrap_or_default();
            if name.is_empty() || is_noise(&name) {
                continue;
            }
            scored.push((
                *pid,
                buf.last_present_qpc.unwrap_or(0),
                name.clone(),
                catalog.contains(&name),
                fg == Some(*pid),
            ));
        }
        let pick = pick_target(&scored).or_else(|| {
            scored
                .iter()
                .max_by_key(|(_, ts, _, _, _)| *ts)
                .cloned()
        });
        match pick {
            Some((pid, _, name, _, _)) => {
                let mut m = buffers
                    .get(&pid)
                    .map(|b| b.compute_metrics())
                    .unwrap_or_default();
                m.target_process = Some(name);
                m
            }
            None => FrameMetrics::default(),
        }
    }
}

fn pick_target(scored: &[(u32, i64, String, bool, bool)]) -> Option<(u32, i64, String, bool, bool)> {
    let cataloged: Vec<_> = scored
        .iter()
        .filter(|(_, _, _, in_cat, _)| *in_cat)
        .cloned()
        .collect();
    if cataloged.is_empty() {
        return None;
    }
    cataloged
        .iter()
        .find(|(_, _, _, _, is_fg)| *is_fg)
        .cloned()
        .or_else(|| cataloged.into_iter().max_by_key(|(_, ts, _, _, _)| *ts))
}

fn evict_dead(buffers: &mut HashMap<u32, FrameBuffer>) {
    buffers.retain(|pid, _| pid_alive(*pid));
}

fn is_noise(exe: &str) -> bool {
    NOISE_EXES.iter().any(|n| exe.eq_ignore_ascii_case(n))
}

fn catalog_exe_names() -> &'static HashSet<String> {
    static CATALOG: OnceLock<HashSet<String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        sable_games::detect_all_games()
            .into_iter()
            .filter_map(|g| {
                Path::new(&g.exe_path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_ascii_lowercase())
            })
            .filter(|n| !n.is_empty() && !is_noise(n))
            .collect()
    })
}

fn exe_basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_ascii_lowercase()
}

fn process_image_name(pid: u32) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 260];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
            .is_ok();
            let _ = CloseHandle(handle);
            if !ok || size == 0 {
                return None;
            }
            let raw = String::from_utf16_lossy(&buf[..size as usize]);
            return Some(exe_basename(&raw));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = pid;
        None
    }
}

fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        unsafe {
            match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                Ok(h) => {
                    let _ = CloseHandle(h);
                    true
                }
                Err(_) => false,
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = pid;
        false
    }
}

fn foreground_pid() -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                None
            } else {
                Some(pid)
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    None
}

impl Default for EtwSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
const DXGI_PROVIDER_GUID: windows::core::GUID = windows::core::GUID {
    data1: 0xCA11C036,
    data2: 0x0102,
    data3: 0x4A2D,
    data4: [0xA6, 0xAD, 0xF0, 0x3C, 0xFE, 0xD5, 0xD3, 0xC9],
};

#[cfg(target_os = "windows")]
const D3D9_PROVIDER_GUID: windows::core::GUID = windows::core::GUID {
    data1: 0x783ACA0A,
    data2: 0x790E,
    data3: 0x4D7F,
    data4: [0x84, 0x51, 0xAA, 0x85, 0x05, 0x11, 0xC6, 0xB9],
};

#[cfg(target_os = "windows")]
fn present_payload_is_test(record: &EVENT_RECORD) -> bool {
    let len = record.UserDataLength as usize;
    if record.UserData.is_null() || len < 12 {
        return false;
    }
    unsafe {
        let flags_ptr = (record.UserData as *const u8).add(8) as *const u32;
        (*flags_ptr) & DXGI_PRESENT_TEST != 0
    }
}

#[cfg(target_os = "windows")]
struct EtwCallbackCtx {
    buffers: *const std::sync::Mutex<HashMap<u32, FrameBuffer>>,
}
#[cfg(target_os = "windows")]
unsafe impl Send for EtwCallbackCtx {}
#[cfg(target_os = "windows")]
unsafe impl Sync for EtwCallbackCtx {}

#[cfg(target_os = "windows")]
unsafe extern "system" fn etw_event_callback(record: *mut EVENT_RECORD) {
    if record.is_null() {
        return;
    }
    let r = &*record;
    let provider = r.EventHeader.ProviderId;
    let id = r.EventHeader.EventDescriptor.Id;
    let is_frame = if provider == DXGI_PROVIDER_GUID {
        is_dxgi_present_start(id)
    } else if provider == D3D9_PROVIDER_GUID {
        is_d3d9_present_start(id)
    } else {
        false
    };
    if !is_frame {
        return;
    }
    if present_payload_is_test(r) {
        return;
    }
    let ctx_ptr = r.UserContext as *const EtwCallbackCtx;
    if ctx_ptr.is_null() {
        return;
    }
    let ctx = &*ctx_ptr;
    let pid = r.EventHeader.ProcessId;
    let ts_100ns = r.EventHeader.TimeStamp;
    if let Ok(mut buffers) = (*ctx.buffers).lock() {
        let buf = buffers
            .entry(pid)
            .or_insert_with(|| FrameBuffer::new(RING_BUFFER_CAPACITY));
        let _ = buf.record_present_qpc(ts_100ns);
    }
}

#[cfg(target_os = "windows")]
fn enable_provider(session: CONTROLTRACE_HANDLE, guid: &windows::core::GUID) -> Result<()> {
    let status = unsafe {
        EnableTraceEx2(
            session,
            guid,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            4,
            0,
            0,
            0,
            None,
        )
    };
    if status.is_err() {
        bail!("EnableTraceEx2({guid:?}) failed: {status:?}");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_etw_consumer(buffers: Arc<Mutex<HashMap<u32, FrameBuffer>>>) -> Result<()> {
    use windows::core::PWSTR;

    let session_name: Vec<u16> = "SableEtwSession\0".encode_utf16().collect();
    let props_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + 256;
    let mut props_buf = vec![0u8; props_size];
    let props = props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;

    let session_handle = unsafe {
        (*props).Wnode.BufferSize = props_size as u32;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;

        let mut handle = CONTROLTRACE_HANDLE::default();
        let result = StartTraceW(&mut handle, PCWSTR(session_name.as_ptr()), props);
        if result.is_err() {
            warn!("ETW StartTrace failed ({result:?}), stopping stale session and retrying");
            let _ = ControlTraceW(
                CONTROLTRACE_HANDLE::default(),
                PCWSTR(session_name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
            props_buf = vec![0u8; props_size];
            let props2 = props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            (*props2).Wnode.BufferSize = props_size as u32;
            (*props2).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*props2).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
            (*props2).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
            let retry = StartTraceW(&mut handle, PCWSTR(session_name.as_ptr()), props2);
            if retry.is_err() {
                bail!("StartTraceW failed after retry: {retry:?}");
            }
        }
        handle
    };

    enable_provider(session_handle, &DXGI_PROVIDER_GUID)?;
    enable_provider(session_handle, &D3D9_PROVIDER_GUID)?;

    let ctx = Box::new(EtwCallbackCtx {
        buffers: Arc::as_ptr(&buffers) as *const _,
    });

    let mut logger_name = session_name.clone();
    let mut log_file: EVENT_TRACE_LOGFILEW = unsafe { std::mem::zeroed() };
    log_file.LoggerName = PWSTR(logger_name.as_mut_ptr());
    log_file.Anonymous1 = EVENT_TRACE_LOGFILEW_0 {
        ProcessTraceMode: PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD,
    };
    log_file.Anonymous2 = EVENT_TRACE_LOGFILEW_1 {
        EventRecordCallback: Some(etw_event_callback),
    };
    log_file.Context = &*ctx as *const EtwCallbackCtx as *mut std::ffi::c_void;

    let trace_handle = unsafe { OpenTraceW(&mut log_file) };
    if trace_handle.Value == u64::MAX {
        unsafe {
            ControlTraceW(
                session_handle,
                PCWSTR::null(),
                props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES,
                EVENT_TRACE_CONTROL_STOP,
            );
        }
        bail!("OpenTraceW returned INVALID_PROCESSTRACE_HANDLE");
    }

    info!("ETW consumer active — DXGI 42/55 + D3D9 1");

    let handles = [trace_handle];
    unsafe {
        let _ = ProcessTrace(&handles, None, None);
        CloseTrace(trace_handle);
        ControlTraceW(
            session_handle,
            PCWSTR::null(),
            props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES,
            EVENT_TRACE_CONTROL_STOP,
        );
    }
    drop(ctx);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::present_ids::*;
    use std::time::Duration;

    #[test]
    fn first_present_does_not_invent_a_frame() {
        let mut buf = FrameBuffer::new(360);
        let start = Instant::now();
        assert!(buf.record_present(start).is_none());
        assert!(buf.frames.is_empty());
        let second = buf.record_present(start + Duration::from_micros(16667));
        assert!(second.unwrap() > 15.0 && second.unwrap() < 18.0);
        assert_eq!(buf.frames.len(), 1);
    }

    #[test]
    fn presentmon_frame_ids_only() {
        assert!(is_dxgi_present_start(DXGI_PRESENT_START));
        assert!(is_dxgi_present_start(DXGI_PRESENT_MPO_START));
        assert!(!is_dxgi_present_start(DXGI_SWAPCHAIN_START));
        assert!(!is_dxgi_present_start(DXGI_RESIZEBUFFERS_START));
        assert!(is_d3d9_present_start(D3D9_PRESENT_START));
    }
}
