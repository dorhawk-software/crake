//! Host names and addresses out of what's shared (Copy for helpdesk, a
//! report; Settings → Privacy & log, "Hide names and addresses when
//! sharing"): "Sent git.acme.internal through ACME." → "Sent a host
//! through ACME." A report keeps the computer's own plumbing (`Keep`), or
//! its routes and DNS would say nothing.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};

/// What hiding leaves: the computer's addresses and networks, its
/// tunnels' addresses and DNS servers, its router (craked's
/// `Computer::keep`), in a form words are compared in.
#[derive(Debug, Default)]
pub struct Keep(HashSet<String>);

impl Keep {
    pub fn new(kept: &[String]) -> Self {
        Self(kept.iter().filter_map(|k| normal(k)).collect())
    }
}

/// "10.40.0.2", "fd40::2", "192.168.1.0/24" as Rust writes them (an
/// address's /32 or /128 is the address).
fn normal(word: &str) -> Option<String> {
    let word = word.split('%').next().unwrap_or(word);
    if let Ok(a) = word.parse::<IpAddr>() {
        return Some(a.to_string());
    }
    let (a, p) = word.split_once('/')?;
    let a: IpAddr = a.split('%').next().unwrap_or(a).parse().ok()?;
    let p: u8 = p.parse().ok()?;
    let whole = if a.is_ipv4() { V4_BITS } else { V6_BITS };
    Some(if p == whole {
        a.to_string()
    } else {
        format!("{a}/{p}")
    })
}

const V4_BITS: u8 = 32;
const V6_BITS: u8 = 128;

/// Copy for helpdesk's hiding: every name and address.
pub fn scrub(text: &str) -> String {
    scrub_keeping(text, &Keep::default())
}

/// Names and addresses out, but those in `keep` and what's never a host
/// reached (loopback, link-local, multicast, a netmask, "everything").
pub fn scrub_keeping(text: &str, keep: &Keep) -> String {
    text.split(' ')
        .map(|word| {
            let start = word
                .find(|c: char| c.is_ascii_alphanumeric())
                .unwrap_or(word.len());
            let end = word
                .rfind(|c: char| c.is_ascii_alphanumeric())
                .map_or(start, |i| i + 1);
            let core = &word[start..end.max(start)];
            if is_host(core, keep) {
                format!("{}a host{}", &word[..start], &word[end.max(start)..])
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_host(core: &str, keep: &Keep) -> bool {
    if let Some(n) = normal(core) {
        let addr: Option<IpAddr> = n.split('/').next().and_then(|a| a.parse().ok());
        return !(keep.0.contains(&n) || addr.is_some_and(never_a_host));
    }
    // A name: dotted labels, the last one letters (not "3.5", not "e.g").
    let labels: Vec<&str> = core.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels
            .last()
            .is_some_and(|l| l.len() >= 2 && l.chars().all(|c| c.is_ascii_alphabetic()))
}

/// Addresses that say nothing about where someone went.
fn never_a_host(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(a) => {
            a.is_loopback()
                || a.is_link_local()
                || a.is_multicast()
                || a.is_broadcast()
                || is_mask(a)
        }
        IpAddr::V6(a) => {
            a.is_loopback() || a.is_unspecified() || a.is_multicast() || a.is_unicast_link_local()
        }
    }
}

/// 255.255.255.0, 0.0.0.0: a netmask (Windows' routes have a column).
fn is_mask(a: Ipv4Addr) -> bool {
    let bits = u32::from(a);
    bits.leading_ones() + bits.trailing_zeros() == u32::from(V4_BITS)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn ipv6_addresses_go_too() {
        assert_eq!(
            scrub("Sent fd40:1::10 and 2001:db8::1, through ACME."),
            "Sent a host and a host, through ACME."
        );
        assert_eq!(scrub("Pinned fd40::/48."), "Pinned a host.");
    }

    #[test]
    fn a_report_keeps_the_computers_plumbing() {
        let keep = Keep::new(&[
            "192.0.2.20".into(),
            "192.0.2.0/24".into(),
            "10.40.255.50".into(),
            "10.40.0.2".into(),
            "fd40:ff::5".into(),
        ]);
        assert_eq!(
            scrub_keeping("10.40.0.2/32 utun4 Crake: ACME's DNS", &keep),
            "10.40.0.2/32 utun4 Crake: ACME's DNS"
        );
        assert_eq!(
            scrub_keeping("10.40.1.10 utun4 ← Crake: learned through ACME", &keep),
            "a host utun4 ← Crake: learned through ACME"
        );
        assert_eq!(
            scrub_keeping("192.0.2.0/24 dev wlp2s0 src 192.0.2.20", &keep),
            "192.0.2.0/24 dev wlp2s0 src 192.0.2.20"
        );
        assert_eq!(
            scrub_keeping("nameserver[0] : 127.53.0.1; fe80::1%en0 fd40:ff::5", &keep),
            "nameserver[0] : 127.53.0.1; fe80::1%en0 fd40:ff::5"
        );
        // Windows' route print: masks and "everything" stay.
        assert_eq!(
            scrub_keeping("0.0.0.0 0.0.0.0 192.0.2.1 192.0.2.20 35", &keep),
            "0.0.0.0 0.0.0.0 a host 192.0.2.20 35"
        );
        assert_eq!(
            scrub_keeping("203.0.113.9 255.255.255.255 On-link 10.40.255.50", &keep),
            "a host 255.255.255.255 On-link 10.40.255.50"
        );
        // A domain is a name like any other.
        assert_eq!(
            scrub_keeping("DNS 10.40.0.2 for acme.internal", &keep),
            "DNS 10.40.0.2 for a host"
        );
        // Times and versions aren't addresses.
        assert_eq!(
            scrub_keeping("17:52 Crake 0.10.4 on macOS 26.1", &keep),
            "17:52 Crake 0.10.4 on macOS 26.1"
        );
    }
}
