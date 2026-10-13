//! Whether craked's own DNS server may stand in for the system's (macOS:
//! `sys::macos::primary_dns`), and which servers it then asks for public
//! names. Every public name goes through it, so without a server to ask
//! every name would fail: the whole internet, for the user.

use std::net::IpAddr;

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
