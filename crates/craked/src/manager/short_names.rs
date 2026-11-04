//! Short names (https://expert/): one label, which the system can't
//! complete without a search domain, and which public DNS doesn't know.
//! craked asks each tunnel that's up whether its DNS knows the name under
//! one of its domains, and asks you which one it means; once you choose,
//! it answers the short name with that tunnel's answer for the full name,
//! routed before the first packet. Only names you chose are completed: a
//! search domain would complete every short name with the company's,
//! `wpad` included (a browser's proxy auto-detection hung on a company's,
//! 2026-10-14), and send every word typed in a browser's address bar to
//! its DNS for good.
//!
//! Where the short name reaches craked: its own DNS server, standing in
//! for the system's (macOS, `sys::dns_server`), and `dns::SHORT_SUFFIX`:
//! the hush link's search domain (Linux, `hush`), each tunnel adapter's
//! suffix (Windows, `sys::windows::resolved`, sent to craked's server),
//! and the network's own suffixes (Windows, `dns::SYSTEM_SUFFIXES`).

use crake_proto::{ShortName, ShortStep};

use super::*;

/// A short name offered isn't offered again for this long ("Not now", or
/// a browser asking for it again and again).
const OFFERED_FOR: Duration = Duration::from_secs(crate::units::SECS_PER_DAY);
/// A short name no tunnel knew isn't looked for again for this long: a
/// tunnel just up may know it a moment later (Windows asks before its
/// adapter is ready; CI, 2026-10-30), and asking costs each tunnel's DNS
/// one question.
const NOT_FOUND_FOR: Duration = Duration::from_secs(10);

impl Manager {
    /// A short name you chose a tunnel's full name for, while that tunnel
    /// is up: its DNS's answer for the full name (routed first), as the
    /// answer to the short name. None: asked as any other name.
    pub async fn short_name_answer(&self, query: &[u8]) -> Option<Vec<u8>> {
        let short = dns::short_question(query)?;
        let (tunnel, device, servers, full) =
            self.routed_tunnels()
                .into_iter()
                .find_map(|(tunnel, device, servers)| {
                    let store = self.hosts.lock().unwrap();
                    let (full, _) = store.tunnels.get(&tunnel)?.short.get(&short.name)?;
                    Some((tunnel.clone(), device, servers, full.clone()))
                })?;
        let reply = ask(&device, &servers, &full, short.qtype).await?;
        let addrs = dns::records(&reply);
        if !addrs.is_empty() {
            let answer = Answer {
                name: full.clone(),
                addrs,
            };
            self.learn_answer(&tunnel, &answer, Why::TunnelDomain).await;
        }
        dns::short_answer(&short, &full, reply)
    }

    /// A short name asked under `dns::SHORT_SUFFIX` (Linux's hush link,
    /// Windows' adapter suffix): the full name you chose, or else offered
    /// and said not to exist, so the system tries its next suffix.
    pub async fn answer_short(self: &Arc<Self>, query: &[u8]) -> Option<Vec<u8>> {
        if let Some(reply) = self.short_name_answer(query).await {
            return Some(reply);
        }
        self.offer_short(query);
        crate::hush::reply_code(query, dns::NXDOMAIN)
    }

    /// A public answer with no address for a short name (Settings: "Ask
    /// every tunnel when a name is unknown"): ask each tunnel that's up, in
    /// the background, whether its DNS knows it under one of its domains.
    pub fn offer_short_name(self: &Arc<Self>, public: &[u8]) {
        if dns::no_address(public) {
            self.offer_short(public);
        }
    }

    /// A short name's question (or answer) nobody else answers (Linux:
    /// asked of the hush link under its search domain): offered as above.
    pub fn offer_short(self: &Arc<Self>, question: &[u8]) {
        if !self.settings.lock().unwrap().ask_unknown {
            return;
        }
        let Some(short) = dns::short_question(question) else {
            return;
        };
        // Chosen already: its tunnel is off, then.
        if self.chosen(&short.name) {
            return;
        }
        // Asked lately, or being asked now: once per name for a while.
        {
            let now = Instant::now();
            let mut offered = self.offered.lock().unwrap();
            offered.retain(|_, until| *until > now);
            if offered.contains_key(&short.name) {
                tracing::debug!("short name {}: asked lately", short.name);
                return;
            }
            offered.insert(short.name.clone(), now + UNKNOWN_FOR);
        }
        let this = Arc::clone(self);
        tokio::spawn(async move { this.look_for_short_name(&short.name).await });
    }

    /// Each tunnel's full name for `short`, the first of its domains its
    /// DNS knows it under; asked which one it means, as "When two tunnels
    /// answer" says.
    async fn look_for_short_name(&self, short: &str) {
        let mut found: Vec<(String, String, Duration)> = Vec::new();
        for (tunnel, device, servers) in self.routed_tunnels() {
            let learning = self.learning(&tunnel);
            let (device, servers) = (&device, &servers);
            let addrs_of = move |name| addresses(device, servers, name);
            // Inside a managed tunnel's scope, and not on the never list.
            let allowed = |full: &str, addrs: &[IpAddr]| {
                addrs
                    .iter()
                    .any(|a| learning.allows(Some(full), *a) && !self.never(Some(full), *a))
            };
            let domains = self.domains(&tunnel);
            if let Some((full, took)) = full_name(short, &domains, addrs_of, allowed).await {
                found.push((tunnel.clone(), full, took));
            }
        }
        let both = self.settings.lock().unwrap().both;
        let found: Vec<(String, String)> = match both {
            crate::settings::Both::Ask => found,
            crate::settings::Both::Faster => {
                found.into_iter().min_by_key(|f| f.2).into_iter().collect()
            }
            crate::settings::Both::First => found.into_iter().take(1).collect(),
        }
        .into_iter()
        .map(|(tunnel, full, _)| (tunnel, full))
        .collect();
        // Chosen while its tunnels were asked: an answer to a question from
        // before craked started (the log keeps it) came first. Asking
        // again would put the question back up (Windows' journey, CI,
        // 2026-11-04).
        if self.chosen(short) {
            tracing::debug!("short name {short}: chosen meanwhile");
            return;
        }
        let Some((tunnel, full)) = found.first().cloned() else {
            tracing::debug!("short name {short}: no tunnel knows it");
            self.offered
                .lock()
                .unwrap()
                .insert(short.to_owned(), Instant::now() + NOT_FOUND_FOR);
            return;
        };
        self.offered
            .lock()
            .unwrap()
            .insert(short.to_owned(), Instant::now() + OFFERED_FOR);
        let label = self.label(&tunnel);
        let (message, reason) = if let [_] = found.as_slice() {
            (
                format!("{label} knows {short} as {full}."),
                format!(
                    "Public DNS doesn\u{2019}t know a name that short. Until you choose, {short} doesn\u{2019}t open."
                ),
            )
        } else {
            let labels: Vec<String> = found.iter().map(|(t, _)| self.label(t)).collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            let fulls: Vec<&str> = found.iter().map(|(_, f)| f.as_str()).collect();
            (
                format!("{} know {short}.", and_list(&labels)),
                format!(
                    "As {}. Until you choose, {short} doesn\u{2019}t open.",
                    and_list(&fulls)
                ),
            )
        };
        self.record_short(
            LogKind::Asked,
            &tunnel,
            &message,
            reason,
            ShortName {
                name: short.to_owned(),
                full: found,
                step: ShortStep::Asked,
            },
        );
    }

    /// Whether a tunnel's full name was chosen for `short`.
    fn chosen(&self, short: &str) -> bool {
        self.hosts
            .lock()
            .unwrap()
            .tunnels
            .values()
            .any(|t| t.short.contains_key(short))
    }

    /// "Use ACME's": `short` is `tunnel`'s `full` from now on, until you
    /// forget it; another tunnel's full name for it goes.
    pub async fn use_short_name(&self, tunnel: &str, short: &str, full: &str) -> Result<String> {
        let (short, full) = (dns::normalize(short), dns::normalize(full));
        let (_, label, _) = self.entry(tunnel)?;
        if !dns::short_name(&short)
            || !dns::valid_domain(&full)
            || !full.starts_with(&format!("{short}."))
        {
            bail!("{full} isn't {short} with a domain after it.");
        }
        {
            let mut store = self.hosts.lock().unwrap();
            for (name, t) in &mut store.tunnels {
                if name != tunnel {
                    t.short.remove(&short);
                }
            }
            store
                .tunnel(tunnel)
                .short
                .insert(short.clone(), (full.clone(), hosts::now()));
            store.save();
        }
        self.offered.lock().unwrap().remove(&short);
        // The system kept public DNS's "no such name" for it.
        self.resolved.flush().await;
        let msg = format!("{short} is {full} now.");
        self.record_short(
            LogKind::System,
            tunnel,
            &msg,
            format!("You chose {label}\u{2019}s name for it. Crake keeps it until you forget it."),
            ShortName {
                name: short,
                full: vec![(tunnel.to_owned(), full)],
                step: ShortStep::Chosen,
            },
        );
        Ok(msg)
    }

    /// The short names chosen for `tunnel`: (short, full, when chosen).
    pub fn short_names(&self, tunnel: &str) -> Vec<(String, String, u64)> {
        let store = self.hosts.lock().unwrap();
        store.tunnels.get(tunnel).map_or_else(Vec::new, |t| {
            t.short
                .iter()
                .map(|(short, (full, at))| (short.clone(), full.clone(), *at))
                .collect()
        })
    }

    /// Forget a short name chosen for `tunnel` (tunnel detail's Forget, the
    /// panel's Undo); None if it isn't one.
    pub(super) async fn forget_short_name(&self, tunnel: &str, short: &str) -> Option<String> {
        let short = dns::normalize(short);
        let (full, _) = {
            let mut store = self.hosts.lock().unwrap();
            let gone = store.tunnels.get_mut(tunnel)?.short.remove(&short)?;
            store.save();
            gone
        };
        self.resolved.flush().await;
        let msg = format!("Forgot {short}.");
        self.record_short(
            LogKind::System,
            tunnel,
            &msg,
            format!("It was {full}. Typed on its own, it\u{2019}s public DNS\u{2019}s again."),
            ShortName {
                name: short,
                full: vec![(tunnel.to_owned(), full)],
                step: ShortStep::Forgotten,
            },
        );
        Some(msg)
    }
}

/// The first of a tunnel's `domains` its DNS (`addrs_of`) knows `short`
/// under, with an address `allowed` lets through, and how long that DNS
/// took to answer. A domain whose DNS gives a made-up name under it the
/// same address answers every name (a company's gave each name under its
/// GitLab's domain GitLab's address, 2026-11-04, so it seemed to know
/// every short name typed): its answer says nothing about `short`, and
/// the next domain is asked.
async fn full_name<F, Fut>(
    short: &str,
    domains: &[String],
    addrs_of: F,
    allowed: impl Fn(&str, &[IpAddr]) -> bool,
) -> Option<(String, Duration)>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Vec<IpAddr>>,
{
    for domain in domains {
        let full = format!("{short}.{domain}");
        let started = Instant::now();
        let addrs = addrs_of(full.clone()).await;
        let took = started.elapsed();
        tracing::debug!("short name {short}: {full} is {addrs:?}");
        if addrs.is_empty() || !allowed(&full, &addrs) {
            continue;
        }
        let made_up = addrs_of(format!("{}.{domain}", made_up_label())).await;
        if made_up.iter().any(|a| addrs.contains(a)) {
            tracing::debug!("short name {short}: {domain}'s DNS answers any name under it");
            continue;
        }
        return Some((full, took));
    }
    None
}

/// A label no network names a host by.
fn made_up_label() -> String {
    use std::hash::BuildHasher;
    let n = std::collections::hash_map::RandomState::new().hash_one(Instant::now());
    format!("crake-made-up-{n:x}")
}

/// A name's addresses by a tunnel's DNS: IPv4's, or else IPv6's.
async fn addresses(device: &str, servers: &[Ipv4Addr], name: String) -> Vec<IpAddr> {
    for qtype in [RecordType::A, RecordType::AAAA] {
        if let Some(reply) = ask(device, servers, &name, qtype).await {
            let addrs = dns::records(&reply);
            if !addrs.is_empty() {
                return addrs;
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOMAINS: [&str; 3] = [
        "gitlab.initech.example",
        "acme.example",
        "corp.acme.example",
    ];

    fn domains() -> Vec<String> {
        DOMAINS.map(String::from).to_vec()
    }

    fn at(a: &str) -> Vec<IpAddr> {
        vec![a.parse().unwrap()]
    }

    /// Initech's DNS answers every name under its GitLab's domain; ACME's
    /// knows expert.corp.acme.example.
    async fn dns(name: String) -> Vec<IpAddr> {
        if name.ends_with(".gitlab.initech.example") {
            at("10.30.1.4")
        } else if name == "expert.corp.acme.example" {
            at("10.40.1.20")
        } else {
            Vec::new()
        }
    }

    fn anything(_: &str, _: &[IpAddr]) -> bool {
        true
    }

    async fn full(domains: &[String], allowed: impl Fn(&str, &[IpAddr]) -> bool) -> Option<String> {
        full_name("expert", domains, dns, allowed)
            .await
            .map(|(f, _)| f)
    }

    #[tokio::test]
    async fn a_domain_that_answers_every_name_doesnt_know_a_short_one() {
        let all = domains();
        assert_eq!(
            full(&all, anything).await.as_deref(),
            Some("expert.corp.acme.example")
        );
        assert_eq!(full(&all[..1], anything).await, None);
        // Outside what's allowed: not a name to offer.
        assert_eq!(
            full(&all, |f: &str, _: &[IpAddr]| f.contains("gitlab")).await,
            None
        );
    }

    #[tokio::test]
    async fn a_name_of_its_own_under_such_a_domain_is_known() {
        let own = |name: String| async move {
            if name == "expert.gitlab.initech.example" {
                at("10.30.1.9")
            } else {
                dns(name).await
            }
        };
        let found = full_name("expert", &domains(), own, anything).await;
        assert_eq!(
            found.map(|(f, _)| f).as_deref(),
            Some("expert.gitlab.initech.example")
        );
    }

    #[test]
    fn a_made_up_label_is_new_each_time_and_a_short_name() {
        let (a, b) = (made_up_label(), made_up_label());
        assert_ne!(a, b);
        assert!(dns::short_name(&a), "{a}");
    }
}
