//! Small things that differ by system: the monitors, private files,
//! whether a program can run, and the development connection to craked.

use std::path::{Path, PathBuf};

/// Windows' small icon at 100 %.
const SMALL_ICON: u32 = 16;
const BASE_DPI: u32 = ui::BASE_DPI;

use crake_os::windows::ui;

use crate::place::Rect;

/// The monitors, primary first, in the logical pixels windows are sized in.
pub fn monitors() -> Vec<Rect> {
    // Tests see a machine without a display, whatever this one has.
    if cfg!(test) {
        return Vec::new();
    }
    let mut found: Vec<(bool, Rect)> = ui::monitors()
        .into_iter()
        .map(|m| {
            let scale = m.dpi.max(1) as f32 / BASE_DPI as f32;
            let r = m.bounds;
            (
                m.primary,
                Rect {
                    x: r.left as f32 / scale,
                    y: r.top as f32 / scale,
                    w: (r.right - r.left) as f32 / scale,
                    h: (r.bottom - r.top) as f32 / scale,
                },
            )
        })
        .collect();
    found.sort_by_key(|(primary, _)| !primary);
    found.into_iter().map(|(_, r)| r).collect()
}

/// A folder only this user can read, for files that live as long as a
/// sign-in (in the user's local application data, private to them).
/// The default browser, for the sign-in window ("Waiting for you in your
/// browser"): not looked up here yet (Linux reads its desktop entry).
pub fn browser_name() -> String {
    "your browser".to_owned()
}

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

/// Each of Crake's windows, opaque and the size of its card (the panel's
/// since 2026-10-12, every window's since 2026-10-26): Windows rounds it
/// and draws its hairline (in `border`, the theme's line) and its shadow,
/// which a transparent window with a card rounding itself didn't get (and
/// DirectX left a light fringe at its corners, a colleague's Windows 11).
pub fn rounded_frame(window: &gpui_kit::Window, border: gpui_kit::Rgba) {
    use windows_sys::Win32::Graphics::Dwm::DWMWCP_ROUND;
    let channel = |c: f32| (c.clamp(0., 1.) * f32::from(u8::MAX)).round() as u8;
    // COLORREF: 0x00BBGGRR.
    let color = u32::from_le_bytes([channel(border.r), channel(border.g), channel(border.b), 0]);
    frame(window, DWMWCP_ROUND, color);
}

/// The panel opening upward from the taskbar, at `height` (logical
/// pixels): resized and moved in one step, its bottom staying at the
/// taskbar, instead of being opened again at the new size (which blinked).
pub fn resize_upward(window: &gpui_kit::Window, height: f32) -> bool {
    let Some(hwnd) = hwnd(window) else {
        return false;
    };
    let Some(rect) = ui::window_rect(hwnd) else {
        return false;
    };
    let new_height = (height * window.scale_factor()).round().max(1.) as u32;
    let old_height = u32::try_from(rect.bottom - rect.top).unwrap_or(0);
    let top = crate::place::top_keeping_bottom(rect.top, old_height, new_height);
    let width = rect.right - rect.left;
    ui::move_window(
        hwnd,
        rect.left,
        top,
        width,
        i32::try_from(new_height).unwrap_or(i32::MAX),
    )
}

/// A GPUI window's handle.
fn hwnd(window: &gpui_kit::Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return None;
    };
    Some(win32.hwnd.get())
}

fn frame(window: &gpui_kit::Window, corners: i32, border: u32) {
    if let Some(hwnd) = hwnd(window) {
        ui::set_frame(hwnd, corners, border);
    }
}

/// A command-line program started from the app (openconnect): without a
/// console window of its own popping up.
pub fn no_console(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// craked.exe gone from beside this app (an antivirus took it).
pub fn craked_missing() -> bool {
    std::env::current_exe().is_ok_and(|exe| !exe.with_file_name("craked.exe").is_file())
}

/// The newest of Crake's service events matching `query`, from the System
/// log (which every signed-in user may read), asked of `wevtutil`. Blocking.
pub fn service_events(query: &str) -> Option<String> {
    let mut cmd = std::process::Command::new("wevtutil");
    no_console(&mut cmd);
    let out = cmd
        .args(["qe", "System", "/rd:true", "/c:1", "/f:xml"])
        .arg(format!("/q:{query}"))
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
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
    crake_os::windows::tcp::listeners_v4()
        .ok()?
        .into_iter()
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
    crake_os::windows::services::pid(name)
}

/// Where the panel goes from: the click, in the logical pixels windows are
/// placed in.
pub fn tray_click(at: tray_icon::dpi::PhysicalPosition<f64>, _icon: tray_icon::Rect) -> (i32, i32) {
    logical(at.x, at.y)
}

/// A click's screen position in the logical pixels windows are placed in
/// (the tray reports physical ones): scaled by its monitor's DPI.
fn logical(x: f64, y: f64) -> (i32, i32) {
    let dpi = ui::dpi_at(x as i32, y as i32);
    let scale = f64::from(dpi.max(1)) / f64::from(BASE_DPI);
    ((x / scale) as i32, (y / scale) as i32)
}

/// Windows' own setting for the taskbar: "Choose your Windows mode".
pub fn light_bar() -> bool {
    use crake_os::windows::registry::{Root, dword};
    dword(
        Root::CurrentUser,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "SystemUsesLightTheme",
    ) == Some(1)
}

/// The notification area's icon size in pixels: Windows' small icon at the
/// screen's scaling (16 at 100 %, 20 at 125 %, 24 at 150 %, 32 at 200 %).
pub fn tray_icon_size() -> u32 {
    u32::try_from(ui::small_icon_size())
        .ok()
        .filter(|s| *s > 0)
        .unwrap_or(SMALL_ICON)
}

/// A command run from a console (`crake report`): its output goes there.
/// Release builds are windowed (no console of their own).
pub fn attach_console() {
    ui::attach_parent_console();
}
