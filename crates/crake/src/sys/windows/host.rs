//! Small things that differ by system: the monitors, private files,
//! whether a program can run, and the development connection to craked.

use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

use crate::place::Rect;

/// The monitors, primary first, in the logical pixels windows are sized in.
pub fn monitors() -> Vec<Rect> {
    let mut found: Vec<(bool, Rect)> = Vec::new();
    // SAFETY: the callback only runs during the call, with `found` alive.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(each_monitor),
            &raw mut found as LPARAM,
        );
    }
    found.sort_by_key(|(primary, _)| !primary);
    found.into_iter().map(|(_, r)| r).collect()
}

unsafe extern "system" fn each_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    found: LPARAM,
) -> windows_sys::core::BOOL {
    // SAFETY: `found` is the Vec `monitors` passed in, alive for the call.
    let found = unsafe { &mut *(found as *mut Vec<(bool, Rect)>) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        // SAFETY: a plain C struct, all zero is valid.
        ..unsafe { std::mem::zeroed() }
    };
    let (mut dpi, mut _y) = (96, 96);
    // SAFETY: `monitor` comes from Windows for this call; `info` is sized.
    unsafe {
        if GetMonitorInfoW(monitor, &raw mut info) == 0 {
            return 1;
        }
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi, &raw mut _y);
    }
    let scale = dpi.max(1) as f32 / 96.;
    let r = info.rcMonitor;
    found.push((
        info.dwFlags & MONITORINFOF_PRIMARY != 0,
        Rect {
            x: r.left as f32 / scale,
            y: r.top as f32 / scale,
            w: (r.right - r.left) as f32 / scale,
            h: (r.bottom - r.top) as f32 / scale,
        },
    ));
    1
}

/// A folder only this user can read, for files that live as long as a
/// sign-in (in the user's local application data, private to them).
pub fn private_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("no LOCALAPPDATA")?;
    let dir = PathBuf::from(base).join("Crake").join("run");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// `bytes` in a file only this user can read (`private_dir`), replacing
/// any older one.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// `path` is a program: a file, with or without the `.exe` its name
/// leaves out (`openconnect` for openconnect.exe).
pub fn executable(path: &Path) -> bool {
    path.is_file() || (path.extension().is_none() && path.with_extension("exe").is_file())
}

/// craked's private socket: not on Windows yet (phase 3).
pub async fn connect_dev(_path: &Path) -> zbus::Result<zbus::Connection> {
    Err(zbus::Error::Unsupported)
}
