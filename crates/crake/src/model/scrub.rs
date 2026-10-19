//! Host names and addresses out of what's shared (Copy for helpdesk, a
//! report; Settings → Privacy & log, "Hide names and addresses when
//! sharing"): "Sent git.acme.internal through ACME." → "Sent a host
//! through ACME." A report keeps the computer's own plumbing (`Keep`), or
//! its routes and DNS would say nothing.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};

/// What hiding leaves: the computer's addresses and networks, its
/// tunnels' addresses and DNS servers, its router (craked's
/// `Computer::keep`), in a form words are compared in; and words that are
/// never hidden (the tunnels' names, which say whose a line is).
#[derive(Debug, Default)]
pub struct Keep {
    kept: HashSet<String>,
    /// Short names to hide alone (`with_names_in`), lowercase.
    short: HashSet<String>,
    /// Never hidden, lowercase.
    words: HashSet<String>,
}

/// A host's first label this long or longer is hidden on its own too
/// ("confluence" typed for confluence.acme.internal); shorter ones ("mail",
/// "git", "www") are words as often as hosts.
const SHORT_NAME_LEN: usize = 5;
/// A host's name has this many labels at least (wiki.acme.internal); with
/// fewer it's a domain (acme.internal), whose first label is often the
/// tunnel's own name.
const HOST_LABELS: usize = 3;

impl Keep {
    pub fn new(kept: &[String]) -> Self {
        Self {
            kept: kept.iter().filter_map(|k| normal(k)).collect(),
            ..Self::default()
        }
    }

    /// Words never hidden, whatever names hold them: the tunnels' names
    /// ("ACME", whose acme.sharepoint.example would otherwise hide it).
    pub fn keeping_words(mut self, words: impl IntoIterator<Item = String>) -> Self {
        self.words
            .extend(words.into_iter().map(|w| w.trim().to_lowercase()));
        self
    }

    /// Also hide, standing alone, the first label of every host name in
    /// `text`: the short name a browser or a log may use for the same host.
    pub fn with_names_in(mut self, text: &str) -> Self {
        let mut rest = text;
        while let Some(start) = rest.find(name_char) {
            let run = &rest[start..];
            let len = run.find(|c: char| !name_char(c)).unwrap_or(run.len());
            let body = run[..len].trim_matches(['.', ':', '/', '-', '_']);
            let (host, _) = split_port(body);
            if normal(host).is_none() && is_name(host) && host.split('.').count() >= HOST_LABELS {
                let first = host.split('.').next().unwrap_or_default().to_lowercase();
                if first.chars().count() >= SHORT_NAME_LEN && !self.words.contains(&first) {
                    self.short.insert(first);
                }
            }
            rest = &run[len..];
        }
        self
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
/// Found wherever they stand, not only between spaces: after "host=", in
/// quotes and lists, with a port ("wiki.acme.internal:443",
/// "[fd40::2]:443"), after a tab, in a range ("10.40.1.10-10.40.1.20").
/// Where a system lists DNS domains ("DNS Domain:", "search domain[0] :",
/// "Namespace :", a "~routing" domain), a one-word domain ("corp") goes too.
pub fn scrub_keeping(text: &str, keep: &Keep) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let domains_from = domains_from(line);
        let mut at = 0;
        let mut rest = line;
        while let Some(start) = rest.find(name_char) {
            out.push_str(&rest[..start]);
            let run = &rest[start..];
            let len = run.find(|c: char| !name_char(c)).unwrap_or(run.len());
            let begins = at + start;
            let domain = domains_from.is_some_and(|d| begins >= d) || line[..begins].ends_with('~');
            out.push_str(&hidden(&run[..len], keep, domain));
            at = begins + len;
            rest = &run[len..];
        }
        out.push_str(rest);
    }
    out
}

/// Where a line lists DNS domains, the byte after its key's colon:
/// resolvectl's "DNS Domain:", scutil's "search domain[0] :" and
/// "domain :", NRPT's "Namespace :".
fn domains_from(line: &str) -> Option<usize> {
    let colon = line.find(':')?;
    let key = line[..colon].trim().to_lowercase();
    let key = key.split('[').next().unwrap_or_default().trim();
    DOMAIN_KEYS.contains(&key).then_some(colon + 1)
}

const DOMAIN_KEYS: [&str; 6] = [
    "dns domain",
    "dns domains",
    "search domain",
    "search domains",
    "domain",
    "namespace",
];

/// What a name or an address is made of, with a port or a zone after it.
fn name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '%' | '/')
}

/// One run of name characters, hidden if it holds a host: the dots around
/// it kept, and a port after it. `domain`: the run stands where a system
/// lists DNS domains, so one word is a domain too.
fn hidden(run: &str, keep: &Keep, domain: bool) -> String {
    let lead = run.len() - run.trim_start_matches(['.', '/', '-']).len();
    let after = &run[lead..];
    // An address whole first: "::ffff:10.40.1.10", "fd40:2::".
    let whole = after.trim_end_matches('.');
    if normal(whole).is_some() {
        return if is_host(whole, keep) {
            format!("{}a host{}", &run[..lead], &after[whole.len()..])
        } else {
            run.to_owned()
        };
    }
    let body = after.trim_end_matches(['.', ':', '/', '-', '_']);
    let tail = &after[body.len()..];
    // A range, as nft writes one.
    if let Some((a, b)) = body.split_once('-')
        && normal(a).is_some()
        && normal(b).is_some()
    {
        let say = |x: &str| {
            if is_host(x, keep) {
                "a host".to_owned()
            } else {
                x.to_owned()
            }
        };
        return format!("{}{}-{}{tail}", &run[..lead], say(a), say(b));
    }
    let (host, port) = split_port(body);
    let one_word_domain = domain
        && !host.is_empty()
        && host.chars().any(char::is_alphabetic)
        && host
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        && !keep.words.contains(&host.to_lowercase());
    if one_word_domain || is_host(host, keep) {
        format!("{}a host{port}{tail}", &run[..lead])
    } else {
        run.to_owned()
    }
}

/// "wiki.acme.internal:443" → ("wiki.acme.internal", ":443"); an IPv6
/// address's own colons stay.
fn split_port(body: &str) -> (&str, &str) {
    match body.rsplit_once(':') {
        Some((host, port))
            if !host.contains(':')
                && !port.is_empty()
                && port.chars().all(|c| c.is_ascii_digit()) =>
        {
            (host, &body[host.len()..])
        }
        _ => (body, ""),
    }
}

fn is_host(core: &str, keep: &Keep) -> bool {
    let lower = core.to_lowercase();
    if keep.words.contains(&lower) {
        return false;
    }
    if keep.short.contains(&lower) {
        return true;
    }
    if let Some(n) = normal(core) {
        let addr: Option<IpAddr> = n.split('/').next().and_then(|a| a.parse().ok());
        return !(keep.kept.contains(&n) || addr.is_some_and(never_a_host));
    }
    is_name(core)
}

/// A name: dotted labels, the last one with a letter in it (not "3.5",
/// "0.10.4" or "e.g"). Not letters only: a company's own domain can end in
/// one with a digit ("wiki.nordbank.n1"), and one did, kept whole in a
/// report with hiding on (2026-10-19). Service records' labels start with
/// "_" ("_ldap._tcp.acme.internal"); names may be in any script
/// ("bücher.acme.internal"). Not a file ("craked.toml") or a reverse-DNS
/// identifier ("org.freedesktop.DBus.Error.AccessDenied"): a report's
/// errors would say nothing.
fn is_name(core: &str) -> bool {
    let labels: Vec<&str> = core.split('.').collect();
    let Some(last) = labels.last() else {
        return false;
    };
    labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        })
        && last.chars().count() >= 2
        && last.chars().all(char::is_alphanumeric)
        && last.chars().any(char::is_alphabetic)
        && !FILE_ENDINGS.contains(&last.to_lowercase().as_str())
        && !(labels.len() >= HOST_LABELS && ID_STARTS.contains(&labels[0]))
}

/// What a file's name ends in, never a domain's (no country or company
/// domain ends so: not "sh", "rs" or "md", which are).
const FILE_ENDINGS: [&str; 26] = [
    "toml", "json", "conf", "service", "socket", "sock", "log", "txt", "ovpn", "plist", "exe",
    "dll", "desktop", "svg", "png", "lock", "timer", "target", "pem", "crt", "p12", "pfx", "ps1",
    "yaml", "yml", "ini",
];

/// How a reverse-DNS identifier starts (D-Bus names, launchd labels).
const ID_STARTS: [&str; 4] = ["org", "net", "com", "software"];

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
    fn names_are_found_wherever_they_stand() {
        let keep = Keep::new(&["10.40.0.2".into()]);
        for (line, want) in [
            (
                "host=wiki.acme.internal tunnel=acme",
                "host=a host tunnel=acme",
            ),
            (
                "GET wiki.acme.internal:443 then 10.40.1.10:443",
                "GET a host:443 then a host:443",
            ),
            ("[fd40:1::10]:443 refused", "[a host]:443 refused"),
            (
                "Namespace : {.acme.internal, .nordbank.n1}",
                "Namespace : {.a host, .a host}",
            ),
            (
                "DNS Domain: ~acme.internal\twiki.acme.internal",
                "DNS Domain: ~a host\ta host",
            ),
            ("_ldap._tcp.acme.internal SRV", "a host SRV"),
            ("name=\"wiki.acme.internal\"", "name=\"a host\""),
            ("10.40.0.2 dev crake-acme", "10.40.0.2 dev crake-acme"),
            (
                "06:53:50 craked::manager: ACME connected",
                "06:53:50 craked::manager: ACME connected",
            ),
        ] {
            assert_eq!(scrub_keeping(line, &keep), want, "{line}");
        }
    }

    /// The audit of 2026-10-19: each system's own lines.
    #[test]
    fn what_each_system_writes_is_hidden() {
        let keep = Keep::new(&[
            "10.40.0.2".into(),
            "192.168.1.20".into(),
            "192.168.1.0/24".into(),
        ])
        .keeping_words(["ACME".to_owned()]);
        for (line, want) in [
            // OpenVPN's peer, in craked's own log.
            (
                "UDPv4 link remote: [AF_INET]203.0.113.5:1194",
                "UDPv4 link remote: [AF_INET]a host:1194",
            ),
            (
                "Peer Connection Initiated with [AF_INET6]2001:db8::5:1194",
                "Peer Connection Initiated with [AF_INET6]a host",
            ),
            // nft ranges, IPv4-mapped IPv6, a network ending in "::".
            (
                "elements = { 10.40.1.10-10.40.1.20 }",
                "elements = { a host-a host }",
            ),
            ("from ::ffff:10.40.1.10 port 443", "from a host port 443"),
            ("[::ffff:10.40.1.10]:443", "[a host]:443"),
            ("fd40:2:: via crake-acme", "a host via crake-acme"),
            // One-word domains where a system lists domains.
            (
                "DNS Domain: ~acme.internal ~nordbank ~.",
                "DNS Domain: ~a host ~a host ~.",
            ),
            ("search domain[1] : corp", "search domain[1] : a host"),
            (
                "Namespace : {.nordbank.example, wiki}",
                "Namespace : {.a host, a host}",
            ),
            // Names in any script.
            ("bücher.acme.internal answered", "a host answered"),
            // What stays: the tunnel's name, files, D-Bus names, plumbing.
            ("tunnel=acme ACME's DNS", "tunnel=acme ACME's DNS"),
            (
                "reading craked.toml and tunnels.json",
                "reading craked.toml and tunnels.json",
            ),
            (
                "org.freedesktop.DBus.Error.AccessDenied: no",
                "org.freedesktop.DBus.Error.AccessDenied: no",
            ),
            ("192.168.1.20 via 10.40.0.2", "192.168.1.20 via 10.40.0.2"),
        ] {
            assert_eq!(scrub_keeping(line, &keep), want, "{line}");
        }
    }

    #[test]
    fn a_tunnels_name_stays_when_a_host_starts_with_it() {
        let text = "Sent acmecorp.sharepoint.example through Acmecorp.";
        let keep = Keep::default()
            .keeping_words(["Acmecorp".to_owned()])
            .with_names_in(text);
        assert_eq!(scrub_keeping(text, &keep), "Sent a host through Acmecorp.");
    }

    #[test]
    fn a_hosts_short_name_goes_with_it() {
        let text = "Sent confluence.nordbank.n1 through Nordbank.\nOpened confluence, then git, via nordbank.n1.";
        let keep = Keep::default().with_names_in(text);
        assert_eq!(
            scrub_keeping(text, &keep),
            "Sent a host through Nordbank.\nOpened a host, then git, via a host."
        );
    }

    #[test]
    fn a_domain_ending_in_a_digit_is_a_name_too() {
        assert_eq!(
            scrub("Nordbank's names: wiki.nordbank.n1 is 10.60.1.4; a connection to it took 3 ms."),
            "Nordbank's names: a host is a host; a connection to it took 3 ms."
        );
        // Versions and numbers stay.
        for kept in [
            "Crake 0.11.0 on macOS 26.6.2",
            "tokio 1.53.3",
            "3.5 s",
            "e.g. this",
        ] {
            assert_eq!(scrub(kept), kept);
        }
    }

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
