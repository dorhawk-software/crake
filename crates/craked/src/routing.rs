//! nftables table `inet crake` plus fwmark routing.
//!
//! Per tunnel (index i, name n):
//! - set `learned_n`: hosts with per-element timeouts, refreshed by traffic
//! - set `pinned_n`: hosts and networks kept until unpinned
//! - chain `out_n` (route, output): destinations in either set get mark M
//! - chain `nat_n` (nat, postrouting): masquerade out of the tunnel, so the
//!   source address matches the tunnel's
//! - chain `dns_n` (filter, input): answers from the tunnel's DNS servers go to
//!   NFQUEUE so craked can route their addresses first (see `dns_queue`)
//! - `ip rule fwmark M lookup T`, and table T's default route is the tunnel.
//!
//! Base chains are per tunnel so a tunnel can be torn down without touching
//! the others. Only locally generated traffic is marked.

use std::net::Ipv4Addr;
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::dns_queue::QUEUE_NUM;
use crate::hosts::Target;

pub const TABLE: &str = "crake";
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

fn learned_set(name: &str) -> String {
    format!("learned_{name}")
}
fn pinned_set(name: &str) -> String {
    format!("pinned_{name}")
}

fn set_defs(name: &str, timeout: u64) -> String {
    // `dynamic` is what nft adds anyway for a set updated from the packet path;
    // spelling it out keeps `add set` idempotent.
    format!(
        "add set inet {TABLE} {} {{ type ipv4_addr; flags dynamic, timeout; timeout {timeout}s; }}\n\
         add set inet {TABLE} {} {{ type ipv4_addr; flags interval; }}\n",
        learned_set(name),
        pinned_set(name)
    )
}

fn chain_defs(name: &str) -> String {
    format!(
        "add chain inet {TABLE} out_{name} {{ type route hook output priority mangle; policy accept; }}\n\
         add chain inet {TABLE} nat_{name} {{ type nat hook postrouting priority srcnat; policy accept; }}\n\
         add chain inet {TABLE} dns_{name} {{ type filter hook input priority filter; policy accept; }}\n"
    )
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
    pub learned: &'a [(Ipv4Addr, u64)],
    /// Full tunnel (a managed policy): everything goes through except these
    /// (the gateway, the local networks). Other tunnels' hosts still go
    /// their own way.
    pub full_except: Option<&'a [Target]>,
}

/// The whole per-tunnel setup as one atomic nft transaction.
pub fn up_script(plan: &Plan) -> String {
    let Plan {
        name,
        index,
        device,
        timeout,
        dns,
        pinned,
        learned,
        full_except,
    } = *plan;
    let mark = ids(index).mark;
    let (ls, ps) = (learned_set(name), pinned_set(name));
    let mut s = format!("add table inet {TABLE}\n");
    s += &set_defs(name, timeout);
    s += &chain_defs(name);
    s += &format!(
        "flush chain inet {TABLE} out_{name}\n\
         flush chain inet {TABLE} nat_{name}\n\
         flush chain inet {TABLE} dns_{name}\n\
         flush set inet {TABLE} {ls}\n\
         flush set inet {TABLE} {ps}\n\
         add rule inet {TABLE} out_{name} ip daddr @{ps} meta mark set {mark:#x}\n\
         add rule inet {TABLE} out_{name} ip daddr @{ls} update @{ls} {{ ip daddr timeout {timeout}s }} meta mark set {mark:#x}\n\
         add rule inet {TABLE} nat_{name} oifname \"{device}\" masquerade\n"
    );
    if let Some(except) = full_except {
        // Only what no other tunnel has marked: their hosts stay theirs,
        // whichever chain runs first.
        let mut list: Vec<String> = [
            "127.0.0.0/8",
            "169.254.0.0/16",
            "224.0.0.0/4",
            "255.255.255.255",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        list.extend(except.iter().map(Target::to_string));
        s += &format!(
            "add rule inet {TABLE} out_{name} meta mark 0 ip daddr != {{ {} }} meta mark set {mark:#x}\n",
            list.join(", ")
        );
    }
    if !dns.is_empty() {
        let servers: Vec<String> = dns.iter().map(Ipv4Addr::to_string).collect();
        s += &format!(
            "add rule inet {TABLE} dns_{name} iifname \"{device}\" ip saddr {{ {} }} udp sport 53 queue num {QUEUE_NUM} bypass\n",
            servers.join(", ")
        );
    }
    if !pinned.is_empty() {
        let list: Vec<String> = pinned.iter().map(Target::to_string).collect();
        s += &format!("add element inet {TABLE} {ps} {{ {} }}\n", list.join(", "));
    }
    let learned: Vec<String> = learned
        .iter()
        .filter(|(_, t)| *t > 0)
        .map(|(a, t)| format!("{a} timeout {t}s"))
        .collect();
    if !learned.is_empty() {
        s += &format!(
            "add element inet {TABLE} {ls} {{ {} }}\n",
            learned.join(", ")
        );
    }
    s
}

/// Remove a tunnel's chains and sets; fine if they don't exist.
/// Tunnel devices Crake didn't name (OpenVPN 3's tun0, ...): their DNS
/// answers are the tunnel's, not public ones.
const TUNNEL_DEVICES: &str = "tunnel_devices";

/// Mark `device` as a tunnel's (or not, when `on` is false). Idempotent.
fn tunnel_device_script(device: &str, on: bool) -> String {
    let mut s = format!(
        "add table inet {TABLE}\nadd set inet {TABLE} {TUNNEL_DEVICES} {{ type ifname; }}\n\
         add element inet {TABLE} {TUNNEL_DEVICES} {{ \"{device}\" }}\n"
    );
    if !on {
        s += &format!("delete element inet {TABLE} {TUNNEL_DEVICES} {{ \"{device}\" }}\n");
    }
    s
}

pub fn down_script(name: &str) -> String {
    // nft has no portable "delete if exists": add (a no-op when present), then delete.
    let mut s = format!("add table inet {TABLE}\n");
    s += &chain_defs(name);
    s += &format!(
        "delete chain inet {TABLE} out_{name}\ndelete chain inet {TABLE} nat_{name}\ndelete chain inet {TABLE} dns_{name}\n"
    );
    s += &set_defs(name, 1);
    s += &format!(
        "delete set inet {TABLE} {}\ndelete set inet {TABLE} {}\n",
        learned_set(name),
        pinned_set(name)
    );
    s
}

#[derive(Debug, Clone, Copy)]
pub enum Routing {
    Real,
    /// Development and tests: decisions are kept, nothing is applied.
    Noop,
}

impl Routing {
    pub fn new(apply: bool) -> Self {
        if apply { Self::Real } else { Self::Noop }
    }

    /// At startup: clear whatever a previous run left behind.
    pub async fn reset_all(&self) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        nft(&format!(
            "add table inet {TABLE}\ndelete table inet {TABLE}\n"
        ))
        .await?;
        let out = Command::new("ip")
            .args(["-j", "rule", "show"])
            .output()
            .await
            .context("running ip")?;
        let rules: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let range = ID_BASE + 1..=ID_BASE + MAX_TUNNELS as u32;
        for r in rules.as_array().into_iter().flatten() {
            if let Some(p) = r["priority"]
                .as_u64()
                .map(|p| p as u32)
                .filter(|p| range.contains(p))
            {
                ip(&["rule", "del", "priority", &p.to_string()]).await.ok();
            }
        }
        Ok(())
    }

    pub async fn up(&self, plan: &Plan<'_>) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        let (index, device) = (plan.index, plan.device);
        let id = ids(index);
        ip(&[
            "route",
            "replace",
            "default",
            "dev",
            device,
            "table",
            &id.table.to_string(),
        ])
        .await?;
        let (mark, prio, table) = (
            format!("{:#x}", id.mark),
            id.priority.to_string(),
            id.table.to_string(),
        );
        // Replace rather than stack up on reconnect.
        ip(&["rule", "del", "priority", &prio]).await.ok();
        ip(&[
            "rule", "add", "fwmark", &mark, "lookup", &table, "priority", &prio,
        ])
        .await?;
        nft(&up_script(plan)).await?;
        if !plan.device.starts_with("crake-") {
            nft(&tunnel_device_script(plan.device, true)).await?;
        }
        Ok(())
    }

    /// Watch public DNS answers (everything not from a tunnel or loopback) so
    /// names that fail publicly can be asked of the tunnels. Idempotent.
    pub async fn public_up(&self) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        nft(&format!(
            "add table inet {TABLE}\n\
             add chain inet {TABLE} public_dns {{ type filter hook input priority filter; policy accept; }}\n\
             add set inet {TABLE} {TUNNEL_DEVICES} {{ type ifname; }}\n\
             flush chain inet {TABLE} public_dns\n\
             add rule inet {TABLE} public_dns iifname \"lo\" return\n\
             add rule inet {TABLE} public_dns iifname \"crake-*\" return\n\
             add rule inet {TABLE} public_dns iifname @{TUNNEL_DEVICES} return\n\
             add rule inet {TABLE} public_dns meta nfproto ipv4 udp sport 53 queue num {QUEUE_NUM} bypass\n"
        ))
        .await
    }

    pub async fn public_down(&self) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        nft(&format!(
            "add table inet {TABLE}\n\
             add chain inet {TABLE} public_dns {{ type filter hook input priority filter; policy accept; }}\n\
             delete chain inet {TABLE} public_dns\n"
        ))
        .await
    }

    /// `device`: the tunnel's device, when Crake didn't name it.
    pub async fn down(&self, name: &str, index: usize, device: Option<&str>) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        if let Some(d) = device.filter(|d| !d.starts_with("crake-")) {
            nft(&tunnel_device_script(d, false)).await.ok();
        }
        let id = ids(index);
        ip(&["rule", "del", "priority", &id.priority.to_string()])
            .await
            .ok();
        ip(&["route", "flush", "table", &id.table.to_string()])
            .await
            .ok();
        nft(&down_script(name)).await
    }

    pub async fn add_learned(&self, name: &str, addr: Ipv4Addr, timeout: u64) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        let ls = learned_set(name);
        // Delete first so a fresh timeout replaces the old one.
        nft(&format!(
            "add element inet {TABLE} {ls} {{ {addr} }}\ndelete element inet {TABLE} {ls} {{ {addr} }}\n\
             add element inet {TABLE} {ls} {{ {addr} timeout {timeout}s }}\n"
        ))
        .await
    }

    pub async fn remove_learned(&self, name: &str, addr: Ipv4Addr) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        let ls = learned_set(name);
        nft(&format!("add element inet {TABLE} {ls} {{ {addr} }}\ndelete element inet {TABLE} {ls} {{ {addr} }}\n")).await
    }

    pub async fn add_pinned(&self, name: &str, t: Target) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        nft(&format!(
            "add element inet {TABLE} {} {{ {t} }}\n",
            pinned_set(name)
        ))
        .await
    }

    pub async fn remove_pinned(&self, name: &str, t: Target) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        let ps = pinned_set(name);
        nft(&format!(
            "add element inet {TABLE} {ps} {{ {t} }}\ndelete element inet {TABLE} {ps} {{ {t} }}\n"
        ))
        .await
    }

    /// Learned hosts as the kernel sees them: address and seconds until expiry.
    /// `None` when nothing is applied (Noop).
    pub async fn read_learned(&self, name: &str) -> Result<Option<Vec<(Ipv4Addr, u64)>>> {
        let Self::Real = self else { return Ok(None) };
        let out = Command::new("nft")
            .args(["-j", "list", "set", "inet", TABLE, &learned_set(name)])
            .output()
            .await
            .context("running nft")?;
        if !out.status.success() {
            bail!(
                "nft list set failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(Some(parse_learned(&out.stdout)?))
    }
}

pub fn parse_learned(json: &[u8]) -> Result<Vec<(Ipv4Addr, u64)>> {
    let v: serde_json::Value = serde_json::from_slice(json).context("nft json")?;
    let mut found = Vec::new();
    for item in v["nftables"].as_array().into_iter().flatten() {
        for e in item["set"]["elem"].as_array().into_iter().flatten() {
            let (val, expires) = match e {
                serde_json::Value::String(s) => (s.as_str(), None),
                _ => (
                    e["elem"]["val"].as_str().unwrap_or(""),
                    e["elem"]["expires"].as_u64(),
                ),
            };
            if let Ok(a) = val.parse() {
                found.push((a, expires.unwrap_or(0)));
            }
        }
    }
    Ok(found)
}

async fn nft(script: &str) -> Result<()> {
    tracing::debug!("nft:\n{script}");
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("running nft")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes()).await?;
    }
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        bail!(
            "nft failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

async fn ip(args: &[&str]) -> Result<()> {
    let out = Command::new("ip")
        .args(args)
        .output()
        .await
        .context("running ip")?;
    if !out.status.success() {
        bail!(
            "ip {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_and_in_range() {
        assert_eq!(
            ids(0),
            Ids {
                mark: 0x4352_0001,
                table: 7301,
                priority: 7301
            }
        );
        assert_eq!(ids(MAX_TUNNELS - 1).priority, ID_BASE + MAX_TUNNELS as u32);
    }

    #[test]
    fn up_script_lists_elements() {
        let s = up_script(&Plan {
            name: "acme",
            index: 0,
            device: "crake-acme",
            timeout: 604800,
            dns: &["10.40.0.53".parse().unwrap()],
            pinned: &["10.40.0.0/16".parse().unwrap()],
            learned: &[
                ("10.33.1.1".parse().unwrap(), 60),
                ("10.33.1.2".parse().unwrap(), 0),
            ],
            full_except: None,
        });
        assert!(s.contains("add element inet crake pinned_acme { 10.40.0.0/16 }"));
        assert!(s.contains("add element inet crake learned_acme { 10.33.1.1 timeout 60s }"));
        assert!(!s.contains("10.33.1.2"), "expired hosts are not loaded");
        assert!(s.contains("meta mark set 0x43520001"));
        assert!(s.contains("oifname \"crake-acme\" masquerade"));
        assert!(s.contains(
            "iifname \"crake-acme\" ip saddr { 10.40.0.53 } udp sport 53 queue num 7300 bypass"
        ));
        assert!(
            !s.contains("meta mark 0 ip daddr !="),
            "only a full tunnel takes everything"
        );
        let full = up_script(&Plan {
            name: "globex",
            index: 1,
            device: "crake-globex",
            timeout: 60,
            dns: &[],
            pinned: &[],
            learned: &[],
            full_except: Some(&[
                "203.0.113.9".parse().unwrap(),
                "192.168.1.0/24".parse().unwrap(),
            ]),
        });
        assert!(full.contains(
            "add rule inet crake out_globex meta mark 0 ip daddr != { 127.0.0.0/8, 169.254.0.0/16, \
             224.0.0.0/4, 255.255.255.255, 203.0.113.9, 192.168.1.0/24 } meta mark set 0x43520002"
        ));
    }

    #[test]
    fn parses_nft_json() {
        let json = br#"{"nftables": [{"metainfo": {}}, {"set": {"name": "learned_acme", "elem": [{"elem": {"val": "10.33.1.1", "timeout": 60, "expires": 604798}}, "10.33.1.9"]}}]}"#;
        let v = parse_learned(json).unwrap();
        assert_eq!(
            v,
            vec![
                ("10.33.1.1".parse().unwrap(), 604798),
                ("10.33.1.9".parse().unwrap(), 0)
            ]
        );
    }
}
