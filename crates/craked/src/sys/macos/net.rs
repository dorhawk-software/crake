//! macOS: a tunnel's address and its DNS route (the routing socket), not
//! built yet.

use std::net::Ipv4Addr;

use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub enum Net {
    Ip,
    /// Do nothing (development and tests).
    Noop,
}

impl Net {
    pub fn new(apply: bool) -> Self {
        if apply { Self::Ip } else { Self::Noop }
    }

    pub async fn assign(
        &self,
        _dev: &str,
        _addr: Ipv4Addr,
        _prefix: u8,
        _mtu: Option<u32>,
    ) -> Result<()> {
        self.not_yet("a tunnel's address")
    }

    pub async fn dummy(&self, _dev: &str, _addr: Ipv4Addr) -> Result<()> {
        self.not_yet("craked's own DNS links")
    }

    pub async fn remove(&self, _dev: &str) {}

    pub fn loose_rp_filter(&self, _dev: &str) -> Result<()> {
        Ok(())
    }

    pub async fn route_dns(&self, _dev: &str, _dns: Ipv4Addr) -> Result<()> {
        self.not_yet("a route to a tunnel's DNS")
    }

    fn not_yet(&self, what: &str) -> Result<()> {
        match self {
            Self::Ip => Err(super::not_yet(what)),
            Self::Noop => Ok(()),
        }
    }
}
