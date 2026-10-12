//! A tunnel's routing and DNS registration put in place and taken away, and
//! learned hosts' expiry read back from the kernel.

use super::*;

impl Manager {
    /// Load the tunnel's pinned and learned hosts into the kernel. Returns how
    /// many destinations now go through it.
    pub(super) async fn apply_routing(
        &self,
        name: &str,
        label: &str,
        info: &TunnelInfo,
    ) -> Result<usize> {
        let _ops = self.ops.lock().await;
        let index = self.entry(name)?.0;
        if self.is_routed(name) {
            // Reconnect: keep what traffic refreshed in the kernel.
            self.sync_learned(name).await;
        }
        let now = hosts::now();
        let (pinned, learned, skipped) = {
            let mut store = self.hosts.lock().unwrap();
            let t = store.tunnel(name);
            t.gateway = info.gateway.or(t.gateway);
            t.expire(now);
            let ok = |x: Target| guard(label, x, t.gateway).is_ok();
            let pinned: Vec<Target> = t.pinned.iter().copied().filter(|x| ok(*x)).collect();
            let learned: Vec<(Ipv4Addr, u64)> = t
                .learned_remaining(now)
                .into_iter()
                .filter(|(a, _)| ok(Target::host(*a)))
                .collect();
            let skipped = t.pinned.len() + t.learned.len() - pinned.len() - learned.len();
            store.save();
            (pinned, learned, skipped)
        };
        if skipped > 0 {
            self.log.system(
                Some(name),
                format!("Left {skipped} saved destination(s) direct."),
                Some(format!("They cover {label}'s gateway.")),
            );
        }
        // A full tunnel takes everything but its own gateway and the LAN.
        let full_except = (self.learning(name) == crate::policy::Learning::FullTunnel).then(|| {
            let mut v: Vec<Target> = crate::probe::Local::load().networks();
            if let Some(gw) = info.gateway {
                v.push(Target::host(gw));
            }
            v
        });
        self.routing
            .up(&Plan {
                name,
                index,
                device: &info.device,
                timeout: self.timeout_for(name),
                dns: &info.dns,
                pinned: &pinned,
                learned: &learned,
                full_except: full_except.as_deref(),
            })
            .await?;
        self.set_routed(name, true);
        self.routing.public_up(self.cfg.probing).await?;
        self.sync_syn_race().await;
        // A tunnel that just came up may know names that failed before.
        self.unknown.lock().unwrap().clear();
        Ok(pinned.len() + learned.len())
    }

    /// Hand the tunnel's DNS servers and domains to systemd-resolved. Done after
    /// the routing is in place, so the first answer is already caught.
    pub(super) async fn register_dns(
        &self,
        name: &str,
        label: &str,
        info: &TunnelInfo,
        domains: &[String],
    ) {
        let everything = self.learning(name) == crate::policy::Learning::FullTunnel;
        if info.dns.is_empty() || (domains.is_empty() && !everything) {
            return;
        }
        let res = self
            .resolved
            .set_link(&info.device, &info.dns, domains, everything)
            .await;
        // Forget answers cached while the tunnel was off (failures included).
        self.resolved.flush().await;
        if let Err(e) = res {
            tracing::error!("registering {label}'s DNS: {e:#}");
            self.log.system(
                Some(name),
                format!("Couldn't hand {label}'s DNS to the system."),
                Some(format!("Names under its domains won't resolve. {e:#}")),
            );
        }
    }

    pub(super) async fn remove_routing(&self, name: &str) {
        let _ops = self.ops.lock().await;
        if !self.is_routed(name) {
            return;
        }
        self.sync_learned(name).await;
        self.resolved.revert(&self.device_of(name)).await;
        self.dns_misses.lock().unwrap().remove(name);
        if self.dns_down.lock().unwrap().remove(name).is_some() {
            self.apply_keep().await;
        }
        let Ok((index, _, _)) = self.entry(name) else {
            return;
        };
        let device = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .and_then(|e| e.info.as_ref().map(|i| i.device.clone()));
        if let Err(e) = self.routing.down(name, index, device.as_deref()).await {
            tracing::error!("removing routing for {name}: {e:#}");
        }
        self.set_routed(name, false);
        self.sync_syn_race().await;
        let any_left = self.tunnels.lock().unwrap().values().any(|e| e.routed);
        if !any_left && let Err(e) = self.routing.public_down().await {
            tracing::error!("removing the public DNS watch: {e:#}");
        }
        // Answers from this tunnel's DNS are no use any more.
        self.resolved.flush().await;
    }

    /// Pull learned hosts' expiry back from the kernel, where traffic refreshes
    /// it. Hosts the kernel has expired are dropped.
    pub(super) async fn sync_learned(&self, name: &str) {
        if !self.is_routed(name) {
            return;
        }
        match self.routing.read_learned(name).await {
            Ok(Some(kernel)) => {
                let now = hosts::now();
                let mut store = self.hosts.lock().unwrap();
                let t = store.tunnel(name);
                let old = std::mem::take(&mut t.learned);
                for (addr, expires) in kernel {
                    let exp = if expires > 0 {
                        now + expires
                    } else {
                        old.get(&addr).copied().unwrap_or(now)
                    };
                    t.learned.insert(addr, exp);
                }
                store.save();
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("reading learned hosts for {name}: {e:#}"),
        }
    }
}
