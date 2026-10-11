//! Windows: a tunnel's address and its DNS route (IP Helper API). openconnect
//! names Wintun's adapter `crake-<name>` (`--interface`) and, with Crake's
//! hook as its script, leaves the rest to craked. The adapter goes away
//! with openconnect, and its address and routes with it.

use std::net::Ipv4Addr;

use anyhow::Result;

use super::iphelper;

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
        dev: &str,
        addr: Ipv4Addr,
        prefix: u8,
        mtu: Option<u32>,
    ) -> Result<()> {
        if let Self::Noop = self {
            return Ok(());
        }
        let luid = iphelper::luid(dev)?;
        iphelper::set_address(luid, addr, prefix)?;
        if let Some(mtu) = mtu {
            iphelper::set_mtu(luid, mtu)?;
        }
        Ok(())
    }

    /// craked's own DNS links (`hush`, `keep`): Linux's systemd-resolved
    /// form. Windows will use NRPT rules to craked's DNS instead.
    pub async fn dummy(&self, _dev: &str, _addr: Ipv4Addr) -> Result<()> {
        match self {
            Self::Ip => Err(super::not_yet("craked's own DNS links")),
            Self::Noop => Ok(()),
        }
    }

    /// Wintun's adapter goes with openconnect.
    pub async fn remove(&self, _dev: &str) {}

    /// Linux's reverse-path filter; Windows has none in the way.
    pub fn loose_rp_filter(&self, _dev: &str) -> Result<()> {
        Ok(())
    }

    /// The tunnel's DNS server, through the tunnel (the one route Crake
    /// adds up front).
    pub async fn route_dns(&self, dev: &str, dns: Ipv4Addr) -> Result<()> {
        match self {
            Self::Ip => iphelper::add_route(iphelper::luid(dev)?, dns, 32),
            Self::Noop => Ok(()),
        }
    }
}
