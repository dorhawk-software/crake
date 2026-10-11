//! macOS: the same interfaces as Linux (the macOS plan in CLAUDE.md):
//! per-host routes on the tunnel's utun device and its address (`route`,
//! `ifconfig`: `routing`, `net`), /etc/resolver files sending the tunnels'
//! domains to craked's own DNS server (`resolved`, `dns_queue`). Not built
//! yet: the TCP connection table (`conntrack`), which says so.

pub mod conntrack;
pub mod dns_queue;
pub mod net;
pub mod resolved;
pub mod routing;

mod cmd;
mod host;
pub use host::*;

/// What every macOS part says until it's built.
pub(crate) fn not_yet(what: &str) -> anyhow::Error {
    anyhow::anyhow!("{what} isn't built for macOS yet")
}

/// macOS names utun devices itself (utun0, utun1, ...): openconnect takes
/// the next free one, and craked goes by the one it reports.
pub const NAMES_DEVICES: bool = false;

/// The device openconnect reported is a utun device.
pub fn tunnel_device(_tunnel: &str, device: &str) -> bool {
    device.starts_with("utun")
}

/// A tunnel's device: Crake's or another VPN's, never a local network.
pub fn is_tunnel_device(name: &str) -> bool {
    name.starts_with("utun")
}
