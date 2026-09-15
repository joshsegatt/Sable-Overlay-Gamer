// sable-overlay: layered-window HUD. No D3D hook.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use sable_core::{OverlayConfig, OverlayPosition, ServiceRequest, ServiceResponse, TelemetrySnapshot};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::info;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const PIPE_NAME: &str = r"\\.\pipe\sable";
const UPDATE_INTERVAL_MS: u64 = 1000;
const COLOR_KEY: u32 = 0x00FF00FF;
const PANEL_BG: u32 = 0x000E1014;
const ACCENT: u32 = 0x00FF8B3D;
const LABEL: u32 = 0x008A92A0;
const VALUE: u32 = 0x00F4F7FB;
const MUTED: u32 = 0x0078808C;
const OK: u32 = 0x0097DC3D;
const WARN: u32 = 0x0042B9F5;
const BAD: u32 = 0x004D5AFF;
const BASE_W: i32 = 196;
const MARGIN: i32 = 14;

struct OverlayState {
    telemetry: TelemetrySnapshot,
    config: OverlayConfig,
    visible: bool,
    hwnd: isize,
}

static STATE: std::sync::OnceLock<Arc<Mutex<OverlayState>>> = std::sync::OnceLock::new();

fn mb_to_gb(mb: u64) -> f64 {
    mb as f64 / 1024.0
}

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

    {
        let state = Arc::clone(&state);
        std::thread::Builder::new()
            .name("overlay-poll".to_string())
            .spawn(move || {
                let mut prev_pos = OverlayPosition::TopLeft;
                let mut prev_scale: f32 = 1.0;
                loop {
                    std::thread::sleep(Duration::from_millis(UPDATE_INTERVAL_MS));
                    if let Some(cfg) = read_overlay_config() {
                        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
                        st.config = cfg;
                        st.visible = st.config.enabled;
                    }
                    if let Some(snap) = read_telemetry() {
                        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
                        st.telemetry = snap;
                    }
                    let st = state.lock().unwrap_or_else(|p| p.into_inner());
                    let cur_pos = st.config.position.clone();
                    let cur_scale = st.config.scale;
                    let hwnd_val = st.hwnd;
                    let (w, h) = hud_size(&st.config);
                    drop(st);
                    if hwnd_val != 0 && (cur_pos != prev_pos || (cur_scale - prev_scale).abs() > 0.01) {
                        let hw = HWND(hwnd_val as *mut _);
                        let (x, y) = position(&cur_pos, w, h);
                        unsafe {
                            let _ = SetWindowPos(hw, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE);
                        }
                        prev_pos = cur_pos;
                        prev_scale = cur_scale;
                    }
                }
            })?;
    }

    unsafe { run_window() }
}

fn hud_size(cfg: &OverlayConfig) -> (i32, i32) {
    let scale = cfg.scale.max(0.75);
    let mut rows = 1;
    if cfg.show_fps {
        rows += 1;
    }
    if cfg.show_frametime {
        rows += 1;
    }
    if cfg.show_gpu_usage {
        rows += 1;
    }
    if cfg.show_cpu_usage {
        rows += 1;
    }
    if cfg.show_gpu_temp && !cfg.streamer_mode {
        rows += 1;
    }
    if cfg.show_vram {
        rows += 1;
    }
    if cfg.show_ram {
        rows += 1;
    }
    let spark = if cfg.show_frametime { 26 } else { 0 };
    let h = 12 + rows * 18 + spark + 10;
    ((BASE_W as f32 * scale) as i32, (h as f32 * scale) as i32)
}

unsafe fn run_window() -> Result<()> {
    let hinstance = GetModuleHandleW(PCWSTR::null())
        .map_err(|e| anyhow::anyhow!("GetModuleHandle failed: {e}"))?;
    let class_name: Vec<u16> = "SableOverlay\0".encode_utf16().collect();
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE(hinstance.0),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or(HCURSOR::default()),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    RegisterClassExW(&wc);

    let cfg = OverlayConfig::default();
    let (w, h) = hud_size(&cfg);
    let (x, y) = position(&cfg.position, w, h);
    let title: Vec<u16> = "Sable Overlay\0".encode_utf16().collect();
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
        PCWSTR(class_name.as_ptr()),
        PCWSTR(title.as_ptr()),
        WS_POPUP | WS_VISIBLE,
        x,
        y,
        w,
        h,
        None,
        None,
        Some(HINSTANCE(hinstance.0)),
        None,
    )
    .map_err(|e| anyhow::anyhow!("CreateWindowEx failed: {e}"))?;

    if let Some(s) = STATE.get() {
        s.lock().unwrap_or_else(|p| p.into_inner()).hwnd = hwnd.0 as isize;
    }
    SetLayeredWindowAttributes(hwnd, COLORREF(COLOR_KEY), 0, LWA_COLORKEY);
    SetTimer(Some(hwnd), 1, UPDATE_INTERVAL_MS as u32, None);
    info!("Overlay window created");

    let mut msg = MSG::default();
    loop {
        if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        } else {
            std::thread::sleep(Duration::from_millis(16));
        }
    }
    Ok(())
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if hdc != HDC::default() {
                paint(hwnd, hdc);
                EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_TIMER => {
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

unsafe fn paint(hwnd: HWND, hdc: HDC) {
    let state = match STATE.get() {
        Some(s) => s.lock().unwrap_or_else(|p| p.into_inner()),
        None => return,
    };
    let mut rc = RECT::default();
    GetClientRect(hwnd, &mut rc);
    let key = CreateSolidBrush(COLORREF(COLOR_KEY));
    FillRect(hdc, &rc, key);
    DeleteObject(HGDIOBJ(key.0));
    if !state.visible {
        return;
    }

    let panel = CreateSolidBrush(COLORREF(PANEL_BG));
    let inner = RECT {
        left: rc.left + 1,
        top: rc.top + 1,
        right: rc.right - 1,
        bottom: rc.bottom - 1,
    };
    FillRect(hdc, &inner, panel);
    DeleteObject(HGDIOBJ(panel.0));

    let accent = CreateSolidBrush(COLORREF(ACCENT));
    let rail = RECT {
        left: inner.left,
        top: inner.top + 8,
        right: inner.left + 3,
        bottom: inner.bottom - 8,
    };
    FillRect(hdc, &rail, accent);
    DeleteObject(HGDIOBJ(accent.0));

    let label_font = make_font(11, 600);
    let value_font = make_font(13, 700);
    SetBkMode(hdc, TRANSPARENT);

    let t = &state.telemetry;
    let cfg = &state.config;
    let left = inner.left + 12;
    let right = inner.right - 10;
    let mut y = inner.top + 8;

    let process = t.frames.target_process.as_deref().unwrap_or("-");
    select_font(hdc, label_font);
    draw_text(hdc, "SABLE", left, y, LABEL);
    draw_text_right(hdc, process, right, y, MUTED);
    y += 18;

    if cfg.show_fps {
        let fps = t.frames.fps_avg;
        let low = t.frames.fps_1_percent_low;
        select_font(hdc, label_font);
        draw_text(hdc, "FPS", left, y, LABEL);
        select_font(hdc, value_font);
        let main = match fps {
            Some(v) => format!("{:.0}", v),
            None => "-".into(),
        };
        let color = fps.map(fps_color).unwrap_or(MUTED);
        if let Some(low) = low {
            let composed = format!("{main}  {:.0}", low);
            draw_text_right(hdc, &composed, right, y, color);
        } else {
            draw_text_right(hdc, &main, right, y, color);
        }
        y += 18;
    }

    if cfg.show_frametime {
        select_font(hdc, label_font);
        draw_text(hdc, "FT", left, y, LABEL);
        select_font(hdc, value_font);
        let text = match t.frames.frametime_avg_ms {
            Some(v) => format!("{v:.1} ms"),
            None => "-".into(),
        };
        draw_text_right(hdc, &text, right, y, VALUE);
        y += 18;
    }

    if cfg.show_gpu_usage {
        row_pct(hdc, label_font, value_font, "GPU", t.gpu.gpu_usage_pct, left, right, y);
        y += 18;
    }
    if cfg.show_cpu_usage {
        row_pct(hdc, label_font, value_font, "CPU", t.cpu.usage_pct, left, right, y);
        y += 18;
    }
    if cfg.show_gpu_temp && !cfg.streamer_mode {
        select_font(hdc, label_font);
        draw_text(hdc, "GPU C", left, y, LABEL);
        select_font(hdc, value_font);
        let (text, color) = match t.gpu.gpu_temp_c {
            Some(v) => (format!("{v:.0}"), temp_color(v)),
            None => ("-".into(), MUTED),
        };
        draw_text_right(hdc, &text, right, y, color);
        y += 18;
    }
    if cfg.show_vram {
        select_font(hdc, label_font);
        draw_text(hdc, "VRAM", left, y, LABEL);
        select_font(hdc, value_font);
        let text = match (t.gpu.vram_used_mb, t.gpu.vram_total_mb) {
            (Some(u), Some(tot)) if tot > 0 => {
                format!("{:.1}/{:.0}", mb_to_gb(u), mb_to_gb(tot))
            }
            (Some(u), _) => format!("{:.1}G", mb_to_gb(u)),
            _ => "-".into(),
        };
        draw_text_right(hdc, &text, right, y, VALUE);
        y += 18;
    }
    if cfg.show_ram {
        select_font(hdc, label_font);
        draw_text(hdc, "RAM", left, y, LABEL);
        select_font(hdc, value_font);
        let text = match t.ram_used_mb {
            Some(u) => format!("{:.1}G", u as f64 / 1024.0),
            None => "-".into(),
        };
        draw_text_right(hdc, &text, right, y, VALUE);
        y += 18;
    }

    if cfg.show_frametime && !t.frames.frametime_history.is_empty() {
        draw_sparkline(
            hdc,
            &t.frames.frametime_history,
            left,
            y + 2,
            right - left,
            20,
        );
    }

    DeleteObject(HGDIOBJ(label_font.0));
    DeleteObject(HGDIOBJ(value_font.0));
}

unsafe fn row_pct(
    hdc: HDC,
    label_font: HFONT,
    value_font: HFONT,
    label: &str,
    pct: Option<f32>,
    left: i32,
    right: i32,
    y: i32,
) {
    select_font(hdc, label_font);
    draw_text(hdc, label, left, y, LABEL);
    select_font(hdc, value_font);
    let (text, color) = match pct {
        Some(v) => (format!("{v:.0}%"), usage_color(v)),
        None => ("-".into(), MUTED),
    };
    draw_text_right(hdc, &text, right, y, color);
}

unsafe fn make_font(px: i32, weight: i32) -> HFONT {
    let name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
    CreateFontW(
        px,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        PCWSTR(name.as_ptr()),
    )
}

unsafe fn select_font(hdc: HDC, font: HFONT) {
    let _ = SelectObject(hdc, HGDIOBJ(font.0));
}

unsafe fn draw_text(hdc: HDC, text: &str, x: i32, y: i32, color: u32) {
    SetTextColor(hdc, COLORREF(color));
    let wide: Vec<u16> = text.encode_utf16().collect();
    let _ = TextOutW(hdc, x, y, &wide);
}

unsafe fn draw_text_right(hdc: HDC, text: &str, right: i32, y: i32, color: u32) {
    SetTextColor(hdc, COLORREF(color));
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut sz = SIZE::default();
    let _ = GetTextExtentPoint32W(hdc, &wide, &mut sz);
    let _ = TextOutW(hdc, right - sz.cx, y, &wide);
}

unsafe fn draw_sparkline(hdc: HDC, history: &[f32], x: i32, y: i32, w: i32, h: i32) {
    if history.len() < 2 || w <= 1 {
        return;
    }
    let take = history.len().min(48);
    let slice = &history[history.len() - take..];
    let pen = CreatePen(PS_SOLID, 1, COLORREF(ACCENT));
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    for i in 0..slice.len() {
        let px = x + (i as i32 * (w - 1)) / (slice.len() as i32 - 1).max(1);
        let norm = (slice[i] / 33.0).clamp(0.05, 1.0);
        let py = y + h - (norm * h as f32) as i32;
        if i == 0 {
            let _ = MoveToEx(hdc, px, py, None);
        } else {
            let _ = LineTo(hdc, px, py);
        }
    }
    let _ = SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

fn fps_color(fps: f32) -> u32 {
    if fps >= 60.0 {
        OK
    } else if fps >= 30.0 {
        WARN
    } else {
        BAD
    }
}
fn usage_color(pct: f32) -> u32 {
    if pct >= 92.0 {
        BAD
    } else if pct >= 80.0 {
        WARN
    } else {
        OK
    }
}
fn temp_color(temp: f32) -> u32 {
    if temp >= 85.0 {
        BAD
    } else if temp >= 75.0 {
        WARN
    } else {
        VALUE
    }
}

fn position(pos: &OverlayPosition, w: i32, h: i32) -> (i32, i32) {
    let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    match pos {
        OverlayPosition::TopLeft => (MARGIN, MARGIN),
        OverlayPosition::TopRight => (sw - w - MARGIN, MARGIN),
        OverlayPosition::BottomLeft => (MARGIN, sh - h - MARGIN),
        OverlayPosition::BottomRight => (sw - w - MARGIN, sh - h - MARGIN),
    }
}

fn pipe_call(req: ServiceRequest) -> Option<ServiceResponse> {
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Storage::FileSystem::*;
    let pipe_name: Vec<u16> = format!("{PIPE_NAME}\0").encode_utf16().collect();
    let pipe = unsafe {
        CreateFileW(
            PCWSTR(pipe_name.as_ptr()),
            GENERIC_READ.0 | 0x40000000u32,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };
    let pipe = match pipe {
        Ok(h) if !h.is_invalid() => h,
        _ => return None,
    };
    let encoded = bincode::serialize(&req).ok()?;
    let len = (encoded.len() as u32).to_le_bytes();
    let ok = unsafe {
        WriteFile(pipe, Some(&len), None, None).is_ok() && WriteFile(pipe, Some(&encoded), None, None).is_ok()
    };
    if !ok {
        unsafe {
            let _ = CloseHandle(pipe);
        }
        return None;
    }
    let mut len_buf = [0u8; 4];
    let mut n = 0u32;
    if unsafe { ReadFile(pipe, Some(&mut len_buf), Some(&mut n), None) }.is_err() || n != 4 {
        unsafe {
            let _ = CloseHandle(pipe);
        }
        return None;
    }
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    if msg_len == 0 || msg_len > 65536 {
        unsafe {
            let _ = CloseHandle(pipe);
        }
        return None;
    }
    let mut buf = vec![0u8; msg_len];
    let mut n2 = 0u32;
    let ok = unsafe { ReadFile(pipe, Some(&mut buf), Some(&mut n2), None) }.is_ok();
    unsafe {
        let _ = CloseHandle(pipe);
    }
    if !ok {
        return None;
    }
    bincode::deserialize(&buf).ok()
}

fn read_telemetry() -> Option<TelemetrySnapshot> {
    match pipe_call(ServiceRequest::GetTelemetry) {
        Some(ServiceResponse::Telemetry(s)) => Some(s),
        _ => None,
    }
}

fn read_overlay_config() -> Option<OverlayConfig> {
    match pipe_call(ServiceRequest::GetOverlayConfig) {
        Some(ServiceResponse::OverlayConfig(c)) => Some(c),
        _ => None,
    }
}
