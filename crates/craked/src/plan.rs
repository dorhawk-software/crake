//! One tunnel's routing as craked wants it (a `Plan`), and the numbers each
//! tunnel gets by its position (mark, routing table, rule priority). What
//! the operating system's routing (`routing`) puts in place. Portable.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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
    /// Its IPv6 DNS servers, likewise.
    pub dns6: &'a [std::net::Ipv6Addr],
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

/// IPv6 that stays on its way whatever a full tunnel does: loopback,
/// link-local, multicast.
pub const KEPT6: [&str; 3] = ["::1/128", "fe80::/10", "ff00::/8"];

/// The IPv6 address ranges (first, last) neither `except`'s IPv6 targets
/// nor `KEPT6` cover: what a full tunnel without an IPv6 address refuses,
/// where a firewall takes ranges rather than "all but".
pub fn uncovered6(except: &[Target]) -> Vec<(Ipv6Addr, Ipv6Addr)> {
    let kept = KEPT6.iter().filter_map(|k| k.parse::<Target>().ok());
    let mut spans: Vec<(u128, u128)> = except
        .iter()
        .copied()
        .chain(kept)
        .filter_map(|t| match t.addr() {
            IpAddr::V6(a) => {
                let size = u128::MAX
                    .checked_shr(u32::from(t.prefix_len()))
                    .unwrap_or(0);
                let start = u128::from(a) & !size;
                Some((start, start | size))
            }
            IpAddr::V4(_) => None,
        })
        .collect();
    spans.sort_unstable();
    let mut gaps = Vec::new();
    let mut next: Option<u128> = Some(0);
    for (start, end) in spans {
        if let Some(from) = next
            && start > from
        {
            gaps.push((from, start - 1));
        }
        next = match next {
            Some(from) if end < from => Some(from),
            _ => end.checked_add(1),
        };
    }
    if let Some(from) = next {
        gaps.push((from, u128::MAX));
    }
    gaps.into_iter()
        .map(|(a, b)| (Ipv6Addr::from(a), Ipv6Addr::from(b)))
        .collect()
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

    #[test]
    fn uncovered6_leaves_out_the_kept_and_the_exceptions() {
        let t = |s: &str| s.parse::<Target>().unwrap();
        let a = |s: &str| s.parse::<Ipv6Addr>().unwrap();
        let gaps = uncovered6(&[t("fd00:1a::/64"), t("192.168.0.0/24")]);
        assert_eq!(gaps.first(), Some(&(a("::"), a("::"))), "up to loopback");
        assert!(gaps.contains(&(a("::2"), a("fd00:19:ffff:ffff:ffff:ffff:ffff:ffff"))));
        assert!(gaps.contains(&(
            a("fd00:1a:0:1::"),
            a("fe7f:ffff:ffff:ffff:ffff:ffff:ffff:ffff")
        )));
        assert!(gaps.contains(&(a("fec0::"), a("feff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"))));
        assert_eq!(gaps.len(), 4, "{gaps:?}");
        // Nothing is in two places: no range touches a kept or excepted one.
        for (first, last) in &gaps {
            for kept in ["::1", "fe80::1", "ff02::1", "fd00:1a::5"] {
                let k = a(kept);
                assert!(!(k >= *first && k <= *last), "{kept} in {first}-{last}");
            }
        }
    }
}
