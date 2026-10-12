//! A tunnel's DNS that stops answering (7h): noticed, said, and known names
//! answered from memory meanwhile.

use super::*;

impl Manager {
    /// Ask each connected tunnel's DNS a small question through the tunnel
    /// (its first domain's SOA): any answer at all means it's there.
    pub(super) async fn check_dns(self: &Arc<Self>) {
        let up: Vec<(String, String, Vec<Ipv4Addr>)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.state == TunnelState::Connected)
            .filter_map(|(n, e)| {
                let i = e.info.as_ref()?;
                (!i.dns.is_empty()).then(|| (n.clone(), i.device.clone(), i.dns.clone()))
            })
            .collect();
        for (name, device, servers) in up {
            let Some(domain) = self.domains(&name).into_iter().next() else {
                continue;
            };
            let alive = ask_within(&device, &servers, &domain, RecordType::SOA, DNS_CHECK_WAIT)
                .await
                .is_some();
            let misses = {
                let mut m = self.dns_misses.lock().unwrap();
                if alive {
                    m.remove(&name);
                    0
                } else {
                    let n = m.entry(name.clone()).or_insert(0);
                    *n = n.saturating_add(1);
                    *n
                }
            };
            if alive || misses >= DNS_CHECK_MISSES {
                self.dns_checked(&name, alive).await;
            }
        }
    }

    /// A tunnel's DNS answers (again) or has stopped: say so once, and move
    /// its names to the memory answerer or back.
    pub async fn dns_checked(self: &Arc<Self>, name: &str, alive: bool) {
        let Some((label, info)) = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .and_then(|e| Some((e.config.display().to_owned(), e.info.clone()?)))
        else {
            return;
        };
        let domains = self.domains(name);
        let was_down = self.dns_down.lock().unwrap().contains_key(name);
        if !alive && !was_down {
            self.dns_down
                .lock()
                .unwrap()
                .insert(name.to_owned(), std::collections::HashSet::new());
            let servers: Vec<String> = info.dns.iter().map(Ipv4Addr::to_string).collect();
            self.log.system(
                Some(name),
                format!("{label}'s DNS ({}) doesn't answer.", servers.join(", ")),
                Some(format!(
                    "Names under {} won't resolve until it answers again. Meanwhile Crake answers the names it has seen with their last address, while that still answers through {label}.",
                    domains.join(", ")
                )),
            );
            // Nobody waits for it meanwhile.
            self.resolved.revert(&info.device).await;
            // The answerer's address exists once its link is up.
            self.apply_keep().await;
            if self.cfg.apply_network
                && self.cfg.use_resolved
                && !self
                    .keep_serving
                    .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                let this = Arc::clone(self);
                tokio::spawn(async move {
                    if let Err(e) = crate::keep::serve(this).await {
                        tracing::error!("answering from memory: {e:#}");
                    }
                });
            }
            let _ = self.changes.send(name.to_owned());
        } else if alive && was_down {
            self.dns_down.lock().unwrap().remove(name);
            self.log
                .system(Some(name), format!("{label}'s DNS answers again."), None);
            self.register_dns(name, &label, &info, &domains).await;
            self.apply_keep().await;
            let _ = self.changes.send(name.to_owned());
        }
    }

    /// The memory answerer's link takes the domains of every tunnel whose
    /// DNS doesn't answer; with none, it goes.
    pub(super) async fn apply_keep(&self) {
        if !(self.cfg.apply_network && self.cfg.use_resolved) {
            return;
        }
        let down: Vec<String> = self.dns_down.lock().unwrap().keys().cloned().collect();
        let mut domains: Vec<String> = Vec::new();
        for t in &down {
            for d in self.domains(t) {
                if !domains.contains(&d) {
                    domains.push(d);
                }
            }
        }
        let _keep = self.keep.lock().await;
        if domains.is_empty() {
            self.resolved.revert(crate::keep::DEVICE).await;
            self.net.remove(crate::keep::DEVICE).await;
            return;
        }
        if let Err(e) = self.net.dummy(crate::keep::DEVICE, crate::keep::ADDR).await {
            tracing::error!("answering from memory: {e:#}");
            return;
        }
        if let Err(e) = self
            .resolved
            .set_link(crate::keep::DEVICE, &[crate::keep::ADDR], &domains, false)
            .await
        {
            tracing::error!("answering from memory: {e:#}");
        }
        self.resolved.flush().await;
    }

    /// The memory answerer's reply to one question: the last address its
    /// tunnel's DNS gave, if that still answers through the tunnel; else
    /// SERVFAIL (it doesn't resolve, honestly).
    pub async fn keep_answer(&self, query: &[u8]) -> Option<Vec<u8>> {
        let (qname, qtype) = crate::keep::question(query)?;
        let down: Vec<String> = self.dns_down.lock().unwrap().keys().cloned().collect();
        let Some(tunnel) = down
            .into_iter()
            .find(|t| dns::in_domains(&qname, &self.domains(t)))
        else {
            return crate::hush::reply_code(query, 2);
        };
        // Its last answer; else the hosts it routes under that name (learned
        // or pinned before answers were remembered, or since forgotten).
        let known = self
            .hosts
            .lock()
            .unwrap()
            .tunnels
            .get(&tunnel)
            .and_then(|t| {
                t.answers.get(&qname).map(|(a, _)| a.clone()).or_else(|| {
                    // Only IPv4 answers are kept for now (AAAA learning is a
                    // later slice), matching `answers` above.
                    let named: Vec<Ipv4Addr> = t
                        .names
                        .iter()
                        .filter_map(|(a, n)| match *a {
                            IpAddr::V4(v) if *n == qname && t.covers(*a) => Some(v),
                            _ => None,
                        })
                        .collect();
                    (!named.is_empty()).then_some(named)
                })
            });
        let Some(addrs) = known else {
            return crate::hush::reply_code(query, 2);
        };
        if qtype != RecordType::A {
            // The name exists; it just has no such record we know of.
            return crate::keep::answer(query, &[]);
        }
        let learning = self.learning(&tunnel);
        let mut ok = Vec::new();
        for a in addrs {
            if learning.allows(Some(&qname), a) && self.still_answers(&tunnel, a).await {
                ok.push(a);
            }
        }
        if ok.is_empty() {
            return crate::hush::reply_code(query, 2);
        }
        // Routed through the tunnel (it may have expired meanwhile).
        {
            let _ops = self.ops.lock().await;
            for a in &ok {
                let covered = self
                    .hosts
                    .lock()
                    .unwrap()
                    .tunnels
                    .get(&tunnel)
                    .is_some_and(|t| t.covers(*a));
                if !covered && let Err(e) = self.learn_one(&tunnel, *a, Some(&qname), "dns").await {
                    tracing::error!("routing {a} for {tunnel}: {e:#}");
                }
            }
        }
        let first = self
            .dns_down
            .lock()
            .unwrap()
            .get_mut(&tunnel)
            .is_some_and(|said| said.insert(qname.clone()));
        if first {
            let label = self.label(&tunnel);
            self.log.system(
                Some(&tunnel),
                format!("{label}'s DNS doesn't answer. Using the address {qname} had last time."),
                Some(format!("{} still answers through {label}.", ok[0])),
            );
        }
        crate::keep::answer(query, &ok)
    }

    /// `addr` answers through `tunnel` on a web port (in development and
    /// tests, where nothing is applied, it's taken as answering).
    pub(super) async fn still_answers(&self, tunnel: &str, addr: Ipv4Addr) -> bool {
        if !self.cfg.apply_network {
            return true;
        }
        let Ok((index, _, _)) = self.entry(tunnel) else {
            return false;
        };
        let mark = crate::routing::ids(index).mark;
        for port in WEB_PORTS {
            if crate::probe::through_tunnel(SocketAddrV4::new(addr, port), mark, WEB_PROBE)
                .await
                .is_some()
            {
                return true;
            }
        }
        false
    }
}
