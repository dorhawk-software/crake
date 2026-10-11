//! What Linux and macOS do alike: private files, whether a program can
//! run, and the development connection to craked.

use std::io::Write;
use std::path::Path;

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

/// craked's private socket (development: `craked --dev`).
pub async fn connect_dev(path: &Path) -> zbus::Result<zbus::Connection> {
    let stream = tokio::net::UnixStream::connect(path).await?;
    zbus::connection::Builder::unix_stream(stream)
        .p2p()
        .build()
        .await
}
