//! What goes where right now (D-Bus `Live`, as JSON): counters craked reads
//! once a second while the app's Traffic window is open, and the rates the
//! window draws from two of them. Nothing here is kept or logged.

use std::collections::BTreeMap;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// One look at the counters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Live {
    /// Bytes through the computer's own network devices so far (Wi-Fi,
    /// Ethernet: tunnels' encrypted traffic crosses them too).
    pub physical: u64,
    /// Bytes through each tunnel's device so far, by tunnel name.
    #[serde(default)]
    pub tunnels: BTreeMap<String, u64>,
    /// The open connections with their bytes; None when craked couldn't
    /// list them (or, in its tests, routes nothing for real).
    #[serde(default)]
    pub connections: Option<Vec<Connection>>,
}

/// An open connection (TCP, or a UDP conversation) from this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub src: IpAddr,
    pub sport: u16,
    pub dst: IpAddr,
    pub dport: u16,
    #[serde(default)]
    pub udp: bool,
    /// Bytes both ways so far.
    pub bytes: u64,
    /// The tunnel it goes through (by name); None: direct.
    #[serde(default)]
    pub tunnel: Option<String>,
    /// The name it was looked up by, when craked saw the answer.
    #[serde(default)]
    pub name: Option<String>,
}

impl Connection {
    /// The same connection in another look.
    pub fn key(&self) -> (IpAddr, u16, IpAddr, u16, bool) {
        (self.src, self.sport, self.dst, self.dport, self.udp)
    }
}

/// Bytes per second between two looks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rates {
    /// What went direct: the physical devices minus the tunnels' share.
    pub direct: f64,
    /// Through each tunnel, by name (only tunnels in both looks).
    pub tunnels: BTreeMap<String, f64>,
    /// Each connection open in the second look, with its rate (a new one's
    /// bytes count from 0).
    pub connections: Vec<(Connection, f64)>,
}

impl Live {
    /// The rates from `before` to `self`, `secs` apart. A counter that went
    /// down (a device came back, a connection's entry was renewed) counts
    /// as nothing that time.
    pub fn rates_since(&self, before: &Live, secs: f64) -> Rates {
        if secs <= 0.0 {
            return Rates::default();
        }
        let per_sec = |now: u64, then: u64| now.saturating_sub(then) as f64 / secs;
        let tunnels: BTreeMap<String, f64> = self
            .tunnels
            .iter()
            .filter_map(|(t, now)| Some((t.clone(), per_sec(*now, *before.tunnels.get(t)?))))
            .collect();
        let through: f64 = tunnels.values().sum();
        let direct = (per_sec(self.physical, before.physical) - through).max(0.0);
        let earlier: BTreeMap<_, u64> = before
            .connections
            .iter()
            .flatten()
            .map(|c| (c.key(), c.bytes))
            .collect();
        let connections = self
            .connections
            .iter()
            .flatten()
            .map(|c| {
                let then = earlier.get(&c.key()).copied().unwrap_or(0);
                (c.clone(), per_sec(c.bytes, then))
            })
            .collect();
        Rates {
            direct,
            tunnels,
            connections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(dport: u16, bytes: u64, tunnel: Option<&str>) -> Connection {
        Connection {
            src: "192.0.2.10".parse().unwrap(),
            sport: 40_000,
            dst: "10.40.1.10".parse().unwrap(),
            dport,
            udp: false,
            bytes,
            tunnel: tunnel.map(str::to_owned),
            name: Some("git.acme.internal".into()),
        }
    }

    #[test]
    fn direct_is_the_devices_minus_the_tunnels() {
        let before = Live {
            physical: 1_000,
            tunnels: [("acme".to_owned(), 100)].into(),
            connections: None,
        };
        let now = Live {
            physical: 5_000,
            tunnels: [("acme".to_owned(), 1_100), ("nordbank".to_owned(), 50)].into(),
            connections: None,
        };
        let r = now.rates_since(&before, 2.0);
        assert_eq!(
            r.tunnels,
            [("acme".to_owned(), 500.0)].into(),
            "new tunnels wait a look"
        );
        assert_eq!(r.direct, 1_500.0);
        assert!(r.connections.is_empty());
        assert_eq!(now.rates_since(&before, 0.0), Rates::default());
    }

    #[test]
    fn a_connection_counts_from_its_last_look() {
        let before = Live {
            connections: Some(vec![conn(443, 1_000, Some("acme"))]),
            ..Live::default()
        };
        let now = Live {
            connections: Some(vec![conn(443, 3_000, Some("acme")), conn(22, 400, None)]),
            ..Live::default()
        };
        let r = now.rates_since(&before, 1.0);
        let rates: Vec<f64> = r.connections.iter().map(|(_, r)| *r).collect();
        assert_eq!(rates, [2_000.0, 400.0], "a new one counts from 0");
        // A counter that went down counts as nothing.
        assert_eq!(before.rates_since(&now, 1.0).connections[0].1, 0.0);
    }

    #[test]
    fn it_travels_as_json() {
        let live = Live {
            physical: 7,
            tunnels: [("acme".to_owned(), 3)].into(),
            connections: Some(vec![conn(443, 9, Some("acme"))]),
        };
        let text = serde_json::to_string(&live).unwrap();
        assert_eq!(serde_json::from_str::<Live>(&text).unwrap(), live);
        let bare: Live = serde_json::from_str(r#"{"physical": 1}"#).unwrap();
        assert_eq!(bare.connections, None, "a system that can't list them");
    }
}
