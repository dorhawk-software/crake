//! macOS: per-host routes on a tunnel's utun device (`route`), the
//! counterpart of Linux's nftables sets and fwmark routing. A learned host
//! or a pinned network is a route through the device; craked keeps which
//! it added per tunnel, to take them away again. Routes don't expire on
//! macOS: expiry is craked's store's (it expires hosts when the tunnel
//! comes up). A full tunnel is two halves of the address space through
//! the tunnel (0/1 and 128/1, more specific than the default route), with
//! the gateway kept on the way it went before. What leaves through a
//! tunnel with another source address (a connection that started before
//! its host was routed, on its next SYN) is NATed to the tunnel's address
//! by pf, in an anchor of Crake's (`nat`), as Linux masquerades. Public
//! DNS answers aren't watched (not yet on macOS).

use std::collections::{BTreeSet, HashMap};
use std::net::Ipv4Addr;
use std::sync::Mutex;

use anyhow::Result;

use super::cmd;
use crate::hosts::Target;
pub use crate::plan::{ID_BASE, Ids, MARK_BASE, MAX_TUNNELS, Plan, ids};

pub struct Routing {
    real: bool,
    /// Per tunnel: its device and the routes craked added through it.
    tunnels: Mutex<HashMap<String, Routes>>,
    /// pf, while Crake has it on: the token `pfctl -E` gave.
    pf: tokio::sync::Mutex<Option<String>>,
}

struct Routes {
    device: String,
    targets: BTreeSet<Target>,
    /// A full tunnel's exceptions, routed the way they went before it:
    /// (destination, next hop).
    kept: Vec<(Target, Ipv4Addr)>,
}

/// A full tunnel's two halves.
fn halves() -> [Target; 2] {
    ["0.0.0.0/1", "128.0.0.0/1"].map(|s| s.parse().expect("a network"))
}

impl Routing {
    pub fn new(apply: bool) -> Self {
        Self {
            real: apply,
            tunnels: Mutex::new(HashMap::new()),
            pf: tokio::sync::Mutex::new(None),
        }
    }

    /// Changes the system (not development's no-op).
    pub fn is_real(&self) -> bool {
        self.real
    }

    /// Nothing to clear at startup: Crake's routes go with their devices.
    pub async fn reset_all(&self) -> Result<()> {
        Ok(())
    }

    /// A tunnel's pinned networks and learned hosts, as routes through it.
    pub async fn up(&self, plan: &Plan<'_>) -> Result<()> {
        // A full tunnel's exceptions first, while their way is still the
        // one they had.
        let kept = match plan.full_except {
            Some(except) if self.real => keep_ways(except).await?,
            _ => Vec::new(),
        };
        // Probes name the tunnel by its mark: which device that is.
        super::set_path(ids(plan.index).mark, Some(plan.device));
        self.tunnels.lock().unwrap().insert(
            plan.name.to_owned(),
            Routes {
                device: plan.device.to_owned(),
                targets: BTreeSet::new(),
                kept,
            },
        );
        // A default route through the tunnel for sockets bound to it only
        // (probes, DNS checks): Linux's per-tunnel table.
        if self.real {
            cmd::route_add_scoped("0.0.0.0", 0, plan.device).await?;
            self.nat().await?;
        }
        let learned = plan.learned.iter().map(|(a, _)| Target::host(*a));
        let full = plan.full_except.map(|_| halves()).into_iter().flatten();
        for t in plan.pinned.iter().copied().chain(learned).chain(full) {
            self.add(plan.name, t).await?;
        }
        Ok(())
    }

    /// Linux watches public DNS answers; not yet on macOS.
    pub async fn public_up(&self, _probing: bool) -> Result<()> {
        Ok(())
    }

    pub async fn public_down(&self) -> Result<()> {
        Ok(())
    }

    /// Take away every route craked added through the tunnel.
    pub async fn down(&self, name: &str, index: usize, _device: Option<&str>) -> Result<()> {
        super::set_path(ids(index).mark, None);
        let Some(routes) = self.tunnels.lock().unwrap().remove(name) else {
            return Ok(());
        };
        if !self.real {
            return Ok(());
        }
        for t in routes.targets {
            cmd::route_delete(&t.addr().to_string(), t.prefix_len(), None).await?;
        }
        cmd::route_delete("0.0.0.0", 0, Some(&routes.device)).await?;
        for (t, _) in routes.kept {
            cmd::route_delete(&t.addr().to_string(), t.prefix_len(), None).await?;
        }
        self.nat().await
    }

    /// pf's NAT for every tunnel that's up, in Crake's anchor (under
    /// macOS' own `com.apple/*`, which its pf.conf evaluates); pf on while
    /// there are any, and released (its token) when there are none.
    async fn nat(&self) -> Result<()> {
        let devices: Vec<String> = self
            .tunnels
            .lock()
            .unwrap()
            .values()
            .map(|r| r.device.clone())
            .collect();
        let mut pf = self.pf.lock().await;
        if devices.is_empty() {
            cmd::pf_rules(ANCHOR, "").await?;
            if let Some(token) = pf.take() {
                cmd::pf_release(&token).await?;
            }
            return Ok(());
        }
        cmd::pf_rules(ANCHOR, &nat_rules(&devices)).await?;
        if pf.is_none() {
            *pf = Some(cmd::pf_enable().await?);
        }
        Ok(())
    }

    pub async fn add_learned(&self, name: &str, addr: Ipv4Addr, _timeout: u64) -> Result<()> {
        self.add(name, Target::host(addr)).await
    }

    pub async fn remove_learned(&self, name: &str, addr: Ipv4Addr) -> Result<()> {
        self.remove(name, Target::host(addr)).await
    }

    pub async fn add_pinned(&self, name: &str, t: Target) -> Result<()> {
        self.add(name, t).await
    }

    pub async fn remove_pinned(&self, name: &str, t: Target) -> Result<()> {
        self.remove(name, t).await
    }

    /// macOS keeps no expiry of its own: craked's store is the record.
    pub async fn read_learned(&self, _name: &str) -> Result<Option<Vec<(Ipv4Addr, u64)>>> {
        Ok(None)
    }

    async fn add(&self, name: &str, t: Target) -> Result<()> {
        // A tunnel that isn't up has nothing to route through yet; its
        // hosts are routed when it comes up (`up`).
        let Some(device) = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .map(|r| r.device.clone())
        else {
            return Ok(());
        };
        if self.real {
            cmd::route_add(&t.addr().to_string(), t.prefix_len(), &device).await?;
        }
        if let Some(r) = self.tunnels.lock().unwrap().get_mut(name) {
            r.targets.insert(t);
        }
        Ok(())
    }

    async fn remove(&self, name: &str, t: Target) -> Result<()> {
        let removed = self
            .tunnels
            .lock()
            .unwrap()
            .get_mut(name)
            .is_some_and(|r| r.targets.remove(&t));
        if removed && self.real {
            cmd::route_delete(&t.addr().to_string(), t.prefix_len(), None).await?;
        }
        Ok(())
    }
}

/// Crake's pf anchor.
const ANCHOR: &str = "com.apple/250.Crake";

/// One NAT rule per tunnel device: anything leaving through it with
/// another source takes the device's address.
fn nat_rules(devices: &[String]) -> String {
    devices
        .iter()
        .map(|d| format!("nat on {d} inet from ! ({d}) to any -> ({d})\n"))
        .collect()
}

/// Each exception whose way is the default route (which the halves would
/// take over) gets a route of its own to the same next hop: the gateway,
/// above all. The LAN's networks have routes more specific already.
async fn keep_ways(except: &[Target]) -> Result<Vec<(Target, Ipv4Addr)>> {
    let mut kept = Vec::new();
    for t in except {
        let (default, via) = cmd::way_to(t.addr()).await?;
        let (true, Some(via)) = (default, via) else {
            continue;
        };
        cmd::route_add_via(&t.addr().to_string(), t.prefix_len(), via).await?;
        kept.push((*t, via));
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_nat_rule_per_tunnel() {
        assert_eq!(
            super::nat_rules(&["utun5".into(), "utun6".into()]),
            "nat on utun5 inet from ! (utun5) to any -> (utun5)\n\
             nat on utun6 inet from ! (utun6) to any -> (utun6)\n"
        );
    }
}
