//! Windows: per-host routes on a tunnel's interface (IP Helper API), the
//! counterpart of Linux's nftables sets and fwmark routing. A learned host
//! or a pinned network is a route through `crake-<name>`; craked keeps
//! which it added per tunnel, to take them away again. Routes don't expire
//! on Windows: expiry is craked's store's (it expires hosts when the
//! tunnel comes up). Not yet: a full tunnel, and the public DNS watch
//! (craked's DNS forwarder will do that).

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
        if plan.full_except.is_some() && self.real {
            return Err(super::not_yet("a full tunnel"));
        }
        // Probes name the tunnel by its mark: which interface that is.
        super::set_path(ids(plan.index).mark, Some(plan.device));
        self.tunnels.lock().unwrap().insert(
            plan.name.to_owned(),
            Routes {
                device: plan.device.to_owned(),
                targets: BTreeSet::new(),
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
        for t in plan.pinned.iter().copied().chain(learned) {
            self.add(plan.name, t)?;
        }
        Ok(())
    }

    /// Linux watches public DNS answers in nftables; on Windows craked's own
    /// DNS forwarder will see them.
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
        // The adapter may be gone already, and its routes with it.
        let Ok(luid) = iphelper::luid(&routes.device) else {
            return Ok(());
        };
        for t in routes.targets {
            iphelper::delete_route(luid, t.addr(), t.prefix_len())?;
        }
        iphelper::delete_route(luid, Ipv4Addr::UNSPECIFIED, 0)?;
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
