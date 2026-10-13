//! macOS: craked's own DNS server (`sys::dns_server`, shared with Windows)
//! on a loopback address of its own, where /etc/resolver files send the
//! tunnels' domains (`resolved`) and, while a tunnel is up, the system
//! every other name (`primary_dns`). Port 53: the system's DNS settings
//! can't name another; an address of its own (as on Windows) keeps clear
//! of a DNS server someone runs on 127.0.0.1.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

use anyhow::{Result, bail};

use crate::manager::Manager;

// literal: craked's address, as on Windows (127.53.0.1)
pub const ADDR: Ipv4Addr = Ipv4Addr::new(127, 53, 0, 1);
pub const PORT: u16 = crate::dns::PORT;

pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    add_address()?;
    super::super::dns_server::spawn(mgr, SocketAddrV4::new(ADDR, PORT))
}

/// craked's address on the loopback interface (macOS has only 127.0.0.1
/// there); it stays until the Mac restarts, and adding it again is fine.
pub fn add_address() -> Result<()> {
    let out = std::process::Command::new("ifconfig")
        .args(["lo0", "alias", &ADDR.to_string(), "up"])
        .output()?;
    if !out.status.success() {
        bail!(
            "ifconfig lo0 alias {ADDR}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}
