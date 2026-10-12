//! Linux: nftables and policy routing (`routing`), netlink and iproute2
//! (`net`), systemd-resolved (`resolved`), conntrack events (`conntrack`)
//! and NFQUEUE (`dns_queue`).

pub mod conntrack;
pub mod dns_queue;
pub mod net;
mod netlink;
pub mod resolved;
pub mod routing;
pub mod syn_queue;

mod host;
pub use host::*;

/// openconnect names the tunnel's device as told: `crake-<name>`.
pub const NAMES_DEVICES: bool = true;

/// The device openconnect reported is the tunnel's own.
pub fn tunnel_device(tunnel: &str, device: &str) -> bool {
    device == crake_proto::interface_name(tunnel)
}

/// One of Crake's tunnels' devices.
pub fn is_tunnel_device(name: &str) -> bool {
    name.starts_with("crake-")
}
