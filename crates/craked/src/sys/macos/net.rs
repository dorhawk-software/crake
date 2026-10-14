//! macOS: a tunnel's address and its DNS route (`ifconfig`, `route`).
//! openconnect makes the utun device and, with Crake's hook as its script,
//! leaves the rest to craked. The device goes away with openconnect, and
//! its address and routes with it.

use std::net::Ipv4Addr;

use anyhow::Result;

use super::cmd;

/// A tun device OpenVPN 3 made: OpenVPN 3 sessions are handed to craked
/// on Linux only.
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

    /// The tunnel's IPv6 address, added to the device (an alias: utun
    /// devices carry a link-local one already).
    pub async fn assign6(&self, dev: &str, addr: std::net::Ipv6Addr, prefix: u8) -> Result<()> {
        if let Self::Noop = self {
            return Ok(());
        }
        let (a, p) = (addr.to_string(), prefix.to_string());
        cmd::run("ifconfig", &[dev, "inet6", &a, "prefixlen", &p, "alias"]).await
    }

    /// The tunnel's address (a utun device is point to point: its own
    /// address at both ends) and MTU.
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
        let mask = Ipv4Addr::from(
            u32::MAX
                .checked_shl(Ipv4Addr::BITS - u32::from(prefix))
                .unwrap_or(0),
        );
        let (a, m) = (addr.to_string(), mask.to_string());
        let mut args = vec![dev, "inet", &a, &a, "netmask", &m];
        let mtu = mtu.map(|m| m.to_string());
        if let Some(mtu) = &mtu {
            args.extend(["mtu", mtu]);
        }
        args.push("up");
        cmd::run("ifconfig", &args).await
    }

    /// craked's own DNS links (`hush`, `keep`) need no interface on macOS:
    /// craked's DNS server answers for them (`dns_queue`).
    pub async fn dummy(&self, _dev: &str, _addr: Ipv4Addr) -> Result<()> {
        Ok(())
    }

    /// The utun device goes with openconnect.
    pub async fn remove(&self, _dev: &str) {}

    /// Linux's reverse-path filter; macOS has none in the way.
    pub fn loose_rp_filter(&self, _dev: &str) -> Result<()> {
        Ok(())
    }

    /// The same for an IPv6 DNS server.
    pub async fn route_dns6(&self, dev: &str, dns: std::net::Ipv6Addr) -> Result<()> {
        match self {
            Self::Ip => cmd::route_add(&dns.to_string(), crate::hosts::V6_BITS, dev).await,
            Self::Noop => Ok(()),
        }
    }

    /// The tunnel's DNS server, through the tunnel (the one route Crake
    /// adds up front).
    pub async fn route_dns(&self, dev: &str, dns: Ipv4Addr) -> Result<()> {
        match self {
            Self::Ip => cmd::route_add(&dns.to_string(), crate::hosts::V4_BITS, dev).await,
            Self::Noop => Ok(()),
        }
    }
}
