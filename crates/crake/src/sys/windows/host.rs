//! Small things that differ by system: the monitors, private files,
//! whether a program can run, and the development connection to craked.

use std::path::{Path, PathBuf};

/// Windows' 100 %: 96 dots an inch; its small icon at that scale.
const BASE_DPI: u32 = 96;
const SMALL_ICON: u32 = 16;

use windows_sys::Win32::Foundation::{LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

use crate::place::Rect;

/// The monitors, primary first, in the logical pixels windows are sized in.
pub fn monitors() -> Vec<Rect> {
    // Tests see a machine without a display, whatever this one has.
    if cfg!(test) {
        return Vec::new();
    }
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
    let (mut dpi, mut _y) = (BASE_DPI, BASE_DPI);
    // SAFETY: `monitor` comes from Windows for this call; `info` is sized.
    unsafe {
        if GetMonitorInfoW(monitor, &raw mut info) == 0 {
            return 1;
        }
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi, &raw mut _y);
    }
    let scale = dpi.max(1) as f32 / BASE_DPI as f32;
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

/// Crake's windows draw their own rounded card on a transparent window:
/// Windows 11 mustn't add its square outline or round the window itself.
pub fn plain_frame(window: &gpui_kit::Window) {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_COLOR_NONE, DWMWCP_DONOTROUND};
    frame(window, DWMWCP_DONOTROUND, DWMWA_COLOR_NONE);
}

/// The panel, opaque and the size of its card: Windows rounds it and draws
/// its hairline (in `border`, the theme's line) and its shadow, which a
/// transparent window with a card rounding itself didn't get (and DirectX
/// left a light fringe at its corners).
pub fn rounded_frame(window: &gpui_kit::Window, border: gpui_kit::Rgba) {
    use windows_sys::Win32::Graphics::Dwm::DWMWCP_ROUND;
    let channel = |c: f32| (c.clamp(0., 1.) * f32::from(u8::MAX)).round() as u8;
    // COLORREF: 0x00BBGGRR.
    let color = u32::from_le_bytes([channel(border.r), channel(border.g), channel(border.b), 0]);
    frame(window, DWMWCP_ROUND, color);
}

fn frame(window: &gpui_kit::Window, corners: i32, border: u32) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute,
    };
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    // SAFETY: a live window of this process; each value is the size given.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            (&raw const border).cast(),
            size_of::<u32>() as u32,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&raw const corners).cast(),
            size_of_val(&corners) as u32,
        );
    }
}

/// A command-line program started from the app (openconnect): without a
/// console window of its own popping up.
pub fn no_console(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// craked's private socket: Linux's development setup only.
pub async fn connect_dev(_path: &Path) -> zbus::Result<zbus::Connection> {
    Err(zbus::Error::Unsupported)
}

/// The craked service, on its loopback port (`APP_ADDRESS`), peer to peer.
/// Only once the port is the service's: any program may listen on a
/// loopback port, and Crake tells craked its sign-ins. `CRAKE_DEV`: a
/// craked started by hand (`craked --console`) is trusted too.
pub async fn connect_craked() -> zbus::Result<(zbus::Connection, bool)> {
    let address: std::net::SocketAddrV4 = crake_proto::APP_ADDRESS
        .parse()
        .map_err(|_| zbus::Error::Address(crake_proto::APP_ADDRESS.into()))?;
    let stream = tokio::net::TcpStream::connect(address).await?;
    if std::env::var_os("CRAKE_DEV").is_none() {
        let listener = listening_pid(address);
        let service = service_pid(crake_proto::SERVICE_NAME);
        if listener.is_none() || listener != service {
            return Err(zbus::Error::Failure(format!(
                "{address} isn't the craked service's"
            )));
        }
    }
    let conn = zbus::connection::Builder::tcp_stream(stream)
        .p2p()
        .build()
        .await?;
    Ok((conn, true))
}

/// The process listening on `address` (TCP, IPv4).
fn listening_pid(address: std::net::SocketAddrV4) -> Option<u32> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::Networking::WinSock::AF_INET;
    let mut size = 0;
    // SAFETY: a size query (null table); then a table of that size.
    unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &raw mut size,
            0,
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }
    let mut buf = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    // SAFETY: `buf` holds `size` bytes, 8-byte aligned.
    let got = unsafe {
        GetExtendedTcpTable(
            buf.as_mut_ptr().cast(),
            &raw mut size,
            0,
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if got != 0 {
        return None;
    }
    // SAFETY: Windows filled `buf` with a MIB_TCPTABLE_OWNER_PID.
    let table = unsafe { &*buf.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>() };
    // SAFETY: `dwNumEntries` rows follow the header in `buf`.
    let rows: &[MIB_TCPROW_OWNER_PID] =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };
    rows.iter()
        .find(|r| {
            // Network byte order: the address as it sits in memory, the
            // port in the low 16 bits.
            std::net::Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes()) == *address.ip()
                && u16::from_be(r.dwLocalPort as u16) == address.port()
        })
        .map(|r| r.dwOwningPid)
}

/// The process of a running service.
fn service_pid(name: &str) -> Option<u32> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
        SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_STATUS_PROCESS,
    };
    let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    // SAFETY: handles are closed before returning; `status` is sized.
    unsafe {
        let manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if manager.is_null() {
            return None;
        }
        let service = OpenServiceW(manager, name.as_ptr(), SERVICE_QUERY_STATUS);
        let mut status: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut needed = 0;
        let ok = !service.is_null()
            && QueryServiceStatusEx(
                service,
                SC_STATUS_PROCESS_INFO,
                (&raw mut status).cast(),
                size_of::<SERVICE_STATUS_PROCESS>() as u32,
                &raw mut needed,
            ) != 0;
        if !service.is_null() {
            CloseServiceHandle(service);
        }
        CloseServiceHandle(manager);
        (ok && status.dwProcessId != 0).then_some(status.dwProcessId)
    }
}

/// A click's screen position in the logical pixels windows are placed in
/// (the tray reports physical ones): scaled by its monitor's DPI.
pub fn logical(x: f64, y: f64) -> (i32, i32) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint};
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    let point = POINT {
        x: x as i32,
        y: y as i32,
    };
    let (mut dpi, mut _y) = (BASE_DPI, BASE_DPI);
    // SAFETY: a monitor handle from Windows, out pointers for the call.
    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi, &raw mut _y);
    }
    let scale = f64::from(dpi.max(1)) / f64::from(BASE_DPI);
    ((x / scale) as i32, (y / scale) as i32)
}

/// Windows' own setting for the taskbar: "Choose your Windows mode".
pub fn light_bar() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("SystemUsesLightTheme");
    let mut light: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: NUL-terminated names; `light` holds the DWORD asked for.
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut light).cast(),
            &raw mut size,
        )
    };
    read == 0 && light == 1
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// The notification area's icon size in pixels: Windows' small icon at the
/// screen's scaling (16 at 100 %, 20 at 125 %, 24 at 150 %, 32 at 200 %).
pub fn tray_icon_size() -> u32 {
    use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSMICON;
    // SAFETY: plain queries.
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
    u32::try_from(size)
        .ok()
        .filter(|s| *s > 0)
        .unwrap_or(SMALL_ICON)
}
