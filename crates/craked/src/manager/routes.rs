//! A tunnel's routing and DNS registration put in place and taken away, and
//! learned hosts' expiry read back from the kernel.

use super::*;

/// Why encrypted public DNS matters to Crake.
const ENCRYPTED_DNS: &str = "Crake can't see when public DNS doesn't know a name, so it can't ask \
     your tunnels about it: a name only a tunnel's DNS knows, outside that tunnel's domains, won't \
     be found. Names under the tunnels' domains work as usual.";

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
            let ok = |x: Target| guard(label, x, t.gateway, self.routing.routes_ipv6()).is_ok();
            let pinned: Vec<Target> = t.pinned.iter().copied().filter(|x| ok(*x)).collect();
            let learned: Vec<(IpAddr, u64)> = t
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
            let local = crate::probe::Local::load();
            let mut v: Vec<Target> = local.networks();
            v.extend(local.networks6());
            v.extend(info.gateway.map(Target::host));
            v.extend(info.gateway6.map(Target::host));
            v
        });
        self.routing
            .up(&Plan {
                name,
                index,
                device: &info.device,
                timeout: self.timeout_for(name),
                dns: &info.dns,
                dns6: &info.dns6,
                pinned: &pinned,
                learned: &learned,
                full_except: full_except.as_deref(),
                ipv6: info.address6.is_some(),
                address: info.address,
                address6: info.address6.map(|(a, _)| a),
            })
            .await?;
        self.set_routed(name, true);
        self.routing.public_up(self.cfg.probing).await?;
        self.sync_syn_race().await;
        // A tunnel that just came up may know names that failed before.
        self.unknown.lock().unwrap().clear();
        Ok(pinned.len() + learned.len())
    }

    /// Another program may rewrite a tunnel's DNS settings in resolved:
    /// openvpn3 does, for all its devices, when one of its sessions
    /// reconnects (its domains as search domains, every name to it), which
    /// sent every name to that company and undid "routed before the first
    /// packet". Crake's own are put back.
    pub(super) async fn keep_dns_links(&self) {
        let up: Vec<(String, String, TunnelInfo)> = {
            let down = self.dns_down.lock().unwrap();
            self.tunnels
                .lock()
                .unwrap()
                .iter()
                .filter(|(n, e)| e.state == TunnelState::Connected && !down.contains_key(*n))
                .filter_map(|(n, e)| {
                    Some((n.clone(), e.config.display().to_owned(), e.info.clone()?))
                })
                .collect()
        };
        for (name, label, info) in up {
            let domains = self.domains(&name);
            let everything = self.learning(&name) == crate::policy::Learning::FullTunnel;
            if info.servers.is_empty() || (domains.is_empty() && !everything) {
                continue;
            }
            if !self
                .resolved
                .link_as_set(&info.device, &domains, everything)
                .await
            {
                tracing::info!(
                    "{label}: its DNS settings were changed elsewhere; Crake's are back"
                );
                self.register_dns(&name, &label, &info, &domains).await;
            }
        }
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
        let servers = &info.servers;
        if servers.is_empty() || (domains.is_empty() && !everything) {
            return;
        }
        let res = self
            .resolved
            .set_link(&info.device, servers, domains, everything)
            .await;
        // Forget answers cached while the tunnel was off (failures included).
        self.resolved.flush().await;
        self.say_if_dns_is_encrypted().await;
        if let Err(e) = res {
            tracing::error!("registering {label}'s DNS: {e:#}");
            self.log.system(
                Some(name),
                format!("Couldn't hand {label}'s DNS to the system."),
                Some(format!("Names under its domains won't resolve. {e:#}")),
            );
        }
    }

    /// Public DNS sent encrypted (DNS over TLS) is out of Crake's sight: it
    /// can't tell when public DNS doesn't know a name, so the tunnels
    /// aren't asked about it. Said once, when a tunnel's DNS is first
    /// registered.
    async fn say_if_dns_is_encrypted(&self) {
        use std::sync::atomic::Ordering;
        if self.encrypted_dns_said.load(Ordering::Relaxed) || !self.resolved.encrypts_dns().await {
            return;
        }
        self.encrypted_dns_said.store(true, Ordering::Relaxed);
        self.log.system(
            None,
            "This computer encrypts its DNS (DNS over TLS).",
            Some(ENCRYPTED_DNS.into()),
        );
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
