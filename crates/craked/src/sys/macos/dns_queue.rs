//! macOS: craked's own DNS server (`sys::dns_server`, shared with Windows)
//! on the loopback address, where /etc/resolver files send the tunnels'
//! domains (`resolved`). Not port 53: an /etc/resolver file names the port.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

use anyhow::Result;

use crate::manager::Manager;

pub const ADDR: Ipv4Addr = Ipv4Addr::LOCALHOST;
pub const PORT: u16 = 53530;

pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    super::super::dns_server::spawn(mgr, SocketAddrV4::new(ADDR, PORT))
}
