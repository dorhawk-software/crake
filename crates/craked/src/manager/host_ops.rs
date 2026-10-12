//! What the user does with hosts: add (by address or name), pin, unpin,
//! forget, undo, redo, move; and the list tunnel detail shows.

use super::*;

impl Manager {
    /// Refuse what a managed tunnel's policy doesn't allow, in its words.
    pub(super) fn check_scope(
        &self,
        name: &str,
        label: &str,
        t: Target,
        dns_name: Option<&str>,
    ) -> Result<()> {
        use crate::policy::Learning;
        let (learning, org) = {
            let map = self.tunnels.lock().unwrap();
            let Some(m) = map.get(name).and_then(|e| e.config.managed.clone()) else {
                return Ok(());
            };
            (m.learning, m.organisation)
        };
        let ok = if t.is_host() {
            learning.allows(dns_name, t.addr())
        } else {
            learning.allows_target(t)
        };
        match learning {
            _ if ok => Ok(()),
            Learning::FullTunnel => bail!("Everything already goes through {label} (full tunnel)."),
            _ => bail!("{org} only allows {} through {label}.", learning.scope()),
        }
    }

    /// "Add a host" with a name: what the tunnel's DNS says it is goes
    /// through the tunnel, as if learned.
    pub(super) async fn add_name(&self, name: &str, host: &str) -> Result<String> {
        let (device, servers, label) = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            let label = e.config.display().to_owned();
            match (&e.info, e.state) {
                (Some(i), TunnelState::Connected) if !i.dns.is_empty() => {
                    (i.device.clone(), i.dns.clone(), label)
                }
                (Some(_), TunnelState::Connected) => {
                    bail!("{label} has no DNS to ask about {host}. Add its address instead.")
                }
                _ => bail!("Switch {label} on first: Crake asks its DNS what {host} is."),
            }
        };
        let reply = ask(&device, &servers, host, RecordType::A)
            .await
            .ok_or_else(|| anyhow!("{label}'s DNS didn't answer."))?;
        let addrs = dns::a_records(&reply);
        if addrs.is_empty() {
            bail!("{label}'s DNS doesn't know {host}.");
        }
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        for a in &addrs {
            guard(&label, Target::host(*a), gw, self.routing.routes_ipv6())?;
            self.check_scope(name, &label, Target::host(*a), Some(host))?;
            if let Some(owner) = self.owner_of(*a, name) {
                let owner = self.label(&owner);
                bail!(
                    "{host} is {a}, which already goes through {owner}. One address goes through one tunnel at a time: forget it there first."
                );
            }
        }
        for a in &addrs {
            self.hosts
                .lock()
                .unwrap()
                .tunnel(name)
                .direct
                .remove(&IpAddr::V4(*a));
            self.learn_one(name, *a, Some(host), "you").await?;
        }
        let expiry = self.expiry_sentence(name);
        let msg = format!("Sent {host} through {label}.");
        self.record_hosts(
            LogKind::Routed,
            name,
            &msg,
            format!("You added it. {expiry}"),
            addrs.iter().map(|a| a.to_string()).collect(),
        );
        Ok(msg)
    }

    /// Send one host through the tunnel until it has gone unused for the expiry time.
    pub async fn add_host(&self, name: &str, input: &str) -> Result<String> {
        let input = input.trim();
        // A name: asked of the tunnel's DNS, its addresses sent through.
        if input.parse::<Target>().is_err() && dns::valid_domain(input) {
            return self.add_name(name, &dns::normalize(input)).await;
        }
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        let t: Target = input.parse()?;
        if !t.is_host() {
            bail!("Only single hosts are learned. To send {t} through {label}, pin it.");
        }
        guard(&label, t, gw, self.routing.routes_ipv6())?;
        self.check_scope(name, &label, t, None)?;
        if let Some(owner) = self.owner_of(t.addr(), name) {
            let owner = self.label(&owner);
            bail!(
                "{t} already goes through {owner}. One address goes through one tunnel at a time: forget it there first."
            );
        }
        let expiry = self.expiry_sentence(name);
        {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if th.pinned.contains(&t) {
                bail!("{t} is already pinned to {label}.");
            }
            // Adding by hand overrides an earlier undo.
            th.direct.remove(&t.addr());
        }
        self.learn_one(name, t.addr(), None, "you").await?;
        let msg = format!("Sent {t} through {label}.");
        self.record_hosts(
            LogKind::Routed,
            name,
            &msg,
            format!("You added it. {expiry}"),
            vec![t.addr().to_string()],
        );
        Ok(msg)
    }

    /// Put one host in the learned set (fresh expiry). Caller holds `ops`.
    /// `how`: why it was learned ("probe", "dns", "inside", "split",
    /// "you"), kept with when, for tunnel detail.
    pub(super) async fn learn_one(
        &self,
        name: &str,
        addr: impl Into<std::net::IpAddr>,
        dns_name: Option<&str>,
        how: &str,
    ) -> Result<()> {
        let addr = addr.into();
        {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            let now = hosts::now();
            if !th.learned.contains_key(&addr) || !th.learned_how.contains_key(&addr) {
                th.learned_how.insert(addr, (now, how.to_owned()));
            }
            th.learned.insert(addr, now + self.timeout_for(name));
            if let Some(n) = dns_name {
                th.names.insert(addr, n.to_owned());
            }
            store.save();
        }
        if self.is_routed(name) {
            self.routing
                .add_learned(name, addr, self.timeout_for(name))
                .await?;
        }
        Ok(())
    }

    /// Send learned hosts direct and keep them direct: Crake won't learn them
    /// for this tunnel again until you redo. Several addresses are one
    /// decision (a name that resolved to several).
    pub async fn undo(&self, name: &str, inputs: &[&str]) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, _) = self.prepare(name)?;
        let addrs = hosts_of(inputs, &label)?;
        let shown = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if let Some(a) = addrs
                .iter()
                .find(|a| th.pinned.contains(&Target::host(**a)))
            {
                bail!("{a} is pinned to {label}. Unpin it first.");
            }
            let mut any = false;
            for a in &addrs {
                any |= th.learned.remove(a).is_some();
                th.direct.insert(*a);
            }
            if !any {
                bail!("{} isn't sent through {label}.", join(&addrs));
            }
            let shown = th
                .names
                .get(&addrs[0])
                .cloned()
                .unwrap_or_else(|| join(&addrs));
            store.save();
            shown
        };
        if self.is_routed(name) {
            for a in &addrs {
                self.routing.remove_learned(name, *a).await?;
            }
        }
        let msg = format!("{shown} goes direct again.");
        self.record_hosts(
            LogKind::Undone,
            name,
            &msg,
            "You undid the route. Crake won\u{2019}t learn this host again on its own.".into(),
            addrs.iter().map(IpAddr::to_string).collect(),
        );
        Ok(msg)
    }

    /// Send hosts through `to` instead of `from` (another tunnel answered for
    /// them too). They stay direct for `from` until redone there.
    pub async fn move_hosts(&self, from: &str, to: &str, inputs: &[&str]) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (from_label, _) = self.prepare(from)?;
        let (to_label, gw) = self.prepare(to)?;
        let addrs = hosts_of(inputs, &to_label)?;
        for a in &addrs {
            guard(&to_label, Target::host(*a), gw, self.routing.routes_ipv6())?;
            let known = th_name(&self.hosts.lock().unwrap(), from, *a);
            self.check_scope(to, &to_label, Target::host(*a), known.as_deref())?;
        }
        let shown = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(from);
            if let Some(a) = addrs
                .iter()
                .find(|a| th.pinned.contains(&Target::host(**a)))
            {
                bail!("{a} is pinned to {from_label}. Unpin it first.");
            }
            for a in &addrs {
                th.learned.remove(a);
                th.direct.insert(*a);
            }
            let shown = th.names.get(&addrs[0]).cloned();
            store.save();
            shown.unwrap_or_else(|| join(&addrs))
        };
        if self.is_routed(from) {
            for a in &addrs {
                self.routing.remove_learned(from, *a).await?;
            }
        }
        for a in &addrs {
            {
                let mut store = self.hosts.lock().unwrap();
                store.tunnel(to).direct.remove(a);
            }
            let known = th_name(&self.hosts.lock().unwrap(), from, *a);
            self.learn_one(to, *a, known.as_deref(), "you").await?;
        }
        let msg = format!("Sent {shown} through {to_label} instead of {from_label}.");
        self.record_hosts(
            LogKind::Routed,
            to,
            &msg,
            format!("You chose {to_label}. {}", self.expiry_sentence(to)),
            addrs.iter().map(IpAddr::to_string).collect(),
        );
        Ok(msg)
    }

    /// Take back an undo: send the hosts through the tunnel again.
    pub async fn redo(&self, name: &str, inputs: &[&str]) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        let addrs = hosts_of(inputs, &label)?;
        for a in &addrs {
            guard(&label, Target::host(*a), gw, self.routing.routes_ipv6())?;
            let known = th_name(&self.hosts.lock().unwrap(), name, *a);
            self.check_scope(name, &label, Target::host(*a), known.as_deref())?;
        }
        let shown = {
            let mut store = self.hosts.lock().unwrap();
            let mut any = false;
            {
                let th = store.tunnel(name);
                for a in &addrs {
                    any |= th.direct.remove(a);
                }
            }
            if !any {
                bail!("{} wasn't sent direct by you.", join(&addrs));
            }
            store.save();
            th_name(&store, name, addrs[0]).unwrap_or_else(|| join(&addrs))
        };
        for a in &addrs {
            self.learn_one(name, *a, None, "you").await?;
        }
        let msg = format!("Sent {shown} through {label} again.");
        self.record_hosts(
            LogKind::Routed,
            name,
            &msg,
            "You redid it.".into(),
            addrs.iter().map(IpAddr::to_string).collect(),
        );
        Ok(msg)
    }

    /// Keep a host or network on the tunnel until unpinned.
    pub async fn pin(&self, name: &str, input: &str) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        let t: Target = input.parse()?;
        guard(&label, t, gw, self.routing.routes_ipv6())?;
        self.check_scope(name, &label, t, None)?;
        let was_learned = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if !th.pinned.insert(t) {
                bail!("{t} is already pinned to {label}.");
            }
            if t.is_host() {
                th.direct.remove(&t.addr());
            }
            let was = t.is_host() && th.learned.remove(&t.addr()).is_some();
            store.save();
            was
        };
        if self.is_routed(name) {
            self.routing.add_pinned(name, t).await?;
            if was_learned {
                self.routing.remove_learned(name, t.addr()).await?;
            }
        }
        let msg = format!("Pinned {t} to {label}.");
        self.record(
            LogKind::Routed,
            name,
            &msg,
            "It stays until you unpin it.".into(),
        );
        Ok(msg)
    }

    /// Unpin. A host goes back to being learned (and expires); a network goes direct.
    pub async fn unpin(&self, name: &str, input: &str) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, _) = self.prepare(name)?;
        let t: Target = input.parse()?;
        {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if !th.pinned.remove(&t) {
                bail!("{t} isn't pinned to {label}.");
            }
            if t.is_host() {
                th.learned
                    .insert(t.addr(), hosts::now() + self.timeout_for(name));
            }
            store.save();
        }
        if self.is_routed(name) {
            if t.is_host() {
                self.routing
                    .add_learned(name, t.addr(), self.timeout_for(name))
                    .await?;
            }
            self.routing.remove_pinned(name, t).await?;
        }
        let msg = format!("Unpinned {t} from {label}.");
        let reason = if t.is_host() {
            self.expiry_sentence(name)
        } else {
            "It goes direct again.".to_owned()
        };
        self.record(LogKind::Undone, name, &msg, reason);
        Ok(msg)
    }

    /// Stop sending a host or network through the tunnel, now.
    pub async fn forget(&self, name: &str, input: &str) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, _) = self.prepare(name)?;
        let t: Target = input.parse()?;
        let (was_pinned, was_learned) = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            let p = th.pinned.remove(&t);
            let l = t.is_host() && th.learned.remove(&t.addr()).is_some();
            if t.is_host() {
                th.names.remove(&t.addr());
                th.learned_how.remove(&t.addr());
            }
            store.save();
            (p, l)
        };
        if !was_pinned && !was_learned {
            bail!("{t} isn't sent through {label}.");
        }
        if self.is_routed(name) {
            if was_pinned {
                self.routing.remove_pinned(name, t).await?;
            }
            if was_learned {
                self.routing.remove_learned(name, t.addr()).await?;
            }
        }
        // Forgotten, not forbidden (that's Undo): the next visit is a first
        // one. Probed again even if it was probed moments ago, and its name
        // asked of DNS again rather than taken from the system's cache, so
        // it's routed before the first packet again, under its name.
        self.probed.lock().unwrap().retain(|a, _| !t.contains(*a));
        self.resolved.flush().await;
        // Open connections to it end, so apps open new ones (a browser
        // reusing a keep-alive connection would wait on it).
        crate::conntrack::close_to(t).await;
        let msg = format!("Stopped sending {t} through {label}.");
        self.record(
            LogKind::Undone,
            name,
            &msg,
            "You forgot it. Next time it's needed, Crake finds out again.".into(),
        );
        Ok(msg)
    }

    /// Pinned first, then learned hosts with time left.
    pub async fn hosts(&self, name: &str) -> Result<Vec<HostRow>> {
        let _ops = self.ops.lock().await;
        self.entry(name)?;
        self.sync_learned(name).await;
        let now = hosts::now();
        let store = self.hosts.lock().unwrap();
        let Some(t) = store.tunnels.get(name) else {
            return Ok(Vec::new());
        };
        let named = |a: IpAddr| t.names.get(&a).cloned();
        let how = |a: IpAddr| t.learned_how.get(&a).cloned();
        let forgets = self.forget_after(name).is_some();
        let timeout = self.timeout_for(name);
        let pinned = t.pinned.iter().map(|p| HostRow {
            target: *p,
            name: if p.is_host() { named(p.addr()) } else { None },
            pinned: true,
            expires_in: None,
            direct: false,
            learned: if p.is_host() { how(p.addr()) } else { None },
            used_ago: None,
        });
        let learned = t
            .learned_remaining(now)
            .into_iter()
            .map(|(a, left)| HostRow {
                target: Target::host(a),
                name: named(a),
                pinned: false,
                expires_in: Some(left),
                direct: false,
                learned: how(a),
                used_ago: forgets.then(|| timeout.saturating_sub(left)),
            });
        let direct = t.direct.iter().map(|a| HostRow {
            target: Target::host(*a),
            name: named(*a),
            pinned: false,
            expires_in: None,
            direct: true,
            learned: None,
            used_ago: None,
        });
        Ok(pinned.chain(learned).chain(direct).collect())
    }
}

/// Single hosts from user input.
pub(super) fn hosts_of(inputs: &[&str], label: &str) -> Result<Vec<IpAddr>> {
    let mut out = Vec::new();
    for i in inputs {
        let t: Target = i.parse()?;
        if !t.is_host() {
            bail!(
                "Only single hosts can be undone. To stop sending {t} through {label}, forget it."
            );
        }
        out.push(t.addr());
    }
    if out.is_empty() {
        bail!("Which host?");
    }
    Ok(out)
}

/// Nothing that would break the machine, and nothing that covers the tunnel's
/// own gateway (that would cut the tunnel off).
pub(super) fn guard(
    label: &str,
    t: Target,
    gateway: Option<Ipv4Addr>,
    ipv6_ok: bool,
) -> Result<()> {
    // IPv6 routing is built on Linux; on platforms where it isn't yet, say so
    // plainly rather than half-route it (the address model carries v6 either
    // way).
    if t.is_ipv6() && !ipv6_ok {
        bail!(
            "Crake doesn't route IPv6 through a tunnel on this platform yet. For now {t} stays direct."
        );
    }
    t.check_routable()?;
    if let Some(gw) = gateway.filter(|gw| t.contains(*gw)) {
        bail!(
            "{t} covers {label}'s gateway ({gw}). Sending it through {label} would cut the tunnel off."
        );
    }
    Ok(())
}
