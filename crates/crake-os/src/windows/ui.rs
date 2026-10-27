//! The app's windows and screen: the monitors and their DPI, moving a
//! window, its frame (corners, border), the small icon's size, a parent's
//! console. A window is named by its handle's value (an `isize`, as
//! raw-window-handle gives it): Windows looks it up in its own table, so
//! one that isn't (or is no longer) valid makes these fail, never touching
//! memory through it.

use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromPoint,
};
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForMonitor, GetDpiForSystem, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, MONITORINFOF_PRIMARY, SM_CXSMICON, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
};

/// Windows' 100 %: 96 dots an inch.
pub const BASE_DPI: u32 = 96;

/// A monitor: whether it's the primary one, its bounds in pixels, its DPI.
#[derive(Clone, Copy)]
pub struct Monitor {
    pub primary: bool,
    pub bounds: RECT,
    /// The monitor less the taskbar and other app bars.
    pub work: RECT,
    pub dpi: u32,
}

/// Every monitor, as Windows lists them.
pub fn monitors() -> Vec<Monitor> {
    let mut found: Vec<Monitor> = Vec::new();
    // SAFETY: the callback runs only during the call, with `found` alive
    // and passed as its LPARAM.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(each_monitor),
            &raw mut found as LPARAM,
        );
    }
    found
}

unsafe extern "system" fn each_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    found: LPARAM,
) -> windows_sys::core::BOOL {
    // SAFETY: `found` is the Vec `monitors` passed in, alive for the call.
    let found = unsafe { &mut *(found as *mut Vec<Monitor>) };
    // SAFETY: a plain C struct, all zero is valid.
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = u32::try_from(size_of::<MONITORINFO>()).unwrap_or(0);
    let (mut dpi, mut _y) = (BASE_DPI, BASE_DPI);
    // SAFETY: `monitor` comes from Windows for this call; `info` is sized
    // as its cbSize says; the DPIs are ours to write.
    unsafe {
        if GetMonitorInfoW(monitor, &raw mut info) == 0 {
            return 1;
        }
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi, &raw mut _y);
    }
    found.push(Monitor {
        primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        bounds: info.rcMonitor,
        work: info.rcWork,
        dpi,
    });
    1
}

/// The DPI of the monitor nearest the point (in pixels).
pub fn dpi_at(x: i32, y: i32) -> u32 {
    let (mut dpi, mut _y) = (BASE_DPI, BASE_DPI);
    // SAFETY: a monitor handle from Windows, out values ours to write.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi, &raw mut _y);
    }
    dpi
}

/// A window's bounds, in pixels.
pub fn window_rect(hwnd: isize) -> Option<RECT> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: a RECT ours to fill; an invalid window is an error.
    (unsafe { GetWindowRect(hwnd as HWND, &raw mut rect) } != 0).then_some(rect)
}

/// Move and size a window in one step, keeping its place in the stack and
/// not activating it.
pub fn move_window(hwnd: isize, x: i32, y: i32, width: i32, height: i32) -> bool {
    // SAFETY: plain values; an invalid window is an error.
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            std::ptr::null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        ) != 0
    }
}

/// A window's frame: its corners (DWMWCP_*) and its border's colour
/// (a COLORREF, or DWMWA_COLOR_NONE).
pub fn set_frame(hwnd: isize, corners: i32, border: u32) {
    // SAFETY: each value is ours and the size given; an invalid window is
    // an error.
    unsafe {
        DwmSetWindowAttribute(
            hwnd as HWND,
            u32::try_from(DWMWA_BORDER_COLOR).unwrap_or_default(),
            (&raw const border).cast(),
            u32::try_from(size_of::<u32>()).unwrap_or_default(),
        );
        DwmSetWindowAttribute(
            hwnd as HWND,
            u32::try_from(DWMWA_WINDOW_CORNER_PREFERENCE).unwrap_or_default(),
            (&raw const corners).cast(),
            u32::try_from(size_of_val(&corners)).unwrap_or_default(),
        );
    }
}

/// The small icon's size at the system's DPI (the tray's), in pixels.
pub fn small_icon_size() -> i32 {
    // SAFETY: plain queries.
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) }
}

/// Attach to the console this process was started from, if any (a
/// windowed program run from a terminal: its output goes there).
pub fn attach_parent_console() {
    // SAFETY: a constant; failing (no parent console) is fine.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}
