//! One tunnel's routing as craked wants it (a `Plan`), and the numbers each
//! tunnel gets by its position (mark, routing table, rule priority). What
//! the operating system's routing (`routing`) puts in place. Portable.

use std::net::{IpAddr, Ipv4Addr};

use crate::hosts::Target;

pub const MARK_BASE: u32 = 0x4352_0000; // "CR"
/// Rule priorities and routing tables are BASE + 1 + tunnel index.
pub const ID_BASE: u32 = 7300;
pub const MAX_TUNNELS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ids {
    pub mark: u32,
    pub table: u32,
    pub priority: u32,
}

pub fn ids(index: usize) -> Ids {
    let n = index as u32 + 1;
    Ids {
        mark: MARK_BASE | n,
        table: ID_BASE + n,
        priority: ID_BASE + n,
    }
}

/// `pins` without the ones another pin already covers (a host or network
/// inside a pinned network): what an nftables interval set takes, as it
/// refuses overlapping elements.
pub fn outermost(pins: &[Target]) -> Vec<Target> {
    let mut out: Vec<Target> = Vec::new();
    for (i, t) in pins.iter().enumerate() {
        let inside = pins
            .iter()
            .enumerate()
            .any(|(j, p)| j != i && p.covers_target(*t) && (*p != *t || j < i));
        if !inside {
            out.push(*t);
        }
    }
    out
}

/// Everything needed to put one tunnel's routing in place.
pub struct Plan<'a> {
    pub name: &'a str,
    pub index: usize,
    pub device: &'a str,
    /// Default expiry for learned hosts, seconds.
    pub timeout: u64,
    /// The tunnel's DNS servers; their answers are queued for learning.
    pub dns: &'a [Ipv4Addr],
    pub pinned: &'a [Target],
    /// Learned hosts with seconds left.
    pub learned: &'a [(IpAddr, u64)],
    /// Full tunnel (a managed policy): everything goes through except these
    /// (the gateway, the local networks). Other tunnels' hosts still go
    /// their own way.
    pub full_except: Option<&'a [Target]>,
    /// The tunnel has an IPv6 address: a full tunnel takes IPv6 too.
    /// Without one, a full tunnel refuses IPv6 rather than let it go
    /// around the tunnel.
    pub ipv6: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outermost_drops_what_another_pin_covers() {
        let t = |s: &str| s.parse::<Target>().unwrap();
        let pins = [
            t("10.40.5.5"),
            t("10.40.0.0/16"),
            t("10.40.1.0/24"),
            t("10.33.1.3"),
            t("fd40::/32"),
            t("fd40:0:1::9"),
        ];
        assert_eq!(
            outermost(&pins),
            [t("10.40.0.0/16"), t("10.33.1.3"), t("fd40::/32")]
        );
        assert_eq!(outermost(&[t("10.40.0.0/16"), t("10.40.0.0/16")]).len(), 1);
    }
}
