//! Small things that differ by system: the monitors, private files,
//! whether a program can run, and the development connection to craked.

use std::io::Write;
use std::path::{Path, PathBuf};

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
        .mode(0o700)
        .create(&dir)
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

/// `bytes` in a file only this user can read, replacing any older one.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| e.to_string())
}

/// `path` is a program this user may run.
pub fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The installed craked, on the system bus (not peer to peer).
pub async fn connect_craked() -> zbus::Result<(zbus::Connection, bool)> {
    Ok((zbus::Connection::system().await?, false))
}

/// craked's private socket (development: `craked --dev`).
pub async fn connect_dev(path: &Path) -> zbus::Result<zbus::Connection> {
    let stream = tokio::net::UnixStream::connect(path).await?;
    zbus::connection::Builder::unix_stream(stream)
        .p2p()
        .build()
        .await
}
