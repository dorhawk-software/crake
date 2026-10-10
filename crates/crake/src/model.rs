//! What the UI knows: craked's tunnels and log, and what follows from them
//! (the tray's shape, which decisions can still be undone, the helpdesk text).

use std::time::{Duration, Instant};

use crake_proto::{LogEntry, LogKind};

use crate::client::{Tunnel, Update};
use crate::tray::{MenuTunnel, Shape};

/// How long the tray shows the "routed" shape after a decision.
const ROUTED_SHAPE_FOR: Duration = Duration::from_secs(4);
/// How many log entries are kept in memory.
const KEEP: usize = 500;

#[derive(Clone, Default)]
pub struct Model {
    /// `None` until the first attempt to reach craked has settled.
    pub reachable: Option<bool>,
    pub tunnels: Vec<Tunnel>,
    /// Oldest first.
    pub log: Vec<LogEntry>,
    pub routed_at: Option<Instant>,
    /// craked's sentence for a command that failed, and when.
    pub failed: Option<(String, Instant)>,
    pub copied_at: Option<Instant>,
    /// Direct traffic per minute: (minute since the epoch, bytes).
    pub traffic: Vec<(u64, u64)>,
    /// All traffic per minute, two days: (minute since the epoch, bytes).
    pub days: Vec<(u64, u64)>,
    /// craked's settings, each in its text form (empty until known).
    pub settings: std::collections::HashMap<String, String>,
    /// Tunnels being signed in to in this app (craked doesn't know yet):
    /// shown as connecting, in the tray and the panel.
    pub signing_in: std::collections::HashSet<String>,
    /// Those whose state says "connecting" only because of that.
    shown_connecting: std::collections::HashSet<String>,
    /// Tunnel detail: each tunnel's hosts, as craked last listed them.
    pub host_rows: std::collections::HashMap<String, Vec<HostRow>>,
    /// Tunnel detail: bytes through each tunnel per minute, two days.
    pub tunnel_traffic: std::collections::HashMap<String, Vec<(u64, u64)>>,
}

/// One of a tunnel's hosts (craked's HostDetails).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostRow {
    /// An address, or a network (a.b.c.d/len) when pinned.
    pub target: String,
    /// The name it was learned under.
    pub name: Option<String>,
    /// pinned | learned | direct
    pub kind: String,
    /// When it was learned (seconds since the epoch) and how: probe, dns,
    /// inside, split, you.
    pub learned: Option<(u64, String)>,
    /// Seconds since it last carried traffic.
    pub used_ago: Option<u64>,
}

impl HostRow {
    pub fn from_map(m: &std::collections::HashMap<String, String>) -> Self {
        let num = |k: &str| m.get(k).and_then(|v| v.parse::<u64>().ok());
        Self {
            target: m.get("target").cloned().unwrap_or_default(),
            name: m.get("name").cloned().filter(|n| !n.is_empty()),
            kind: m.get("kind").cloned().unwrap_or_default(),
            learned: num("learned_at").zip(m.get("how").cloned()),
            used_ago: num("used_ago"),
        }
    }
}

/// What a decision row's button does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Undo,
    Redo,
    /// "Ask me first": send the host through the tunnel.
    Send,
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub entry: LogEntry,
    /// "13:21", local time.
    pub time: String,
    pub action: Option<Action>,
}

impl Model {
    /// A sign-in in this app started or ended. `handed_over`: the session
    /// went to craked, whose state takes over (no flicker back to off).
    pub fn signing_in(&mut self, tunnel: &str, on: bool, handed_over: bool) {
        if on {
            self.signing_in.insert(tunnel.to_owned());
            self.show_signing_in();
            return;
        }
        self.signing_in.remove(tunnel);
        if self.shown_connecting.remove(tunnel)
            && !handed_over
            && let Some(t) = self.tunnels.iter_mut().find(|t| t.name == tunnel)
        {
            t.state = "off".into();
        }
    }

    fn show_signing_in(&mut self) {
        for t in &mut self.tunnels {
            if self.signing_in.contains(&t.name)
                && !matches!(t.state.as_str(), "connected" | "connecting" | "trouble")
            {
                t.state = "connecting".into();
                self.shown_connecting.insert(t.name.clone());
            }
        }
    }

    /// Apply one update; returns a newly written entry, if that's what it was.
    pub fn apply(&mut self, update: Update) -> Option<LogEntry> {
        match update {
            Update::Reachable(r) => {
                self.reachable = Some(r);
                if !r {
                    self.tunnels.clear();
                }
            }
            Update::Tunnels(t) => {
                self.tunnels = t;
                self.shown_connecting.clear();
                self.show_signing_in();
            }
            Update::Log(log) => self.log = log,
            Update::Appended(entry) => {
                if entry.kind == LogKind::Routed {
                    self.routed_at = Some(Instant::now());
                }
                self.log.push(entry.clone());
                if self.log.len() > KEEP {
                    self.log.drain(..self.log.len() - KEEP);
                }
                return Some(entry);
            }
            Update::Failed(text) => self.failed = Some((text, Instant::now())),
            Update::Hosts(tunnel, rows) => {
                self.host_rows
                    .insert(tunnel, rows.iter().map(HostRow::from_map).collect());
            }
            Update::TunnelTraffic(tunnel, minutes) => {
                self.tunnel_traffic.insert(tunnel, minutes);
            }
            // Handled by the app (it ends the session).
            Update::AttachRefused(_) => {}
            Update::Traffic(t) => self.traffic = t,
            Update::Days(d) => self.days = d,
            Update::Settings(s) => self.settings = s,
            Update::Added(_) => {}
        }
        None
    }

    /// One of craked's settings, or the default the window shows.
    pub fn setting(&self, key: &str) -> &str {
        self.settings.get(key).map_or_else(
            || match key {
                "probe_wait_secs" => "3",
                "forget_after_days" => "7",
                "both" => "ask",
                "ask_unknown" | "keep_private" => "true",
                "log_days" => "30",
                _ => "",
            },
            String::as_str,
        )
    }

    /// A list setting (one item per line).
    pub fn setting_list(&self, key: &str) -> Vec<String> {
        self.setting(key)
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// The tunnels in try order (Settings): the saved order first, the rest
    /// as configured. Full tunnels aren't tried: everything goes there anyway.
    pub fn try_order(&self) -> Vec<&Tunnel> {
        let order = self.setting_list("order");
        let mut v: Vec<&Tunnel> = self.tunnels.iter().filter(|t| !t.full_tunnel).collect();
        v.sort_by_key(|t| {
            order
                .iter()
                .position(|n| *n == t.name)
                .unwrap_or(usize::MAX)
        });
        v
    }

    pub fn shape(&self) -> Shape {
        let any = |s: &str| self.tunnels.iter().any(|t| t.state == s);
        if self.reachable != Some(true) {
            Shape::Off
        } else if any("trouble") {
            Shape::Trouble
        } else if any("connecting") {
            Shape::Connecting
        } else if self
            .routed_at
            .is_some_and(|t| t.elapsed() < ROUTED_SHAPE_FOR)
        {
            Shape::Routed
        } else if any("connected") {
            Shape::Listening
        } else {
            Shape::Off
        }
    }

    /// The panel header's word for the whole of Crake.
    pub fn status(&self) -> &'static str {
        match (self.reachable, self.shape()) {
            (None, _) => "Starting",
            (Some(false), _) => "Not running",
            (_, Shape::Trouble) => "Trouble",
            (_, Shape::Connecting) => "Connecting",
            (_, Shape::Listening | Shape::Routed) => "Listening",
            (_, Shape::Off) => "Off",
        }
    }

    /// The tray's tooltip and menu line.
    pub fn summary(&self) -> String {
        match self.reachable {
            Some(true) => {
                let on: Vec<&str> = self
                    .tunnels
                    .iter()
                    .filter(|t| t.state == "connected")
                    .map(|t| t.display.as_str())
                    .collect();
                if on.is_empty() {
                    "No tunnel is on".into()
                } else {
                    format!("Listening · {}", on.join(", "))
                }
            }
            Some(false) => "Crake isn't running".into(),
            None => "Crake".into(),
        }
    }

    pub fn menu_tunnels(&self) -> Vec<MenuTunnel> {
        self.tunnels
            .iter()
            .map(|t| MenuTunnel {
                name: t.name.clone(),
                display: t.display.clone(),
                on: matches!(t.state.as_str(), "connected" | "connecting"),
            })
            .collect()
    }

    /// The latest decisions (not system events), newest first, each with
    /// Undo or Redo while it's still the last word on its hosts.
    pub fn decisions(&self, count: usize) -> Vec<Decision> {
        let decisions: Vec<&LogEntry> = self
            .log
            .iter()
            .filter(|e| e.kind != LogKind::System)
            .collect();
        let mut out = Vec::new();
        for (i, e) in decisions.iter().enumerate().rev() {
            if out.len() == count {
                break;
            }
            let later = &decisions[i + 1..];
            let superseded = later.iter().any(|l| {
                l.tunnel == e.tunnel
                    && matches!(l.kind, LogKind::Routed | LogKind::Undone)
                    && l.hosts.iter().any(|h| e.hosts.contains(h))
            });
            let action = match e.kind {
                _ if e.hosts.is_empty() || superseded => None,
                LogKind::Routed if !e.message.starts_with("Pinned") => Some(Action::Undo),
                LogKind::Undone => Some(Action::Redo),
                LogKind::Asked => Some(Action::Send),
                _ => None,
            };
            out.push(Decision {
                entry: (*e).clone(),
                time: local_time(&e.ts),
                action,
            });
        }
        out
    }

    /// The chart's marks for the last hour (DESIGN.md, Charts): routed
    /// decisions as two-stroke ticks, left-alone ones as circles; marks less
    /// than a minute and a half apart merge into one, with a count.
    pub fn marks(&self) -> Vec<Mark> {
        let now = jiff::Timestamp::now();
        let mut events: Vec<(f32, &LogEntry)> = self
            .log
            .iter()
            .filter(|e| matches!(e.kind, LogKind::Routed | LogKind::LeftAlone))
            .filter_map(|e| {
                let t: jiff::Timestamp = e.ts.parse().ok()?;
                let ago = (now.as_second() - t.as_second()) as f32 / 60.;
                (0. ..=60.).contains(&ago).then_some((ago, e))
            })
            .collect();
        // Oldest first.
        events.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut groups: Vec<(f32, bool, Vec<&LogEntry>)> = Vec::new();
        for (ago, e) in events {
            let routed = e.kind == LogKind::Routed;
            match groups.iter_mut().rev().find(|g| g.1 == routed) {
                Some(g) if g.0 - ago <= MERGE_MINUTES => {
                    g.0 = ago; // the mark stands where the latest of them is
                    g.2.push(e);
                }
                _ => groups.push((ago, routed, vec![e])),
            }
        }
        groups
            .into_iter()
            .map(|(ago, routed, entries)| Mark {
                ago,
                routed,
                count: entries.len(),
                ts: entries.last().map(|e| e.ts.clone()).unwrap_or_default(),
                time: entries
                    .last()
                    .map(|e| local_time(&e.ts))
                    .unwrap_or_default(),
                text: mark_text(&entries),
            })
            .collect()
    }

    /// "Copy for helpdesk": a plain-text summary anyone can read.
    /// With `hide_hosts`, host names and addresses become "a host": tunnels
    /// and timing only (Settings, "Hide host names in helpdesk copies").
    pub fn helpdesk(&self, hide_hosts: bool) -> String {
        let say = |t: &str| if hide_hosts { scrub(t) } else { t.to_owned() };
        let now = jiff::Zoned::now();
        let mut s = format!("Crake, {}\n\nTunnels\n", now.strftime("%Y-%m-%d %H:%M %Z"));
        if self.tunnels.is_empty() {
            s += "  (none)\n";
        }
        for t in &self.tunnels {
            s += &format!("  {}: {}", t.display, tunnel_state(t));
            if !t.address.is_empty() && !hide_hosts {
                s += &format!(", address {}", t.address);
            }
            s += "\n";
        }
        s += "\nRecent decisions (newest first)\n";
        let recent = self.decisions(20);
        if recent.is_empty() {
            s += "  (none)\n";
        }
        for d in recent {
            s += &format!("  {}  {}\n", d.time, say(&d.entry.message));
            if let Some(r) = &d.entry.reason {
                s += &format!("         {}\n", say(r));
            }
        }
        s
    }
}

/// Host names and addresses out, everything else kept: "Sent
/// git.acme.internal through ACME." → "Sent a host through ACME."
pub fn scrub(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let start = word
                .find(|c: char| c.is_ascii_alphanumeric())
                .unwrap_or(word.len());
            let end = word
                .rfind(|c: char| c.is_ascii_alphanumeric())
                .map_or(start, |i| i + 1);
            let core = &word[start..end.max(start)];
            let is_ip = core.parse::<std::net::Ipv4Addr>().is_ok()
                || core
                    .split_once('/')
                    .is_some_and(|(a, _)| a.parse::<std::net::Ipv4Addr>().is_ok());
            // A name: dotted labels, the last one letters (not "3.5", not "e.g").
            let labels: Vec<&str> = core.split('.').collect();
            let is_name = labels.len() >= 2
                && labels.iter().all(|l| {
                    !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                })
                && labels
                    .last()
                    .is_some_and(|l| l.len() >= 2 && l.chars().all(|c| c.is_ascii_alphabetic()));
            if is_ip || is_name {
                format!("{}a host{}", &word[..start], &word[end.max(start)..])
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Marks closer than this (minutes) merge into one.
const MERGE_MINUTES: f32 = 1.5;

/// One mark on the Last hour chart.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// Minutes ago (of the latest decision it stands for).
    pub ago: f32,
    /// A two-stroke tick (routed) or a circle (left alone).
    pub routed: bool,
    /// How many decisions it stands for.
    pub count: usize,
    /// The latest of them (RFC 3339): a new one makes the tick grow again.
    pub ts: String,
    /// "13:21", local time.
    pub time: String,
    /// The readout's sentence, in the log's words.
    pub text: String,
}

/// One decision: its own sentence. Several: "Sent git, iam.intranet and
/// wiki through ACME." (the domain they share left off), or "Sent 3 hosts
/// through ACME and Nordbank.", or "Left a, b and c alone."
pub fn mark_text(entries: &[&LogEntry]) -> String {
    if let [one] = entries {
        return one.message.clone();
    }
    let routed = entries[0].kind == LogKind::Routed;
    let parts: Vec<(String, String)> = entries
        .iter()
        .filter_map(|e| {
            let m = e.message.trim_end_matches('.');
            if routed {
                let rest = m.strip_prefix("Sent ")?;
                let (host, label) = rest.rsplit_once(" through ")?;
                Some((host.to_owned(), label.to_owned()))
            } else {
                let host = m.strip_prefix("Left ")?.strip_suffix(" alone")?;
                Some((host.to_owned(), String::new()))
            }
        })
        .collect();
    let mut labels: Vec<&str> = parts.iter().map(|p| p.1.as_str()).collect();
    labels.dedup();
    if parts.len() != entries.len() || (routed && labels.len() > 1) {
        let mut all: Vec<&str> = labels.clone();
        all.sort_unstable();
        all.dedup();
        return if routed {
            format!("Sent {} hosts through {}.", entries.len(), join_and(&all))
        } else {
            format!("Left {} hosts alone.", entries.len())
        };
    }
    let names = shorten(parts.iter().map(|p| p.0.as_str()).collect());
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    if routed {
        format!("Sent {} through {}.", join_and(&names), labels[0])
    } else {
        format!("Left {} alone.", join_and(&names))
    }
}

/// Leaves off the domain all the names share (git.acme.internal, iam.intranet.acme.internal
/// → git, iam.intranet); addresses stay whole.
fn shorten(names: Vec<&str>) -> Vec<String> {
    if names.iter().any(|n| n.parse::<std::net::IpAddr>().is_ok()) {
        return names.into_iter().map(str::to_owned).collect();
    }
    let split: Vec<Vec<&str>> = names.iter().map(|n| n.split('.').collect()).collect();
    let shortest = split.iter().map(Vec::len).min().unwrap_or(0);
    let mut common = 0;
    while common + 1 < shortest {
        let label = split[0][split[0].len() - 1 - common];
        if split.iter().all(|s| s[s.len() - 1 - common] == label) {
            common += 1;
        } else {
            break;
        }
    }
    split
        .iter()
        .map(|s| s[..s.len() - common].join("."))
        .collect()
}

/// "a", "a and b", "a, b and c".
fn join_and(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// "850 KB", "6.7 MB", "1.2 GB".
pub fn human_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.0} KB", (b / 1e3).max(1.))
    }
}

/// "Connected · 4 hosts", "Off", ...
pub fn tunnel_state(t: &Tunnel) -> String {
    match t.state.as_str() {
        "connected" if t.dns_down => "Connected · its DNS doesn\u{2019}t answer".into(),
        "connected" if t.hosts == 1 => "Connected · 1 host".into(),
        "connected" => format!("Connected · {} hosts", t.hosts),
        "connecting" => "Connecting…".into(),
        "trouble" => "Couldn't stay connected".into(),
        _ => "Off".into(),
    }
}

fn local_time(ts: &str) -> String {
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

impl Show {
    pub const ALL: [Show; 4] = [
        Show::Everything,
        Show::Routed,
        Show::LeftAlone,
        Show::Undone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Show::Everything => "Everything",
            Show::Routed => "Routed",
            Show::LeftAlone => "Left alone",
            Show::Undone => "Undone",
        }
    }

    fn keeps(self, kind: LogKind) -> bool {
        match self {
            Show::Everything => true,
            Show::Routed => kind == LogKind::Routed,
            Show::LeftAlone => kind == LogKind::LeftAlone,
            Show::Undone => kind == LogKind::Undone,
        }
    }
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
}

/// The log's rows for one day, newest first.
#[derive(Clone, Debug)]
pub struct Day {
    /// "Today", "Yesterday", or "Monday 7 October".
    pub title: String,
    pub rows: Vec<Row>,
}

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

    /// "7 hosts routed, 1 left alone, 1 undone" for today.
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
                (at.date() == today)
                    .then(|| (at.hour() as f32 * 60. + at.minute() as f32, e.clone()))
            })
            .collect()
    }

    /// Today's traffic per minute: (minutes since midnight, bytes).
    pub fn today_traffic(&self) -> Vec<(f32, u64)> {
        let midnight = jiff::Zoned::now()
            .start_of_day()
            .map(|z| z.timestamp().as_second() as u64 / 60)
            .unwrap_or(0);
        self.days
            .iter()
            .filter(|(m, _)| *m >= midnight)
            .map(|(m, b)| ((m - midnight) as f32, *b))
            .collect()
    }
}

/// The design's sample (design/screens/tray-panel.html), for `CRAKE_DEMO=1`:
/// a panel to compare with the screen, with no craked needed.
pub fn demo() -> Model {
    let now = jiff::Timestamp::now();
    let at = |mins_ago: i64| {
        now.checked_sub(jiff::SignedDuration::from_mins(mins_ago))
            .unwrap_or(now)
            .to_string()
    };
    let entry =
        |mins: i64, kind: LogKind, tunnel: &str, msg: &str, why: &str, hosts: &[&str]| LogEntry {
            ts: at(mins),
            kind,
            tunnel: Some(tunnel.into()),
            message: msg.into(),
            reason: Some(why.into()),
            hosts: hosts.iter().map(|h| h.to_string()).collect(),
            others: Vec::new(),
        };
    let tunnel = |name: &str, display: &str, state: &str, hosts: u32| Tunnel {
        name: name.into(),
        display: display.into(),
        state: state.into(),
        hosts,
        address: String::new(),
        ..Default::default()
    };
    // Steady traffic with two busier stretches, like the design's.
    let minute = now.as_second() as u64 / 60;
    let mut seed = 5u64;
    let traffic = (0..=60u64)
        .map(|i| {
            seed = (seed * 9301 + 49297) % 233_280;
            let busy = (8..22).contains(&i) || (40..52).contains(&i);
            let bytes = 200_000 + seed * if busy { 40 } else { 8 };
            (minute - 60 + i, bytes)
        })
        .collect();
    Model {
        reachable: Some(true),
        tunnels: vec![
            Tunnel {
                managed: true,
                policy: "ACME IT allows learning for *.acme.internal".into(),
                managed_file: "/etc/crake/managed.d/acme.toml".into(),
                learn: vec!["*.acme.internal".into(), "10.40.0.0/16".into()],
                dns_domains: vec!["acme.internal".into()],
                source: "managed".into(),
                organisation: "ACME IT".into(),
                gateway: "vpn.acme.example".into(),
                protocol: "anyconnect".into(),
                ..tunnel("acme", "ACME", "connected", 5)
            },
            Tunnel {
                dns_domains: vec!["nordbank.example".into()],
                source: "app".into(),
                gateway: "vpn.nordbank.example".into(),
                protocol: "gp".into(),
                split_include: vec!["172.22.0.0/16".into()],
                own: [
                    ("display_name", "Nordbank"),
                    ("gateway", "vpn.nordbank.example"),
                    ("protocol", "gp"),
                    ("sign_in", "browser"),
                    ("mode", "learn"),
                    ("forget_after_days", "7"),
                    ("keep_signed_in", "true"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
                ..tunnel("nord", "Nordbank", "connected", 2)
            },
            Tunnel {
                gateway: "vpn.globex.example".into(),
                protocol: "anyconnect".into(),
                auth_args: vec!["--certificate=/home/you/Downloads/globex.p12".into()],
                managed: true,
                policy: "Full tunnel required by Globex IT. Crake won\u{2019}t learn here.".into(),
                full_tunnel: true,
                managed_file: "/etc/crake/managed.d/globex.toml".into(),
                source: "managed".into(),
                organisation: "Globex IT".into(),
                ..tunnel("globex", "Globex", "off", 0)
            },
            Tunnel {
                source: "app".into(),
                gateway: "203.0.113.7".into(),
                protocol: "anyconnect".into(),
                own: [
                    ("display_name", "Home lab"),
                    ("gateway", "203.0.113.7"),
                    ("protocol", "anyconnect"),
                    ("sign_in", "password"),
                    ("user", "me"),
                    ("mode", "learn"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
                ..tunnel("home", "Home lab", "off", 3)
            },
        ],
        log: vec![
            entry(
                140,
                LogKind::Routed,
                "nord",
                "Sent sftp.nordbank.example through Nordbank.",
                "Refused directly. Through Nordbank it answered in 61 ms.",
                &["10.20.0.4"],
            ),
            entry(
                22,
                LogKind::LeftAlone,
                "acme",
                "Left 10.62.0.77 alone.",
                "No answer directly or through ACME. Probably down.",
                &["10.62.0.77"],
            ),
            entry(
                3,
                LogKind::Routed,
                "acme",
                "Sent git.acme.internal through ACME.",
                "It didn\u{2019}t answer directly. Through ACME it answered in 38 ms.",
                &["10.40.1.20"],
            ),
        ]
        .into_iter()
        // CRAKE_DEMO=busy: two more right next to it, to see a merged tick.
        .chain(
            (std::env::var("CRAKE_DEMO").as_deref() == Ok("busy"))
                .then(|| {
                    [
                        entry(
                            3,
                            LogKind::Routed,
                            "acme",
                            "Sent wiki.acme.internal through ACME.",
                            "Only ACME\u{2019}s DNS knows it.",
                            &["10.40.1.21"],
                        ),
                        entry(
                            2,
                            LogKind::Routed,
                            "acme",
                            "Sent jira.acme.internal through ACME.",
                            "Only ACME\u{2019}s DNS knows it.",
                            &["10.40.1.22"],
                        ),
                    ]
                })
                .into_iter()
                .flatten(),
        )
        .collect(),
        traffic,
        // As in settings.html.
        settings: [
            ("probe_wait_secs", "3"),
            ("forget_after_days", "7"),
            ("order", "acme\nnord\nhome"),
            ("both", "ask"),
            ("never", "*.mybank.example\n192.168.1.0/24"),
            ("ask_unknown", "true"),
            ("keep_private", "true"),
            ("log_days", "30"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect(),
        ..Default::default()
    }
}

/// Tunnel detail's sample (design/screens/tunnel-detail.html), for
/// `CRAKE_DEMO=1 CRAKE_SHOW=detail:acme`: ACME connected for 4 h 53 min,
/// its five learned hosts (wiki pinned), its DNS and pushed network, and
/// traffic since the morning, all relative to now.
pub fn demo_detail() -> Model {
    let mut m = demo();
    let now = jiff::Timestamp::now().as_second() as u64;
    if let Some(acme) = m.tunnels.iter_mut().find(|t| t.name == "acme") {
        acme.address = "10.40.12.7".into();
        acme.since = now - (4 * 60 + 53) * 60;
        acme.transport = "DTLS".into();
        acme.dns = vec!["10.40.0.53".into()];
        acme.split_include = vec!["10.40.0.0/16".into()];
        acme.hosts = 6;
    }
    // (target, name, minutes since learned, how, minutes since used, pinned)
    let hosts = [
        ("10.40.3.21", "git.acme.internal", 3, "probe", 0, false),
        ("10.62.0.18", "", 157, "probe", 12, false),
        ("10.40.8.4", "wiki.acme.internal", 250, "dns", 60, true),
        ("10.40.8.9", "jira.acme.internal", 252, "dns", 3, false),
        ("10.40.9.30", "builds.acme.example", 284, "split", 120, false),
    ];
    m.host_rows.insert(
        "acme".into(),
        hosts
            .iter()
            .map(|(target, name, learned, how, used, pinned)| HostRow {
                target: (*target).into(),
                name: (!name.is_empty()).then(|| (*name).into()),
                kind: if *pinned { "pinned" } else { "learned" }.into(),
                learned: Some((now - learned * 60, (*how).into())),
                used_ago: (!pinned).then_some(used * 60),
            })
            .collect(),
    );
    // Busy since about 4 h 54 min ago, as the design's 8:30 to 13:24.
    let minute = now / 60;
    let mut seed = 9u64;
    let traffic = (0..=294u64)
        .map(|i| {
            seed = (seed * 9301 + 49297) % 233_280;
            (minute - 294 + i, 50_000 + seed * 40)
        })
        .collect();
    m.tunnel_traffic.insert("acme".into(), traffic);
    for (target, name, learned, how, ..) in hosts {
        let ts = jiff::Timestamp::from_second((now - learned * 60) as i64)
            .unwrap_or_else(|_| jiff::Timestamp::now())
            .to_string();
        let shown = if name.is_empty() { target } else { name };
        m.log.push(LogEntry {
            ts,
            kind: LogKind::Routed,
            tunnel: Some("acme".into()),
            message: format!("Sent {shown} through ACME."),
            reason: Some(how.into()),
            hosts: vec![target.into()],
            others: Vec::new(),
        });
    }
    m.log.sort_by(|a, b| a.ts.cmp(&b.ts));
    m
}

/// The log design's sample (design/screens/log.html), for `CRAKE_DEMO=log`:
/// the same rows at the same times today and yesterday, and a day of
/// traffic that gets busy at 8:18, as in its chart.
pub fn demo_log() -> Model {
    use LogKind::*;
    let mut m = demo();
    let today = jiff::Zoned::now();
    let at = |days_ago: i64, hm: &str| {
        let (h, mi) = hm.split_once(':').unwrap();
        today
            .checked_sub(jiff::SignedDuration::from_hours(24 * days_ago))
            .ok()
            .and_then(|z| {
                z.with()
                    .hour(h.parse().unwrap())
                    .minute(mi.parse().unwrap())
                    .second(0)
                    .build()
                    .ok()
            })
            .map(|z| z.timestamp().to_string())
            .unwrap_or_default()
    };
    let row =
        |d: i64, hm: &str, kind: LogKind, tunnel: Option<&str>, msg: &str, why: &str| LogEntry {
            ts: at(d, hm),
            kind,
            tunnel: tunnel.map(str::to_owned),
            message: msg.into(),
            reason: Some(why.into()),
            hosts: vec!["10.0.0.1".into()],
            others: Vec::new(),
        };
    let mut log = vec![
        row(
            1,
            "14:03",
            Routed,
            Some("nord"),
            "Sent 172.22.8.4 through Nordbank.",
            "No answer directly. Nordbank answered in 66 ms.",
        ),
        row(
            1,
            "16:20",
            LeftAlone,
            Some("nord"),
            "Left status.nordbank.example alone.",
            "No answer anywhere. It was down for 4 minutes.",
        ),
        row(
            1,
            "17:55",
            System,
            None,
            "Nordbank disconnected.",
            "You switched it off.",
        ),
        row(
            0,
            "08:31",
            System,
            None,
            "ACME and Nordbank connected.",
            "Started with your session.",
        ),
        row(
            0,
            "08:40",
            Routed,
            Some("acme"),
            "Sent builds.acme.example through ACME.",
            "Public name, private address. ACME answered in 47 ms.",
        ),
        row(
            0,
            "09:12",
            Routed,
            Some("acme"),
            "Sent jira.acme.internal through ACME.",
            "Only ACME\u{2019}s DNS knows the name. Routed before the first packet.",
        ),
        row(
            0,
            "09:14",
            Routed,
            Some("acme"),
            "Sent wiki.acme.internal through ACME.",
            "Only ACME\u{2019}s DNS knows the name. Routed before the first packet.",
        ),
        row(
            0,
            "10:12",
            Undone,
            Some("acme"),
            "You sent cdn.acme.example direct again.",
            "It was public after all. Crake won\u{2019}t relearn it on its own.",
        ),
        row(
            0,
            "10:47",
            Routed,
            Some("acme"),
            "Sent 10.62.0.18 through ACME.",
            "No answer directly. ACME answered in 52 ms.",
        ),
        row(
            0,
            "11:04",
            Routed,
            Some("nord"),
            "Sent sftp.nordbank.example through Nordbank.",
            "Refused directly. Nordbank answered in 61 ms.",
        ),
        row(
            0,
            "11:30",
            Routed,
            Some("nord"),
            "Sent 172.22.8.4 through Nordbank.",
            "No answer directly. Nordbank answered in 70 ms.",
        ),
        row(
            0,
            "13:02",
            LeftAlone,
            Some("acme"),
            "Left 10.62.0.77 alone.",
            "No answer directly or through ACME. Probably down.",
        ),
        row(
            0,
            "13:21",
            Routed,
            Some("acme"),
            "Sent git.acme.internal through ACME.",
            "No answer directly. ACME answered in 38 ms.",
        ),
    ];
    log.retain(|e| {
        e.ts.parse::<jiff::Timestamp>()
            .is_ok_and(|t| t <= jiff::Timestamp::now())
    });
    m.log = log;
    let midnight = today
        .start_of_day()
        .map(|z| z.timestamp().as_second() as u64 / 60)
        .unwrap_or(0);
    let now = jiff::Timestamp::now().as_second() as u64 / 60;
    let mut seed = 3u64;
    m.days = (0..)
        .map(|i| midnight + i)
        .take_while(|slot| *slot <= now)
        .map(|slot| {
            seed = (seed * 9301 + 49297) % 233_280;
            let hour = (slot - midnight) as f32 / 60.;
            let busy = hour > 8.3;
            let bytes = if busy {
                2_000_000 + seed * 120
            } else if seed % 10 < 3 {
                300_000 + seed * 4
            } else {
                0
            };
            (slot, bytes)
        })
        .collect();
    m
}

#[cfg(test)]
mod tests {

    #[test]
    fn helpdesk_copies_can_leave_hosts_out() {
        assert_eq!(
            scrub("Sent git.acme.internal through ACME."),
            "Sent a host through ACME."
        );
        assert_eq!(
            scrub("Only ACME\u{2019}s DNS knows it (10.40.1.20). Routed before the first packet."),
            "Only ACME\u{2019}s DNS knows it (a host). Routed before the first packet."
        );
        assert_eq!(
            scrub("It didn't answer directly. Through ACME it answered in 38 ms."),
            "It didn't answer directly. Through ACME it answered in 38 ms."
        );
        assert_eq!(
            scrub("Pinned 10.40.0.0/16 to ACME."),
            "Pinned a host to ACME."
        );
    }

    use super::*;

    fn entry(kind: LogKind, msg: &str, hosts: &[&str]) -> LogEntry {
        LogEntry {
            ts: "2026-10-09T15:00:00Z".into(),
            kind,
            tunnel: Some("acme".into()),
            message: msg.into(),
            reason: Some("It didn't answer directly.".into()),
            hosts: hosts.iter().map(|h| h.to_string()).collect(),
            others: Vec::new(),
        }
    }

    #[test]
    fn signing_in_shows_as_connecting_until_it_ends() {
        let off = || {
            Update::Tunnels(vec![Tunnel {
                name: "initech".into(),
                state: "off".into(),
                ..Default::default()
            }])
        };
        let mut m = Model {
            reachable: Some(true),
            ..Default::default()
        };
        m.apply(off());
        m.signing_in("initech", true, false);
        assert_eq!(m.shape(), Shape::Connecting);
        // craked's list arriving meanwhile doesn't undo it.
        m.apply(off());
        assert_eq!(m.shape(), Shape::Connecting);
        // Cancelled or failed: off again.
        m.signing_in("initech", false, false);
        assert_eq!(m.tunnels[0].state, "off");
        // Handed over: stays connecting until craked says otherwise.
        m.signing_in("initech", true, false);
        m.signing_in("initech", false, true);
        assert_eq!(m.shape(), Shape::Connecting);
        m.apply(Update::Tunnels(vec![Tunnel {
            name: "initech".into(),
            state: "connected".into(),
            ..Default::default()
        }]));
        assert_eq!(m.shape(), Shape::Listening);
    }

    #[test]
    fn only_the_last_word_on_a_host_can_be_undone_or_redone() {
        let m = Model {
            log: vec![
                entry(LogKind::Routed, "Sent a through ACME.", &["10.0.0.1"]),
                entry(LogKind::Routed, "Sent b through ACME.", &["10.0.0.2"]),
                entry(LogKind::Undone, "a goes direct again.", &["10.0.0.1"]),
                entry(LogKind::LeftAlone, "Left c alone.", &["10.0.0.3"]),
                entry(LogKind::System, "ACME is up.", &[]),
            ],
            ..Default::default()
        };
        let d = m.decisions(10);
        assert_eq!(d.len(), 4, "system events aren't decisions");
        assert_eq!(d[0].entry.message, "Left c alone.");
        assert_eq!(d[0].action, None);
        assert_eq!(d[1].action, Some(Action::Redo));
        assert_eq!(d[2].action, Some(Action::Undo), "b is still routed");
        assert_eq!(d[3].action, None, "a was undone since");
    }

    #[test]
    fn marks_close_together_merge_and_read_well() {
        let at = |mins: i64, kind: LogKind, msg: &str| LogEntry {
            ts: jiff::Timestamp::now()
                .checked_sub(jiff::SignedDuration::from_secs(mins * 60))
                .unwrap()
                .to_string(),
            ..entry(kind, msg, &["10.0.0.1"])
        };
        let m = Model {
            log: vec![
                at(30, LogKind::LeftAlone, "Left 10.62.0.77 alone."),
                at(10, LogKind::Routed, "Sent git.acme.internal through ACME."),
                at(9, LogKind::Routed, "Sent iam.intranet.acme.internal through ACME."),
                at(9, LogKind::Routed, "Sent wiki.acme.internal through ACME."),
                at(2, LogKind::Routed, "Sent 10.58.192.12 through ACME."),
            ],
            ..Default::default()
        };
        let marks = m.marks();
        assert_eq!(marks.len(), 3, "{marks:?}");
        assert_eq!((marks[0].routed, marks[0].count), (false, 1));
        assert_eq!(marks[1].count, 3);
        assert_eq!(
            marks[1].text,
            "Sent git, iam.intranet and wiki through ACME."
        );
        assert_eq!(marks[2].text, "Sent 10.58.192.12 through ACME.");
        assert_eq!(human_bytes(6_700_000), "6.7 MB");
        assert_eq!(human_bytes(850_000), "850 KB");
        assert_eq!(join_and(&["a", "b", "c"]), "a, b and c");
    }

    #[test]
    fn the_log_groups_by_day_and_filters() {
        let m = demo_log();
        let all = m.log_days(Show::Everything, None, "");
        // Yesterday's rows always exist (today's depend on the time of day).
        assert_eq!(all.last().unwrap().title, "Yesterday");
        let routed = m.log_days(Show::Routed, None, "");
        assert!(
            routed
                .iter()
                .flat_map(|d| &d.rows)
                .all(|r| r.kind == LogKind::Routed)
        );
        let nord = m.log_days(Show::Everything, Some("nord"), "");
        assert!(
            nord.iter()
                .flat_map(|d| &d.rows)
                .all(|r| r.badge == "Nordbank" || r.badge == "\u{2014}")
        );
        let found = m.log_days(Show::Everything, None, "STATUS nordbank");
        let rows: Vec<&Row> = found.iter().flat_map(|d| &d.rows).collect();
        assert_eq!(rows.len(), 1, "every word must match");
        assert_eq!(rows[0].text, "Left status.nordbank.example alone.");
        let system = all
            .iter()
            .flat_map(|d| &d.rows)
            .find(|r| r.kind == LogKind::System)
            .unwrap();
        assert_eq!(system.badge, "System");
    }

    #[test]
    fn shape_follows_the_tunnels() {
        let mut m = Model::default();
        assert_eq!(m.shape(), Shape::Off);
        m.apply(Update::Reachable(true));
        m.apply(Update::Tunnels(vec![Tunnel {
            state: "connected".into(),
            ..Default::default()
        }]));
        assert_eq!(m.shape(), Shape::Listening);
        m.apply(Update::Appended(entry(
            LogKind::Routed,
            "Sent a through ACME.",
            &["10.0.0.1"],
        )));
        assert_eq!(m.shape(), Shape::Routed);
        m.apply(Update::Tunnels(vec![Tunnel {
            state: "trouble".into(),
            ..Default::default()
        }]));
        assert_eq!(m.shape(), Shape::Trouble);
    }
}
