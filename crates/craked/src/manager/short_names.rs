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
//! suffix (Windows, `sys::windows::resolved`, sent to craked's server).

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
        if self
            .hosts
            .lock()
            .unwrap()
            .tunnels
            .values()
            .any(|t| t.short.contains_key(&short.name))
        {
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
            for domain in self.domains(&tunnel) {
                let full = format!("{short}.{domain}");
                let started = Instant::now();
                let mut addrs = Vec::new();
                for qtype in [RecordType::A, RecordType::AAAA] {
                    if addrs.is_empty()
                        && let Some(reply) = ask(&device, &servers, &full, qtype).await
                    {
                        addrs = dns::records(&reply);
                    }
                }
                tracing::debug!("short name {short}: {tunnel}'s DNS says {full} is {addrs:?}");
                // Inside a managed tunnel's scope, and not on the never list.
                let allowed = addrs
                    .iter()
                    .any(|a| learning.allows(Some(&full), *a) && !self.never(Some(&full), *a));
                if allowed {
                    found.push((tunnel.clone(), full, started.elapsed()));
                    break;
                }
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
                    "Public DNS doesn\u{2019}t know a name that short. Crake can use {label}\u{2019}s from now on."
                ),
            )
        } else {
            let labels: Vec<String> = found.iter().map(|(t, _)| self.label(t)).collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            let fulls: Vec<&str> = found.iter().map(|(_, f)| f.as_str()).collect();
            (
                format!("{} know {short}.", and_list(&labels)),
                format!(
                    "As {}. Public DNS doesn\u{2019}t know a name that short.",
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
