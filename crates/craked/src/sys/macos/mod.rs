//! macOS: the same interfaces as Linux, not built yet (the macOS plan in
//! CLAUDE.md): per-host routes on the tunnel's utun interface (the routing
//! socket), /etc/resolver files pointing the tunnels' domains at craked's
//! own DNS server, the TCP connection table. Until then each says so
//! instead of doing nothing.

pub mod conntrack;
pub mod dns_queue;
pub mod net;
pub mod resolved;
pub mod routing;

mod host;
pub use host::*;

/// What every macOS part says until it's built.
pub(crate) fn not_yet(what: &str) -> anyhow::Error {
    anyhow::anyhow!("{what} isn't built for macOS yet")
}
