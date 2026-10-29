//! Public hosts learned by mistake, tried directly again every hour.
//!
//! One lost connection on a hiccuping network can still send a public site
//! through a company's tunnel when the direct second try fails too (the
//! user's Mac, 2026-10-26: a public site through Initech, on a morning when
//! even github.com took two seconds to answer directly). Until 2026-10-28
//! this ran once per version, so such a host stayed for days.

use super::*;
use crate::hosts::LearnedOn;

/// Ports a host learned before Crake kept how it was reached is tried on
/// directly: its web server, as split-horizon tries one.
const RECHECK_PORTS: [u16; 2] = [443, 80];
/// How often each tunnel's learned public hosts are tried directly.
const RECHECK_EVERY: Duration = Duration::from_secs(60 * 60);

/// Whether a tunnel last rechecked at `last` (seconds since the epoch) is
/// due again at `now`.
fn due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|at| now.saturating_sub(at) >= RECHECK_EVERY.as_secs())
}

/// The ports to try a host on directly: the one it was learned on, or the
/// web server's for one learned before that was kept. None for a UDP host:
/// there's no datagram to try it with, and a web server answering says
/// nothing about it.
fn ports(on: Option<LearnedOn>) -> Option<Vec<u16>> {
    match on {
        Some(LearnedOn::Tcp(port)) => Some(vec![port]),
        Some(LearnedOn::Udp(_)) => None,
        None => Some(RECHECK_PORTS.to_vec()),
    }
}

impl Manager {
    /// Note what the connection that taught `name` the host `addr` was.
    /// Caller holds `ops`, right after `learn_one`.
    pub(super) fn note_learned_on(
        &self,
        name: &str,
        addr: IpAddr,
        attempt: &crate::probe::Attempt,
    ) {
        let on = match attempt {
            crate::probe::Attempt::Tcp(port) => LearnedOn::Tcp(*port),
            crate::probe::Attempt::Udp(port, _) => LearnedOn::Udp(*port),
        };
        let mut store = self.hosts.lock().unwrap();
        store.tunnel(name).learned_on.insert(addr, on);
        store.save();
    }

    /// Every hour, for each tunnel that's up, every public address it
    /// learned by probing is tried directly on the port it was learned on
    /// (craked's direct mark passes the tunnels); one that answers is
    /// forgotten, and said. A company's host that answers only from inside
    /// stays: probing learns only what doesn't answer directly, so the two
    /// can't take turns. On Windows and macOS a learned host is a route,
    /// which craked's sockets follow too: the direct mark binds there to
    /// the default route's interface.
    pub(super) async fn recheck_public(&self) {
        if !self.cfg.apply_network {
            return;
        }
        let up: Vec<(String, String)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.state == TunnelState::Connected)
            .map(|(n, e)| (n.clone(), e.config.display().to_owned()))
            .collect();
        for (name, label) in up {
            let last = self.hosts.lock().unwrap().tunnel(&name).rechecked_at;
            if due(last, hosts::now()) {
                self.recheck_tunnel(&name, &label).await;
            }
        }
    }

    /// Try the tunnel's learned public hosts directly now (the background
    /// loop does it hourly; tests call it), forgetting those that answer.
    pub async fn recheck_tunnel(&self, name: &str, label: &str) {
        let due: Vec<(IpAddr, Option<String>, Vec<u16>)> = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            th.learned
                .keys()
                .filter(|a| !crate::dns::is_private(**a))
                .filter(|a| th.learned_how.get(a).is_some_and(|(_, how)| how == "probe"))
                .filter_map(|a| {
                    ports(th.learned_on.get(a).copied()).map(|p| (*a, th.names.get(a).cloned(), p))
                })
                .collect()
        };
        let mut tries = tokio::task::JoinSet::new();
        for (addr, shown, ports) in due {
            tries.spawn(async move {
                for port in ports {
                    let took = crate::probe::Attempt::Tcp(port)
                        .through(addr, crate::plan::DIRECT_MARK, PROBE_TIMEOUT)
                        .await;
                    if took.is_some() {
                        return (addr, shown, took);
                    }
                }
                (addr, shown, None)
            });
        }
        let mut direct: Vec<(IpAddr, Option<String>, Duration)> = Vec::new();
        while let Some(Ok((addr, shown, took))) = tries.join_next().await {
            if let Some(took) = took {
                direct.push((addr, shown, took));
            }
        }
        self.send_direct_again(name, label, direct).await;
    }

    /// Forget what answers directly (not Undo: Crake may learn it again if
    /// it's ever needed), say so once per name, and note the time.
    async fn send_direct_again(
        &self,
        name: &str,
        label: &str,
        direct: Vec<(IpAddr, Option<String>, Duration)>,
    ) {
        let _ops = self.ops.lock().await;
        {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            for (addr, _, _) in &direct {
                th.learned.remove(addr);
                th.names.remove(addr);
                th.learned_how.remove(addr);
                th.learned_on.remove(addr);
            }
            th.rechecked_at = Some(hosts::now());
            store.save();
        }
        let mut said: BTreeMap<String, Duration> = BTreeMap::new();
        for (addr, shown, took) in &direct {
            if self.is_routed(name)
                && let Err(e) = self.routing.remove_learned(name, *addr).await
            {
                tracing::warn!("{name}: {addr}: {e:#}");
            }
            crate::conntrack::close_to(Target::host(*addr)).await;
            said.entry(shown.clone().unwrap_or_else(|| addr.to_string()))
                .or_insert(*took);
        }
        for (shown, took) in said {
            self.log.system(
                Some(name),
                format!("{shown} goes direct again."),
                Some(format!(
                    "It answers directly now ({}\u{a0}ms): Crake had sent it through {label} when a connection to it failed.",
                    took.as_millis()
                )),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tunnel_is_rechecked_every_hour() {
        let hour = RECHECK_EVERY.as_secs();
        assert!(due(None, 1_000), "never rechecked: now");
        assert!(!due(Some(1_000), 1_000 + hour - 1));
        assert!(due(Some(1_000), 1_000 + hour));
        assert!(!due(Some(1_000), 900), "a clock set back waits");
    }

    #[test]
    fn a_host_is_tried_on_the_port_it_was_learned_on() {
        assert_eq!(ports(Some(LearnedOn::Tcp(5432))), Some(vec![5432]));
        assert_eq!(
            ports(None),
            Some(vec![443, 80]),
            "learned before it was kept"
        );
        assert_eq!(
            ports(Some(LearnedOn::Udp(51820))),
            None,
            "no datagram to try"
        );
    }

    #[test]
    fn how_it_was_learned_is_kept_as_json() {
        let on: LearnedOn = serde_json::from_str(r#"{"tcp":5432}"#).unwrap();
        assert_eq!(on, LearnedOn::Tcp(5432));
        assert_eq!(
            serde_json::to_string(&LearnedOn::Udp(53)).unwrap(),
            r#"{"udp":53}"#
        );
    }
}
