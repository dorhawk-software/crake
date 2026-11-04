//! A question that waits for you (design/proposals/waiting-for-you.html,
//! chosen 2026-11-04): a short name a tunnel's DNS knows ("ACME knows
//! expert as …"), asked and not answered. Until you choose, the name
//! doesn't open, so for an hour it's a notice that needs you at the top
//! of the panel, with the amber triangle on the bird, and the log's row
//! keeps its Use actions for the rest of the day. Read from the log,
//! which craked keeps on disk: a restart neither loses a question nor
//! brings an answered one back. Until 0.18.1 it was only a line in What
//! Crake did, which three newer lines pushed out of the panel.

use std::time::Duration;

use crake_proto::{LogEntry, LogKind, ShortStep};

use super::Model;

/// How long a question is a notice (and the bird's triangle): the person
/// who typed the name is waiting now or not at all, and a browser looks
/// up a word typed as a search too.
pub const SHOWN_FOR: Duration = Duration::from_secs(crate::units::SECS_PER_HOUR);
/// How long the log's row can still answer it: craked doesn't ask about
/// the name again for a day (`OFFERED_FOR`).
pub const ANSWERABLE_FOR: Duration = Duration::from_secs(crate::units::SECS_PER_DAY);

/// A short name's question that can still be answered.
#[derive(Clone, Debug)]
pub struct Waiting {
    pub entry: LogEntry,
    /// The short name ("expert").
    pub name: String,
    /// Each tunnel still up that knows it: (tunnel, its full name), in
    /// try order.
    pub choices: Vec<(String, String)>,
    /// Among the notices dismissed: Not now.
    pub key: String,
}

impl Model {
    /// The questions shown as notices, newest first: asked within the
    /// hour, not answered, not put off with Not now.
    pub fn waiting(&self) -> Vec<Waiting> {
        self.questions(jiff::Timestamp::now(), SHOWN_FOR)
            .into_iter()
            .filter(|w| !self.not_now.contains(&w.key))
            .collect()
    }

    /// The tray menu's words for a question waiting: " · a question about
    /// expert" (nothing when none waits).
    pub(super) fn question_line(&self) -> String {
        self.waiting()
            .first()
            .map(|w| format!(" \u{b7} a question about {}", w.name))
            .unwrap_or_default()
    }

    /// `e`'s question, while the log's row can still answer it.
    pub fn answerable(&self, e: &LogEntry) -> Option<Waiting> {
        self.questions(jiff::Timestamp::now(), ANSWERABLE_FOR)
            .into_iter()
            .find(|w| w.entry.ts == e.ts && w.name == question_name(e).unwrap_or_default())
    }

    /// The short names' questions asked within `within` of `now` and not
    /// answered since (a later word on the same name: chosen, forgotten
    /// or asked again), with a tunnel still up to answer them; newest first.
    pub(super) fn questions(&self, now: jiff::Timestamp, within: Duration) -> Vec<Waiting> {
        let since = now.checked_sub(within).unwrap_or(now);
        let mut out = Vec::new();
        for (i, e) in self.log.iter().enumerate().rev() {
            let Some(name) = question_name(e) else {
                continue;
            };
            if e.ts.parse::<jiff::Timestamp>().is_ok_and(|at| at < since) {
                break;
            }
            let answered = self.log[i + 1..]
                .iter()
                .any(|l| l.short.as_ref().is_some_and(|s| s.name == name));
            let up = |tunnel: &str| {
                self.tunnels
                    .iter()
                    .any(|t| t.name == tunnel && t.state == "connected")
            };
            let choices: Vec<(String, String)> = e
                .short
                .iter()
                .flat_map(|s| s.full.iter())
                .filter(|(tunnel, _)| up(tunnel))
                .cloned()
                .collect();
            if answered || choices.is_empty() {
                continue;
            }
            out.push(Waiting {
                entry: e.clone(),
                name: name.to_owned(),
                choices,
                key: key(e),
            });
        }
        out
    }
}

/// The short name an entry asks about, when it's a question.
fn question_name(e: &LogEntry) -> Option<&str> {
    let s = e.short.as_ref()?;
    (e.kind == LogKind::Asked && s.step == ShortStep::Asked).then_some(s.name.as_str())
}

/// What a question's key among the notices dismissed starts with.
const KEY: &str = "short:";

/// A question's key among the notices dismissed ("Not now").
pub fn key(e: &LogEntry) -> String {
    format!("{KEY}{}", e.ts)
}

/// The questions' keys among the notices dismissed.
pub fn not_now(
    dismissed: &std::collections::BTreeSet<String>,
) -> std::collections::BTreeSet<String> {
    dismissed
        .iter()
        .filter(|k| k.starts_with(KEY))
        .cloned()
        .collect()
}

/// "ACME's", "ACME Holdings'": a tunnel's name as the owner of its full
/// name, on a Use button.
pub fn possessive(name: &str) -> String {
    if name.ends_with('s') {
        format!("{name}\u{2019}")
    } else {
        format!("{name}\u{2019}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crake_proto::ShortName;

    fn tunnel(name: &str, state: &str) -> crate::model::Tunnel {
        crate::model::Tunnel {
            name: name.into(),
            display: name.to_uppercase(),
            state: state.into(),
            ..Default::default()
        }
    }

    fn asked(ts: &str, name: &str, full: &[(&str, &str)]) -> LogEntry {
        LogEntry {
            ts: ts.into(),
            kind: LogKind::Asked,
            tunnel: Some(full[0].0.into()),
            message: format!("ACME and Nordbank know {name}."),
            reason: None,
            hosts: Vec::new(),
            others: Vec::new(),
            short: Some(ShortName {
                name: name.into(),
                full: full
                    .iter()
                    .map(|(t, f)| ((*t).to_owned(), (*f).to_owned()))
                    .collect(),
                step: ShortStep::Asked,
            }),
        }
    }

    fn chosen(ts: &str, name: &str) -> LogEntry {
        let mut e = asked(ts, name, &[("acme", "expert.corp.acme.internal")]);
        e.kind = LogKind::System;
        e.short.as_mut().unwrap().step = ShortStep::Chosen;
        e
    }

    fn model() -> Model {
        Model {
            tunnels: vec![tunnel("acme", "connected"), tunnel("nord", "connected")],
            ..Default::default()
        }
    }

    const BOTH: [(&str, &str); 2] = [
        ("acme", "expert.corp.acme.internal"),
        ("nord", "expert.nordbank.internal"),
    ];

    fn at(ts: &str) -> jiff::Timestamp {
        ts.parse().unwrap()
    }

    #[test]
    fn a_question_waits_an_hour_as_a_notice_and_a_day_in_the_log() {
        let mut m = model();
        m.log = vec![asked("2026-11-04T09:42:00Z", "expert", &BOTH)];
        let names = |m: &Model, now, within| -> Vec<String> {
            m.questions(at(now), within)
                .into_iter()
                .map(|w| w.name)
                .collect()
        };
        assert_eq!(names(&m, "2026-11-04T10:02:00Z", SHOWN_FOR), ["expert"]);
        assert!(names(&m, "2026-11-04T10:43:00Z", SHOWN_FOR).is_empty());
        assert_eq!(
            names(&m, "2026-11-04T10:43:00Z", ANSWERABLE_FOR),
            ["expert"]
        );
        assert!(names(&m, "2026-11-05T09:43:00Z", ANSWERABLE_FOR).is_empty());
    }

    #[test]
    fn an_answer_ends_it_and_a_tunnel_gone_off_takes_its_choice() {
        let now = "2026-11-04T10:00:00Z";
        let mut m = model();
        m.log = vec![asked("2026-11-04T09:42:00Z", "expert", &BOTH)];
        m.tunnels[1].state = "off".into();
        let w = m.questions(at(now), SHOWN_FOR);
        assert_eq!(
            w[0].choices,
            [BOTH[0]].map(|(t, f)| (t.to_owned(), f.to_owned()))
        );
        m.tunnels[0].state = "off".into();
        assert!(m.questions(at(now), SHOWN_FOR).is_empty(), "no tunnel left");
        m.tunnels[0].state = "connected".into();
        m.log.push(chosen("2026-11-04T09:43:00Z", "expert"));
        assert!(m.questions(at(now), SHOWN_FOR).is_empty(), "answered");
    }

    #[test]
    fn not_now_puts_the_notice_off_but_the_log_can_still_answer() {
        let mut m = model();
        let e = asked(&jiff::Timestamp::now().to_string(), "expert", &BOTH);
        m.log = vec![e.clone()];
        assert_eq!(m.waiting().len(), 1);
        m.not_now.insert(key(&e));
        assert!(m.waiting().is_empty());
        assert_eq!(m.answerable(&e).map(|w| w.name).as_deref(), Some("expert"));
    }

    #[test]
    fn several_are_newest_first() {
        let mut m = model();
        m.log = vec![
            asked("2026-11-04T09:40:00Z", "expert", &BOTH),
            asked("2026-11-04T09:50:00Z", "wiki", &BOTH),
        ];
        let w = m.questions(at("2026-11-04T10:00:00Z"), SHOWN_FOR);
        assert_eq!(
            w.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
            ["wiki", "expert"]
        );
    }

    #[test]
    fn a_name_ending_in_s_takes_an_apostrophe_alone() {
        assert_eq!(possessive("ACME"), "ACME\u{2019}s");
        assert_eq!(possessive("ACME Holdings"), "ACME Holdings\u{2019}");
    }
}
