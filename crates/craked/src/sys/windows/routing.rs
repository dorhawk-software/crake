//! Windows: per-host routes on a tunnel's interface (IP Helper API), the
//! counterpart of Linux's nftables sets and fwmark routing. A learned host
//! or a pinned network is a route through `crake-<name>`; craked keeps
//! which it added per tunnel, to take them away again. Routes don't expire
//! on Windows: expiry is craked's store's (it expires hosts when the
//! tunnel comes up). A full tunnel is two halves of the address space
//! through the tunnel (0/1 and 128/1: more specific than the LAN's default
//! route, whatever its metric), with its exceptions (the gateway, the LAN)
//! kept on the way they went before. Public DNS answers aren't watched
//! here: craked's own DNS server gets public names (`resolved::set_public`).

use std::collections::{BTreeSet, HashMap};
use std::net::Ipv4Addr;
use std::sync::Mutex;

use anyhow::Result;

use super::iphelper;
use crate::hosts::Target;
pub use crate::plan::{ID_BASE, Ids, MARK_BASE, MAX_TUNNELS, Plan, ids};

/// The tunnel's own default route: never better than the real one.
const PATH_METRIC: u32 = 9000;

pub struct Routing {
    real: bool,
    /// Per tunnel: its interface and the routes craked added through it.
    tunnels: Mutex<HashMap<String, Routes>>,
}

struct Routes {
    device: String,
    targets: BTreeSet<Target>,
    /// A full tunnel's exceptions, routed the way they went before it.
    kept: Vec<iphelper::Route>,
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
        }
    }

    /// Changes the system (not development's no-op).
    pub fn is_real(&self) -> bool {
        self.real
    }

    /// Nothing to clear at startup: Crake's routes go with their adapters.
    pub async fn reset_all(&self) -> Result<()> {
        Ok(())
    }

    /// A tunnel's pinned networks and learned hosts, as routes through it.
    pub async fn up(&self, plan: &Plan<'_>) -> Result<()> {
        // A full tunnel's exceptions first, while their way is still the
        // one they had.
        let kept = match plan.full_except {
            Some(except) if self.real => keep_ways(except)?,
            _ => Vec::new(),
        };
        // Probes name the tunnel by its mark: which interface that is.
        super::set_path(ids(plan.index).mark, Some(plan.device));
        self.tunnels.lock().unwrap().insert(
            plan.name.to_owned(),
            Routes {
                device: plan.device.to_owned(),
                targets: BTreeSet::new(),
                kept,
            },
        );
        // A default route through the tunnel that normal traffic never
        // takes (the real one has a far lower metric), for sockets bound to
        // the tunnel (probes, DNS checks): Windows only looks at routes on
        // the interface they're bound to. Linux's per-tunnel table.
        if self.real {
            iphelper::add_route_metric(
                iphelper::luid(plan.device)?,
                Ipv4Addr::UNSPECIFIED,
                0,
                PATH_METRIC,
            )?;
        }
        let learned = plan.learned.iter().map(|(a, _)| Target::host(*a));
        let full = plan.full_except.map(|_| halves()).into_iter().flatten();
        for t in plan.pinned.iter().copied().chain(learned).chain(full) {
            self.add(plan.name, t)?;
        }
        Ok(())
    }

    /// Public names to craked's own DNS server, which can rescue what
    /// public DNS doesn't know (Linux watches the answers in nftables).
    pub async fn public_up(&self, _probing: bool) -> Result<()> {
        if self.real {
            super::resolved::set_public(true)?;
        }
        Ok(())
    }

    pub async fn public_down(&self) -> Result<()> {
        if self.real {
            super::resolved::set_public(false)?;
        }
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
        // The adapter may be gone already, and its routes with it.
        let Ok(luid) = iphelper::luid(&routes.device) else {
            return Ok(());
        };
        for t in routes.targets {
            iphelper::delete_route(luid, t.addr(), t.prefix_len())?;
        }
        iphelper::delete_route(luid, Ipv4Addr::UNSPECIFIED, 0)?;
        for r in routes.kept {
            iphelper::delete(r)?;
        }
        Ok(())
    }

    pub async fn add_learned(&self, name: &str, addr: Ipv4Addr, _timeout: u64) -> Result<()> {
        self.add(name, Target::host(addr))
    }

    pub async fn remove_learned(&self, name: &str, addr: Ipv4Addr) -> Result<()> {
        self.remove(name, Target::host(addr))
    }

    pub async fn add_pinned(&self, name: &str, t: Target) -> Result<()> {
        self.add(name, t)
    }

    pub async fn remove_pinned(&self, name: &str, t: Target) -> Result<()> {
        self.remove(name, t)
    }

    /// Windows keeps no expiry of its own: craked's store is the record.
    pub async fn read_learned(&self, _name: &str) -> Result<Option<Vec<(Ipv4Addr, u64)>>> {
        Ok(None)
    }

    fn add(&self, name: &str, t: Target) -> Result<()> {
        let mut map = self.tunnels.lock().unwrap();
        // A tunnel that isn't up has nothing to route through yet; its
        // hosts are routed when it comes up (`up`).
        let Some(routes) = map.get_mut(name) else {
            return Ok(());
        };
        if self.real {
            iphelper::add_route(iphelper::luid(&routes.device)?, t.addr(), t.prefix_len())?;
        }
        routes.targets.insert(t);
        Ok(())
    }

    fn remove(&self, name: &str, t: Target) -> Result<()> {
        let mut map = self.tunnels.lock().unwrap();
        let Some(routes) = map.get_mut(name) else {
            return Ok(());
        };
        if routes.targets.remove(&t) && self.real {
            iphelper::delete_route(iphelper::luid(&routes.device)?, t.addr(), t.prefix_len())?;
        }
        Ok(())
    }
}

/// Each exception whose way is the default route (which the halves would
/// take over) gets a route of its own along that way: the gateway, above
/// all. The LAN's networks, loopback and the like have routes more
/// specific than a half already.
fn keep_ways(except: &[Target]) -> Result<Vec<iphelper::Route>> {
    let mut kept = Vec::new();
    for t in except {
        let (mut way, prefix) = iphelper::best_route(t.addr())?;
        if prefix > 0 {
            continue;
        }
        way.prefix = t.prefix_len();
        iphelper::add(way)?;
        kept.push(way);
    }
    Ok(kept)
}
