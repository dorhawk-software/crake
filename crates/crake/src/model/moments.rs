//! The panel's moments (design/screens/tray-panel.html?moment=…): Crake's
//! first route, and a question still open about a route.

use super::*;

/// What the panel asks about a route when another tunnel answered for its
/// host too: (that tunnel's name, its display name), and the key that
/// keeps the first tunnel ("Keep ACME").
#[derive(Clone, Debug, PartialEq)]
pub struct AlsoAnswered {
    pub others: Vec<(String, String)>,
    pub keep_key: String,
}

impl Model {
    /// Crake's first route, while it's the only one the log knows of
    /// (pinning is the user's, not Crake's).
    pub fn first_route(&self) -> Option<&LogEntry> {
        let mut routed = self
            .log
            .iter()
            .filter(|e| e.kind == LogKind::Routed && !e.message.starts_with("Pinned"));
        let first = routed.next()?;
        routed.next().is_none().then_some(first)
    }

    /// The other tunnels that answered for a routed host too (craked's
    /// "… answers for it too.", with "When two tunnels answer: Ask me"),
    /// until the host moves or goes direct.
    pub fn also_answered(&self, routed: &LogEntry) -> Option<AlsoAnswered> {
        let at = |e: &LogEntry| e.ts.parse::<jiff::Timestamp>().ok();
        let shares = |e: &LogEntry| e.hosts.iter().any(|h| routed.hosts.contains(h));
        let asked = self.log.iter().find(|e| {
            e.kind == LogKind::System
                && !e.others.is_empty()
                && e.tunnel == routed.tunnel
                && shares(e)
                && at(e) >= at(routed)
        })?;
        // Moved ("Use Nordbank"), undone or routed again since: answered.
        let since = self.log.iter().any(|e| {
            matches!(e.kind, LogKind::Routed | LogKind::Undone) && shares(e) && at(e) > at(routed)
        });
        let others: Vec<(String, String)> = asked
            .others
            .iter()
            .filter_map(|o| self.tunnels.iter().find(|t| &t.name == o))
            .map(|t| (t.name.clone(), t.display.clone()))
            .collect();
        (!since && !others.is_empty()).then(|| AlsoAnswered {
            others,
            keep_key: format!("keep:{}", asked.ts),
        })
    }
}

/// The demo at one of the panel's moments, as tray-panel.html?moment=…
/// (CRAKE_DEMO=first, back or ask).
pub fn demo_moment(mut m: Model, moment: &str) -> Model {
    let routed = |e: &LogEntry| e.message == "Sent git.acme.internal through ACME.";
    match moment {
        // Crake's first route: only it and the host left alone before it.
        "first" => {
            let keep = |e: &LogEntry| routed(e) || e.message == "Left 10.62.0.77 alone.";
            m.log.retain(|e| e.kind == LogKind::System || keep(e));
            for t in &mut m.tunnels {
                match t.name.as_str() {
                    "acme" => t.hosts = 1,
                    "nord" => (t.state, t.hosts) = ("off".into(), 0),
                    _ => {}
                }
            }
        }
        // ACME lost its gateway an hour after it came up.
        "back" => {
            if let Some(acme) = m.tunnels.iter_mut().find(|t| t.name == "acme") {
                acme.state = "connecting".into();
                acme.since = jiff::Timestamp::now().as_second().max(0).unsigned_abs()
                    - crate::units::SECS_PER_HOUR;
            }
        }
        // Nordbank answered for git.acme.internal too ("Ask me").
        "ask" => {
            if let Some(mut asked) = m.log.iter().find(|e| routed(e)).cloned() {
                asked.kind = LogKind::System;
                asked.message = "Nordbank answers for git.acme.internal too.".into();
                asked.reason =
                    Some("Crake sent it through ACME, first in order. You can move it.".into());
                asked.others = vec!["nord".into()];
                m.log.push(asked);
            }
        }
        _ => {}
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Tunnel;

    fn entry(ts: &str, kind: LogKind, tunnel: &str, msg: &str, host: &str) -> LogEntry {
        LogEntry {
            ts: ts.into(),
            kind,
            tunnel: Some(tunnel.into()),
            message: msg.into(),
            reason: None,
            hosts: vec![host.into()],
            others: Vec::new(),
        }
    }

    fn model(log: Vec<LogEntry>) -> Model {
        let tunnel = |name: &str, display: &str| Tunnel {
            name: name.into(),
            display: display.into(),
            ..Default::default()
        };
        Model {
            tunnels: vec![tunnel("acme", "ACME"), tunnel("nord", "Nordbank")],
            log,
            ..Default::default()
        }
    }

    const ROUTED: &str = "Sent git.acme.internal through ACME.";

    #[test]
    fn the_first_route_is_the_only_one_crake_made() {
        let pinned = entry(
            "2026-10-29T09:00:00Z",
            LogKind::Routed,
            "acme",
            "Pinned 10.40.0.0/16 to ACME.",
            "10.40.0.0/16",
        );
        let first = entry(
            "2026-10-29T10:00:00Z",
            LogKind::Routed,
            "acme",
            ROUTED,
            "10.40.1.2",
        );
        assert!(model(vec![pinned.clone()]).first_route().is_none());
        let m = model(vec![pinned.clone(), first.clone()]);
        assert_eq!(
            m.first_route().map(|e| e.ts.as_str()),
            Some(first.ts.as_str())
        );
        let second = entry(
            "2026-10-29T11:00:00Z",
            LogKind::Routed,
            "nord",
            "Sent b.",
            "172.22.0.9",
        );
        assert!(model(vec![pinned, first, second]).first_route().is_none());
    }

    #[test]
    fn a_question_stays_until_the_host_moves() {
        let routed = entry(
            "2026-10-29T10:00:00Z",
            LogKind::Routed,
            "acme",
            ROUTED,
            "10.40.1.2",
        );
        let asked = LogEntry {
            others: vec!["nord".into()],
            ..entry(
                "2026-10-29T10:00:01Z",
                LogKind::System,
                "acme",
                "Nordbank answers for git.acme.internal too.",
                "10.40.1.2",
            )
        };
        assert_eq!(model(vec![routed.clone()]).also_answered(&routed), None);
        let m = model(vec![routed.clone(), asked.clone()]);
        assert_eq!(
            m.also_answered(&routed),
            Some(AlsoAnswered {
                others: vec![("nord".into(), "Nordbank".into())],
                keep_key: "keep:2026-10-29T10:00:01Z".into(),
            })
        );
        let moved = entry(
            "2026-10-29T10:05:00Z",
            LogKind::Routed,
            "nord",
            "Sent git.acme.internal through Nordbank instead of ACME.",
            "10.40.1.2",
        );
        assert_eq!(
            model(vec![routed.clone(), asked, moved]).also_answered(&routed),
            None
        );
    }
}
