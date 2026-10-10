//! What Crake sends through each tunnel: pinned hosts and networks (kept until
//! unpinned) and learned hosts (expire after a while without traffic).
//! Saved to disk so decisions survive restarts; the kernel sets are a projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// A host (`/32`) or a network, always stored with host bits cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Target {
    addr: Ipv4Addr,
    len: u8,
}

impl Target {
    pub fn host(addr: Ipv4Addr) -> Self {
        Self { addr, len: 32 }
    }

    pub fn addr(self) -> Ipv4Addr {
        self.addr
    }

    pub fn prefix_len(self) -> u8 {
        self.len
    }

    pub fn is_host(self) -> bool {
        self.len == 32
    }

    fn mask(self) -> u32 {
        u32::MAX.checked_shl(32 - u32::from(self.len)).unwrap_or(0)
    }

    pub fn contains(self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & self.mask() == u32::from(self.addr)
    }

    /// Refuse what would break the machine or can't be meant: loopback,
    /// multicast, link-local, and anything wider than a /8.
    pub fn check_routable(self) -> Result<()> {
        let a = self.addr;
        if self.len < 8 {
            bail!("{self} is too wide; the widest network Crake will route is a /8");
        }
        if a.is_unspecified()
            || a.is_loopback()
            || a.is_multicast()
            || a.is_broadcast()
            || a.is_link_local()
        {
            bail!("{self} can't go through a tunnel");
        }
        Ok(())
    }
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        let (addr, len) = match s.split_once('/') {
            Some((a, l)) => (
                a,
                l.parse::<u8>()
                    .ok()
                    .filter(|l| *l <= 32)
                    .with_context(|| format!("{s:?} has a bad prefix length"))?,
            ),
            None => (s, 32),
        };
        let addr: Ipv4Addr = addr
            .parse()
            .with_context(|| format!("{s:?} is not an IPv4 address or network"))?;
        let t = Self { addr, len };
        let normalized = Self {
            addr: Ipv4Addr::from(u32::from(addr) & t.mask()),
            len,
        };
        if normalized.addr != addr {
            bail!("{s} has host bits set; did you mean {normalized}?");
        }
        Ok(normalized)
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
    pub learned: BTreeMap<Ipv4Addr, u64>,
    /// Hosts you sent direct (undid): never learned again until you redo.
    #[serde(default)]
    pub direct: BTreeSet<Ipv4Addr>,
    /// The name a learned host was learned under, when it came from DNS.
    #[serde(default)]
    pub names: BTreeMap<Ipv4Addr, String>,
    /// When and how a learned host was learned: (seconds since the epoch,
    /// "probe" | "dns" | "inside" | "split" | "you").
    #[serde(default)]
    pub learned_how: BTreeMap<Ipv4Addr, (u64, String)>,
    /// Last gateway address seen; nothing that covers it may be routed.
    #[serde(default)]
    pub gateway: Option<Ipv4Addr>,
    /// Domains it pushed when last connected: kept private while it's off.
    #[serde(default)]
    pub domains: Vec<String>,
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
    pub fn covers(&self, addr: Ipv4Addr) -> bool {
        self.learned.contains_key(&addr) || self.pinned.iter().any(|p| p.contains(addr))
    }

    /// Seconds left for each learned host.
    pub fn learned_remaining(&self, now: u64) -> Vec<(Ipv4Addr, u64)> {
        self.learned
            .iter()
            .filter(|(_, e)| **e > now)
            .map(|(a, e)| (*a, e - now))
            .collect()
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
        let n: Target = "10.40.0.0/16".parse().unwrap();
        assert!(n.contains("10.40.9.9".parse().unwrap()));
        assert!(!n.contains("10.41.0.1".parse().unwrap()));
        assert!("10.40.0.1/16".parse::<Target>().is_err());
        assert!("10.40.0.0/33".parse::<Target>().is_err());
        assert!("intranet".parse::<Target>().is_err());
        assert!(
            "0.0.0.0/0"
                .parse::<Target>()
                .unwrap()
                .check_routable()
                .is_err()
        );
        assert!(
            "10.0.0.0/7"
                .parse::<Target>()
                .unwrap()
                .check_routable()
                .is_err()
        );
        assert!(
            "127.0.0.1"
                .parse::<Target>()
                .unwrap()
                .check_routable()
                .is_err()
        );
        assert!(
            "169.254.1.1"
                .parse::<Target>()
                .unwrap()
                .check_routable()
                .is_err()
        );
        assert!(
            "10.0.0.0/8"
                .parse::<Target>()
                .unwrap()
                .check_routable()
                .is_ok()
        );
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
