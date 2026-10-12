//! Small things that differ by system: the monitors, the private folder,
//! and the connection to craked (the rest is `sys::unix`'s, shared with
//! macOS).

use std::path::PathBuf;

use crate::place::Rect;

/// The monitors as X11 (XRandR) knows them, primary first. GPUI's X11 backend
/// reports one screen spanning them all, which can't tell where one ends.
pub fn monitors() -> Vec<Rect> {
    // Tests see a machine without a display, whatever this one has.
    if cfg!(test) {
        return Vec::new();
    }
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt;
    let Ok((conn, screen)) = x11rb::connect(None) else {
        return Vec::new();
    };
    let Some(root) = conn.setup().roots.get(screen).map(|s| s.root) else {
        return Vec::new();
    };
    let Ok(Ok(reply)) = conn.randr_get_monitors(root, true).map(|c| c.reply()) else {
        return Vec::new();
    };
    let mut list: Vec<(bool, Rect)> = reply
        .monitors
        .iter()
        .map(|m| {
            let r = Rect {
                x: f32::from(m.x),
                y: f32::from(m.y),
                w: f32::from(m.width),
                h: f32::from(m.height),
            };
            (m.primary, r)
        })
        .collect();
    list.sort_by_key(|(primary, _)| !primary);
    list.into_iter().map(|(_, r)| r).collect()
}

/// A folder only this user can read, for files that live as long as a
/// sign-in ($XDG_RUNTIME_DIR/crake, on tmpfs).
pub fn private_dir() -> Result<PathBuf, String> {
    use std::os::unix::fs::DirBuilderExt;
    let base = std::env::var_os("XDG_RUNTIME_DIR").ok_or("no XDG_RUNTIME_DIR")?;
    let dir = PathBuf::from(base).join("crake");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(crate::sys::OWNER_ONLY_DIR)
        .create(&dir)
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Linux windows need nothing more: Crake's own frame is the frame.
pub fn plain_frame(_window: &gpui_kit::Window) {}

/// Windows rounds the panel itself; here it rounds its own card.
pub fn rounded_frame(_window: &gpui_kit::Window, _border: gpui_kit::Rgba) {}

/// A command-line program started from the app: on Linux it has no window.
pub fn no_console(_cmd: &mut std::process::Command) {}

/// The installed craked, on the system bus (not peer to peer).
pub async fn connect_craked() -> zbus::Result<(zbus::Connection, bool)> {
    Ok((zbus::Connection::system().await?, false))
}
