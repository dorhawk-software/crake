//! macOS: the same interfaces as Linux (the macOS plan in CLAUDE.md):
//! per-host routes on the tunnel's utun device and its address (`route`,
//! `ifconfig`: `routing`, `net`), /etc/resolver files sending the tunnels'
//! domains to craked's own DNS server (`resolved`, `dns_queue`), and
//! every name while a tunnel is up (`primary_dns`), the
//! kernel's TCP connection list for probing (`conntrack`).

pub mod conntrack;
pub mod dns_queue;
pub mod net;
pub mod report;
pub mod resolved;
pub mod routing;
pub mod syn_queue;

mod cmd;
mod host;
mod primary_dns;
pub use host::*;

/// macOS names utun devices itself (utun0, utun1, ...): openconnect takes
/// the next free one, and craked goes by the one it reports.
pub const NAMES_DEVICES: bool = false;

/// OpenVPN 3 sessions are handed to craked: not here (OpenVPN 3 Linux,
/// the service the app drives, is Linux's only).
pub const ADOPTS_OPENVPN3: bool = false;

/// The device openconnect reported is a utun device.
pub fn tunnel_device(_tunnel: &str, device: &str) -> bool {
    device.starts_with("utun")
}

/// A tunnel's device: Crake's or another VPN's, never a local network.
pub fn is_tunnel_device(name: &str) -> bool {
    name.starts_with("utun")
}
