//! What the UI knows: craked's tunnels and log, and what follows from them
//! (the tray's shape, which decisions can still be undone, the helpdesk text).

use std::time::{Duration, Instant};

use crake_proto::{LogEntry, LogKind};

use crate::client::{Tunnel, Update};
use crate::tray::{MenuTunnel, Shape};

mod days;
mod demo;
mod kept;
// Which decisions still stand (undo, redo), for the charts.
mod standing;

// The log by day and the sample data, as the rest of the app knows them.
use self::days::local_time;
pub use self::days::{Day, Row, Show};
#[cfg(test)]
pub use self::demo::demo_log_whole_day;
pub use self::demo::{demo, demo_detail, demo_log};

/// How long the tray shows the "routed" shape after a decision.
const ROUTED_SHAPE_FOR: Duration = Duration::from_secs(4);

/// How many log entries are kept in memory.
const KEEP: usize = 500;

/// Bytes in a kilobyte, megabyte, gigabyte (decimal, as people read them).
const KB: f64 = 1e3;
const MB: f64 = 1e6;
const GB: f64 = 1e9;
/// Decisions in a helpdesk copy.
const HELPDESK_DECISIONS: usize = 20;

/// How long a failed command's sentence stays on screen.
pub const FAILURE_SHOWN: Duration = Duration::from_secs(8);

#[derive(Clone, Default)]
pub struct Model {
    /// `None` until the first attempt to reach craked has settled.
    pub reachable: Option<bool>,
    pub tunnels: Vec<Tunnel>,
    /// Oldest first.
    pub log: Vec<LogEntry>,
    pub routed_at: Option<Instant>,
    /// craked's sentence for a command that failed, and when (shown for
    /// FAILURE_SHOWN).
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
    /// craked's view of the computer (its tunnels' devices and DNS, who
    /// answers the system's DNS), for helpdesk copies.
    pub diagnostics: Vec<String>,
    /// Browsers that look names up themselves (`browser_dns`), by name.
    pub own_dns_browsers: Vec<String>,
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
    /// How many decisions it stands for (still standing: not undone).
    pub count: usize,
    /// Every decision it stands for was undone: drawn grey.
    pub undone: bool,
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
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.0} KB", (b / KB).max(1.))
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

    /// A tunnel being signed in to shows as connecting, even one craked
    /// still has in trouble (its last session ended badly): signing in is
    /// what the user is doing about it.
    fn show_signing_in(&mut self) {
        for t in &mut self.tunnels {
            if self.signing_in.contains(&t.name)
                && !matches!(t.state.as_str(), "connected" | "connecting")
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
            Update::Diagnostics(lines) => self.diagnostics = lines,
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
        self.marks_within(crate::units::MINUTES_PER_HOUR_F, None, MERGE_MINUTES)
    }

    /// The marks of the last `span` minutes (of one tunnel's decisions, or
    /// all), decisions at most `merge` minutes apart sharing one.
    pub fn marks_within(&self, span: f32, tunnel: Option<&str>, merge: f32) -> Vec<Mark> {
        let now = jiff::Timestamp::now();
        let events: Vec<(f32, &LogEntry, bool)> = standing::decisions(&self.log)
            .into_iter()
            .filter(|(e, _)| tunnel.is_none_or(|t| e.tunnel.as_deref() == Some(t)))
            .filter_map(|(e, stands)| {
                let t: jiff::Timestamp = e.ts.parse().ok()?;
                let ago =
                    (now.as_second() - t.as_second()) as f32 / crate::units::SECS_PER_MINUTE_F;
                (0. ..=span).contains(&ago).then_some((ago, e, stands))
            })
            .collect();
        // Oldest first (`decisions` is).
        let mut groups: Vec<(f32, bool, Vec<standing::Decision>)> = Vec::new();
        for (ago, e, stands) in events {
            let routed = e.kind == LogKind::Routed;
            match groups.iter_mut().rev().find(|g| g.1 == routed) {
                Some(g) if g.0 - ago <= merge => {
                    g.0 = ago; // the mark stands where the latest of them is
                    g.2.push((e, stands));
                }
                _ => groups.push((ago, routed, vec![(e, stands)])),
            }
        }
        groups
            .into_iter()
            .map(|(ago, routed, entries)| {
                let standing: Vec<&LogEntry> = entries
                    .iter()
                    .filter(|(_, s)| *s)
                    .map(|(e, _)| *e)
                    .collect();
                let all: Vec<&LogEntry> = entries.iter().map(|(e, _)| *e).collect();
                // The latest of them, undone or not: undo and redo don't
                // make the tick grow in again.
                let last = all.last().copied();
                Mark {
                    ago,
                    routed,
                    count: if standing.is_empty() {
                        all.len()
                    } else {
                        standing.len()
                    },
                    undone: routed && standing.is_empty(),
                    ts: last.map(|e| e.ts.clone()).unwrap_or_default(),
                    time: last.map(|e| local_time(&e.ts)).unwrap_or_default(),
                    text: mark_text(if standing.is_empty() { &all } else { &standing }),
                }
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
        let recent = self.decisions(HELPDESK_DECISIONS);
        if recent.is_empty() {
            s += "  (none)\n";
        }
        for d in recent {
            s += &format!("  {}  {}\n", d.time, say(&d.entry.message));
            if let Some(r) = &d.entry.reason {
                s += &format!("         {}\n", say(r));
            }
        }
        // What only craked sees: saves asking the user to run commands.
        if !self.diagnostics.is_empty() || !self.own_dns_browsers.is_empty() {
            s += "\nCrake's view of this computer\n";
            for line in &self.diagnostics {
                s += &format!("  {}\n", say(line));
            }
            if !self.own_dns_browsers.is_empty() {
                let names = crate::browser_dns::and_list(&self.own_dns_browsers);
                let verb = if self.own_dns_browsers.len() == 1 {
                    "looks"
                } else {
                    "look"
                };
                s += &format!(
                    "  {names} {verb} names up themselves (DNS over HTTPS): Crake can't route a company's names there before the first packet.\n"
                );
            }
        }
        s
    }
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

    /// Seen after a session expired: switching the tunnel on to sign in
    /// again left the tray's red "!" up all through the sign-in.
    #[test]
    fn signing_in_again_after_trouble_shows_as_connecting() {
        let trouble = || {
            Update::Tunnels(vec![Tunnel {
                name: "initech".into(),
                state: "trouble".into(),
                ..Default::default()
            }])
        };
        let mut m = Model {
            reachable: Some(true),
            ..Default::default()
        };
        m.apply(trouble());
        assert_eq!(m.shape(), Shape::Trouble);
        m.signing_in("initech", true, false);
        assert_eq!(m.shape(), Shape::Connecting);
        // craked still says trouble until the session is handed over.
        m.apply(trouble());
        assert_eq!(m.shape(), Shape::Connecting);
        // Cancelled: craked's word again.
        m.signing_in("initech", false, false);
        m.apply(trouble());
        assert_eq!(m.shape(), Shape::Trouble);
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
                at(
                    9,
                    LogKind::Routed,
                    "Sent iam.intranet.acme.internal through ACME.",
                ),
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

    /// Undo takes back the decision it's about, Redo brings that same one
    /// back: the tick's count goes down and up (grey once nothing stands),
    /// no decision is added, and the tick doesn't grow in again. Any run of
    /// undos and redos ends where it began.
    #[test]
    fn undo_and_redo_change_their_tick_and_add_none() {
        let at = |secs: i64, kind: LogKind, msg: &str, host: &str| LogEntry {
            ts: jiff::Timestamp::now()
                .checked_sub(jiff::SignedDuration::from_secs(secs))
                .unwrap()
                .to_string(),
            ..entry(kind, msg, &[host])
        };
        let (a, b) = ("10.0.0.1", "10.0.0.2");
        let mut m = Model {
            log: vec![
                at(600, LogKind::Routed, "Sent a through ACME.", a),
                at(580, LogKind::Routed, "Sent b through ACME.", b),
            ],
            ..Default::default()
        };
        // What a tick shows, without its "minutes ago": that follows the
        // clock, which may move on between two looks.
        let shown = |marks: Vec<Mark>| -> Vec<Mark> {
            marks.into_iter().map(|k| Mark { ago: 0., ..k }).collect()
        };
        let start = shown(m.marks());
        assert_eq!(start.len(), 1, "{start:?}");
        assert_eq!((start[0].count, start[0].undone), (2, false));
        let mut secs = 500;
        let mut then = |m: &mut Model, kind: LogKind, msg: &str, host: &str| {
            secs -= 10;
            m.log.push(at(secs, kind, msg, host));
            shown(m.marks())
        };
        let undone = then(&mut m, LogKind::Undone, "a goes direct again.", a);
        assert_eq!(undone.len(), 1, "{undone:?}");
        assert_eq!(
            (undone[0].count, undone[0].undone),
            (1, false),
            "undo counts one less"
        );
        let redone = then(&mut m, LogKind::Routed, "Sent a through ACME again.", a);
        assert_eq!(redone, start, "redo is back where it began");
        then(&mut m, LogKind::Undone, "a goes direct again.", a);
        let both = then(&mut m, LogKind::Undone, "b goes direct again.", b);
        assert_eq!(both.len(), 1, "{both:?}");
        assert!(both[0].undone, "nothing stands: grey");
        for _ in 0..10 {
            then(&mut m, LogKind::Routed, "Sent a through ACME again.", a);
            then(&mut m, LogKind::Undone, "a goes direct again.", a);
        }
        then(&mut m, LogKind::Routed, "Sent a through ACME again.", a);
        let back = then(&mut m, LogKind::Routed, "Sent b through ACME again.", b);
        assert_eq!(back, start, "ten undos and redos later: as it was");
    }

    /// The helpdesk copy carries craked's view of the computer; with host
    /// names hidden, its addresses and domains are left out too.
    #[test]
    fn helpdesk_copies_carry_crakeds_view_and_hide_its_hosts() {
        let m = Model {
            diagnostics: vec![
                "Initech: tun0, address 10.8.0.6, DNS 10.8.0.1 for initech.internal; 2 hosts learned, 0 pinned".into(),
                "Public names: Crake answers the Mac's DNS, asking 192.168.1.1.".into(),
            ],
            ..Default::default()
        };
        let shown = m.helpdesk(false);
        assert!(shown.contains("Crake's view of this computer"), "{shown}");
        assert!(
            shown.contains("DNS 10.8.0.1 for initech.internal"),
            "{shown}"
        );
        let hidden = m.helpdesk(true);
        assert!(hidden.contains("Crake's view of this computer"), "{hidden}");
        for host in ["10.8.0.6", "10.8.0.1", "initech.internal", "192.168.1.1"] {
            assert!(!hidden.contains(host), "{host} in: {hidden}");
        }
    }

    /// A browser that looks names up itself is in the helpdesk copy: why
    /// a company's names don't route before the first packet there.
    #[test]
    fn helpdesk_copies_say_which_browsers_look_names_up_themselves() {
        let m = Model {
            own_dns_browsers: vec!["Firefox".into(), "Chrome".into()],
            ..Default::default()
        };
        let copy = m.helpdesk(false);
        assert!(
            copy.contains("Firefox and Chrome look names up themselves (DNS over HTTPS)"),
            "{copy}"
        );
    }
}
