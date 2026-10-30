//! What the UI knows: craked's tunnels and log, and what follows from them
//! (the tray's shape, which decisions can still be undone, the helpdesk text).

use std::time::{Duration, Instant};

use crake_proto::{LogEntry, LogKind};

use crate::client::{Tunnel, Update};
use crate::tray::{Badge, MenuTunnel, Shape};

mod days;
mod demo;
mod helpdesk;
mod kept;
pub mod live;
mod moments;
mod scrub;
// Which decisions still stand (undo, redo), for the charts.
mod standing;

// The log by day and the sample data, as the rest of the app knows them.
use self::days::local_time;
pub use self::days::{Day, Row, Show};
#[cfg(test)]
pub use self::demo::demo_log_whole_day;
pub use self::demo::{
    demo, demo_detail, demo_expired, demo_leftover, demo_log, demo_long_names, demo_openvpn3,
    demo_staying,
};
pub use self::moments::{AlsoAnswered, demo_moment};
pub use self::scrub::{Keep, scrub, scrub_keeping};

/// How long the tray shows the "routed" shape after a decision.
const ROUTED_SHAPE_FOR: Duration = Duration::from_secs(4);

/// How many log entries are kept in memory.
const KEEP: usize = 500;

/// Bytes in a kilobyte, megabyte, gigabyte (decimal, as people read them).
const KB: f64 = 1e3;
const MB: f64 = 1e6;
const GB: f64 = 1e9;
/// The menu's first line names the tunnels that are up, up to this many;
/// past it, their count ("Listening · 4 tunnels").
const MENU_NAMES: usize = 3;

/// How long a failed command's sentence stays on screen.
pub const FAILURE_SHOWN: Duration = Duration::from_secs(8);

/// An OpenVPN 3 process that outlived its session.
#[derive(Clone, Debug, PartialEq)]
pub struct Leftover {
    pub pid: u32,
    /// When it started (ticks since boot): tells it from a later process
    /// with the same number.
    pub started: u64,
    /// Its session, as the panel names it: its tunnel, else its profile.
    pub session: String,
}

/// An OpenVPN 3 tunnel that stays on OpenVPN 3 (CLAUDE.md, 7i): its
/// profile is locked there, or Crake can't run it. The panel says so, with
/// "Import .ovpn…" (design/proposals/openvpn-tunnel.html, 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staying {
    pub tunnel: String,
    pub title: String,
    pub why: String,
}

impl Staying {
    /// What "Not now" remembers.
    pub fn dismiss_key(&self) -> String {
        format!("openvpn3-stays:{}", self.tunnel)
    }
}

impl Leftover {
    /// What "Got it" remembers: this process, not its number alone.
    pub fn dismiss_key(&self) -> String {
        format!("leftover:{}:{}", self.pid, self.started)
    }
}

#[derive(Clone, Default)]
pub struct Model {
    /// `None` until the first attempt to reach craked has settled.
    pub reachable: Option<bool>,
    /// What the user has to do for craked to run (macOS: switch it on in
    /// System Settings), when anything.
    pub service: Option<crate::service::Wanted>,
    /// Why Windows says craked isn't running, while it's out of reach.
    pub stopped: Option<crate::service::Stopped>,
    /// craked's version: newer than this app's after a package updated it
    /// (the panel then asks for a restart, `versions`).
    pub service_version: Option<String>,
    /// The newest Crake, when asked (`versions::fetch`).
    pub newest: Option<String>,
    pub tunnels: Vec<Tunnel>,
    /// OpenVPN 3 processes that outlived their sessions and still run
    /// (`ovpn3::leftover`): the panel says how to stop each.
    pub leftovers: Vec<Leftover>,
    /// OpenVPN 3 tunnels that stay on OpenVPN 3 (`openvpn::move_from_openvpn3`).
    pub staying: Vec<Staying>,
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
    /// The Traffic window's rates, while it's open (shared: the panel
    /// copies the model on every frame).
    pub live: std::sync::Arc<live::LiveTraffic>,
    /// Nothing shows tray icons: what would bring the bird back (Linux).
    pub no_tray: Option<crate::no_tray::Remedy>,
    /// The panel's note about it was put off ("Not now") until next start.
    pub no_tray_quiet: bool,
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

    /// Crake decided to send this host through the tunnel (it found it by
    /// DNS, probing, a split...): it wears the two-stroke tick, also once
    /// pinned. A host the user added ("you"), a network, or one sent
    /// direct doesn't.
    pub fn crakes_decision(&self) -> bool {
        let by_crake = |how: &str| how != "you";
        match self.kind.as_str() {
            "learned" => self.learned.as_ref().is_none_or(|(_, how)| by_crake(how)),
            "pinned" => self.learned.as_ref().is_some_and(|(_, how)| by_crake(how)),
            _ => false,
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
    /// A short name: use a tunnel's full name for it ("Use ACME's").
    Use,
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub entry: LogEntry,
    /// "13:21", local time.
    pub time: String,
    pub action: Option<Action>,
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

/// A tunnel in trouble: why, in a few words, when Crake recognises the
/// reason (design/proposals/trouble-reason.html, option A).
pub fn trouble_words(t: &Tunnel) -> &'static str {
    crake_proto::trouble::label(&t.why).unwrap_or("Couldn\u{2019}t stay connected")
}

/// "Connected · 4 hosts", "Off", ...
/// Coming up again after it lost its gateway (craked keeps `since` across
/// a reconnect), not coming up for the first time.
pub fn reconnecting(t: &Tunnel) -> bool {
    t.state == "connecting" && t.since > 0
}

pub fn tunnel_state(t: &Tunnel) -> String {
    match t.state.as_str() {
        "connected" if t.dns_down => "Connected · its DNS doesn\u{2019}t answer".into(),
        "connected" if t.hosts == 1 => "Connected · 1 host".into(),
        "connected" => format!("Connected · {} hosts", t.hosts),
        "connecting" if reconnecting(t) => "Lost the gateway \u{b7} trying again…".into(),
        "connecting" => "Connecting…".into(),
        "trouble" => trouble_words(t).into(),
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
            Update::Live(live) => {
                std::sync::Arc::make_mut(&mut self.live).add(live, Instant::now())
            }
            // Handled by the app (it ends the session).
            Update::AttachRefused(_) => {}
            Update::Diagnostics(lines) => self.diagnostics = lines,
            Update::Traffic(t) => self.traffic = t,
            Update::Days(d) => self.days = d,
            Update::Settings(s) => self.settings = s,
            Update::ServiceVersion(v) => self.service_version = (!v.is_empty()).then_some(v),
            // The shell's: shown in a sign-in window (`openvpn::asked`).
            Update::Added(_) | Update::Asking(..) => {}
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

    /// The tray's shape: what the tunnels are doing, with a badge on the
    /// head as it would be without one (asleep, dashed or awake): red when a
    /// tunnel stopped on its own (design/proposals/dismiss-trouble.html),
    /// else amber when something needs you (an OpenVPN 3 process left
    /// running; design/proposals/leftover-warning.html). Red wins.
    pub fn shape(&self) -> Shape {
        let shape = self.shape_untroubled();
        if self.reachable != Some(true) {
            return shape;
        }
        if self.tunnels.iter().any(|t| t.state == "trouble") {
            shape.with(Badge::Trouble)
        } else if !self.leftovers.is_empty() {
            shape.with(Badge::Warning)
        } else {
            shape
        }
    }

    fn shape_untroubled(&self) -> Shape {
        let any = |s: &str| self.tunnels.iter().any(|t| t.state == s);
        if self.reachable != Some(true) {
            Shape::Off
        } else if any("connecting") {
            Shape::Connecting
        } else if !any("connected") {
            // A host routed a moment ago by a tunnel that's gone since
            // doesn't wake the head.
            Shape::Off
        } else if self
            .routed_at
            .is_some_and(|t| t.elapsed() < ROUTED_SHAPE_FOR)
        {
            Shape::Routed
        } else {
            Shape::Listening
        }
    }

    /// The panel header's word for the whole of Crake.
    pub fn status(&self) -> &'static str {
        match (self.reachable, self.shape()) {
            (None, _) => "Starting",
            (Some(false), _) => "Not running",
            (_, shape) => match shape.badged() {
                (_, Some(Badge::Trouble)) => "Trouble",
                (Shape::Connecting, _) => "Connecting",
                (Shape::Listening | Shape::Routed, _) => "Listening",
                _ => "Off",
            },
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
                    let names = if on.len() > MENU_NAMES {
                        format!("{} tunnels", on.len())
                    } else {
                        on.join(", ")
                    };
                    format!("Listening · {names} · {}", self.last_route_today())
                }
            }
            Some(false) => "Crake isn't running".into(),
            None => "Crake".into(),
        }
    }

    /// When Crake last sent a host through a tunnel today, for the menu's
    /// first line: "last route 13:21", or "nothing routed today". Crake's
    /// own decisions only (a host pinned by hand isn't one).
    fn last_route_today(&self) -> String {
        let today = jiff::Zoned::now().date();
        self.log
            .iter()
            .rev()
            .filter(|e| e.kind == LogKind::Routed && !e.message.starts_with("Pinned"))
            .find_map(|e| {
                let at = e.ts.parse::<jiff::Timestamp>().ok()?;
                let at = at.to_zoned(jiff::tz::TimeZone::system());
                (at.date() == today).then(|| format!("last route {}", at.strftime("%H:%M")))
            })
            .unwrap_or_else(|| "nothing routed today".into())
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

    /// The latest decisions (not system events, but short names chosen or
    /// forgotten), newest first, each with Undo or Redo while it's still
    /// the last word on its hosts (or short name).
    pub fn decisions(&self, count: usize) -> Vec<Decision> {
        let decisions: Vec<&LogEntry> = self
            .log
            .iter()
            .filter(|e| e.kind != LogKind::System || e.short.is_some())
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
            if let Some(d) = moments::short_decision(e, later) {
                out.push(d);
                continue;
            }
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
}

#[cfg(test)]
mod tests {

    #[test]
    fn crakes_decisions_wear_the_tick_also_when_pinned() {
        let row = |kind: &str, how: Option<&str>| HostRow {
            target: "10.40.8.4".into(),
            name: None,
            kind: kind.into(),
            learned: how.map(|h| (1, h.into())),
            used_ago: None,
        };
        assert!(row("learned", Some("probe")).crakes_decision());
        assert!(row("learned", None).crakes_decision(), "an older store");
        assert!(
            row("pinned", Some("dns")).crakes_decision(),
            "learned, then pinned"
        );
        assert!(
            !row("pinned", Some("you")).crakes_decision(),
            "added by hand"
        );
        assert!(
            !row("learned", Some("you")).crakes_decision(),
            "added by hand"
        );
        assert!(!row("pinned", None).crakes_decision(), "a network pinned");
        assert!(
            !row("direct", Some("probe")).crakes_decision(),
            "sent direct"
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
            short: None,
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

    /// A tunnel in trouble names why, when it's a reason Crake recognises
    /// (design/proposals/trouble-reason.html, option A).
    #[test]
    fn trouble_names_the_reason_it_knows() {
        let trouble = |why: &str| Tunnel {
            state: "trouble".into(),
            why: why.into(),
            ..Default::default()
        };
        let expired = format!(
            "{}: the server ends sessions after a while (this one after 54 minutes). {}",
            crake_proto::trouble::EXPIRED,
            crake_proto::trouble::SIGN_IN_AGAIN
        );
        assert_eq!(tunnel_state(&trouble(&expired)), "Session expired");
        assert_eq!(
            tunnel_state(&trouble("openconnect exited with exit status: 1.")),
            "Couldn\u{2019}t stay connected"
        );
        assert_eq!(tunnel_state(&trouble("")), "Couldn\u{2019}t stay connected");
    }

    /// A tunnel coming up again after it lost its gateway says so; one
    /// coming up for the first time is just connecting
    /// (design/screens/tray-panel.html?moment=back).
    #[test]
    fn a_reconnect_says_it_lost_the_gateway() {
        let connecting = |since| Tunnel {
            state: "connecting".into(),
            since,
            ..Default::default()
        };
        assert_eq!(tunnel_state(&connecting(0)), "Connecting…");
        assert!(!reconnecting(&connecting(0)));
        assert_eq!(
            tunnel_state(&connecting(1_790_000_000)),
            "Lost the gateway \u{b7} trying again…"
        );
        assert!(reconnecting(&connecting(1_790_000_000)));
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
        assert_eq!(m.shape(), Shape::OffTrouble);
        m.signing_in("initech", true, false);
        assert_eq!(m.shape(), Shape::Connecting);
        // craked still says trouble until the session is handed over.
        m.apply(trouble());
        assert_eq!(m.shape(), Shape::Connecting);
        // Cancelled: craked's word again (asleep: nothing else is up).
        m.signing_in("initech", false, false);
        m.apply(trouble());
        assert_eq!(m.shape(), Shape::OffTrouble);
    }

    /// The badge goes on the head as it would be without it
    /// (design/proposals/dismiss-trouble.html): a tunnel that stopped in
    /// the night with nothing else up looked as if one were connected.
    #[test]
    fn trouble_shows_on_the_head_as_it_is() {
        let tunnels = |states: &[&str]| {
            Update::Tunnels(
                states
                    .iter()
                    .enumerate()
                    .map(|(i, s)| Tunnel {
                        name: format!("t{i}"),
                        state: (*s).into(),
                        ..Default::default()
                    })
                    .collect(),
            )
        };
        let mut m = Model {
            reachable: Some(true),
            ..Default::default()
        };
        m.apply(tunnels(&["trouble", "off"]));
        assert_eq!(m.shape(), Shape::OffTrouble);
        assert_eq!(m.status(), "Trouble");
        m.apply(tunnels(&["trouble", "connecting"]));
        assert_eq!(m.shape(), Shape::ConnectingTrouble);
        assert_eq!(m.status(), "Trouble");
        m.apply(tunnels(&["trouble", "connected"]));
        assert_eq!(m.shape(), Shape::Trouble);
        // Dismissed: craked says off. Asleep, even right after a host was
        // routed (the routed head is for a tunnel that's up).
        m.routed_at = Some(std::time::Instant::now());
        m.apply(tunnels(&["off", "off"]));
        assert_eq!(m.shape(), Shape::Off);
        assert_eq!(m.status(), "Off");
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
        assert_eq!(m.shape(), Shape::OffTrouble);
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

    /// The helpdesk copy says what Crake does, and since when each tunnel
    /// is up, to which gateway and with what; hiding names leaves the
    /// gateway and the address out.
    /// The menu's first line: listening, through which tunnels, and when
    /// Crake last routed a host today (design/screens/tray-menu.html).
    #[test]
    fn the_menus_summary_says_when_crake_last_routed() {
        let now = jiff::Timestamp::now();
        let routed = |ts: jiff::Timestamp, message: &str| LogEntry {
            ts: ts.to_string(),
            kind: LogKind::Routed,
            tunnel: Some("acme".into()),
            message: message.into(),
            reason: None,
            hosts: vec!["10.40.1.20".into()],
            others: Vec::new(),
            short: None,
        };
        let mut m = demo();
        m.log.clear();
        assert_eq!(
            m.summary(),
            "Listening · ACME, Nordbank · nothing routed today"
        );
        m.log.push(routed(now, "Pinned 10.40.0.0/16 to ACME."));
        assert_eq!(
            m.summary(),
            "Listening · ACME, Nordbank · nothing routed today"
        );
        m.log
            .push(routed(now, "Sent git.acme.internal through ACME."));
        let at = now
            .to_zoned(jiff::tz::TimeZone::system())
            .strftime("%H:%M")
            .to_string();
        assert_eq!(
            m.summary(),
            format!("Listening · ACME, Nordbank · last route {at}")
        );
        for t in &mut m.tunnels {
            t.state = "connected".into();
        }
        assert!(m.summary().starts_with("Listening · 4 tunnels · "));
    }

    #[test]
    fn helpdesk_copies_say_how_each_tunnel_is_connected() {
        let at = jiff::civil::date(2026, 10, 9)
            .at(8, 31, 0, 0)
            .to_zoned(jiff::tz::TimeZone::system())
            .unwrap();
        let m = Model {
            reachable: Some(true),
            tunnels: vec![Tunnel {
                name: "acme".into(),
                display: "ACME".into(),
                state: "connected".into(),
                hosts: 5,
                address: "10.40.12.7".into(),
                gateway: "vpn.acme.example".into(),
                protocol: "anyconnect".into(),
                since: at.timestamp().as_second() as u64,
                ..Default::default()
            }],
            ..Default::default()
        };
        let shown = m.helpdesk(false);
        assert!(shown.contains(helpdesk::OPENER), "{shown}");
        assert!(
            shown.contains(
                "  ACME: Connected · 5 hosts, since 08:31 to vpn.acme.example with openconnect, address 10.40.12.7\n"
            ),
            "{shown}"
        );
        let hidden = m.helpdesk(true);
        assert!(
            hidden.contains("  ACME: Connected · 5 hosts, since 08:31 with openconnect\n"),
            "{hidden}"
        );
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

    /// Something needs you (an OpenVPN 3 process left running): the amber
    /// badge on the head as it is; a tunnel that stopped: red, which wins.
    #[test]
    fn red_trouble_wins_over_amber_needs_you() {
        let mut m = demo();
        m.routed_at = None;
        let (plain, badge) = m.shape().badged();
        assert_eq!(badge, None, "the sample needs nothing");
        let status = m.status();
        m.leftovers.push(Leftover {
            pid: 4242,
            started: 1,
            session: "Globex".into(),
        });
        assert_eq!(m.shape(), plain.with(Badge::Warning));
        assert_eq!(m.status(), status, "the header's word stays");
        m.tunnels[0].state = "trouble".into();
        assert_eq!(m.shape().badged().1, Some(Badge::Trouble));
        // Nothing up: asleep, with its amber badge.
        m.tunnels.iter_mut().for_each(|t| t.state = "off".into());
        assert_eq!(m.shape(), Shape::OffWarning);
    }
}
