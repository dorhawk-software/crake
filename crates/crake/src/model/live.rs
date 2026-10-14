//! The Traffic window's numbers (design/proposals/traffic-window.html):
//! craked's counters, looked at once a second while the window is open
//! (D-Bus `Live`), turned into rates, with the last five minutes of them for
//! the strip under the graph. Nothing is written anywhere.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

use crake_proto::live::{Connection, Live, Rates};

/// The strip under the graph: five minutes.
pub const HISTORY: Duration = Duration::from_secs(5 * 60);

/// A connection's identity across looks (`Connection::key`).
type Key = (IpAddr, u16, IpAddr, u16, bool);

#[derive(Clone, Debug, Default)]
pub struct LiveTraffic {
    /// The last look, and when it came.
    last: Option<(Instant, Live)>,
    /// The rates between the last two looks.
    pub rates: Option<Rates>,
    /// (when, direct bytes/s, through tunnels bytes/s), oldest first, five
    /// minutes.
    pub history: VecDeque<(Instant, f64, f64)>,
    /// When each open connection was first seen (its "Since").
    pub first_seen: HashMap<Key, Instant>,
}

impl LiveTraffic {
    /// A new look at the counters, taken `at`.
    pub fn add(&mut self, live: Live, at: Instant) {
        if let Some((then, before)) = &self.last {
            let secs = at.duration_since(*then).as_secs_f64();
            let rates = live.rates_since(before, secs);
            let through = rates.tunnels.values().sum();
            self.history.push_back((at, rates.direct, through));
            self.rates = Some(rates);
        }
        while self
            .history
            .front()
            .is_some_and(|(when, _, _)| at.duration_since(*when) > HISTORY)
        {
            self.history.pop_front();
        }
        let open: Vec<Key> = live
            .connections
            .iter()
            .flatten()
            .map(Connection::key)
            .collect();
        self.first_seen.retain(|k, _| open.contains(k));
        for k in open {
            self.first_seen.entry(k).or_insert(at);
        }
        self.last = Some((at, live));
    }

    /// Whether craked lists connections on this system (None: not asked yet).
    pub fn lists_connections(&self) -> Option<bool> {
        self.last.as_ref().map(|(_, l)| l.connections.is_some())
    }

    /// How long ago `c` was first seen.
    pub fn open_for(&self, c: &Connection, now: Instant) -> Option<Duration> {
        self.first_seen
            .get(&c.key())
            .map(|at| now.saturating_duration_since(*at))
    }
}

/// CRAKE_DEMO's counters `second` seconds in: the proposal's ways and
/// destinations, each at its rate with a little wobble.
// literal: the proposal's sample (design/proposals/traffic-window.html)
pub fn demo(second: u64) -> Live {
    let hosts: [(&str, &str, u16, Option<&str>, u64); 10] = [
        (
            "10.40.1.10",
            "git.acme.internal",
            443,
            Some("acme"),
            1_100_000,
        ),
        (
            "10.40.1.20",
            "jira.acme.internal",
            443,
            Some("acme"),
            85_000,
        ),
        ("10.40.8.21", "", 5432, Some("acme"), 40_000),
        (
            "10.40.1.11",
            "wiki.acme.internal",
            443,
            Some("acme"),
            12_000,
        ),
        (
            "172.22.4.9",
            "sftp.nordbank.example",
            22,
            Some("nord"),
            420_000,
        ),
        (
            "172.22.4.30",
            "portal.nordbank.example",
            443,
            Some("nord"),
            30_000,
        ),
        ("198.51.100.20", "teams.microsoft.com", 443, None, 310_000),
        ("198.51.100.30", "github.com", 443, None, 60_000),
        ("198.51.100.40", "open.spotify.com", 443, None, 180_000),
        ("198.51.100.50", "", 443, None, 95_000),
    ];
    // Each host's rate swings ±30 % around its own over ~44 s: its bytes
    // so far are that rate's running total, so they only ever grow.
    let sent = |i: usize, rate: f64| {
        let (period, swing, offset) = (7.0, 0.3, i as f64 * 1.3);
        let s = second as f64;
        rate * (s - swing * period * ((s / period + offset).cos() - offset.cos()))
    };
    let mut tunnels = std::collections::BTreeMap::new();
    let mut physical = 0;
    let connections = hosts
        .iter()
        .enumerate()
        .map(|(i, (ip, name, port, tunnel, rate))| {
            let bytes = sent(i, *rate as f64) as u64;
            physical += bytes;
            if let Some(t) = tunnel {
                *tunnels.entry((*t).to_owned()).or_insert(0) += bytes;
            }
            Connection {
                src: "192.0.2.10".parse().expect("an address"),
                // literal: an ephemeral port per host
                sport: 40_000 + i as u16,
                dst: ip.parse().expect("an address"),
                dport: *port,
                udp: false,
                bytes,
                tunnel: tunnel.map(str::to_owned),
                name: (!name.is_empty()).then(|| (*name).to_owned()),
            }
        })
        .collect();
    Live {
        physical,
        tunnels,
        connections: Some(connections),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_come_from_two_looks_and_five_minutes_are_kept() {
        let mut l = LiveTraffic::default();
        let t0 = Instant::now();
        l.add(demo(10), t0);
        assert!(l.rates.is_none(), "one look gives no rate yet");
        assert_eq!(l.lists_connections(), Some(true));
        l.add(demo(11), t0 + Duration::from_secs(1));
        let rates = l.rates.clone().expect("rates");
        assert!(rates.tunnels["acme"] > 1_000_000.0, "{rates:?}");
        assert!(rates.direct > 500_000.0, "{rates:?}");
        assert_eq!(l.history.len(), 1);
        // Six minutes on: the first second has left the strip.
        l.add(demo(371), t0 + Duration::from_secs(371));
        assert_eq!(l.history.len(), 1);
        let c = rates.connections[0].0.clone();
        assert_eq!(
            l.open_for(&c, t0 + Duration::from_secs(400)),
            Some(Duration::from_secs(400)),
            "open since it was first seen"
        );
        // A system that doesn't list connections.
        let mut none = LiveTraffic::default();
        none.add(Live::default(), t0);
        assert_eq!(none.lists_connections(), Some(false));
        assert_eq!(LiveTraffic::default().lists_connections(), None);
    }
}
