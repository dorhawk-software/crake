//! Windows: the machine's interfaces (GetAdaptersAddresses), sockets bound
//! to a tunnel's interface (IP_UNICAST_IF), openconnect's process. Not
//! built yet: what has no Windows form yet says so.

use std::net::Ipv4Addr;
use std::path::Path;

use tokio::process::{Child, Command};

/// Every IPv4 address on the machine: interface, address, netmask.
pub fn interfaces() -> Vec<(String, Ipv4Addr, u32)> {
    Vec::new()
}

/// A socket that takes the tunnel's path.
pub fn mark(_sock: &tokio::net::TcpSocket, _mark: u32) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "a tunnel's path for probes isn't built for Windows yet",
    ))
}

/// A socket that leaves through this interface.
pub fn bind_device(_sock: &tokio::net::UdpSocket, _device: &str) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "binding to a tunnel isn't built for Windows yet",
    ))
}

/// openconnect stays in craked's console group on Windows.
pub fn own_group(_cmd: &mut Command) {}

/// Stop openconnect. Windows has no SIGHUP to keep its session: it ends.
pub async fn stop(child: &mut Child, _keep_session: bool) {
    let _ = child.kill().await;
}

/// A new file for craked alone, replacing any older one (the service's
/// folder is only open to SYSTEM and administrators).
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let _ = std::fs::remove_file(path);
    std::fs::File::create(path)
}
