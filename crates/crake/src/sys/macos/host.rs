//! Small things that differ by system: the monitors, the private folder,
//! the menu bar's look, and the connection to craked (the rest is
//! `sys::unix`'s, shared with Linux).

use std::path::PathBuf;

use crate::place::Rect;

/// GPUI knows macOS' displays itself; Crake's own placement has none to
/// add yet.
pub fn monitors() -> Vec<Rect> {
    Vec::new()
}

/// A folder only this user can read, for files that live as long as a
/// sign-in ($TMPDIR, which macOS gives each user, /crake).
pub fn private_dir() -> Result<PathBuf, String> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::env::temp_dir().join("crake");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(crate::sys::OWNER_ONLY_DIR)
        .create(&dir)
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

/// macOS windows need nothing more: Crake's own frame is the frame.
pub fn plain_frame(_window: &gpui_kit::Window) {}

/// A command-line program started from the app: on macOS it has no window.
pub fn no_console(_cmd: &mut std::process::Command) {}

/// The installed craked, on its socket (peer to peer: no system bus on
/// macOS).
pub async fn connect_craked() -> zbus::Result<(zbus::Connection, bool)> {
    Ok((
        super::super::connect_dev(std::path::Path::new(crake_proto::APP_SOCKET)).await?,
        true,
    ))
}

/// A click on the menu bar icon, already in the points windows are placed
/// in.
pub fn logical(x: f64, y: f64) -> (i32, i32) {
    (x as i32, y as i32)
}

/// The menu bar is light unless the system's appearance is dark.
pub fn light_bar() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map_or(true, |o| {
            !String::from_utf8_lossy(&o.stdout).contains("Dark")
        })
}

/// The menu bar's icon size in pixels: 18 points, on a Retina screen.
pub fn tray_icon_size() -> u32 {
    const RETINA_18_POINTS: u32 = 36;
    RETINA_18_POINTS
}
