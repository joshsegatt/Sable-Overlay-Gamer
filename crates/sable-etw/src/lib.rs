// sable-etw: PresentMon-style ETW frame time consumer
//
// Subscribes to Microsoft-Windows-DXGI ETW provider.
// Tracks IDXGISwapChain::Present() call timestamps per process.
// Derives: avg FPS, 1% low (p99), frametime avg, frametime variance.
//
// Ring buffer: 3600 frames (~60s at 60fps, ~115KB). Thread-safe read/write.

#![allow(unused_imports, unused_variables, dead_code, unused_must_use, unreachable_code, unused_mut)]

use anyhow::{Context, Result};
use sable_core::FrameMetrics;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info, warn};
use windows::Win32::System::Diagnostics::Etw::*;
use windows::Win32::Foundation::*;
use windows::core::PCWSTR;

const RING_BUFFER_CAPACITY: usize = 3600;

// ─── Frame Ring Buffer ────────────────────────────────────────────────────────

/// Thread-safe ring buffer for per-process frame times.
#[derive(Debug)]
pub struct FrameBuffer {
    frames: VecDeque<f32>,
    capacity: usize,
    last_present_ts: Option<Instant>,
    /// Last timestamp from ETW in 100-ns units (FILETIME resolution, no RAW_TIMESTAMP flag).
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

    /// Record a new Present() call. Returns derived frame time in ms.
    pub fn record_present(&mut self, now: Instant) -> f32 {
        let frametime_ms = if let Some(last) = self.last_present_ts {
            let dt = now.duration_since(last);
            dt.as_secs_f32() * 1000.0
        } else {
            16.67 // Assume 60fps on first frame
        };

        self.last_present_ts = Some(now);

        if self.frames.len() == self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(frametime_ms);

        frametime_ms
    }

    /// Record a Present() event using the 100-ns timestamp delivered by ETW
    /// (FILETIME units — no PROCESS_TRACE_MODE_RAW_TIMESTAMP flag).
    pub fn record_present_qpc(&mut self, ts_100ns: i64) -> f32 {
        let frametime_ms = if let Some(last) = self.last_present_qpc {
            let delta = ts_100ns.saturating_sub(last);
            // 100-ns units → ms: divide by 10 000
            (delta as f32 / 10_000.0).clamp(0.1, 2000.0)
        } else {
            16.67 // Assume 60 fps on first event
        };
        self.last_present_qpc = Some(ts_100ns);
        if self.frames.len() == self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(frametime_ms);
        frametime_ms
    }
    pub fn compute_metrics(&self) -> FrameMetrics {
        if self.frames.is_empty() {
            return FrameMetrics::default();
        }

        let mut sorted: Vec<f32> = self.frames.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let count = sorted.len() as f32;
        let avg_frametime = sorted.iter().sum::<f32>() / count;
        let fps_avg = if avg_frametime > 0.0 {
            1000.0 / avg_frametime
        } else {
            0.0
        };

        // p99 frametime = 1% low FPS
        let p99_idx = ((sorted.len() as f32 * 0.99) as usize).min(sorted.len() - 1);
        let p99_frametime = sorted[p99_idx];
        let fps_1pct_low = if p99_frametime > 0.0 {
            1000.0 / p99_frametime
        } else {
            0.0
        };

        // p99.9 = 0.1% low
        let p999_idx = ((sorted.len() as f32 * 0.999) as usize).min(sorted.len() - 1);
        let p999_frametime = sorted[p999_idx];
        let fps_01pct_low = if p999_frametime > 0.0 {
            1000.0 / p999_frametime
        } else {
            0.0
        };

        // Variance / stddev
        let variance = sorted
            .iter()
            .map(|f| (f - avg_frametime).powi(2))
            .sum::<f32>()
            / count;
        let stddev = variance.sqrt();

        // History: last 360 frames for transfer (1/10th of ring buffer)
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
            frametime_stddev_ms: Some(stddev),
            frametime_history: history,
            target_process: None,
        }
    }
}

// ─── ETW Session ──────────────────────────────────────────────────────────────

/// Active ETW monitoring session. Call `start()` to begin consuming events.
pub struct EtwSession {
    /// Per-process frame buffers. Key = process ID.
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

    /// Start ETW consumer thread. Non-blocking — spawns background thread.
    /// Returns error if ETW session cannot be acquired (e.g., conflict).
    pub fn start(&self) -> Result<()> {
        let mut running = self.is_running.lock().unwrap_or_else(|p| p.into_inner());
        if *running {
            return Ok(()); // Already running
        }

        #[cfg(target_os = "windows")]
        {
            let buffers = Arc::clone(&self.buffers);
            let is_running = Arc::clone(&self.is_running);

            std::thread::Builder::new()
                .name("sable-etw-consumer".to_string())
                .spawn(move || {
                    if let Err(e) = run_etw_consumer(buffers, is_running) {
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
        // Terminate the ETW session so ProcessTrace() unblocks in the consumer thread.
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::Diagnostics::Etw::*;
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

    /// Get current FrameMetrics for a specific process (by PID).
    pub fn get_metrics_for_process(&self, pid: u32) -> FrameMetrics {
        let buffers = self.buffers.lock().unwrap_or_else(|p| p.into_inner());
        buffers
            .get(&pid)
            .map(|b| b.compute_metrics())
            .unwrap_or_default()
    }

    /// Get metrics for the most recently active process (most recent Present() event).
    pub fn get_active_metrics(&self) -> FrameMetrics {
        let buffers = self.buffers.lock().unwrap_or_else(|p| p.into_inner());
        // Select the process whose last Present() event arrived most recently.
        // This correctly handles a freshly launched game over a long-running background process.
        buffers
            .values()
            .filter(|b| !b.frames.is_empty())
            .max_by_key(|b| b.last_present_qpc.unwrap_or(0))
            .map(|b| b.compute_metrics())
            .unwrap_or_default()
    }
}

impl Default for EtwSession {
    fn default() -> Self {
        Self::new()
    }
}

// ─── ETW Consumer (Windows-only) ──────────────────────────────────────────────

// DXGI provider: Microsoft-Windows-DXGI {CA11C036-0102-4A2D-A6AD-F03CFED5D3C9}
#[cfg(target_os = "windows")]
const DXGI_PROVIDER_GUID: windows::core::GUID = windows::core::GUID {
    data1: 0xCA11C036,
    data2: 0x0102,
    data3: 0x4A2D,
    data4: [0xA6, 0xAD, 0xF0, 0x3C, 0xFE, 0xD5, 0xD3, 0xC9],
};

#[cfg(target_os = "windows")]
struct EtwCallbackCtx {
    /// Raw pointer to the Arc<Mutex<...>> inner allocation.
    /// Valid for the entire duration of ProcessTrace because the Arc itself lives in
    /// run_etw_consumer which blocks on ProcessTrace.
    buffers: *const std::sync::Mutex<HashMap<u32, FrameBuffer>>,
}
// The pointer is read-only and only dereferenced while the callback holds the mutex lock.
#[cfg(target_os = "windows")]
unsafe impl Send for EtwCallbackCtx {}
#[cfg(target_os = "windows")]
unsafe impl Sync for EtwCallbackCtx {}

/// ETW event record callback — fires once per DXGI Present() entry call.
#[cfg(target_os = "windows")]
unsafe extern "system" fn etw_event_callback(record: *mut windows::Win32::System::Diagnostics::Etw::EVENT_RECORD) {
    if record.is_null() {
        return;
    }
    let r = &*record;

    // Only handle DXGI Present Start events (opcode 1 = ETW Start opcode).
    // We're already subscribed only to this provider, but double-check for safety.
    if r.EventHeader.ProviderId != DXGI_PROVIDER_GUID {
        return;
    }
    if r.EventHeader.EventDescriptor.Opcode != 1 {
        return;
    }

    let ctx_ptr = r.UserContext as *const EtwCallbackCtx;
    if ctx_ptr.is_null() {
        return;
    }
    let ctx = &*ctx_ptr;

    let pid = r.EventHeader.ProcessId;
    let ts_100ns = r.EventHeader.TimeStamp; // i64 in 100-ns units

    if let Ok(mut buffers) = (*ctx.buffers).lock() {
        let buf = buffers
            .entry(pid)
            .or_insert_with(|| FrameBuffer::new(RING_BUFFER_CAPACITY));
        buf.record_present_qpc(ts_100ns);
    }
}

#[cfg(target_os = "windows")]
fn run_etw_consumer(
    buffers: Arc<Mutex<HashMap<u32, FrameBuffer>>>,
    _is_running: Arc<Mutex<bool>>,
) -> Result<()> {
    use windows::Win32::System::Diagnostics::Etw::*;
    use windows::core::{GUID, PWSTR};

    let session_name: Vec<u16> = "SableEtwSession\0".encode_utf16().collect();

    // ── Start the ETW session (producer side) ────────────────────────────────
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
            // Another session with this name may exist — try to stop it and retry.
            warn!("ETW StartTrace failed ({result:?}), stopping any stale session and retrying");
            let _ = ControlTraceW(
                CONTROLTRACE_HANDLE::default(),
                PCWSTR(session_name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
            // Reinitialise the properties buffer after ControlTraceW modifies it.
            props_buf = vec![0u8; props_size];
            let props2 = props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            (*props2).Wnode.BufferSize = props_size as u32;
            (*props2).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*props2).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
            (*props2).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
            let retry = StartTraceW(&mut handle, PCWSTR(session_name.as_ptr()), props2);
            if retry.is_err() {
                warn!("ETW StartTrace retry also failed: {retry:?}");
                return Ok(());
            }
        }
        handle
    };

    unsafe {
        EnableTraceEx2(
            session_handle,
            &DXGI_PROVIDER_GUID,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            4, // TRACE_LEVEL_INFORMATION
            0,
            0,
            0,
            None,
        );
    }

    // ── Open the consumer (real-time, EVENT_RECORD callback) ─────────────────
    // Box the context so its address is stable for the duration of ProcessTrace.
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

    // OpenTraceW returns INVALID_PROCESSTRACE_HANDLE (0xFFFF...FFFF) on failure.
    if trace_handle.Value == u64::MAX {
        warn!("OpenTraceW failed — ETW frame capture unavailable");
        unsafe {
            ControlTraceW(
                session_handle,
                PCWSTR::null(),
                props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES,
                EVENT_TRACE_CONTROL_STOP,
            );
        }
        return Ok(());
    }

    info!("ETW consumer active — recording DXGI Present() events for frame time tracking");

    // ProcessTrace blocks until CloseTrace is called or the session stops.
    // etw_event_callback fires for every matching event on this thread.
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

    // Explicit drop to keep the Box alive through the entire ProcessTrace call.
    drop(ctx);
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_frame_buffer_metrics() {
        let mut buf = FrameBuffer::new(360);
        let start = Instant::now();

        // Simulate 60fps for 1 second (16.67ms per frame)
        for i in 0..60 {
            let ts = start + Duration::from_micros(16667 * (i + 1));
            buf.record_present(ts);
        }

        let m = buf.compute_metrics();
        let fps = m.fps_avg.unwrap();
        assert!(fps > 55.0 && fps < 65.0, "Expected ~60fps, got {fps}");
        assert!(m.frametime_avg_ms.is_some());
        assert!(m.fps_1_percent_low.is_some());
    }

    #[test]
    fn test_frame_buffer_ring_capacity() {
        let mut buf = FrameBuffer::new(10);
        let start = Instant::now();
        for i in 0..20 {
            let ts = start + Duration::from_millis(16 * (i + 1));
            buf.record_present(ts);
        }
        assert_eq!(buf.frames.len(), 10, "Ring buffer should cap at capacity");
    }

    #[test]
    fn test_etw_session_no_panic_on_new() {
        let session = EtwSession::new();
        let m = session.get_active_metrics();
        assert!(m.fps_avg.is_none());
    }
}
