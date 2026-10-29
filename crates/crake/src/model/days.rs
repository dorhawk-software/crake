//! The log by day: what's shown, its rows, and today's summary, events and
//! traffic.

use super::*;

impl Model {
    pub fn display_of(&self, tunnel: Option<&str>) -> String {
        let Some(t) = tunnel else {
            return "System".into();
        };
        self.tunnels
            .iter()
            .find(|x| x.name == t)
            .map(|x| x.display.clone())
            .unwrap_or_else(|| t.to_owned())
    }

    /// The log by day, newest first, filtered: by kind, by tunnel (its
    /// name), by search words (in the sentence, its reason, the addresses,
    /// the tunnel).
    pub fn log_days(&self, show: Show, tunnel: Option<&str>, search: &str) -> Vec<Day> {
        let tz = jiff::tz::TimeZone::system();
        let today = jiff::Zoned::now().date();
        let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
        let mut days: Vec<(jiff::civil::Date, Day)> = Vec::new();
        for e in self.log.iter().rev() {
            if !show.keeps(e.kind) || tunnel.is_some_and(|t| e.tunnel.as_deref() != Some(t)) {
                continue;
            }
            let Ok(ts) = e.ts.parse::<jiff::Timestamp>() else {
                continue;
            };
            let at = ts.to_zoned(tz.clone());
            let badge = match e.kind {
                LogKind::LeftAlone => "\u{2014}".to_owned(),
                LogKind::System => "System".to_owned(),
                _ => self.display_of(e.tunnel.as_deref()),
            };
            let why = e.reason.clone().unwrap_or_default();
            let haystack = format!(
                "{} {} {} {} {}",
                e.message,
                why,
                e.hosts.join(" "),
                badge,
                self.display_of(e.tunnel.as_deref())
            )
            .to_lowercase();
            if !words.iter().all(|w| haystack.contains(w)) {
                continue;
            }
            let row = Row {
                time: at.strftime("%H:%M").to_string(),
                kind: e.kind,
                text: e.message.clone(),
                why,
                badge,
                show_in: match (e.kind, e.tunnel.as_ref(), e.hosts.first()) {
                    (LogKind::Routed, Some(t), Some(h)) => Some((t.clone(), h.clone())),
                    _ => None,
                },
            };
            let date = at.date();
            match days.last_mut() {
                Some((d, day)) if *d == date => day.rows.push(row),
                _ => {
                    let title = if date == today {
                        "Today".to_owned()
                    } else if today.yesterday().ok() == Some(date) {
                        "Yesterday".to_owned()
                    } else {
                        date.strftime("%A %-d %B").to_string()
                    };
                    days.push((
                        date,
                        Day {
                            title,
                            rows: vec![row],
                        },
                    ));
                }
            }
        }
        days.into_iter().map(|(_, d)| d).collect()
    }

    /// "7 hosts routed, 1 left alone, 1 undone" for today; nothing when
    /// there's nothing to count (the empty day says so once).
    pub fn today_summary(&self) -> String {
        let today = jiff::Zoned::now().date();
        let tz = jiff::tz::TimeZone::system();
        let (mut routed, mut left, mut undone) = (0, 0, 0);
        for e in &self.log {
            let Ok(ts) = e.ts.parse::<jiff::Timestamp>() else {
                continue;
            };
            if ts.to_zoned(tz.clone()).date() != today {
                continue;
            }
            match e.kind {
                LogKind::Routed => routed += 1,
                LogKind::LeftAlone => left += 1,
                LogKind::Undone => undone += 1,
                LogKind::System | LogKind::Asked => {}
            }
        }
        let hosts = if routed == 1 { "host" } else { "hosts" };
        if routed + left + undone == 0 {
            return String::new();
        }
        format!("{routed} {hosts} routed, {left} left alone, {undone} undone")
    }

    /// Today's routed and left-alone decisions: (minutes since midnight, entry),
    /// oldest first.
    pub fn today_events(&self) -> Vec<(f32, LogEntry)> {
        let today = jiff::Zoned::now().date();
        let tz = jiff::tz::TimeZone::system();
        self.log
            .iter()
            .filter(|e| matches!(e.kind, LogKind::Routed | LogKind::LeftAlone))
            .filter_map(|e| {
                let at = e.ts.parse::<jiff::Timestamp>().ok()?.to_zoned(tz.clone());
                (at.date() == today).then(|| {
                    (
                        at.hour() as f32 * crate::units::MINUTES_PER_HOUR_F + at.minute() as f32,
                        e.clone(),
                    )
                })
            })
            .collect()
    }

    /// Today's traffic per minute: (minutes since midnight, bytes).
    pub fn today_traffic(&self) -> Vec<(f32, u64)> {
        let midnight = jiff::Zoned::now()
            .start_of_day()
            .map(|z| z.timestamp().as_second() as u64 / crate::units::SECS_PER_MINUTE)
            .unwrap_or(0);
        self.days
            .iter()
            .filter(|(m, _)| *m >= midnight)
            .map(|(m, b)| ((m - midnight) as f32, *b))
            .collect()
    }
}

pub(super) fn local_time(ts: &str) -> String {
    ts.parse::<jiff::Timestamp>()
        .map(|t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

/// What the log window shows (DESIGN.md: Log). One filter at a time per kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Show {
    #[default]
    Everything,
    Routed,
    LeftAlone,
    Undone,
}

/// One row of the log.
#[derive(Clone, Debug)]
pub struct Row {
    /// "13:21", local time.
    pub time: String,
    pub kind: LogKind,
    pub text: String,
    pub why: String,
    /// The tunnel's name; "—" when it was left alone, "System" for system events.
    pub badge: String,
    /// A routed row's tunnel (its name) and host: "Show in ACME" opens
    /// tunnel detail there.
    pub show_in: Option<(String, String)>,
}

/// The log's rows for one day, newest first.
#[derive(Clone, Debug)]
pub struct Day {
    /// "Today", "Yesterday", or "Monday 7 October".
    pub title: String,
    pub rows: Vec<Row>,
}
