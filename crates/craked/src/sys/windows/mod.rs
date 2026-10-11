//! Windows: the same interfaces as Linux (the Windows plan in CLAUDE.md):
//! per-host routes through the IP Helper API (`routing`, `net`), NRPT rules
//! to craked's own DNS server (`resolved`, `dns_queue`), the TCP connection
//! table (`conntrack`).

pub mod conntrack;
pub mod dns_queue;
pub mod net;
pub mod resolved;
pub mod routing;

mod host;
mod iphelper;
pub use host::*;
