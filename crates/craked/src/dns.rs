//! Reading DNS answers that come back from a tunnel's DNS servers, spotting
//! public answers that say a name doesn't exist, and deciding which names
//! belong to a tunnel.

use std::net::{IpAddr, Ipv4Addr};

use hickory_proto::op::{Message, MessageType, Query, ResponseCode};
use hickory_proto::rr::{DNSClass, RData, RecordType};

use crate::packet;

/// DNS's port.
pub const PORT: u16 = 53;
/// The fixed header every message starts with (RFC 1035 4.1.1).
pub const HEADER: usize = 12;
/// Big enough for any message read over UDP (EDNS asks for at most 1232).
pub const BUFFER: usize = 4096;
/// The header's fields after the ID (RFC 1035 4.1.1): a byte of flags (QR,
/// opcode, AA, TC, RD), a byte of codes (RA, Z, RCODE), then how many
/// questions, answers, authority and additional records.
pub const FLAGS: usize = 2;
pub const CODES: usize = 3;
pub const QUESTIONS: usize = 4;
pub const ANSWERS_LOW_BYTE: usize = 7;
pub const RECORD_COUNTS: std::ops::Range<usize> = 6..12;
/// In FLAGS: a response; what a reply keeps of a question's (opcode, RD).
pub const QR: u8 = 0x80;
pub const OPCODE_AND_RD: u8 = 0x79;
/// In CODES: recursion available; the response code.
pub const RA: u8 = 0x80;
pub const RCODE: u8 = 0x0f;
pub const REFUSED: u8 = 5;
/// After a question's name (ending in a zero byte): its type and class.
pub const QUESTION_FIELDS: usize = 4;
/// How an answer names the question's name: a pointer to it, right after
/// the header.
pub const NAME_POINTER: [u8; 2] = [0xc0, HEADER as u8];
/// Over TCP each message comes after its two-byte length (RFC 1035 4.2.2).
const TCP_LENGTH: usize = size_of::<u16>();
/// A name's limits (RFC 1035 2.3.4): each label, the whole name in text.
const MAX_LABEL: usize = 63;
const MAX_NAME: usize = 253;
/// EDNS (RFC 6891): an OPT record (type 41, root name) offering answers up to
/// 1232 bytes over UDP (the DNS flag day's size), no flags, no options: its
/// TTL's four bytes and RDLENGTH's two are all zero.
const EDNS_OPT: u16 = 41;
const EDNS_UDP_SIZE: u16 = 1232;
const EDNS_ZEROS: usize = 6;
/// Room for a typical question with its OPT record.
const QUERY_CAPACITY: usize = 64;
/// A registrable domain is two labels ("acme.example"), three under a
/// country's own second level ("acme.co.uk").
const REGISTRABLE: usize = 2;
const REGISTRABLE_UNDER_COUNTRY_SLD: usize = 3;
/// Carrier-grade NAT's shared addresses, 100.64.0.0/10 (RFC 6598).
const CARRIER_NAT: Ipv4Addr = Ipv4Addr::new(100, 64, 0, 0);
const CARRIER_NAT_BITS: u32 = 10;

/// One DNS answer: the name asked for and the addresses it resolved to
/// (IPv4 from A records, IPv6 from AAAA).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// Lower case, no trailing dot.
    pub name: String,
    pub addrs: Vec<IpAddr>,
}

/// An IPv4 packet carrying a UDP DNS response, as NFQUEUE hands it over.
/// A tunnel DNS server's answer: over UDP, or over TCP when one segment
/// carries the whole message.
pub fn parse_packet(packet: &[u8]) -> Option<Answer> {
    parse_message(dns_payload(packet).or_else(|| tcp_dns_payload(packet))?)
}

/// The DNS message in an IPv4 TCP segment from port 53, when the segment
/// carries exactly one whole message (its two-byte length first). A message
/// spread over several segments isn't read; probing still sees the
/// connection that follows.
pub fn tcp_dns_payload(packet: &[u8]) -> Option<&[u8]> {
    let tcp = packet::V4::parse(packet, packet::TCP)?.payload_to_total()?;
    if tcp.len() < packet::TCP_MIN || packet::source_port(tcp)? != PORT {
        return None;
    }
    let data = packet::tcp_data(tcp)?;
    let len = usize::from(u16::from_be_bytes([*data.first()?, *data.get(1)?]));
    (data.len() == TCP_LENGTH + len).then(|| &data[TCP_LENGTH..])
}

/// The DNS message inside an IPv4 UDP packet from port 53.
pub fn dns_payload(packet: &[u8]) -> Option<&[u8]> {
    let udp = packet::V4::parse(packet, packet::UDP)?.payload()?;
    if udp.len() < packet::UDP_HEADER || packet::source_port(udp)? != PORT {
        return None;
    }
    Some(&udp[packet::UDP_HEADER..])
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
        addrs: records(&msg),
    })
}

/// Every A and AAAA record in the answer, so a CNAME chain ends at the right
/// addresses whatever their family.
pub fn records(msg: &Message) -> Vec<IpAddr> {
    msg.answers
        .iter()
        .filter_map(|r| match &r.data {
            RData::A(a) => Some(IpAddr::V4(a.0)),
            RData::AAAA(a) => Some(IpAddr::V6(a.0)),
            _ => None,
        })
        .collect()
}

/// Only the IPv4 (A) records: the split-horizon and public-failure rescue work
/// on private IPv4 addresses.
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
    let mut m = Vec::with_capacity(QUERY_CAPACITY);
    m.extend_from_slice(&id.to_be_bytes());
    // Recursion desired; one question, one additional record (the OPT).
    m.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 1]);
    for label in name.split('.').filter(|l| !l.is_empty()) {
        let label = &label.as_bytes()[..label.len().min(MAX_LABEL)];
        m.push(label.len() as u8);
        m.extend_from_slice(label);
    }
    m.push(0);
    m.extend_from_slice(&u16::from(qtype).to_be_bytes());
    m.extend_from_slice(&u16::from(DNSClass::IN).to_be_bytes());
    m.push(0); // the OPT record's name: the root
    m.extend_from_slice(&EDNS_OPT.to_be_bytes());
    m.extend_from_slice(&EDNS_UDP_SIZE.to_be_bytes());
    m.extend_from_slice(&[0; EDNS_ZEROS]);
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
    packet::with_udp_data(packet::V4::parse(packet, packet::UDP)?, dns)
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
    if labels.len() < REGISTRABLE {
        return None;
    }
    // "co.uk", "com.au", "ac.jp": a second level that belongs to the country.
    let country_sld = labels.len() >= REGISTRABLE_UNDER_COUNTRY_SLD
        && labels[labels.len() - 1].len() == 2 // a country code
        && ["co", "com", "net", "org", "gov", "ac", "edu", "or", "ne"]
            .contains(&labels[labels.len() - 2]);
    let keep = if country_sld {
        REGISTRABLE_UNDER_COUNTRY_SLD
    } else {
        REGISTRABLE
    };
    Some(labels[labels.len() - keep..].join("."))
}

/// An address only reachable inside some network: RFC 1918 or carrier-grade NAT.
pub fn is_private(a: Ipv4Addr) -> bool {
    let shift = Ipv4Addr::BITS - CARRIER_NAT_BITS;
    a.is_private() || u32::from(a) >> shift == u32::from(CARRIER_NAT) >> shift
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
        && d.len() <= MAX_NAME
        && d.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= MAX_LABEL
                && l.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

#[cfg(test)]
pub(crate) mod tests {

    #[test]
    fn the_organisation_is_the_gateways_own_domain() {
        assert_eq!(
            organisation("vpn.acme.example").as_deref(),
            Some("acme.example")
        );
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

    /// A minimal response with AAAA records (type 28, 16-byte addresses).
    pub fn response_aaaa(name: &str, addrs: &[std::net::Ipv6Addr]) -> Vec<u8> {
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
        m.extend_from_slice(&[0, 0, 0x1c, 0, 1]); // qtype AAAA, class IN
        for a in addrs {
            m.extend_from_slice(&[0xc0, 0x0c, 0, 0x1c, 0, 1, 0, 0, 0, 60, 0, 16]);
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

    fn tcp_packet(sport: u16, data: &[u8]) -> Vec<u8> {
        let mut p = vec![
            0x45, 0, 0, 0, 0, 0, 0, 0, 64, 6, 0, 0, 10, 40, 0, 2, 10, 40, 255, 7,
        ];
        p.extend_from_slice(&sport.to_be_bytes());
        p.extend_from_slice(&[
            0xd4, 0x31, 0, 0, 0, 1, 0, 0, 0, 1, 0x50, 0x18, 0xff, 0xff, 0, 0, 0, 0,
        ]);
        p.extend_from_slice(data);
        let total = p.len() as u16;
        p[2..4].copy_from_slice(&total.to_be_bytes());
        p
    }

    #[test]
    fn reads_whole_answers_over_tcp() {
        let ip: Ipv4Addr = "10.33.7.7".parse().unwrap();
        let msg = response("git.acme.internal", &[ip]);
        let mut whole = (msg.len() as u16).to_be_bytes().to_vec();
        whole.extend_from_slice(&msg);
        let a = parse_packet(&tcp_packet(53, &whole)).unwrap();
        assert_eq!(
            (a.name.as_str(), a.addrs),
            ("git.acme.internal", vec![IpAddr::V4(ip)])
        );
        // Half a message (the rest in the next segment), another port, no data.
        assert!(parse_packet(&tcp_packet(53, &whole[..whole.len() - 4])).is_none());
        assert!(parse_packet(&tcp_packet(5353, &whole)).is_none());
        assert!(parse_packet(&tcp_packet(53, &[])).is_none());
        // Never taken for UDP, which the public-DNS rescue rewrites.
        assert!(dns_payload(&tcp_packet(53, &whole)).is_none());
    }

    #[test]
    fn reads_a_records() {
        let ip: Ipv4Addr = "10.33.7.7".parse().unwrap();
        let a = parse_packet(&udp_packet(53, &response("Git.Acme.Internal", &[ip]))).unwrap();
        assert_eq!(a.name, "git.acme.internal");
        assert_eq!(a.addrs, vec![IpAddr::V4(ip)]);
        assert!(parse_packet(&udp_packet(5353, &response("x.y", &[ip]))).is_none());
        assert!(parse_packet(b"garbage").is_none());
        assert!(parse_message(&nxdomain("nope.acme.internal")).is_none());
    }

    #[test]
    fn reads_aaaa_records() {
        let v6: std::net::Ipv6Addr = "fd00:dead:beef::7".parse().unwrap();
        let a = parse_packet(&udp_packet(53, &response_aaaa("git.acme.internal", &[v6]))).unwrap();
        assert_eq!(a.name, "git.acme.internal");
        assert_eq!(a.addrs, vec![IpAddr::V6(v6)]);
        // records() takes both families; a_records() only IPv4.
        let msg = Message::from_vec(&response_aaaa("x.acme.internal", &[v6])).unwrap();
        assert_eq!(records(&msg), vec![IpAddr::V6(v6)]);
        assert!(a_records(&msg).is_empty());
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
        assert_eq!(parse_packet(&packet).unwrap().addrs, vec![IpAddr::V4(ip)]);
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
            crate::packet::checksum(&packet[..packet::IPV4_MIN]),
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
