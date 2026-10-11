//! Windows: the same interfaces as Linux, not built yet (phase 3 of the
//! Windows plan in CLAUDE.md): per-host routes through the IP Helper API
//! (`routing`, `net`), NRPT rules to craked's own DNS forwarder
//! (`resolved`, `dns_queue`), the TCP connection table (`conntrack`).
//! Until then each says so instead of doing nothing.

pub mod conntrack;
pub mod dns_queue;
pub mod net;
pub mod resolved;
pub mod routing;

mod host;
pub use host::*;

/// What every Windows part says until it's built.
pub(crate) fn not_yet(what: &str) -> anyhow::Error {
    anyhow::anyhow!("{what} isn't built for Windows yet")
}
