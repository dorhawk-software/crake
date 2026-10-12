//! Windows: craked's own DNS server (`sys::dns_server`, shared with macOS)
//! on a loopback address of its own, where NRPT rules send the tunnels'
//! domains (`resolved`).

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

use anyhow::Result;

use crate::manager::Manager;

/// Where the server listens (port 53: NRPT rules name a server, no port).
pub const ADDR: Ipv4Addr = Ipv4Addr::new(127, 53, 0, 1);

pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    super::super::dns_server::spawn(mgr, SocketAddrV4::new(ADDR, crate::dns::PORT))
}
