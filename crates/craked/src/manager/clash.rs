//! Tunnels that can't be up together, and an address another tunnel already
//! has (milestone 5).

use super::*;

/// "Two companies, one address" is said once in this long.
const SAID_EVERY: Duration = Duration::from_secs(600);

impl Manager {
    /// A tunnel that's up and can't be up together with `name`, and why:
    /// they share a DNS server (craked routes each server through its own
    /// tunnel), their own networks overlap, or a network is pinned to both.
    pub(super) fn clash(
        &self,
        name: &str,
        dns: &[Ipv4Addr],
        net: Option<Target>,
    ) -> Option<(String, String)> {
        // The tunnels that are up, copied out: one lock at a time.
        let up: Vec<(String, String, TunnelInfo)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(other, e)| *other != name && e.state == TunnelState::Connected)
            .filter_map(|(other, e)| {
                e.info
                    .clone()
                    .map(|i| (other.clone(), e.config.display().to_owned(), i))
            })
            .collect();
        let pinned_of = |t: &str| -> Vec<Target> {
            self.hosts
                .lock()
                .unwrap()
                .tunnels
                .get(t)
                .map(|h| h.pinned.iter().copied().collect())
                .unwrap_or_default()
        };
        let pinned = pinned_of(name);
        for (other, label, info) in up {
            if let Some(d) = dns.iter().find(|d| info.dns.contains(d)) {
                return Some((label, format!("Both use {d} for DNS.")));
            }
            if let (Some(a), Some(b)) = (net, own_net(&info))
                && overlap(a, b)
            {
                let wider = if a.prefix_len() <= b.prefix_len() {
                    a
                } else {
                    b
                };
                return Some((label, format!("Both are on {wider}.")));
            }
            let theirs = pinned_of(&other);
            if let Some(p) = pinned
                .iter()
                .find(|p| theirs.iter().any(|q| overlap(**p, *q)))
            {
                return Some((label, format!("{p} is pinned to both.")));
            }
        }
        None
    }

    /// The tunnel that's up and already sends `addr` through itself
    /// (learned or pinned), other than `except`.
    pub(super) fn owner_of(
        &self,
        addr: impl Into<std::net::IpAddr>,
        except: &str,
    ) -> Option<String> {
        let addr = addr.into();
        let up: Vec<String> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, e)| *n != except && e.state == TunnelState::Connected)
            .map(|(n, _)| n.clone())
            .collect();
        let store = self.hosts.lock().unwrap();
        up.into_iter()
            .find(|t| store.tunnels.get(t).is_some_and(|h| h.covers(addr)))
    }

    /// Say (once per SAID_EVERY) that `host` on `tunnel` has an address that
    /// already goes through `owner`, and offer to move it.
    pub(super) fn contested(
        &self,
        tunnel: &str,
        label: &str,
        owner: &str,
        host: &str,
        addr: IpAddr,
    ) {
        {
            let mut seen = self.contested.lock().unwrap();
            let key = (tunnel.to_owned(), addr);
            if seen.get(&key).is_some_and(|t| t.elapsed() < SAID_EVERY) {
                return;
            }
            seen.insert(key, Instant::now());
        }
        let owner_label = self.label(owner);
        let theirs = th_name(&self.hosts.lock().unwrap(), owner, addr)
            .map(|n| format!(" ({n})"))
            .unwrap_or_default();
        self.log.append(LogEntry {
            ts: crate::log::now(),
            kind: LogKind::System,
            tunnel: Some(owner.to_owned()),
            message: format!("{host} can't go through {label}."),
            reason: Some(format!(
                "Its address, {addr}, already goes through {owner_label}{theirs}. One address goes through one tunnel at a time; you can move it to {label}."
            )),
            hosts: vec![addr.to_string()],
            others: vec![tunnel.to_owned()],
            short: None,
        });
    }

    /// `clash` with what `name` got when it was last up, before it's
    /// switched on again: said before signing in, not after.
    pub(super) fn remembered_clash(&self, name: &str) -> Option<String> {
        let (dns, net) = {
            let store = self.hosts.lock().unwrap();
            let t = store.tunnels.get(name)?;
            (t.last_dns.clone(), t.last_net)
        };
        let display = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .map(|e| e.config.display().to_owned())?;
        self.clash(name, &dns, net)
            .map(|(other, why)| format!("Turn {other} off to use {display}. {why}"))
    }
}

/// The network a tunnel's own address is on (none for a single address).
pub(super) fn own_net(info: &TunnelInfo) -> Option<Target> {
    let (addr, len) = (info.address?, info.prefix?);
    if len == 0 || len >= crate::hosts::V4_BITS {
        return None;
    }
    let mask = u32::MAX << (Ipv4Addr::BITS - u32::from(len));
    format!("{}/{len}", Ipv4Addr::from(u32::from(addr) & mask))
        .parse()
        .ok()
}

/// Two networks (or hosts) share an address.
pub(super) fn overlap(a: Target, b: Target) -> bool {
    a.contains(b.addr()) || b.contains(a.addr())
}
