//! Reading DNS answers that come back from a tunnel's DNS servers, spotting
//! public answers that say a name doesn't exist, and deciding which names
//! belong to a tunnel.

use std::net::Ipv4Addr;

use hickory_proto::op::{Message, MessageType, Query, ResponseCode};
use hickory_proto::rr::{DNSClass, RData, RecordType};

/// One DNS answer: the name asked for and the IPv4 addresses it resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// Lower case, no trailing dot.
    pub name: String,
    pub addrs: Vec<Ipv4Addr>,
}

/// An IPv4 packet carrying a UDP DNS response, as NFQUEUE hands it over.
pub fn parse_packet(packet: &[u8]) -> Option<Answer> {
    parse_message(dns_payload(packet)?)
}

/// The DNS message inside an IPv4 UDP packet from port 53.
pub fn dns_payload(packet: &[u8]) -> Option<&[u8]> {
    if packet.len() < 20 || packet[0] >> 4 != 4 || packet[9] != 17 {
        return None;
    }
    let ihl = usize::from(packet[0] & 0x0f) * 4;
    let udp = packet.get(ihl..)?;
    if udp.len() < 8 || u16::from_be_bytes([udp[0], udp[1]]) != 53 {
        return None;
    }
    Some(&udp[8..])
}

/// A DNS response message. Every A record in the answer section counts, so a
/// CNAME chain ends at the right address.
pub fn parse_message(payload: &[u8]) -> Option<Answer> {
    let msg = Message::from_vec(payload).ok()?;
    if msg.metadata.message_type != MessageType::Response
        || msg.metadata.response_code != ResponseCode::NoError
    {
        return None;
    }
    let name = normalize(&msg.queries.first()?.name.to_ascii());
    Some(Answer {
        name,
        addrs: a_records(&msg),
    })
}

pub fn a_records(msg: &Message) -> Vec<Ipv4Addr> {
    msg.answers
        .iter()
        .filter_map(|r| match &r.data {
            RData::A(a) => Some(a.0),
            _ => None,
        })
        .collect()
}

/// A public answer saying a name doesn't exist (NXDOMAIN), for an A or AAAA
/// question. Only these are worth asking a tunnel about.
#[derive(Debug, Clone)]
pub struct Failure {
    pub id: u16,
    /// Lower case, no trailing dot.
    pub name: String,
    pub qtype: RecordType,
    query: Query,
}

pub fn public_failure(payload: &[u8]) -> Option<Failure> {
    let msg = Message::from_vec(payload).ok()?;
    if msg.metadata.response_code != ResponseCode::NXDomain {
        return None;
    }
    question(&msg, &[RecordType::A, RecordType::AAAA])
}

/// A public answer with IPv4 addresses: the question, and what it said.
/// Names under an organisation's domain may have a private address too
/// (split-horizon DNS), which only the tunnel's DNS knows.
pub fn public_answer(payload: &[u8]) -> Option<(Failure, Vec<Ipv4Addr>)> {
    let msg = Message::from_vec(payload).ok()?;
    if msg.metadata.response_code != ResponseCode::NoError {
        return None;
    }
    let addrs = a_records(&msg);
    Some((question(&msg, &[RecordType::A])?, addrs))
}

/// The single question of a response, if it is one a tunnel could answer.
fn question(msg: &Message, types: &[RecordType]) -> Option<Failure> {
    if msg.metadata.message_type != MessageType::Response || msg.queries.len() != 1 {
        return None;
    }
    let query = msg.queries[0].clone();
    if !types.contains(&query.query_type) || query.query_class != DNSClass::IN {
        return None;
    }
    let name = normalize(&query.name.to_ascii());
    // Local and reverse names are never a tunnel's business; single labels
    // aren't sent to unicast DNS anyway.
    if !name.contains('.') || name.ends_with(".local") || name.ends_with(".arpa") {
        return None;
    }
    Some(Failure {
        id: msg.metadata.id,
        name,
        qtype: query.query_type,
        query,
    })
}

/// A recursive question for `name`, with EDNS (1232 bytes).
pub fn query(id: u16, name: &str, qtype: RecordType) -> Vec<u8> {
    let mut m = Vec::with_capacity(64);
    m.extend_from_slice(&id.to_be_bytes());
    m.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 1]);
    for label in name.split('.').filter(|l| !l.is_empty()) {
        let label = &label.as_bytes()[..label.len().min(63)];
        m.push(label.len() as u8);
        m.extend_from_slice(label);
    }
    m.push(0);
    m.extend_from_slice(&u16::from(qtype).to_be_bytes());
    m.extend_from_slice(&[0, 1]);
    m.extend_from_slice(&[0, 0, 41, 0x04, 0xd0, 0, 0, 0, 0, 0, 0]);
    m
}

/// A tunnel DNS server's reply to [`query`], if it is one.
pub fn parse_reply(bytes: &[u8], id: u16, name: &str) -> Option<Message> {
    let msg = Message::from_vec(bytes).ok()?;
    let ok = msg.metadata.id == id
        && msg.metadata.message_type == MessageType::Response
        && msg
            .queries
            .first()
            .is_some_and(|q| normalize(&q.name.to_ascii()) == name);
    ok.then_some(msg)
}

/// The tunnel's reply, dressed as the answer to the original public question.
pub fn rewrite(failure: &Failure, mut reply: Message) -> Option<Vec<u8>> {
    reply.metadata.id = failure.id;
    reply.queries = vec![failure.query.clone()];
    reply.to_vec().ok()
}

/// The same packet with a different DNS message: lengths and the IPv4 header
/// checksum fixed, the (optional) UDP checksum cleared.
pub fn replace_payload(packet: &[u8], dns: &[u8]) -> Option<Vec<u8>> {
    dns_payload(packet)?;
    let ihl = usize::from(packet[0] & 0x0f) * 4;
    let total = u16::try_from(ihl + 8 + dns.len()).ok()?;
    let mut out = Vec::with_capacity(usize::from(total));
    out.extend_from_slice(&packet[..ihl + 8]);
    out.extend_from_slice(dns);
    out[2..4].copy_from_slice(&total.to_be_bytes());
    out[10..12].copy_from_slice(&[0, 0]);
    let sum = ipv4_checksum(&out[..ihl]);
    out[10..12].copy_from_slice(&sum.to_be_bytes());
    out[ihl + 4..ihl + 6].copy_from_slice(&((8 + dns.len()) as u16).to_be_bytes());
    out[ihl + 6..ihl + 8].copy_from_slice(&[0, 0]);
    Some(out)
}

fn ipv4_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = header
        .chunks(2)
        .map(|c| u32::from(u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)])))
        .sum();
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// The organisation's own domain, from its gateway's name:
/// "vpn.acme.example" → "acme.example", "vpn.acme.co.uk" → "acme.co.uk". None for an
/// address.
pub fn organisation(gateway: &str) -> Option<String> {
    let host = gateway
        .split_once("://")
        .map_or(gateway, |(_, rest)| rest)
        .split(['/', ':'])
        .next()?;
    let host = normalize(host);
    if host.parse::<Ipv4Addr>().is_ok() || !valid_domain(&host) {
        return None;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    // "co.uk", "com.au", "ac.jp": a second level that belongs to the country.
    let country_sld = labels.len() >= 3
        && labels[labels.len() - 1].len() == 2
        && ["co", "com", "net", "org", "gov", "ac", "edu", "or", "ne"]
            .contains(&labels[labels.len() - 2]);
    let keep = if country_sld { 3 } else { 2 };
    Some(labels[labels.len() - keep..].join("."))
}

/// An address only reachable inside some network: RFC 1918 or carrier-grade NAT.
pub fn is_private(a: Ipv4Addr) -> bool {
    a.is_private() || (a.octets()[0] == 100 && (64..128).contains(&a.octets()[1]))
}

pub fn normalize(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// `git.acme.internal` is in `acme.internal`; `notacme.internal` is not.
pub fn in_domains(name: &str, domains: &[String]) -> bool {
    domains.iter().any(|d| {
        let d = normalize(d);
        !d.is_empty() && (name == d || name.ends_with(&format!(".{d}")))
    })
}

/// A domain we're willing to hand to systemd-resolved.
pub fn valid_domain(d: &str) -> bool {
    let d = normalize(d);
    !d.is_empty()
        && d.len() <= 253
        && d.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

#[cfg(test)]
pub(crate) mod tests {

    #[test]
    fn the_organisation_is_the_gateways_own_domain() {
        assert_eq!(organisation("vpn.acme.example").as_deref(), Some("acme.example"));
        assert_eq!(
            organisation("https://vpn.acme.co.uk/staff").as_deref(),
            Some("acme.co.uk")
        );
        assert_eq!(
            organisation("vpn.example.com:8443").as_deref(),
            Some("example.com")
        );
        assert_eq!(organisation("192.0.2.1"), None);
        assert!(is_private("10.40.9.30".parse().unwrap()));
        assert!(is_private("100.64.1.1".parse().unwrap()));
        assert!(!is_private("198.51.100.32".parse().unwrap()));
    }

    use super::*;

    /// A minimal response: one question, A records for it.
    pub fn response(name: &str, addrs: &[Ipv4Addr]) -> Vec<u8> {
        let mut m = vec![
            0x12,
            0x34,
            0x81,
            0x80,
            0,
            1,
            0,
            addrs.len() as u8,
            0,
            0,
            0,
            0,
        ];
        for label in name.split('.') {
            m.push(label.len() as u8);
            m.extend_from_slice(label.as_bytes());
        }
        m.extend_from_slice(&[0, 0, 1, 0, 1]);
        for a in addrs {
            m.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4]);
            m.extend_from_slice(&a.octets());
        }
        m
    }

    pub fn udp_packet(sport: u16, payload: &[u8]) -> Vec<u8> {
        let mut p = vec![
            0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0, 10, 40, 0, 2, 10, 40, 255, 7,
        ];
        p.extend_from_slice(&sport.to_be_bytes());
        p.extend_from_slice(&[0xd4, 0x31, 0, 0, 0, 0]);
        p.extend_from_slice(payload);
        p
    }

    fn nxdomain(name: &str) -> Vec<u8> {
        let mut m = response(name, &[]);
        m[3] = 0x83;
        m
    }

    #[test]
    fn reads_a_records() {
        let ip: Ipv4Addr = "10.33.7.7".parse().unwrap();
        let a = parse_packet(&udp_packet(53, &response("Git.Acme.Internal", &[ip]))).unwrap();
        assert_eq!(a.name, "git.acme.internal");
        assert_eq!(a.addrs, vec![ip]);
        assert!(parse_packet(&udp_packet(5353, &response("x.y", &[ip]))).is_none());
        assert!(parse_packet(b"garbage").is_none());
        assert!(parse_message(&nxdomain("nope.acme.internal")).is_none());
    }

    #[test]
    fn spots_public_failures_and_rewrites_them() {
        let f = public_failure(&nxdomain("db.example.cloud")).unwrap();
        assert_eq!(
            (f.id, f.name.as_str(), f.qtype),
            (0x1234, "db.example.cloud", RecordType::A)
        );
        // Successes, local and single-label names are not failures to rescue.
        assert!(public_failure(&response("db.example.cloud", &[])).is_none());
        assert!(public_failure(&nxdomain("printer.local")).is_none());
        assert!(public_failure(&nxdomain("intranet")).is_none());

        let ip: Ipv4Addr = "10.58.70.119".parse().unwrap();
        let mut tunnel = response("db.example.cloud", &[ip]);
        tunnel[0] = 0x77; // the tunnel query had its own ID
        tunnel[1] = 0x77;
        let reply = parse_reply(&tunnel, 0x7777, "db.example.cloud").unwrap();
        assert_eq!(a_records(&reply), vec![ip]);
        assert!(parse_reply(&tunnel, 0x1111, "db.example.cloud").is_none());
        assert!(parse_reply(&tunnel, 0x7777, "other.example.cloud").is_none());

        let dns = rewrite(&f, reply).unwrap();
        let packet = replace_payload(&udp_packet(53, &nxdomain("db.example.cloud")), &dns).unwrap();
        assert_eq!(parse_packet(&packet).unwrap().addrs, vec![ip]);
        assert_eq!(
            &packet[28..30],
            &[0x12, 0x34],
            "keeps the public question's ID"
        );
        assert_eq!(
            usize::from(u16::from_be_bytes([packet[2], packet[3]])),
            packet.len()
        );
        assert_eq!(
            ipv4_checksum(&packet[..20]),
            0,
            "valid IPv4 header checksum"
        );
    }

    #[test]
    fn builds_queries() {
        let q = query(0xbeef, "db.example.cloud", RecordType::A);
        let m = Message::from_vec(&q).unwrap();
        assert_eq!(m.metadata.id, 0xbeef);
        assert_eq!(m.queries[0].query_type, RecordType::A);
        assert_eq!(normalize(&m.queries[0].name.to_ascii()), "db.example.cloud");
    }

    #[test]
    fn domains() {
        let d = vec!["acme.internal".to_owned(), "corp.internal.".to_owned()];
        assert!(in_domains("git.acme.internal", &d));
        assert!(in_domains("acme.internal", &d));
        assert!(in_domains("git.corp.internal", &d));
        assert!(!in_domains("notacme.internal", &d));
        assert!(!in_domains("example.com", &d));
        assert!(valid_domain("corp.example"));
        assert!(!valid_domain("bad domain"));
        assert!(!valid_domain("a..b"));
    }
}
