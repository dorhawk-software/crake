//! Learning from DNS: answers from a tunnel's DNS, names public DNS doesn't
//! know, and public names with a private address (split-horizon).

use super::*;

/// Recent public answers kept to give log lines a name, before old ones go.
const SEEN_NAMES_MAX: usize = 4096;

impl Manager {
    /// Called with each DNS answer from a tunnel's DNS servers, before the
    /// answer is let through. Names outside the tunnel's domains are ignored.
    pub async fn learn_from_dns(&self, name: &str, answer: Answer) {
        if answer.addrs.is_empty() || !dns::in_domains(&answer.name, &self.domains(name)) {
            return;
        }
        // The last good answer, for when this DNS stops answering (7h).
        {
            let mut store = self.hosts.lock().unwrap();
            if store
                .tunnel(name)
                .remember(&answer.name, &answer.addrs, hosts::now())
            {
                store.save();
            }
        }
        self.learn_answer(name, &answer, Why::TunnelDomain).await;
    }

    /// Called with each public DNS answer (a whole packet), before it's let
    /// through: `rescue_answer`'s packet carrying the tunnel's answer instead.
    pub async fn rescue(&self, packet: &[u8]) -> Option<Vec<u8>> {
        let payload = dns::public_payload(packet)?;
        let answer = self.rescue_answer(payload).await?;
        dns::replace_payload(packet, &answer)
    }

    /// Called with each public DNS answer (the DNS message), before it's let
    /// through. When it says a name doesn't exist, ask each tunnel that is
    /// up, in order. If one knows the name, route its addresses and return
    /// the tunnel's answer instead (as is a public name with a private
    /// address, `split_horizon`).
    pub async fn rescue_answer(&self, payload: &[u8]) -> Option<Vec<u8>> {
        if let Some(answer) = dns::parse_message(payload) {
            self.remember_names(&answer);
            if !self.settings.lock().unwrap().ask_unknown {
                return None;
            }
            return self.split_horizon(payload).await;
        }
        // Settings: "Ask every tunnel when a name is unknown".
        if !self.settings.lock().unwrap().ask_unknown {
            return None;
        }
        let failure = dns::public_failure(payload)?;
        let key = (failure.name.clone(), failure.qtype);
        {
            let mut unknown = self.unknown.lock().unwrap();
            unknown.retain(|_, at| at.elapsed() < UNKNOWN_FOR);
            if unknown.contains_key(&key) {
                return None;
            }
        }
        for (tunnel, device, servers) in self.routed_tunnels() {
            if dns::in_domains(&failure.name, &self.domains(&tunnel)) {
                continue; // already this tunnel's name, and it said no
            }
            let learning = self.learning(&tunnel);
            if learning == crate::policy::Learning::FullTunnel {
                continue; // its DNS answers everything already
            }
            let Some(reply) = ask(&device, &servers, &failure.name, failure.qtype).await else {
                continue;
            };
            if reply.metadata.response_code != ResponseCode::NoError {
                // The tunnel's DNS knows the name, just not this record
                // (it refused or failed an AAAA for a name it has an A for,
                // as some company DNS servers do): "no such record", never
                // the public "no such name", which made Windows drop the
                // name's A answer too (CI's Windows, 2026-10-20).
                if let Some(other) = other_family(failure.qtype)
                    && ask(&device, &servers, &failure.name, other)
                        .await
                        .is_some_and(|r| !dns::records(&r).is_empty())
                {
                    let mut empty = reply;
                    empty.metadata.response_code = ResponseCode::NoError;
                    empty.answers.clear();
                    return dns::rewrite(&failure, empty);
                }
                continue;
            }
            let addrs = dns::records(&reply);
            // A managed tunnel answers only for what its policy allows.
            if !addrs.is_empty()
                && !addrs
                    .iter()
                    .any(|a| learning.allows(Some(&failure.name), *a))
            {
                continue;
            }
            if !addrs.is_empty() {
                let answer = Answer {
                    name: failure.name.clone(),
                    addrs,
                };
                self.learn_answer(&tunnel, &answer, Why::PublicFailed).await;
            }
            return dns::rewrite(&failure, reply);
        }
        self.unknown.lock().unwrap().insert(key, Instant::now());
        None
    }

    /// A public answer for a name under a tunnel's organisation (the
    /// gateway's own domain, e.g. acme.example): ask that tunnel's DNS too. When it
    /// knows a private address the public DNS doesn't, that is the one that
    /// works (the public one often only lets the company's addresses in):
    /// route it and hand the app the tunnel's answer instead.
    pub(super) async fn split_horizon(&self, payload: &[u8]) -> Option<Vec<u8>> {
        let (question, public) = dns::public_answer(payload)?;
        let tunnels: Vec<_> = self
            .routed_tunnels()
            .into_iter()
            .filter(|(t, _, _)| {
                self.organisation(t)
                    .is_some_and(|org| dns::in_domains(&question.name, &[org]))
            })
            .collect();
        if tunnels.is_empty() {
            return None;
        }
        {
            let mut no_split = self.no_split.lock().unwrap();
            no_split.retain(|_, at| at.elapsed() < UNKNOWN_FOR);
            if no_split.contains_key(&(question.name.clone(), question.qtype)) {
                return None;
            }
        }
        for (tunnel, device, servers) in tunnels {
            let learning = self.learning(&tunnel);
            if learning == crate::policy::Learning::FullTunnel {
                continue;
            }
            let Some(reply) = ask(&device, &servers, &question.name, question.qtype).await else {
                continue;
            };
            if reply.metadata.response_code != ResponseCode::NoError {
                continue;
            }
            let private: Vec<IpAddr> = dns::records(&reply)
                .into_iter()
                .filter(|a| dns::is_private(*a) && !public.contains(a))
                .filter(|a| learning.allows(Some(&question.name), *a))
                .collect();
            if private.is_empty() {
                continue;
            }
            // Only if the private address answers through this tunnel (web
            // ports): some internal answers are for the office network only,
            // and the public address would have been the better bet.
            let Ok((index, _, _)) = self.entry(&tunnel) else {
                continue;
            };
            let mark = crate::routing::ids(index).mark;
            let mut answers = false;
            for port in WEB_PORTS {
                let addr = SocketAddr::new(private[0], port);
                if crate::probe::through_tunnel(addr, mark, WEB_PROBE)
                    .await
                    .is_some()
                {
                    answers = true;
                    break;
                }
            }
            if !answers {
                continue;
            }
            let answer = Answer {
                name: question.name.clone(),
                addrs: private.clone(),
            };
            self.learn_answer(&tunnel, &answer, Why::PrivateAddress)
                .await;
            // Only hand out the private address if it now goes through (not
            // when you sent it direct, or the never list has it).
            let routed = {
                let store = self.hosts.lock().unwrap();
                store
                    .tunnels
                    .get(&tunnel)
                    .is_some_and(|t| private.iter().all(|a| t.covers(*a)))
            };
            if !routed {
                return None;
            }
            return dns::rewrite(&question, reply);
        }
        self.no_split
            .lock()
            .unwrap()
            .insert((question.name.clone(), question.qtype), Instant::now());
        None
    }

    /// The tunnel's organisation domain, from its gateway.
    pub(super) fn organisation(&self, tunnel: &str) -> Option<String> {
        let map = self.tunnels.lock().unwrap();
        dns::organisation(&map.get(tunnel)?.config.gateway)
    }

    pub(super) fn remember_names(&self, answer: &Answer) {
        if answer.addrs.is_empty() {
            return;
        }
        let mut seen = self.seen_names.lock().unwrap();
        if seen.len() > SEEN_NAMES_MAX {
            seen.retain(|_, (_, at)| at.elapsed() < SEEN_NAME_FOR);
        }
        let now = Instant::now();
        for a in &answer.addrs {
            let kept = keep_name(seen.get(a), &answer.name, now);
            seen.insert(*a, kept);
        }
    }

    pub(super) fn name_for(&self, addr: IpAddr) -> Option<String> {
        let seen = self
            .seen_names
            .lock()
            .unwrap()
            .get(&addr)
            .filter(|(_, at)| at.elapsed() < SEEN_NAME_FOR)
            .map(|(n, _)| n.clone());
        // Failing that, the name a tunnel's DNS gave this address last (an
        // app that kept the address in its own DNS cache, as browsers do,
        // reaches it without asking: probing finds it by address).
        seen.or_else(|| {
            let store = self.hosts.lock().unwrap();
            store
                .tunnels
                .values()
                .flat_map(|t| &t.answers)
                .filter(|(_, (addrs, _))| addrs.contains(&addr))
                .max_by_key(|(_, (_, at))| *at)
                .map(|(name, _)| name.clone())
        })
    }

    /// Route the addresses of one answer through the tunnel, and say so.
    pub(super) async fn learn_answer(&self, name: &str, answer: &Answer, why: Why) {
        let _ops = self.ops.lock().await;
        let Ok((label, gw)) = self.prepare(name) else {
            return;
        };
        let learning = self.learning(name);
        let asks = self.asks(name);
        let mut sent = Vec::new();
        for addr in &answer.addrs {
            // Outside a managed tunnel's scope: not this tunnel's business.
            if !learning.allows(Some(&answer.name), *addr) {
                continue;
            }
            {
                let mut store = self.hosts.lock().unwrap();
                let th = store.tunnel(name);
                // Already sent, or sent direct by you.
                if th.covers(*addr) || th.direct.contains(addr) {
                    continue;
                }
            }
            // Already another tunnel's: two companies using the same
            // private address. One address goes through one tunnel; it
            // stays where it is, and the user may move it.
            if let Some(owner) = self.owner_of(*addr, name) {
                self.contested(name, &label, &owner, &answer.name, *addr);
                continue;
            }
            if self.never(Some(&answer.name), *addr) {
                continue;
            }
            if let Err(e) = guard(&label, Target::host(*addr), gw, self.routing.routes_ipv6()) {
                self.record(
                    LogKind::LeftAlone,
                    name,
                    &format!("Left {} alone.", answer.name),
                    format!("{e:#}"),
                );
                continue;
            }
            // "Ask me first": found, not routed; the user decides.
            if asks {
                sent.push(addr.to_string());
                continue;
            }
            let how = match &why {
                Why::TunnelDomain => "dns",
                Why::PublicFailed => "inside",
                Why::PrivateAddress => "split",
            };
            match self.learn_one(name, *addr, Some(&answer.name), how).await {
                Ok(()) => sent.push(addr.to_string()),
                Err(e) => tracing::error!("learning {addr} for {name}: {e:#}"),
            }
        }
        if asks {
            let addrs: Vec<IpAddr> = sent.iter().filter_map(|a| a.parse().ok()).collect();
            let why = match why {
                Why::TunnelDomain => format!("Only {label}'s DNS knows it."),
                Why::PublicFailed => format!("Public DNS didn't know it; {label}'s DNS did."),
                Why::PrivateAddress => {
                    format!("Public name, private address: {label}'s DNS knows it.")
                }
            };
            self.ask_first(name, &label, &answer.name, why, &addrs);
            return;
        }
        if !sent.is_empty() {
            self.record_hosts(
                LogKind::Routed,
                name,
                &format!("Sent {} through {label}.", answer.name),
                                match why {
                    Why::TunnelDomain => format!(
                        "Only {label}'s DNS knows it ({}). Routed before the first packet.",
                        sent.join(", ")
                    ),
                    Why::PublicFailed => format!(
                        "Public DNS didn't know it; {label}'s DNS did ({}). Routed before the first packet.",
                        sent.join(", ")
                    ),
                    Why::PrivateAddress => format!(
                        "Public name, private address: {label}'s DNS knows it as {}. Routed before the first packet.",
                        sent.join(", ")
                    ),
                },
                sent.clone(),
            );
        }
    }
}

/// Ask a tunnel's DNS servers, in order, about `name` (e.g. the question
/// that failed publicly). The socket is bound to the tunnel, so the
/// question can't leak elsewhere.
pub(super) async fn ask(
    device: &str,
    servers: &[Ipv4Addr],
    name: &str,
    qtype: RecordType,
) -> Option<Message> {
    ask_within(device, servers, name, qtype, TUNNEL_DNS_WAIT).await
}

/// `ask`, giving each server `wait` to answer.
pub(super) async fn ask_within(
    device: &str,
    servers: &[Ipv4Addr],
    name: &str,
    qtype: RecordType,
    wait: Duration,
) -> Option<Message> {
    use std::hash::BuildHasher;
    for server in servers {
        let id = std::collections::hash_map::RandomState::new().hash_one(Instant::now()) as u16;
        let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
        crate::sys::bind_device(&sock, device).ok()?;
        if sock.connect((*server, dns::PORT)).await.is_err() {
            continue;
        }
        if sock.send(&dns::query(id, name, qtype)).await.is_err() {
            continue;
        }
        let mut buf = vec![0u8; dns::BUFFER];
        let wait = tokio::time::timeout(wait, async {
            loop {
                let n = sock.recv(&mut buf).await.ok()?;
                if let Some(m) = dns::parse_reply(&buf[..n], id, name) {
                    return Some(m);
                }
            }
        });
        if let Ok(Some(reply)) = wait.await {
            return Some(reply);
        }
    }
    None
}

/// The name an address goes by after `name` was answered with it: the one
/// it has while that's fresh (looked up within SEEN_NAME_FOR; looking it
/// up again keeps it fresh), another name only once it has gone stale.
/// Many names can share one address (a company's DNS gave every name under
/// gitlab.initech.example the same one): the latest answer won, so one
/// stray lookup (a proxy auto-detect's wpad.gitlab.initech.example) renamed
/// every connection to it in the Traffic window.
fn keep_name(kept: Option<&(String, Instant)>, name: &str, now: Instant) -> (String, Instant) {
    match kept {
        Some((n, at)) if n != name && now.saturating_duration_since(*at) < SEEN_NAME_FOR => {
            (n.clone(), *at)
        }
        _ => (name.to_owned(), now),
    }
}

/// An address question's other family (A for AAAA, AAAA for A).
fn other_family(qtype: RecordType) -> Option<RecordType> {
    match qtype {
        RecordType::A => Some(RecordType::AAAA),
        RecordType::AAAA => Some(RecordType::A),
        _ => None,
    }
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn an_address_keeps_its_name_while_it_is_fresh() {
        let t0 = Instant::now();
        let gitlab = keep_name(None, "gitlab.initech.example", t0);
        assert_eq!(gitlab.0, "gitlab.initech.example");
        // A stray lookup of another name with the same address: kept.
        let later = t0 + Duration::from_secs(60);
        let after = keep_name(Some(&gitlab), "wpad.gitlab.initech.example", later);
        assert_eq!(after, gitlab);
        // Looked up again: still that name, fresh again.
        let again = keep_name(Some(&after), "gitlab.initech.example", later);
        assert_eq!(again, ("gitlab.initech.example".to_owned(), later));
        // Gone stale: the next name takes it.
        let stale = later + SEEN_NAME_FOR;
        let other = keep_name(Some(&again), "git.initech.example", stale);
        assert_eq!(other, ("git.initech.example".to_owned(), stale));
    }
}
