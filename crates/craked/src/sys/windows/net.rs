//! Windows: a tunnel's address and its DNS route (IP Helper API). openconnect
//! names Wintun's adapter `crake-<name>` (`--interface`) and, with Crake's
//! hook as its script, leaves the rest to craked. The adapter goes away
//! with openconnect, and its address and routes with it.

use std::net::Ipv4Addr;

use anyhow::Result;

use super::iphelper;

/// A tun device OpenVPN 3 made. Never asked here: `attach` refuses
/// OpenVPN 3 sessions first (`ADOPTS_OPENVPN3`). Whoever lets craked adopt
/// sessions it doesn't run here gives this system its own check.
pub fn is_tun(_dev: &str) -> bool {
    false
}

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

    /// The tunnel's IPv6 address.
    pub async fn assign6(&self, dev: &str, addr: std::net::Ipv6Addr, prefix: u8) -> Result<()> {
        if let Self::Noop = self {
            return Ok(());
        }
        iphelper::set_address(iphelper::luid(dev)?, addr.into(), prefix)
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
        iphelper::set_address(luid, addr.into(), prefix)?;
        if let Some(mtu) = mtu {
            iphelper::set_mtu(luid, mtu, false)?;
            // IPv6 needs at least 1280; an adapter without it says no.
            if let Err(e) = iphelper::set_mtu(luid, mtu, true) {
                tracing::debug!("{dev}'s IPv6 MTU: {e:#}");
            }
        }
        Ok(())
    }

    /// craked's own DNS links (`hush`, `keep`) need no interface on Windows:
    /// craked's DNS server answers for them (`dns_queue`).
    pub async fn dummy(&self, _dev: &str, _addr: Ipv4Addr) -> Result<()> {
        Ok(())
    }

    /// Wintun's adapter goes with openconnect.
    pub async fn remove(&self, _dev: &str) {}

    /// Linux's reverse-path filter; Windows has none in the way.
    pub fn loose_rp_filter(&self, _dev: &str) -> Result<()> {
        Ok(())
    }

    /// The same for an IPv6 DNS server.
    pub async fn route_dns6(&self, dev: &str, dns: std::net::Ipv6Addr) -> Result<()> {
        match self {
            Self::Ip => {
                iphelper::add_route(iphelper::luid(dev)?, dns.into(), crate::hosts::V6_BITS)
            }
            Self::Noop => Ok(()),
        }
    }

    /// The tunnel's DNS server, through the tunnel (the one route Crake
    /// adds up front).
    pub async fn route_dns(&self, dev: &str, dns: Ipv4Addr) -> Result<()> {
        match self {
            Self::Ip => {
                iphelper::add_route(iphelper::luid(dev)?, dns.into(), crate::hosts::V4_BITS)
            }
            Self::Noop => Ok(()),
        }
    }
}
