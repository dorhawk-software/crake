//! Connections that get no answer directly, tried through each tunnel; when
//! two tunnels answer; asking the user first.

use super::*;

impl Manager {
    /// "Ask me first": say what was found and route nothing; the app offers
    /// "Send through …". Not asked again about a host for a while.
    pub(super) fn ask_first(
        &self,
        name: &str,
        label: &str,
        shown: &str,
        why: String,
        addrs: &[Ipv4Addr],
    ) {
        let fresh: Vec<Ipv4Addr> = {
            let mut asked = self.asked.lock().unwrap();
            asked.retain(|_, at| at.elapsed() < PROBED_FOR);
            addrs
                .iter()
                .copied()
                .filter(|a| {
                    asked
                        .insert((name.to_owned(), *a), Instant::now())
                        .is_none()
                })
                .collect()
        };
        if fresh.is_empty() {
            return;
        }
        self.record_hosts(
            LogKind::Asked,
            name,
            &format!("{shown} needs {label}."),
            format!("{why} You asked to decide these yourself."),
            fresh.iter().map(Ipv4Addr::to_string).collect(),
        );
    }

    /// A private address a tunnel's DNS gave for a name: inside one of
    /// the tunnels, almost surely (`probe::KNOWN_INSIDE_WAIT`).
    pub fn known_inside(&self, addr: Ipv4Addr) -> bool {
        if !crate::dns::is_private(addr) {
            return false;
        }
        let store = self.hosts.lock().unwrap();
        store
            .tunnels
            .values()
            .any(|t| t.answers.values().any(|(addrs, _)| addrs.contains(&addr)))
    }

    pub fn probe_wait(&self) -> Duration {
        Duration::from_secs(self.settings.lock().unwrap().probe_wait_secs)
    }

    /// At least one tunnel's routing is in place.
    pub fn any_routed(&self) -> bool {
        self.tunnels.lock().unwrap().values().any(|e| e.routed)
    }

    /// A direct connection to `flow.dst` got no answer, or was refused. Try it
    /// through each tunnel that's up, in order; route it through the first that
    /// answers. The stuck connection is forgotten by conntrack, so the app's
    /// next retry already goes through the tunnel.
    pub async fn probe(&self, flow: crate::conntrack::Flow, why: crate::probe::Direct) {
        let Some(candidates) = self.probe_candidacy(flow.dst, flow.src) else {
            return;
        };
        self.try_tunnels(flow.dst, flow.dport, candidates, why, Some(flow))
            .await;
    }

    /// Whether `dst` (reached from `src`) may be probed, and through which
    /// tunnels: routable, not on the never list, this machine's own and not
    /// the local network, not already routed, not probed in the last 10 min,
    /// and in reach of at least one routed tunnel. Marks it probed.
    fn probe_candidacy(&self, dst: Ipv4Addr, src: Ipv4Addr) -> Option<Vec<(String, String, u32)>> {
        if Target::host(dst).check_routable().is_err()
            || self.never(self.name_for(dst).as_deref(), dst)
        {
            return None;
        }
        let local = crate::probe::Local::load();
        // Only this machine's own connections, and never the local network.
        if !local.is_own(src) || local.on_link(dst) {
            return None;
        }
        if self
            .hosts
            .lock()
            .unwrap()
            .tunnels
            .values()
            .any(|t| t.covers(dst))
        {
            return None;
        }
        {
            let mut probed = self.probed.lock().unwrap();
            probed.retain(|_, at| at.elapsed() < PROBED_FOR);
            if probed.insert(dst, Instant::now()).is_some() {
                return None;
            }
        }
        let candidates = self.probe_candidates(dst);
        (!candidates.is_empty()).then_some(candidates)
    }

    /// Keep the first-SYN race's nftables rule in step with the tunnels
    /// that are up: which addresses are worth holding a SYN for (`watch`)
    /// and which never (`skip`). Called when a tunnel comes up or goes down.
    pub(super) async fn sync_syn_race(&self) {
        use std::collections::BTreeSet;
        if !self.routing.is_real() || !self.cfg.probing || !self.cfg.race_first_syn {
            return;
        }
        // (name, gateway, pushed networks) of each routed tunnel.
        let routed: Vec<(String, Option<Ipv4Addr>, Vec<String>)> = {
            let map = self.tunnels.lock().unwrap();
            map.values()
                .filter(|e| e.routed)
                .map(|e| {
                    let gw = e.info.as_ref().and_then(|i| i.gateway);
                    let splits = e.info.as_ref().map(|i| i.split_include.clone());
                    (e.config.name.clone(), gw, splits.unwrap_or_default())
                })
                .collect()
        };
        let mut watch: BTreeSet<Target> = BTreeSet::new();
        let mut skip: BTreeSet<Target> = BTreeSet::new();
        let mut anything = false;
        for (name, gw, splits) in &routed {
            if let Some(gw) = gw {
                skip.insert(Target::host(*gw));
            }
            match self.learning(name) {
                crate::policy::Learning::Anything => anything = true,
                crate::policy::Learning::Only { nets, .. } => watch.extend(nets),
                // A full tunnel marks everything; nothing to race.
                crate::policy::Learning::FullTunnel => {}
            }
            // The networks the gateway said are behind it.
            watch.extend(splits.iter().filter_map(|n| n.parse::<Target>().ok()));
        }
        // A learn-anything tunnel reaches internal hosts by their private
        // address: race those (the local networks and gateways are skipped).
        if anything {
            watch.extend(
                ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]
                    .iter()
                    .filter_map(|n| n.parse::<Target>().ok()),
            );
        }
        skip.extend(crate::probe::Local::load().networks());
        let (watch, skip): (Vec<Target>, Vec<Target>) =
            (watch.into_iter().collect(), skip.into_iter().collect());
        if let Err(e) = self.routing.race_up(&watch, &skip).await {
            tracing::error!("the first-SYN race rule: {e:#}");
        }
    }

    /// The first SYN to `flow.dst` was held in NFQUEUE (`syn_queue`): race a
    /// direct attempt against the tunnels and route it before the SYN is
    /// ever sent, so even the first connection succeeds. Direct is preferred
    /// when it works; it gets a short window (a slow but reachable host
    /// answers within it), then the tunnels are raced. Returns true when a
    /// tunnel took it (the held SYN is then let through and routes); false
    /// for a direct or down host (let through direct).
    pub async fn race_held(&self, flow: crate::conntrack::Flow) -> bool {
        let Some(candidates) = self.probe_candidacy(flow.dst, flow.src) else {
            return false;
        };
        let addr = SocketAddrV4::new(flow.dst, flow.dport);
        // A reachable host answers directly within a round trip; past this it
        // is treated as silently dropped (only a tunnel might reach it), so
        // the first connection isn't held for the whole probe wait.
        const DIRECT_WAIT: Duration = Duration::from_millis(400);
        let why = match crate::probe::direct(addr, DIRECT_WAIT).await {
            crate::probe::DirectTry::Answered(_) => return false,
            crate::probe::DirectTry::Failed(why) => why,
        };
        self.try_tunnels(flow.dst, flow.dport, candidates, why, None)
            .await
    }

    /// Race the routed tunnels for `dst:dport` and route it through the one
    /// that answers (the `both` setting decides between several). `why` is how
    /// the direct attempt went, for the log. `forget`: a stuck conntrack entry
    /// to delete so the app's retry is routed (passive probing); None when the
    /// held SYN itself routes (the first-SYN race).
    async fn try_tunnels(
        &self,
        dst: Ipv4Addr,
        dport: u16,
        candidates: Vec<(String, String, u32)>,
        why: crate::probe::Direct,
        forget: Option<crate::conntrack::Flow>,
    ) -> bool {
        let shown = self.name_for(dst).unwrap_or_else(|| dst.to_string());
        let addr = SocketAddrV4::new(dst, dport);
        let both = self.settings.lock().unwrap().both;
        // Every eligible tunnel is tried at once; the setting decides which
        // answer counts (CLAUDE.md, "When two tunnels answer").
        let mut tries = tokio::task::JoinSet::new();
        for (i, (_, _, mark)) in candidates.iter().enumerate() {
            let mark = *mark;
            tries.spawn(async move {
                (
                    i,
                    crate::probe::through_tunnel(addr, mark, PROBE_TIMEOUT).await,
                )
            });
        }
        let mut results: Vec<Option<Option<Duration>>> = vec![None; candidates.len()];
        let chosen = loop {
            if let Some(i) = pick(&results, both) {
                break Some(i);
            }
            match tries.join_next().await {
                Some(Ok((i, r))) => results[i] = Some(r),
                Some(Err(_)) => {}
                None => break pick(&results, both),
            }
        };
        if let Some(i) = chosen {
            let (name, label, _) = &candidates[i];
            let took = results[i].flatten().unwrap_or_default();
            if self.asks(name) {
                let ms = took.as_millis().max(1);
                let why = format!("{} Through {label} it answered in {ms} ms.", why.said());
                self.ask_first(name, label, &shown, why, &[dst]);
                return false;
            }
            let _ops = self.ops.lock().await;
            let dns_name = self.name_for(dst);
            if let Err(e) = self
                .learn_one(name, dst, dns_name.as_deref(), "probe")
                .await
            {
                tracing::error!("learning {dst} for {name}: {e:#}");
                return false;
            }
            drop(_ops);
            // Passive probing: forget the app's stuck attempt so its retry is
            // routed afresh. The first-SYN race has no stuck entry (`forget`
            // is None): the held SYN itself routes once the host is learned.
            if let Some(flow) = forget {
                let _ = tokio::task::spawn_blocking(move || crate::conntrack::delete(flow)).await;
            }
            let ms = took.as_millis().max(1);
            let mut reason = format!("{} Through {label} it answered in {ms} ms.", why.said());
            if both == crate::settings::Both::Faster
                && results
                    .iter()
                    .filter(|r| matches!(r, Some(Some(_))))
                    .count()
                    > 1
            {
                reason += " The fastest of the tunnels that answered.";
            }
            self.record_hosts(
                LogKind::Routed,
                name,
                &format!("Sent {shown} through {label}."),
                reason,
                vec![dst.to_string()],
            );
            // "Ask me": hear the others out, and say which answered too.
            if both == crate::settings::Both::Ask {
                while let Some(r) = tries.join_next().await {
                    if let Ok((j, r)) = r {
                        results[j] = Some(r);
                    }
                }
                let others: Vec<usize> = (0..candidates.len())
                    .filter(|j| *j != i && matches!(results[*j], Some(Some(_))))
                    .collect();
                if !others.is_empty() {
                    let labels: Vec<&str> =
                        others.iter().map(|j| candidates[*j].1.as_str()).collect();
                    let verb = if labels.len() == 1 {
                        "answers"
                    } else {
                        "answer"
                    };
                    self.log.append(LogEntry {
                        ts: crate::log::now(),
                        kind: LogKind::System,
                        tunnel: Some(name.clone()),
                        message: format!("{} {verb} for {shown} too.", and_list(&labels)),
                        reason: Some(format!(
                            "Crake sent it through {label}, first in order. You can move it."
                        )),
                        hosts: vec![dst.to_string()],
                        others: others.iter().map(|j| candidates[*j].0.clone()).collect(),
                    });
                }
            }
            return true;
        }
        let labels: Vec<&str> = candidates.iter().map(|(_, l, _)| l.as_str()).collect();
        let through = labels.join(" or ");
        let reason = match why {
            crate::probe::Direct::NoAnswer => {
                format!("It didn't answer directly or through {through}. Probably down.")
            }
            crate::probe::Direct::Refused => {
                format!("Refused directly, and no answer through {through}.")
            }
            crate::probe::Direct::Unreachable => {
                format!("Unreachable directly, and no answer through {through}.")
            }
        };
        let log_tunnel = candidates[0].0.clone();
        self.record_hosts(
            LogKind::LeftAlone,
            &log_tunnel,
            &format!("Left {shown} alone."),
            reason,
            vec![dst.to_string()],
        );
        false
    }

    /// Tunnels a host may be probed through, in order: routed, not sent direct
    /// by you for that tunnel, and not covering the tunnel's gateway.
    pub(super) fn probe_candidates(&self, dst: Ipv4Addr) -> Vec<(String, String, u32)> {
        let name = self.name_for(dst);
        let map = self.tunnels.lock().unwrap();
        let store = self.hosts.lock().unwrap();
        let mut v: Vec<_> = map
            .values()
            .filter(|e| e.routed)
            .filter(|e| {
                let t = store.tunnels.get(&e.config.name);
                !t.is_some_and(|t| t.direct.contains(&dst))
            })
            .filter(|e| {
                let gw = e.info.as_ref().and_then(|i| i.gateway);
                guard(e.config.display(), Target::host(dst), gw).is_ok()
            })
            .filter(|e| e.config.learning().allows(name.as_deref(), dst))
            .map(|e| {
                (
                    (self.rank(&e.config.name, e.index), e.index),
                    e.config.name.clone(),
                    e.config.display().to_owned(),
                )
            })
            .collect();
        v.sort_by_key(|t| t.0);
        v.into_iter()
            .map(|((_, i), n, l)| (n, l, crate::routing::ids(i).mark))
            .collect()
    }

    /// Tunnels whose routing is in place, in config order (the try order),
    /// with their device and DNS servers.
    pub(super) fn routed_tunnels(&self) -> Vec<(String, String, Vec<Ipv4Addr>)> {
        let map = self.tunnels.lock().unwrap();
        let mut v: Vec<_> = map
            .values()
            .filter(|e| e.routed)
            .filter_map(|e| {
                let info = e.info.as_ref()?;
                (!info.dns.is_empty()).then(|| {
                    (
                        self.rank(&e.config.name, e.index),
                        e.config.name.clone(),
                        info.device.clone(),
                        info.dns.clone(),
                    )
                })
            })
            .collect();
        v.sort_by_key(|t| t.0);
        v.into_iter().map(|(_, n, d, s)| (n, d, s)).collect()
    }
}

/// Which tunnel's answer counts so far: `results[i]` is None while tunnel i
/// is still being tried, then whether (and how fast) it answered.
pub(super) fn pick(
    results: &[Option<Option<Duration>>],
    both: crate::settings::Both,
) -> Option<usize> {
    use crate::settings::Both;
    match both {
        // The first to answer is the fastest.
        Both::Faster => results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.flatten().map(|t| (i, t)))
            .min_by_key(|(_, t)| *t)
            .map(|(i, _)| i),
        // The first in order that answers, once those before it haven't.
        Both::First | Both::Ask => {
            for (i, r) in results.iter().enumerate() {
                match r {
                    Some(Some(_)) => return Some(i),
                    Some(None) => continue,
                    None => return None,
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Both;

    #[test]
    fn which_tunnels_answer_counts() {
        let ms = |n| Some(Some(Duration::from_millis(n)));
        let silent = Some(None);
        // ACME (first in order) still trying, Nordbank answered.
        let r = [None, ms(40)];
        assert_eq!(pick(&r, Both::First), None, "wait for the first in order");
        assert_eq!(pick(&r, Both::Ask), None);
        assert_eq!(pick(&r, Both::Faster), Some(1), "the first to answer");
        // ACME gave up: Nordbank it is.
        assert_eq!(pick(&[silent, ms(40)], Both::First), Some(1));
        assert_eq!(pick(&[ms(90), ms(40)], Both::First), Some(0));
        assert_eq!(pick(&[ms(90), ms(40)], Both::Faster), Some(1));
        assert_eq!(pick(&[silent, silent], Both::Ask), None);
        assert_eq!(
            and_list(&["ACME", "Globex", "Nordbank"]),
            "ACME, Globex and Nordbank"
        );
    }
}
