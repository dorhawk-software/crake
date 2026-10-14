//! Windows: the same interfaces as Linux (the Windows plan in CLAUDE.md):
//! per-host routes through the IP Helper API (`routing`, `net`), NRPT rules
//! to craked's own DNS server (`resolved`, `dns_queue`), the TCP connection
//! table (`conntrack`).

pub mod conntrack;
pub mod dns_queue;
pub mod net;
pub mod resolved;
pub mod routing;
pub mod syn_queue;

mod firewall;
mod host;
mod iphelper;
pub use host::*;

/// openconnect names Wintun's adapter as told: `crake-<name>`.
pub const NAMES_DEVICES: bool = true;

/// OpenVPN 3 sessions are handed to craked: not here (OpenVPN 3 Linux,
/// the service the app drives, is Linux's only).
pub const ADOPTS_OPENVPN3: bool = false;

/// The device openconnect reported is the tunnel's own.
pub fn tunnel_device(tunnel: &str, device: &str) -> bool {
    device == crake_proto::interface_name(tunnel)
}

/// One of Crake's tunnels' devices.
pub fn is_tunnel_device(name: &str) -> bool {
    name.starts_with("crake-")
}
