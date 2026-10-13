//! Whether craked's own DNS server may stand in for the system's (macOS:
//! `sys::macos::primary_dns`), and which servers it then asks for public
//! names. Every public name goes through it, so without a server to ask
//! every name would fail: the whole internet, for the user.

use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Public questions in a row the system's servers leave unanswered before
/// craked gives the system its own DNS back.
pub const MISSES: u32 = 3;
/// How long the system keeps its own DNS before craked stands in again.
pub const RETRY: Duration = Duration::from_secs(120);

/// Whether the servers craked asks in the system's stead still answer.
/// When they stop (a captive portal, a network that changed under it),
/// craked steps aside rather than fail every name, and tries again later.
#[derive(Debug, Default)]
pub struct Watch {
    misses: u32,
    /// When craked gave the system its DNS back.
    since: Option<Instant>,
}

impl Watch {
    pub const fn new() -> Self {
        Self {
            misses: 0,
            since: None,
        }
    }

    /// The system's servers answered.
    pub fn answered(&mut self) {
        self.misses = 0;
    }

    /// They didn't: true when it's time to give the system its DNS back
    /// (once, until `retry`).
    pub fn unanswered(&mut self, now: Instant) -> bool {
        if self.since.is_some() {
            return false;
        }
        self.misses += 1;
        if self.misses < MISSES {
            return false;
        }
        self.since = Some(now);
        true
    }

    /// Whether the system has its own DNS back now.
    pub fn given_back(&self) -> bool {
        self.since.is_some()
    }

    /// Time to stand in again (`RETRY` after giving back): counting starts
    /// afresh.
    pub fn retry(&mut self, now: Instant) -> bool {
        let due = self.since.is_some_and(|at| now.duration_since(at) >= RETRY);
        if due {
            *self = Self::default();
        }
        due
    }
}

/// The servers craked asks for public names in place of the system's: the
/// ones the user typed (they win), or else DHCP's, less craked's own and
/// any it can't ask (a link-local one with its zone, `fe80::1%en0`).
pub fn servers_to_ask(
    typed: Option<&[String]>,
    dhcp: Option<&[String]>,
    own: IpAddr,
) -> Vec<IpAddr> {
    typed
        .filter(|s| !s.is_empty())
        .or(dhcp)
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s.parse().ok())
        .filter(|a| *a != own)
        .collect()
}

/// Whether craked may take the system's DNS servers' place: only while it
/// has one to ask in their stead.
pub fn may_stand_in(typed: Option<&[String]>, dhcp: Option<&[String]>, own: IpAddr) -> bool {
    !servers_to_ask(typed, dhcp, own).is_empty()
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    const OWN: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 53, 0, 1));

    fn list(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Three public questions in a row unanswered: the system gets its
    /// DNS back, once; an answer in between starts the count again; two
    /// minutes later craked stands in again, counting afresh.
    #[test]
    fn unanswered_questions_give_the_system_its_dns_back() {
        let t0 = Instant::now();
        let mut w = Watch::default();
        assert!(!w.unanswered(t0));
        assert!(!w.unanswered(t0));
        w.answered();
        assert!(!w.unanswered(t0));
        assert!(!w.unanswered(t0));
        assert!(w.unanswered(t0), "three in a row: give it back");
        assert!(w.given_back());
        assert!(!w.unanswered(t0), "given back once");
        assert!(!w.retry(t0 + RETRY / 2), "not yet");
        assert!(w.retry(t0 + RETRY), "two minutes later: again");
        assert!(!w.given_back());
        assert!(!w.unanswered(t0 + RETRY), "counting afresh");
    }

    #[test]
    fn the_servers_typed_win_over_dhcps() {
        let typed = list(&["192.0.2.53"]);
        let dhcp = list(&["198.51.100.1"]);
        assert_eq!(
            servers_to_ask(Some(&typed), Some(&dhcp), OWN),
            vec!["192.0.2.53".parse::<IpAddr>().unwrap()]
        );
        assert_eq!(
            servers_to_ask(Some(&[]), Some(&dhcp), OWN),
            vec!["198.51.100.1".parse::<IpAddr>().unwrap()]
        );
    }

    /// A Mac whose DNS doesn't come from its network service's DNS key
    /// (nothing to displace), or whose only server is the router's
    /// link-local IPv6 one, has nothing craked could ask: standing in
    /// would fail every name (a colleague's Mac: "no internet at all"
    /// while a tunnel was up).
    #[test]
    fn no_stand_in_without_a_server_to_ask() {
        assert!(!may_stand_in(None, None, OWN), "no DNS key at all");
        assert!(
            !may_stand_in(None, Some(&[]), OWN),
            "a key naming no server"
        );
        assert!(
            !may_stand_in(None, Some(&list(&["fe80::1%en0"])), OWN),
            "only the router's link-local server"
        );
        assert!(
            !may_stand_in(None, Some(&list(&["127.53.0.1"])), OWN),
            "only craked's own server"
        );
        assert!(may_stand_in(None, Some(&list(&["192.0.2.53"])), OWN));
        assert!(may_stand_in(
            None,
            Some(&list(&["fe80::1%en0", "2001:db8::53"])),
            OWN
        ));
    }
}
