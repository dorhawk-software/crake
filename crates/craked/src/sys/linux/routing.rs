//! nftables table `inet crake` plus fwmark routing.
//!
//! Per tunnel (index i, name n):
//! - set `learned_n`: hosts with per-element timeouts, refreshed by traffic
//! - set `pinned_n`: hosts and networks kept until unpinned
//! - chain `out_n` (route, output): destinations in either set get mark M
//! - chain `nat_n` (nat, postrouting): masquerade out of the tunnel, so the
//!   source address matches the tunnel's
//! - chain `dns_n` (filter, input): answers from the tunnel's DNS servers
//!   (UDP and TCP) go to NFQUEUE so craked can route their addresses first
//!   (see `dns_queue`)
//! - `ip rule fwmark M lookup T`, and table T's default route is the tunnel.
//!
//! Base chains are per tunnel so a tunnel can be torn down without touching
//! the others. Only locally generated traffic is marked.

use std::net::{IpAddr, Ipv4Addr};
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::dns_queue::QUEUE_NUM;
use crate::hosts::Target;
pub use crate::plan::{ID_BASE, Ids, MARK_BASE, MAX_TUNNELS, Plan, ids};

pub const TABLE: &str = "crake";
fn learned_set(name: &str) -> String {
    format!("learned_{name}")
}
fn pinned_set(name: &str) -> String {
    format!("pinned_{name}")
}
// IPv6 has its own sets (nftables sets hold one address family each).
fn learned6_set(name: &str) -> String {
    format!("learned6_{name}")
}
fn pinned6_set(name: &str) -> String {
    format!("pinned6_{name}")
}

/// The learned set for `addr`'s family.
fn learned_set_for(name: &str, addr: IpAddr) -> String {
    if addr.is_ipv6() {
        learned6_set(name)
    } else {
        learned_set(name)
    }
}

fn set_defs(name: &str, timeout: u64) -> String {
    // `dynamic` is what nft adds anyway for a set updated from the packet path;
    // spelling it out keeps `add set` idempotent.
    format!(
        "add set inet {TABLE} {} {{ type ipv4_addr; flags dynamic, timeout; timeout {timeout}s; }}\n\
         add set inet {TABLE} {} {{ type ipv4_addr; flags interval; }}\n\
         add set inet {TABLE} {} {{ type ipv6_addr; flags dynamic, timeout; timeout {timeout}s; }}\n\
         add set inet {TABLE} {} {{ type ipv6_addr; flags interval; }}\n",
        learned_set(name),
        pinned_set(name),
        learned6_set(name),
        pinned6_set(name)
    )
}

fn chain_defs(name: &str) -> String {
    format!(
        "add chain inet {TABLE} out_{name} {{ type route hook output priority mangle; policy accept; }}\n\
         add chain inet {TABLE} nat_{name} {{ type nat hook postrouting priority srcnat; policy accept; }}\n\
         add chain inet {TABLE} dns_{name} {{ type filter hook input priority filter; policy accept; }}\n\
         add chain inet {TABLE} block_{name} {{ type filter hook output priority filter; policy accept; }}\n"
    )
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
        ipv6,
    } = *plan;
    let mark = ids(index).mark;
    let (ls, ps) = (learned_set(name), pinned_set(name));
    let (ls6, ps6) = (learned6_set(name), pinned6_set(name));
    let mut s = format!("add table inet {TABLE}\n");
    s += &set_defs(name, timeout);
    s += &chain_defs(name);
    s += &format!(
        "flush chain inet {TABLE} out_{name}\n\
         flush chain inet {TABLE} nat_{name}\n\
         flush chain inet {TABLE} dns_{name}\n\
         flush chain inet {TABLE} block_{name}\n\
         flush set inet {TABLE} {ls}\n\
         flush set inet {TABLE} {ps}\n\
         flush set inet {TABLE} {ls6}\n\
         flush set inet {TABLE} {ps6}\n\
         add rule inet {TABLE} out_{name} ip daddr @{ps} meta mark set {mark:#x}\n\
         add rule inet {TABLE} out_{name} ip daddr @{ls} update @{ls} {{ ip daddr timeout {timeout}s }} meta mark set {mark:#x}\n\
         add rule inet {TABLE} out_{name} ip6 daddr @{ps6} meta mark set {mark:#x}\n\
         add rule inet {TABLE} out_{name} ip6 daddr @{ls6} update @{ls6} {{ ip6 daddr timeout {timeout}s }} meta mark set {mark:#x}\n\
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
        list.extend(
            except
                .iter()
                .filter(|t| !t.is_ipv6())
                .map(Target::to_string),
        );
        s += &format!(
            "add rule inet {TABLE} out_{name} meta mark 0 ip daddr != {{ {} }} meta mark set {mark:#x}\n",
            list.join(", ")
        );
        // IPv6 likewise: loopback, link-local and multicast stay.
        let mut list6: Vec<String> = ["::1", "fe80::/10", "ff00::/8"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        list6.extend(except.iter().filter(|t| t.is_ipv6()).map(Target::to_string));
        let list6 = list6.join(", ");
        s += &if ipv6 {
            format!(
                "add rule inet {TABLE} out_{name} meta mark 0 ip6 daddr != {{ {list6} }} meta mark set {mark:#x}\n"
            )
        } else {
            // The tunnel has no IPv6 address: never sent around the tunnel.
            // TCP is reset, so apps fall back to IPv4 at once (an ICMPv6
            // reject doesn't reach a local socket); the rest is dropped.
            format!(
                "add rule inet {TABLE} block_{name} meta mark 0 ip6 daddr != {{ {list6} }} meta l4proto tcp reject with tcp reset\n\
                 add rule inet {TABLE} block_{name} meta mark 0 ip6 daddr != {{ {list6} }} drop\n"
            )
        };
    }
    if !dns.is_empty() {
        let servers: Vec<String> = dns.iter().map(Ipv4Addr::to_string).collect();
        s += &format!(
            "add rule inet {TABLE} dns_{name} iifname \"{device}\" ip saddr {{ {} }} meta l4proto {{ udp, tcp }} th sport 53 queue num {QUEUE_NUM} bypass\n",
            servers.join(", ")
        );
    }
    let elems = |set: &str, items: Vec<String>, out: &mut String| {
        if !items.is_empty() {
            *out += &format!(
                "add element inet {TABLE} {set} {{ {} }}\n",
                items.join(", ")
            );
        }
    };
    // Each family goes in its own set (nftables sets hold one family each).
    let pinned = crate::plan::outermost(pinned);
    let pins = |v6: bool| -> Vec<String> {
        pinned
            .iter()
            .filter(|t| t.is_ipv6() == v6)
            .map(Target::to_string)
            .collect()
    };
    elems(&ps, pins(false), &mut s);
    elems(&ps6, pins(true), &mut s);
    let learns = |v6: bool| -> Vec<String> {
        learned
            .iter()
            .filter(|(a, t)| *t > 0 && a.is_ipv6() == v6)
            .map(|(a, t)| format!("{a} timeout {t}s"))
            .collect()
    };
    elems(&ls, learns(false), &mut s);
    elems(&ls6, learns(true), &mut s);
    s
}

/// A tunnel's pinned sets, rewritten in one transaction: emptied, then
/// filled with the pins no other pin covers (nft refuses overlapping
/// intervals, and a host pinned inside a pinned network would otherwise
/// fail the pin, and every later connect).
fn pinned_script(name: &str, pins: &[Target]) -> String {
    let (ps, ps6) = (pinned_set(name), pinned6_set(name));
    let mut s = format!("flush set inet {TABLE} {ps}\nflush set inet {TABLE} {ps6}\n");
    let pins = crate::plan::outermost(pins);
    for (set, v6) in [(&ps, false), (&ps6, true)] {
        let items: Vec<String> = pins
            .iter()
            .filter(|t| t.is_ipv6() == v6)
            .map(Target::to_string)
            .collect();
        if !items.is_empty() {
            s += &format!(
                "add element inet {TABLE} {set} {{ {} }}\n",
                items.join(", ")
            );
        }
    }
    s
}

/// The first-SYN race's sets and rule, one set pair per address family.
/// The sets merge what overlaps (a tunnel's pushed 10.40.0.0/16 inside a
/// learn-anything tunnel's 10.0.0.0/8): nft refuses overlapping intervals
/// otherwise, and the whole race with them.
fn race_script(watch: &[Target], skip: &[Target]) -> String {
    let queue = crate::syn_queue::QUEUE_NUM;
    let mut s = format!(
        "add table inet {TABLE}\n\
         add set inet {TABLE} syn_watch {{ type ipv4_addr; flags interval; auto-merge; }}\n\
         add set inet {TABLE} syn_skip {{ type ipv4_addr; flags interval; auto-merge; }}\n\
         add set inet {TABLE} syn_watch6 {{ type ipv6_addr; flags interval; auto-merge; }}\n\
         add set inet {TABLE} syn_skip6 {{ type ipv6_addr; flags interval; auto-merge; }}\n\
         add chain inet {TABLE} syn_race {{ type route hook output priority mangle - 10; policy accept; }}\n\
         flush set inet {TABLE} syn_watch\n\
         flush set inet {TABLE} syn_skip\n\
         flush set inet {TABLE} syn_watch6\n\
         flush set inet {TABLE} syn_skip6\n\
         flush chain inet {TABLE} syn_race\n"
    );
    for (set, list, v6) in [
        ("syn_watch", watch, false),
        ("syn_skip", skip, false),
        ("syn_watch6", watch, true),
        ("syn_skip6", skip, true),
    ] {
        let items: Vec<String> = list
            .iter()
            .filter(|t| t.is_ipv6() == v6)
            .map(Target::to_string)
            .collect();
        if !items.is_empty() {
            s += &format!(
                "add element inet {TABLE} {set} {{ {} }}\n",
                items.join(", ")
            );
        }
    }
    let syn = "ct state new meta l4proto tcp tcp flags & (fin | syn | rst | ack) == syn";
    s += &format!(
        "add rule inet {TABLE} syn_race {syn} ip daddr @syn_watch ip daddr != @syn_skip queue num {queue} bypass\n\
         add rule inet {TABLE} syn_race {syn} ip6 daddr @syn_watch6 ip6 daddr != @syn_skip6 queue num {queue} bypass\n"
    );
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
        "delete chain inet {TABLE} out_{name}\ndelete chain inet {TABLE} nat_{name}\n\
         delete chain inet {TABLE} dns_{name}\ndelete chain inet {TABLE} block_{name}\n"
    );
    s += &set_defs(name, 1);
    s += &format!(
        "delete set inet {TABLE} {}\ndelete set inet {TABLE} {}\n\
         delete set inet {TABLE} {}\ndelete set inet {TABLE} {}\n",
        learned_set(name),
        pinned_set(name),
        learned6_set(name),
        pinned6_set(name)
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

    /// Changes the system (not development's no-op).
    pub fn is_real(&self) -> bool {
        matches!(self, Self::Real)
    }

    /// Linux routes IPv6 through a tunnel (v6 nft sets, `ip -6` rules/routes).
    pub fn routes_ipv6(&self) -> bool {
        true
    }

    /// At startup and at shutdown: clear everything Crake sets up (its
    /// nftables table, policy rules and routing tables), whatever is left.
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
        // And the routing tables those rules pointed at (v4 and v6; the v6
        // rules share the same priority/table numbers).
        for table in range {
            let t = table.to_string();
            ip(&["route", "flush", "table", &t]).await.ok();
            ip(&["-6", "rule", "del", "priority", &t]).await.ok();
            ip(&["-6", "route", "flush", "table", &t]).await.ok();
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
        // The same for IPv6: best-effort, so a v4-only tunnel (or a kernel
        // with IPv6 off) isn't held up. v6 hosts only reach these when the
        // tunnel actually carries IPv6.
        ip(&["-6", "rule", "del", "priority", &prio]).await.ok();
        ip(&[
            "-6", "rule", "add", "fwmark", &mark, "lookup", &table, "priority", &prio,
        ])
        .await
        .ok();
        ip(&[
            "-6", "route", "replace", "default", "dev", device, "table", &table,
        ])
        .await
        .ok();
        nft(&up_script(plan)).await?;
        if !plan.device.starts_with("crake-") {
            nft(&tunnel_device_script(plan.device, true)).await?;
        }
        Ok(())
    }

    /// Watch public DNS answers (everything not from a tunnel or loopback) so
    /// names that fail publicly can be asked of the tunnels; with `probing`,
    /// also routers' "unreachable" notices, so a host one can't reach is
    /// tried through the tunnels at once (`probe::unreachable`). Idempotent.
    pub async fn public_up(&self, probing: bool) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        let unreachable = if probing {
            format!(
                "add rule inet {TABLE} public_dns meta nfproto ipv4 icmp type destination-unreachable \
                 icmp code {{ net-unreachable, host-unreachable, net-prohibited, host-prohibited, admin-prohibited }} \
                 queue num {QUEUE_NUM} bypass\n\
                 add rule inet {TABLE} public_dns meta nfproto ipv6 icmpv6 type destination-unreachable \
                 icmpv6 code {{ no-route, admin-prohibited, addr-unreachable, policy-fail, reject-route }} \
                 queue num {QUEUE_NUM} bypass\n"
            )
        } else {
            String::new()
        };
        nft(&format!(
            "add table inet {TABLE}\n\
             add chain inet {TABLE} public_dns {{ type filter hook input priority filter; policy accept; }}\n\
             add set inet {TABLE} {TUNNEL_DEVICES} {{ type ifname; }}\n\
             flush chain inet {TABLE} public_dns\n\
             add rule inet {TABLE} public_dns iifname \"lo\" return\n\
             add rule inet {TABLE} public_dns iifname \"crake-*\" return\n\
             add rule inet {TABLE} public_dns iifname @{TUNNEL_DEVICES} return\n\
             add rule inet {TABLE} public_dns meta nfproto ipv4 udp sport 53 queue num {QUEUE_NUM} bypass\n\
             {unreachable}"
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

    /// Hold the first SYN of a new connection to a `watch` address that
    /// isn't in `skip` (NFQUEUE, `syn_queue`), so craked can route it before
    /// it is sent (the first-SYN race). `watch` = the tunnels' scopes and
    /// pushed networks, plus private ranges for a learn-anything tunnel;
    /// `skip` = the local networks and the gateways, never raced. Empty
    /// `watch` removes the rule. Idempotent.
    pub async fn race_up(&self, watch: &[Target], skip: &[Target]) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        if watch.is_empty() {
            return self.race_down().await;
        }
        nft(&race_script(watch, skip)).await
    }

    pub async fn race_down(&self) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        nft(&format!(
            "add table inet {TABLE}\n\
             add chain inet {TABLE} syn_race {{ type route hook output priority mangle - 10; policy accept; }}\n\
             delete chain inet {TABLE} syn_race\n"
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
        ip(&["-6", "rule", "del", "priority", &id.priority.to_string()])
            .await
            .ok();
        ip(&["-6", "route", "flush", "table", &id.table.to_string()])
            .await
            .ok();
        nft(&down_script(name)).await
    }

    pub async fn add_learned(&self, name: &str, addr: IpAddr, timeout: u64) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        let ls = learned_set_for(name, addr);
        // Delete first so a fresh timeout replaces the old one.
        nft(&format!(
            "add element inet {TABLE} {ls} {{ {addr} }}\ndelete element inet {TABLE} {ls} {{ {addr} }}\n\
             add element inet {TABLE} {ls} {{ {addr} timeout {timeout}s }}\n"
        ))
        .await
    }

    pub async fn remove_learned(&self, name: &str, addr: IpAddr) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        let ls = learned_set_for(name, addr);
        nft(&format!("add element inet {TABLE} {ls} {{ {addr} }}\ndelete element inet {TABLE} {ls} {{ {addr} }}\n")).await
    }

    /// `t` was pinned; `pins` are all the tunnel's pins now. The pinned
    /// sets are rewritten whole (`pinned_script`).
    pub async fn add_pinned(&self, name: &str, _t: Target, pins: &[Target]) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        nft(&pinned_script(name, pins)).await
    }

    /// `t` was unpinned; `pins` are all the tunnel's pins now.
    pub async fn remove_pinned(&self, name: &str, _t: Target, pins: &[Target]) -> Result<()> {
        let Self::Real = self else { return Ok(()) };
        nft(&pinned_script(name, pins)).await
    }

    /// Learned hosts as the kernel sees them: address and seconds until expiry.
    /// `None` when nothing is applied (Noop).
    pub async fn read_learned(&self, name: &str) -> Result<Option<Vec<(IpAddr, u64)>>> {
        let Self::Real = self else { return Ok(None) };
        let mut found = Vec::new();
        for set in [learned_set(name), learned6_set(name)] {
            let out = Command::new("nft")
                .args(["-j", "list", "set", "inet", TABLE, &set])
                .output()
                .await
                .context("running nft")?;
            if !out.status.success() {
                bail!(
                    "nft list set failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
            found.extend(parse_learned(&out.stdout)?);
        }
        Ok(Some(found))
    }
}

pub fn parse_learned(json: &[u8]) -> Result<Vec<(IpAddr, u64)>> {
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
            pinned: &[
                "10.40.0.0/16".parse().unwrap(),
                "fd40::/32".parse().unwrap(),
            ],
            learned: &[
                ("10.33.1.1".parse().unwrap(), 60),
                ("10.33.1.2".parse().unwrap(), 0),
                ("fd40::1".parse().unwrap(), 60),
            ],
            full_except: None,
            ipv6: false,
        });
        assert!(s.contains("add element inet crake pinned_acme { 10.40.0.0/16 }"));
        assert!(s.contains("add element inet crake learned_acme { 10.33.1.1 timeout 60s }"));
        assert!(!s.contains("10.33.1.2"), "expired hosts are not loaded");
        // IPv6 goes in its own sets, with its own ip6 marking rules.
        assert!(s.contains("add element inet crake pinned6_acme { fd40::/32 }"));
        assert!(s.contains("add element inet crake learned6_acme { fd40::1 timeout 60s }"));
        assert!(s.contains("ip6 daddr @pinned6_acme meta mark set 0x43520001"));
        assert!(s.contains("ip6 daddr @learned6_acme update @learned6_acme"));
        assert!(s.contains("meta mark set 0x43520001"));
        assert!(s.contains("oifname \"crake-acme\" masquerade"));
        assert!(s.contains(
            "iifname \"crake-acme\" ip saddr { 10.40.0.53 } meta l4proto { udp, tcp } th sport 53 queue num 7300 bypass"
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
                "fd00:1a::/64".parse().unwrap(),
            ]),
            ipv6: true,
        });
        assert!(full.contains(
            "add rule inet crake out_globex meta mark 0 ip daddr != { 127.0.0.0/8, 169.254.0.0/16, \
             224.0.0.0/4, 255.255.255.255, 203.0.113.9, 192.168.1.0/24 } meta mark set 0x43520002"
        ));
        assert!(full.contains(
            "add rule inet crake out_globex meta mark 0 ip6 daddr != { ::1, fe80::/10, ff00::/8, \
             fd00:1a::/64 } meta mark set 0x43520002"
        ));
        assert!(!full.contains("reject"), "{full}");
        let no_v6 = up_script(&Plan {
            name: "globex",
            index: 1,
            device: "crake-globex",
            timeout: 60,
            dns: &[],
            pinned: &[],
            learned: &[],
            full_except: Some(&[]),
            ipv6: false,
        });
        assert!(no_v6.contains(
            "add rule inet crake block_globex meta mark 0 ip6 daddr != { ::1, fe80::/10, ff00::/8 } \
             meta l4proto tcp reject with tcp reset"
        ));
    }

    #[test]
    fn race_script_has_a_set_pair_per_family() {
        let s = race_script(
            &["10.0.0.0/8".parse().unwrap(), "fc00::/7".parse().unwrap()],
            &["192.168.0.0/24".parse().unwrap()],
        );
        assert!(s.contains("add element inet crake syn_watch { 10.0.0.0/8 }"));
        assert!(s.contains("add element inet crake syn_watch6 { fc00::/7 }"));
        assert!(s.contains("add element inet crake syn_skip { 192.168.0.0/24 }"));
        assert!(!s.contains("add element inet crake syn_skip6"), "{s}");
        assert!(s.contains("ip6 daddr @syn_watch6 ip6 daddr != @syn_skip6 queue num"));
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
