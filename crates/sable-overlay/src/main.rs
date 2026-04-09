// sable-overlay: Ultra-light in-game overlay binary.
//
// Architecture:
//   - Separate exe — spawned by the Tauri app when a game is detected
//   - Reads telemetry from sable-service via named pipe (1Hz poll)
//   - Renders via Win32 layered window + GDI (MVP) / Direct2D (V1)
//   - WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT
//   - Anti-cheat safe: no D3D hook in MVP — uses layered window compositing
//
// Performance budget:
//   - RAM: <5MB private bytes
//   - CPU: <0.3% at 1Hz update

#![allow(unused_imports, unused_variables, dead_code, unused_must_use, unreachable_code, unused_mut, unused_assignments, unused_parens)]
//   - GPU: near zero (GDI composited by DWM, not the game's D3D pipeline)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use sable_core::{OverlayConfig, OverlayPosition, TelemetrySnapshot};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

const PIPE_NAME: &str = r"\\.\pipe\sable";
const UPDATE_INTERVAL_MS: u64 = 1000; // 1Hz — sufficient, keeps CPU near zero
const OVERLAY_WIDTH: i32 = 200;
const OVERLAY_HEIGHT: i32 = 130;
const PADDING: i32 = 8;

// ─── Shared State ─────────────────────────────────────────────────────────────

struct OverlayState {
    telemetry: TelemetrySnapshot,
    config: OverlayConfig,
    visible: bool,
    /// Window handle stored as isize (HWND.0) so OverlayState is Send.
    hwnd: isize,
}

static STATE: std::sync::OnceLock<Arc<Mutex<OverlayState>>> = std::sync::OnceLock::new();

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("sable_overlay=info,warn")
        .init();

    let state = Arc::new(Mutex::new(OverlayState {
        telemetry: TelemetrySnapshot::default(),
        config: OverlayConfig::default(),
        visible: true,
        hwnd: 0,
    }));
    STATE.set(state.clone()).ok();

    // Background thread: poll service for telemetry updates
    {
        let state = Arc::clone(&state);
        std::thread::Builder::new()
            .name("overlay-poll".to_string())
            .spawn(move || {
                let mut prev_pos = OverlayPosition::TopLeft;
                let mut prev_scale: f32 = 1.0;
                loop {
                    std::thread::sleep(Duration::from_millis(UPDATE_INTERVAL_MS));
                    if let Some(snap) = read_telemetry_from_service() {
                        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
                        st.telemetry = snap;
                        // Apply position or scale changes whenever config changes
                        let cur_pos = st.config.position.clone();
                        let cur_scale = st.config.scale;
                        let hwnd_val = st.hwnd;
                        drop(st); // release before calling Win32
                        if hwnd_val != 0 {
                            let hw = HWND(hwnd_val as *mut _);
                            if cur_pos != prev_pos || (cur_scale - prev_scale).abs() > 0.01 {
                                let w = (OVERLAY_WIDTH as f32 * cur_scale) as i32;
                                let h = (OVERLAY_HEIGHT as f32 * cur_scale) as i32;
                                let (x, y) = compute_overlay_position_with_size(
                                    &OverlayConfig {
                                        position: cur_pos.clone(),
                                        scale: cur_scale,
                                        ..OverlayConfig::default()
                                    },
                                    w,
                                    h,
                                );
                                unsafe {
                                    SetWindowPos(
                                        hw,
                                        Some(HWND_TOPMOST),
                                        x, y, w, h,
                                        SWP_NOACTIVATE,
                                    ).ok();
                                }
                                prev_pos = cur_pos;
                                prev_scale = cur_scale;
                            }
                        }
                    }
                }
            })?;
    }

    // Main thread: Win32 message loop
    unsafe { run_overlay_window() }
}

// ─── Win32 Window ─────────────────────────────────────────────────────────────

unsafe fn run_overlay_window() -> Result<()> {
    let hinstance = GetModuleHandleW(PCWSTR::null())
        .map_err(|e| anyhow::anyhow!("GetModuleHandle failed: {e}"))?;

    let class_name_wide: Vec<u16> = "SableOverlay\0".encode_utf16().collect();

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE(hinstance.0),
        hCursor: LoadCursorW(None, IDC_ARROW)
            .unwrap_or(HCURSOR::default()),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 as usize + 1) as *mut _),
        lpszClassName: PCWSTR(class_name_wide.as_ptr()),
        ..Default::default()
    };

    RegisterClassExW(&wc);

    // Compute initial position (top-left of primary monitor by default)
    let (x, y) = compute_overlay_position(&OverlayConfig::default());

    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
        PCWSTR(class_name_wide.as_ptr()),
        PCWSTR("Sable Overlay\0".encode_utf16().collect::<Vec<u16>>().as_ptr()),
        WS_POPUP | WS_VISIBLE,
        x,
        y,
        OVERLAY_WIDTH,
        OVERLAY_HEIGHT,
        None,
        None,
        Some(HINSTANCE(hinstance.0)),
        None,
    )
    .map_err(|e| anyhow::anyhow!("CreateWindowEx failed: {e}"))?;

    // Store the HWND in shared state so the polling thread can reposition the window.
    if let Some(s) = STATE.get() {
        let mut st = s.lock().unwrap_or_else(|p| p.into_inner());
        st.hwnd = hwnd.0 as isize;
    };

    // Set layered window attributes — transparent key color (pure magenta = transparent)
    SetLayeredWindowAttributes(hwnd, COLORREF(0x00FF00FF), 0, LWA_COLORKEY);

    info!("Overlay window created");

    // WM_TIMER for periodic repaint
    SetTimer(Some(hwnd), 1, UPDATE_INTERVAL_MS as u32, None);

    // Message loop
    let mut msg = MSG::default();
    loop {
        if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        } else {
            std::thread::sleep(Duration::from_millis(16)); // ~60Hz responsiveness
        }
    }

    Ok(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if hdc != HDC::default() {
                paint_overlay(hwnd, hdc);
                EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            // Force repaint on timer tick
            InvalidateRect(Some(hwnd), None, true);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn paint_overlay(hwnd: HWND, hdc: HDC) {
    let state = match STATE.get() {
        Some(s) => s.lock().unwrap_or_else(|p| p.into_inner()),
        None => return,
    };

    if !state.visible {
        return;
    }

    let t = &state.telemetry;
    let cfg = &state.config;

    // Background fill — semi-transparent dark panel
    // Use magenta as the transparent color key, everything else is painted
    let bg_brush = CreateSolidBrush(COLORREF(0x001A1D24)); // Surface color
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: OVERLAY_WIDTH,
        bottom: OVERLAY_HEIGHT,
    };
    FillRect(hdc, &rect, bg_brush);
    DeleteObject(HGDIOBJ(bg_brush.0));

    // Font for metrics — monospace, small
    let font_name: Vec<u16> = "Consolas\0".encode_utf16().collect();
    let font = CreateFontW(
        13,    // height
        0, 0, 0,
        FW_NORMAL.0 as i32,
        0, 0, 0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        PCWSTR(font_name.as_ptr()),
    );
    let old_font = SelectObject(hdc, HGDIOBJ(font.0));
    SetBkMode(hdc, TRANSPARENT);

    let mut y_offset = PADDING;

    // FPS
    if cfg.show_fps {
        let fps_str = match t.frames.fps_avg {
            Some(fps) => format!("FPS   {:>5.0}", fps),
            None => "FPS      --".to_string(),
        };
        let color = fps_color(t.frames.fps_avg.unwrap_or(0.0));
        draw_metric_line(hdc, &fps_str, PADDING, y_offset, color);
        y_offset += 16;

        if let Some(low) = t.frames.fps_1_percent_low {
            let low_str = format!("1% L  {:>5.0}", low);
            draw_metric_line(hdc, &low_str, PADDING, y_offset, 0x008892A0);
            y_offset += 16;
        }
    }

    // Frametime
    if cfg.show_frametime {
        let ft_str = match t.frames.frametime_avg_ms {
            Some(ft) => format!("FT    {:>5.1}ms", ft),
            None => "FT       --".to_string(),
        };
        draw_metric_line(hdc, &ft_str, PADDING, y_offset, 0x00F0F2F5);
        y_offset += 16;
    }

    // GPU usage
    if cfg.show_gpu_usage {
        let gpu_str = match t.gpu.gpu_usage_pct {
            Some(g) => format!("GPU   {:>5.0}%", g),
            None => "GPU      --".to_string(),
        };
        let color = usage_color(t.gpu.gpu_usage_pct.unwrap_or(0.0));
        draw_metric_line(hdc, &gpu_str, PADDING, y_offset, color);
        y_offset += 16;
    }

    // CPU usage
    if cfg.show_cpu_usage {
        let cpu_str = match t.cpu.usage_pct {
            Some(c) => format!("CPU   {:>5.0}%", c),
            None => "CPU      --".to_string(),
        };
        let color = usage_color(t.cpu.usage_pct.unwrap_or(0.0));
        draw_metric_line(hdc, &cpu_str, PADDING, y_offset, color);
        y_offset += 16;
    }

    // GPU temp
    if cfg.show_gpu_temp && !cfg.streamer_mode {
        let temp_str = match t.gpu.gpu_temp_c {
            Some(temp) => format!("TEMP  {:>5.0}°C", temp),
            None => "TEMP     --".to_string(),
        };
        let color = temp_color(t.gpu.gpu_temp_c.unwrap_or(0.0));
        draw_metric_line(hdc, &temp_str, PADDING, y_offset, color);
        y_offset += 16;
    }

    // VRAM
    if cfg.show_vram {
        let vram_str = match (t.gpu.vram_used_mb, t.gpu.vram_total_mb) {
            (Some(u), Some(t_)) => format!("VRAM {:>4}M/{}", u, t_),
            _ => "VRAM     --".to_string(),
        };
        draw_metric_line(hdc, &vram_str, PADDING, y_offset, 0x00F0F2F5);
    }

    SelectObject(hdc, old_font);
    DeleteObject(HGDIOBJ(font.0));
}

unsafe fn draw_metric_line(hdc: HDC, text: &str, x: i32, y: i32, color_bgr: u32) {
    SetTextColor(hdc, COLORREF(color_bgr));
    let wide: Vec<u16> = text.encode_utf16().collect();
    TextOutW(hdc, x, y, &wide);
}

// ─── Color Coding ─────────────────────────────────────────────────────────────

fn fps_color(fps: f32) -> u32 {
    if fps >= 60.0 { 0x008CD63D } // Green
    else if fps >= 30.0 { 0x0023A6F5 } // Amber (BGR)
    else { 0x003C45E8 } // Red (BGR)
}

fn usage_color(pct: f32) -> u32 {
    if pct < 70.0 { 0x00A0A0F0 } // light blue
    else if pct < 90.0 { 0x0023A6F5 } // amber
    else { 0x003C45E8 } // red
}

fn temp_color(temp: f32) -> u32 {
    if temp < 70.0 { 0x00A0A0F0 }
    else if temp < 85.0 { 0x0023A6F5 }
    else { 0x003C45E8 }
}

// ─── Overlay Position ─────────────────────────────────────────────────────────

fn compute_overlay_position(cfg: &OverlayConfig) -> (i32, i32) {
    let w = (OVERLAY_WIDTH as f32 * cfg.scale) as i32;
    let h = (OVERLAY_HEIGHT as f32 * cfg.scale) as i32;
    compute_overlay_position_with_size(cfg, w, h)
}

fn compute_overlay_position_with_size(cfg: &OverlayConfig, w: i32, h: i32) -> (i32, i32) {
    // Get primary monitor dimensions
    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let margin = 12;

    match cfg.position {
        OverlayPosition::TopLeft => (margin, margin),
        OverlayPosition::TopRight => (screen_width - w - margin, margin),
        OverlayPosition::BottomLeft => (margin, screen_height - h - margin),
        OverlayPosition::BottomRight => (screen_width - w - margin, screen_height - h - margin),
    }
}

// ─── Service Pipe Client ──────────────────────────────────────────────────────

fn read_telemetry_from_service() -> Option<TelemetrySnapshot> {
    use sable_core::{ServiceRequest, ServiceResponse};
    use windows::Win32::Storage::FileSystem::*;
    use windows::Win32::Foundation::GENERIC_READ;

    let pipe_name: Vec<u16> = format!("{}\0", PIPE_NAME).encode_utf16().collect();

    // Open pipe (non-blocking attempt)
    let pipe = unsafe {
        CreateFileW(
            PCWSTR(pipe_name.as_ptr()),
            (GENERIC_READ.0 | 0x40000000u32), // GENERIC_READ | GENERIC_WRITE
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };

    let pipe = match pipe {
        Ok(h) if !h.is_invalid() => h,
        _ => return None, // Service not running
    };

    let request = ServiceRequest::GetTelemetry;
    let encoded = bincode::serialize(&request).ok()?;
    let len = (encoded.len() as u32).to_le_bytes();

    let write_ok = unsafe {
        WriteFile(pipe, Some(&len), None, None).is_ok()
            && WriteFile(pipe, Some(&encoded), None, None).is_ok()
    };

    if !write_ok {
        unsafe { CloseHandle(pipe) };
        return None;
    }

    // Read response length
    let mut len_buf = [0u8; 4];
    let mut bytes_read = 0u32;
    let read_ok = unsafe {
        ReadFile(pipe, Some(&mut len_buf), Some(&mut bytes_read), None).is_ok()
    };

    if !read_ok || bytes_read != 4 {
        unsafe { CloseHandle(pipe) };
        return None;
    }

    let msg_len = u32::from_le_bytes(len_buf) as usize;
    if msg_len == 0 || msg_len > 65536 {
        unsafe { CloseHandle(pipe) };
        return None;
    }

    let mut msg_buf = vec![0u8; msg_len];
    let mut bytes_read2 = 0u32;
    let read_ok2 = unsafe {
        ReadFile(pipe, Some(&mut msg_buf), Some(&mut bytes_read2), None).is_ok()
    };

    unsafe { CloseHandle(pipe) };

    if !read_ok2 {
        return None;
    }

    match bincode::deserialize::<ServiceResponse>(&msg_buf) {
        Ok(ServiceResponse::Telemetry(snap)) => Some(snap),
        _ => None,
    }
}
