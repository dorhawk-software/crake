//! What Linux and macOS do alike: the machine's interfaces (getifaddrs),
//! openconnect's process (its own group, stopped with signals) and private
//! files.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::time::Duration;

use tokio::process::{Child, Command};

/// Every IPv4 address on the machine: interface, address, netmask.
pub fn interfaces() -> Vec<(String, Ipv4Addr, u32)> {
    crake_os::unix::interfaces()
}

/// Every IPv6 address on the machine: interface, address, prefix length.
pub fn interfaces6() -> Vec<(String, Ipv6Addr, u8)> {
    crake_os::unix::interfaces6()
}

/// openconnect in its own process group: Ctrl+C on a terminal reaches
/// craked only, which then stops openconnect in order.
pub fn own_group(cmd: &mut Command) {
    cmd.process_group(0);
}

/// Stop openconnect: SIGHUP keeps its session (to be resumed with the same
/// cookie), SIGTERM logs off; either way it calls the hook with
/// `disconnect`. Killed after a grace period.
pub async fn stop(child: &mut Child, keep_session: bool) {
    if let Some(pid) = child.id() {
        let signal = if keep_session {
            libc::SIGHUP
        } else {
            libc::SIGTERM
        };
        let _ = crake_os::unix::signal(pid, signal);
    }
    if tokio::time::timeout(STOP_WAIT, child.wait()).await.is_err() {
        let _ = child.kill().await;
    }
}

/// Read and written by its owner alone.
pub const OWNER_ONLY: u32 = 0o600;
/// A stopped child gets this long before it's killed.
const STOP_WAIT: Duration = Duration::from_secs(10);

/// A folder for root alone (0700): OpenVPN's profiles and runs.
pub fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    /// Owner only, for a folder.
    const OWNER_ONLY_DIR: u32 = 0o700;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(OWNER_ONLY_DIR))
}

/// A new file for root alone (0600), replacing any older one.
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(OWNER_ONLY)
        .open(path)
}
