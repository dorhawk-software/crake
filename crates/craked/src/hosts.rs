//! What Crake sends through each tunnel: pinned hosts and networks (kept until
//! unpinned) and learned hosts (expire after a while without traffic).
//! Saved to disk so decisions survive restarts; the kernel sets are a projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Address lengths in bits: a single host's prefix.
pub const V4_BITS: u8 = Ipv4Addr::BITS as u8;
pub const V6_BITS: u8 = Ipv6Addr::BITS as u8;
/// The widest networks Crake will route; anything wider is most of the
/// internet.
const V4_WIDEST: u8 = 8;
const V6_WIDEST: u8 = 16;
/// Link-local unicast, fe80::/10, in an address's first 16 bits.
const LINK_LOCAL_V6: u16 = 0xfe80;
const LINK_LOCAL_V6_MASK: u16 = 0xffc0;

/// A host (a full-length prefix: `/32` for IPv4, `/128` for IPv6) or a
/// network, always stored with host bits cleared. Either address family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Target {
    addr: IpAddr,
    len: u8,
}

impl Target {
    pub fn host(addr: impl Into<IpAddr>) -> Self {
        let addr = addr.into();
        Self {
            len: full_len(addr),
            addr,
        }
    }

    pub fn addr(self) -> IpAddr {
        self.addr
    }

    pub fn prefix_len(self) -> u8 {
        self.len
    }

    pub fn is_host(self) -> bool {
        self.len == full_len(self.addr)
    }

    pub fn is_ipv6(self) -> bool {
        self.addr.is_ipv6()
    }

    fn mask(self) -> u128 {
        mask_bits(self.len, full_len(self.addr))
    }

    pub fn contains(self, ip: impl Into<IpAddr>) -> bool {
        let ip = ip.into();
        if ip.is_ipv6() != self.addr.is_ipv6() {
            return false;
        }
        host_bits(ip) & self.mask() == host_bits(self.addr)
    }

    /// `other` lies wholly inside this one (the same, or narrower).
    pub fn covers_target(self, other: Target) -> bool {
        self.len <= other.len && self.contains(other.addr)
    }

    /// Refuse what would break the machine or can't be meant: loopback,
    /// multicast, link-local, and anything far too wide (wider than a /8 for
    /// IPv4, /16 for IPv6).
    pub fn check_routable(self) -> Result<()> {
        let min = if self.addr.is_ipv6() {
            V6_WIDEST
        } else {
            V4_WIDEST
        };
        if self.len < min {
            bail!("{self} is too wide; the widest network Crake will route is a /{min}");
        }
        let bad = match self.addr {
            IpAddr::V4(a) => {
                a.is_unspecified()
                    || a.is_loopback()
                    || a.is_multicast()
                    || a.is_broadcast()
                    || a.is_link_local()
            }
            IpAddr::V6(a) => {
                a.is_unspecified()
                    || a.is_loopback()
                    || a.is_multicast()
                    // Link-local unicast fe80::/10.
                    || a.segments()[0] & LINK_LOCAL_V6_MASK == LINK_LOCAL_V6
            }
        };
        if bad {
            bail!("{self} can't go through a tunnel");
        }
        Ok(())
    }
}

/// How many bits a full-length prefix has for this address's family.
fn full_len(addr: IpAddr) -> u8 {
    if addr.is_ipv6() { V6_BITS } else { V4_BITS }
}

/// The address as its bits in the low end of a u128 (IPv4 in the low 32).
fn host_bits(addr: IpAddr) -> u128 {
    match addr {
        IpAddr::V4(a) => u128::from(u32::from(a)),
        IpAddr::V6(a) => u128::from(a),
    }
}

/// The top `len` bits set within a `full`-bit (32 or 128) address space.
fn mask_bits(len: u8, full: u8) -> u128 {
    if len == 0 {
        return 0;
    }
    let full = u32::from(full);
    let base = if full >= u32::from(V6_BITS) {
        u128::MAX
    } else {
        (1u128 << full) - 1
    };
    (base << (full - u32::from(len))) & base
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        let (a_part, l_part) = match s.split_once('/') {
            Some((a, l)) => (a, Some(l)),
            None => (s, None),
        };
        let addr: IpAddr = a_part
            .parse()
            .with_context(|| format!("{s:?} is not an IP address or network"))?;
        let max = full_len(addr);
        let len = match l_part {
            Some(l) => l
                .parse::<u8>()
                .ok()
                .filter(|l| *l <= max)
                .with_context(|| format!("{s:?} has a bad prefix length"))?,
            None => max,
        };
        let t = Self { addr, len };
        let bits = host_bits(addr) & t.mask();
        let normalized = match addr {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::from(bits as u32)),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::from(bits)),
        };
        if normalized != addr {
            bail!(
                "{s} has host bits set; did you mean {}?",
                Self {
                    addr: normalized,
                    len
                }
            );
        }
        Ok(Self {
            addr: normalized,
            len,
        })
    }
}

impl TryFrom<String> for Target {
    type Error = anyhow::Error;
    fn try_from(s: String) -> Result<Self> {
        s.parse()
    }
}

impl From<Target> for String {
    fn from(t: Target) -> String {
        t.to_string()
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_host() {
            write!(f, "{}", self.addr)
        } else {
            write!(f, "{}/{}", self.addr, self.len)
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TunnelHosts {
    #[serde(default)]
    pub pinned: BTreeSet<Target>,
    /// Host -> expiry, seconds since the epoch.
    #[serde(default)]
    pub learned: BTreeMap<IpAddr, u64>,
    /// Hosts you sent direct (undid): never learned again until you redo.
    #[serde(default)]
    pub direct: BTreeSet<IpAddr>,
    /// The name a learned host was learned under, when it came from DNS.
    #[serde(default)]
    pub names: BTreeMap<IpAddr, String>,
    /// When and how a learned host was learned: (seconds since the epoch,
    /// "probe" | "dns" | "inside" | "split" | "you").
    #[serde(default)]
    pub learned_how: BTreeMap<IpAddr, (u64, String)>,
    /// Last gateway address seen; nothing that covers it may be routed.
    #[serde(default)]
    pub gateway: Option<Ipv4Addr>,
    /// Domains it pushed when last connected: kept private while it's off.
    #[serde(default)]
    pub domains: Vec<String>,
    /// What it got when last connected: its DNS servers and its own
    /// network. Before it's switched on again, checked against the tunnels
    /// that are up (they can't share them).
    #[serde(default)]
    pub last_dns: Vec<Ipv4Addr>,
    #[serde(default)]
    pub last_net: Option<Target>,
    /// The last good answer its DNS gave for each name: (addresses, when),
    /// to keep known names working while that DNS doesn't answer.
    #[serde(default)]
    pub answers: BTreeMap<String, (Vec<Ipv4Addr>, u64)>,
}

impl TunnelHosts {
    /// Drop learned hosts whose time is up.
    pub fn expire(&mut self, now: u64) {
        self.learned.retain(|_, exp| *exp > now);
        let (learned, pinned, direct) = (&self.learned, &self.pinned, &self.direct);
        self.names.retain(|a, _| {
            learned.contains_key(a) || pinned.contains(&Target::host(*a)) || direct.contains(a)
        });
        self.learned_how
            .retain(|a, _| learned.contains_key(a) || pinned.contains(&Target::host(*a)));
    }

    /// Already sent through the tunnel, learned or pinned (directly or by network).
    pub fn covers(&self, addr: impl Into<IpAddr>) -> bool {
        let addr = addr.into();
        self.learned.contains_key(&addr) || self.pinned.iter().any(|p| p.contains(addr))
    }

    /// Seconds left for each learned host.
    pub fn learned_remaining(&self, now: u64) -> Vec<(IpAddr, u64)> {
        self.learned
            .iter()
            .filter(|(_, e)| **e > now)
            .map(|(a, e)| (*a, e - now))
            .collect()
    }

    /// Remember what its DNS said for `name`. True when that's news.
    pub fn remember(&mut self, name: &str, addrs: &[Ipv4Addr], now: u64) -> bool {
        const KEEP: usize = 1000;
        let fresh = self.answers.get(name).is_none_or(|(a, at)| {
            a != addrs || now.saturating_sub(*at) > crate::units::SECS_PER_HOUR
        });
        if fresh {
            self.answers.insert(name.to_owned(), (addrs.to_vec(), now));
            if self.answers.len() > KEEP {
                // The longest unseen goes.
                if let Some(old) = self
                    .answers
                    .iter()
                    .min_by_key(|(_, (_, at))| *at)
                    .map(|(n, _)| n.clone())
                {
                    self.answers.remove(&old);
                }
            }
        }
        fresh
    }

    pub fn is_empty(&self) -> bool {
        self.pinned.is_empty() && self.learned.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct HostStore {
    path: Option<PathBuf>,
    pub tunnels: BTreeMap<String, TunnelHosts>,
}

impl HostStore {
    /// A missing file is an empty store. A damaged one is set aside, not lost.
    pub fn load(path: &Path) -> Self {
        let tunnels = match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(t) => t,
                Err(e) => {
                    let aside = path.with_extension("damaged");
                    tracing::error!(
                        "{} is damaged ({e}); moved to {}",
                        path.display(),
                        aside.display()
                    );
                    let _ = std::fs::rename(path, &aside);
                    BTreeMap::new()
                }
            },
            Err(_) => BTreeMap::new(),
        };
        Self {
            path: Some(path.to_owned()),
            tunnels,
        }
    }

    pub fn tunnel(&mut self, name: &str) -> &mut TunnelHosts {
        self.tunnels.entry(name.to_owned()).or_default()
    }

    /// Write atomically (temp file plus rename).
    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let res = (|| -> Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(&self.tunnels)?)?;
            std::fs::rename(&tmp, path)?;
            Ok(())
        })();
        if let Err(e) = res {
            tracing::error!("saving {}: {e:#}", path.display());
        }
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        let h: Target = "10.33.1.1".parse().unwrap();
        assert!(h.is_host());
        assert_eq!(h.to_string(), "10.33.1.1");
        let v4 = |s: &str| s.parse::<IpAddr>().unwrap();
        let n: Target = "10.40.0.0/16".parse().unwrap();
        assert!(n.contains(v4("10.40.9.9")));
        assert!(!n.contains(v4("10.41.0.1")));
        assert!("10.40.0.1/16".parse::<Target>().is_err());
        assert!("10.40.0.0/33".parse::<Target>().is_err());
        assert!("intranet".parse::<Target>().is_err());
        let bad = |s: &str| s.parse::<Target>().unwrap().check_routable().is_err();
        let ok = |s: &str| s.parse::<Target>().unwrap().check_routable().is_ok();
        assert!(bad("0.0.0.0/0"));
        assert!(bad("10.0.0.0/7"));
        assert!(bad("127.0.0.1"));
        assert!(bad("169.254.1.1"));
        assert!(ok("10.0.0.0/8"));
    }

    #[test]
    fn ipv6_targets() {
        let v6 = |s: &str| s.parse::<IpAddr>().unwrap();
        let h: Target = "fd00:dead:beef::1".parse().unwrap();
        assert!(h.is_host());
        assert!(h.is_ipv6());
        assert_eq!(h.prefix_len(), 128);
        assert_eq!(h.to_string(), "fd00:dead:beef::1");

        let n: Target = "fd00:dead:beef::/48".parse().unwrap();
        assert!(!n.is_host());
        assert!(n.contains(v6("fd00:dead:beef:1234::9")));
        assert!(!n.contains(v6("fd00:dead:beff::1")));
        // Families don't mix: a v6 network never contains a v4 address.
        assert!(!n.contains("10.0.0.1".parse::<IpAddr>().unwrap()));
        assert_eq!(n.to_string(), "fd00:dead:beef::/48");

        assert!("fd00::1/48".parse::<Target>().is_err()); // host bits set
        assert!("fd00::/129".parse::<Target>().is_err()); // bad prefix
        let bad = |s: &str| s.parse::<Target>().unwrap().check_routable().is_err();
        let ok = |s: &str| s.parse::<Target>().unwrap().check_routable().is_ok();
        assert!(bad("::/0")); // too wide
        assert!(bad("fd00::/8")); // wider than /16
        assert!(bad("::1")); // loopback
        assert!(bad("fe80::1")); // link-local
        assert!(bad("ff02::1")); // multicast
        assert!(ok("fd00:dead:beef::/48"));
        assert!(ok("2001:db8::1")); // documentation host
    }

    #[test]
    fn store_round_trip_and_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let mut s = HostStore::load(&path);
        let t = s.tunnel("acme");
        t.pinned.insert("10.40.0.0/16".parse().unwrap());
        t.learned.insert("10.33.1.1".parse().unwrap(), 1000);
        t.learned.insert("10.33.1.2".parse().unwrap(), 10);
        s.save();

        let mut s = HostStore::load(&path);
        let t = s.tunnel("acme");
        assert_eq!(t.pinned.len(), 1);
        assert_eq!(
            t.learned_remaining(100),
            vec![("10.33.1.1".parse().unwrap(), 900)]
        );
        t.expire(100);
        assert_eq!(t.learned.len(), 1);
    }

    #[test]
    fn damaged_store_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        std::fs::write(&path, "{not json").unwrap();
        let s = HostStore::load(&path);
        assert!(s.tunnels.is_empty());
        assert!(dir.path().join("hosts.damaged").exists());
    }
}
