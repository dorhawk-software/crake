//! An IPsec gateway's certificate, when charon can't trust it
//! (design/proposals/ipsec-certificates.html, 5; the spike in
//! scripts/spikes/ipsec-certificate): read out of charon's decryption dump
//! ("enc 3", `from_dump`), its names and dates read here (`parse`) for the
//! trust question and the wrong-name failure, and its key's pin, the
//! fingerprint AnyConnect's question shows too. What's read here only
//! informs the user: once trusted, the certificate is handed to charon,
//! which checks it again, key and name, on every connection.

use base64::Engine as _;
use sha2::{Digest as _, Sha256};

/// A certificate's DER tags this reader needs (X.690).
const SEQUENCE: u8 = 0x30;
const SET: u8 = 0x31;
const OID: u8 = 0x06;
const OCTET_STRING: u8 = 0x04;
const UTC_TIME: u8 = 0x17;
const GENERALIZED_TIME: u8 = 0x18;
/// `[0]` (the version) and `[3]` (the extensions) in tbsCertificate.
const VERSION: u8 = 0xA0;
const EXTENSIONS: u8 = 0xA3;
/// Where tbsCertificate keeps the key, after its version.
const SPKI: usize = 5;
/// A subjectAltName's dNSName (`[2]`) and iPAddress (`[7]`).
const DNS_NAME: u8 = 0x82;
const IP_ADDRESS: u8 = 0x87;
/// The DER of the object identifiers for commonName, organizationName
/// and subjectAltName.
const COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];
const ORGANIZATION: &[u8] = &[0x55, 0x04, 0x0A];
const SUBJECT_ALT_NAME: &[u8] = &[0x55, 0x1D, 0x11];
/// A long form length's flag, and how many bytes of length it may say.
const LONG_FORM: u8 = 0x80;
const MAX_LENGTH_BYTES: usize = 4;
/// IPv4 and IPv6 addresses' sizes in an iPAddress.
const IPV4: usize = 4;
const IPV6: usize = 16;
/// UTCTime's years: 50 to 99 are 1950 to 1999 (RFC 5280, 4.1.2.5.1).
const UTC_PIVOT: i32 = 50;
/// A GeneralizedTime's year's digits, and a month's and day's together.
const YEAR: usize = 4;
const MONTH_DAY: usize = 4;
const CENTURY: i32 = 100;
const NINETEEN_HUNDRED: i32 = 1900;
const TWO_THOUSAND: i32 = 2000;

/// A certificate, as far as the trust question and the checks need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    pub der: Vec<u8>,
    /// Its subject's common name, else the whole subject's first name.
    pub subject: String,
    /// Who signed it: the issuer's common name, else its organisation.
    pub issuer: String,
    /// The names it's for: its subjectAltName's DNS names and addresses,
    /// else its common name (RFC 6125 falls back to it).
    pub names: Vec<String>,
    pub not_before: time::Date,
    pub not_after: time::Date,
    /// Its SubjectPublicKeyInfo, whole (the pin is its SHA-256).
    spki: Vec<u8>,
}

impl Certificate {
    /// "pin-sha256:…", as openconnect writes a certificate's key.
    pub fn pin(&self) -> String {
        format!(
            "pin-sha256:{}",
            base64::engine::general_purpose::STANDARD.encode(Sha256::digest(&self.spki))
        )
    }

    /// It's for `id` (a name or an address): one of its names, a
    /// wildcard's one label included ("*.acme.example" is for
    /// "vpn.acme.example", not for "acme.example").
    pub fn is_for(&self, id: &str) -> bool {
        let id = id.trim_end_matches('.').to_ascii_lowercase();
        self.names.iter().any(|n| {
            let n = n.to_ascii_lowercase();
            match n.strip_prefix("*.") {
                Some(base) => id
                    .split_once('.')
                    .is_some_and(|(label, rest)| !label.is_empty() && rest == base),
                None => n == id,
            }
        })
    }

    /// The names it's for, for the wrong-name failure ("vpn.globex.example").
    pub fn names_said(&self) -> String {
        self.names
            .first()
            .cloned()
            .unwrap_or_else(|| self.subject.clone())
    }
}

/// `der` as PEM, as charon takes a certificate over vici.
pub fn pem(der: &[u8]) -> String {
    /// PEM's line length (RFC 7468).
    const LINE: usize = 64;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    for line in b64.as_bytes().chunks(LINE) {
        out.push_str(&String::from_utf8_lossy(line));
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}

/// A DER element: its tag and contents, and what follows it.
fn element(der: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = der.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first & LONG_FORM == 0 {
        (usize::from(first), rest)
    } else {
        let n = usize::from(first & !LONG_FORM);
        if n == 0 || n > MAX_LENGTH_BYTES || rest.len() < n {
            return None;
        }
        let len = rest[..n]
            .iter()
            .fold(0usize, |l, &b| (l << u8::BITS) | usize::from(b));
        (len, &rest[n..])
    };
    if rest.len() < len {
        return None;
    }
    Some((tag, &rest[..len], &rest[len..]))
}

/// The elements inside `der`, in order.
fn elements(der: &[u8]) -> Vec<(u8, &[u8])> {
    whole_elements(der)
        .into_iter()
        .map(|(t, b, _)| (t, b))
        .collect()
}

/// The elements inside `der`, each with its whole encoding too.
fn whole_elements(mut der: &[u8]) -> Vec<(u8, &[u8], &[u8])> {
    let mut out = Vec::new();
    while let Some((tag, body, rest)) = element(der) {
        out.push((tag, body, &der[..der.len() - rest.len()]));
        der = rest;
    }
    out
}

/// A Name's attribute of type `oid`, as text.
fn attribute(name: &[u8], oid: &[u8]) -> Option<String> {
    elements(name)
        .into_iter()
        .filter(|(t, _)| *t == SET)
        .flat_map(|(_, set)| elements(set))
        .filter(|(t, _)| *t == SEQUENCE)
        .find_map(|(_, atv)| match elements(atv).as_slice() {
            [(OID, o), (_, value), ..] if *o == oid => {
                Some(String::from_utf8_lossy(value).into_owned())
            }
            _ => None,
        })
}

/// A UTCTime or GeneralizedTime's day.
fn date(tag: u8, body: &[u8]) -> Option<time::Date> {
    let text = std::str::from_utf8(body).ok()?;
    let num = |s: &str| s.parse::<i32>().ok();
    let (year, rest) = match tag {
        UTC_TIME => {
            let yy = num(text.get(..2)?)?;
            let century = if yy >= UTC_PIVOT {
                NINETEEN_HUNDRED
            } else {
                TWO_THOUSAND
            };
            (century + yy % CENTURY, text.get(2..)?)
        }
        GENERALIZED_TIME => (num(text.get(..YEAR)?)?, text.get(YEAR..)?),
        _ => return None,
    };
    let month = u8::try_from(num(rest.get(..2)?)?).ok()?;
    let day = u8::try_from(num(rest.get(2..MONTH_DAY)?)?).ok()?;
    time::Date::from_calendar_date(year, time::Month::try_from(month).ok()?, day).ok()
}

/// The names a subjectAltName extension's value lists.
fn alt_names(value: &[u8]) -> Vec<String> {
    let Some((SEQUENCE, names, _)) = element(value) else {
        return Vec::new();
    };
    elements(names)
        .into_iter()
        .filter_map(|(tag, body)| match (tag, body.len()) {
            (DNS_NAME, _) => Some(String::from_utf8_lossy(body).into_owned()),
            (IP_ADDRESS, IPV4) => {
                Some(std::net::Ipv4Addr::from(<[u8; IPV4]>::try_from(body).ok()?).to_string())
            }
            (IP_ADDRESS, IPV6) => {
                Some(std::net::Ipv6Addr::from(<[u8; IPV6]>::try_from(body).ok()?).to_string())
            }
            _ => None,
        })
        .collect()
}

/// The certificate in `der` (X.509, RFC 5280), or None when it isn't one.
pub fn parse(der: &[u8]) -> Option<Certificate> {
    let (SEQUENCE, cert, _) = element(der)? else {
        return None;
    };
    let (SEQUENCE, tbs, _) = element(cert)? else {
        return None;
    };
    let mut whole = whole_elements(tbs);
    if whole.first().is_some_and(|(t, _, _)| *t == VERSION) {
        whole.remove(0);
    }
    let parts: Vec<(u8, &[u8])> = whole.iter().map(|(t, b, _)| (*t, *b)).collect();
    // serialNumber, signature, issuer, validity, subject, its key (kept
    // whole: the pin is of all of it), then the optional unique IDs and
    // extensions.
    let [
        _,
        _,
        (SEQUENCE, issuer),
        (SEQUENCE, validity),
        (SEQUENCE, subject),
        (SEQUENCE, _),
        ..,
    ] = parts.as_slice()
    else {
        return None;
    };
    let spki = whole.get(SPKI)?.2.to_vec();
    let validity = elements(validity);
    let [(t1, b1), (t2, b2)] = validity.as_slice() else {
        return None;
    };
    let names_from_san = parts
        .iter()
        .filter(|(t, _)| *t == EXTENSIONS)
        .flat_map(|(_, ext)| match element(ext) {
            Some((SEQUENCE, list, _)) => elements(list),
            _ => Vec::new(),
        })
        .filter(|(t, _)| *t == SEQUENCE)
        .find_map(|(_, e)| {
            let fields = elements(e);
            let (OID, oid) = *fields.first()? else {
                return None;
            };
            if oid != SUBJECT_ALT_NAME {
                return None;
            }
            fields
                .iter()
                .skip(1)
                .find(|(t, _)| *t == OCTET_STRING)
                .map(|(_, v)| alt_names(v))
        })
        .unwrap_or_default();
    let common = attribute(subject, COMMON_NAME);
    let names = if names_from_san.is_empty() {
        common.clone().into_iter().collect()
    } else {
        names_from_san
    };
    Some(Certificate {
        der: der.to_vec(),
        subject: common
            .or_else(|| attribute(subject, ORGANIZATION))
            .unwrap_or_default(),
        issuer: attribute(issuer, COMMON_NAME)
            .or_else(|| attribute(issuer, ORGANIZATION))
            .unwrap_or_default(),
        names,
        not_before: date(*t1, b1)?,
        not_after: date(*t2, b2)?,
        spki,
    })
}

/// IKEv2's numbers this needs (RFC 7296, 7383): the IKE_AUTH exchange,
/// a response's flag, the payloads' types, and X.509's certificate
/// encoding.
const IKE_AUTH: u8 = 35;
const RESPONSE: u8 = 0x20;
const ENCRYPTED_FRAGMENT: u8 = 53;
const CERT: u8 = 37;
const X509_SIGNATURE: u8 = 4;
/// Where an IKE header keeps its next payload, exchange, flags and
/// message ID, and how long it is with the encrypted payload's header
/// (a fragment's has its number and how many there are after it).
const NEXT_PAYLOAD: usize = 16;
const EXCHANGE: usize = 18;
const FLAGS: usize = 19;
const MESSAGE_ID: std::ops::Range<usize> = 20..24;
const IKE_HEADER: usize = 28;
const FRAGMENT_NUMBER: std::ops::Range<usize> = 32..34;
const GENERIC_HEADER: usize = 4;
/// How many bytes a dump's row has at most, written in hex.
const ROW: usize = 16;
const HEX: u32 = 16;
/// Where a payload's generic header keeps its length.
const PAYLOAD_LENGTH: std::ops::Range<usize> = 2..4;

/// A chunk dump (strongSwan's `%B`) in charon's file log: its label and
/// bytes ("plain => 1295 bytes @ 0x7f…", then rows of 16 bytes).
fn dumps(log: &str, ike: &str) -> Vec<(String, Vec<u8>)> {
    let mine = format!("<{ike}|");
    let mut out: Vec<(String, Vec<u8>, usize)> = Vec::new();
    let mut open = false;
    for line in log.lines() {
        // charon's prefix ("05[ENC] <acme|1> "): this connection's only.
        let Some(at) = line.find(&mine) else {
            open = false;
            continue;
        };
        let Some(text) = line[at..].split_once("> ").map(|(_, t)| t) else {
            continue;
        };
        if let Some((label, size)) = text.split_once(" => ") {
            let bytes = size
                .split_once(" bytes @ ")
                .and_then(|(n, _)| n.parse().ok());
            if let Some(n) = bytes {
                out.push((label.trim().to_owned(), Vec::new(), n));
                open = true;
                continue;
            }
        }
        let Some(last) = out.last_mut().filter(|_| open) else {
            continue;
        };
        // "   16: 2E 65 78 61 …  .example'..": the hex between ": " and "  ".
        let row = text
            .trim_start()
            .split_once(": ")
            .map(|(offset, hex)| (offset.parse::<usize>().ok(), hex));
        match row {
            Some((Some(_), hex)) if last.1.len() < last.2 => {
                let hex = hex.split("  ").next().unwrap_or("");
                let bytes: Option<Vec<u8>> = hex
                    .split_whitespace()
                    .take(ROW)
                    .map(|b| u8::from_str_radix(b, HEX).ok())
                    .collect();
                match bytes {
                    Some(b) => last.1.extend(b),
                    None => open = false,
                }
            }
            _ => open = false,
        }
    }
    out.into_iter()
        .filter(|(_, b, n)| b.len() == *n)
        .map(|(l, b, _)| (l, b))
        .collect()
}

/// The certificate the gateway sent in its IKE_AUTH reply to the
/// connection `ike`, out of charon's file log at "enc 3": each decrypted
/// part ("plain") follows the header it was decrypted with ("assoc"),
/// fragments are put back in order, and the payloads walked to the first
/// X.509 certificate.
pub fn from_dump(log: &str, ike: &str) -> Option<Vec<u8>> {
    let mut parts: Vec<(u16, u8, Vec<u8>)> = Vec::new();
    let mut header: Option<Vec<u8>> = None;
    let mut message: Option<Vec<u8>> = None;
    for (label, bytes) in dumps(log, ike) {
        match label.as_str() {
            "assoc" => header = Some(bytes),
            "plain" => {
                let Some(h) = header
                    .take()
                    .filter(|h| h.len() >= IKE_HEADER + GENERIC_HEADER)
                else {
                    continue;
                };
                if h[EXCHANGE] != IKE_AUTH || h[FLAGS] & RESPONSE == 0 {
                    continue;
                }
                let id = h[MESSAGE_ID].to_vec();
                if message.as_ref().is_some_and(|m| *m != id) {
                    continue;
                }
                message = Some(id);
                let number = if h[NEXT_PAYLOAD] == ENCRYPTED_FRAGMENT {
                    let n = h.get(FRAGMENT_NUMBER)?;
                    u16::from_be_bytes([n[0], n[1]])
                } else {
                    1
                };
                parts.push((number, h[IKE_HEADER], bytes));
            }
            _ => {}
        }
    }
    parts.sort_by_key(|p| p.0);
    let first = parts.iter().find(|p| p.0 == 1)?.1;
    let payloads: Vec<u8> = parts.into_iter().flat_map(|p| p.2).collect();
    let (mut kind, mut at) = (first, 0);
    while kind != 0 && at + GENERIC_HEADER <= payloads.len() {
        let next = payloads[at];
        let at_len = at + PAYLOAD_LENGTH.start..at + PAYLOAD_LENGTH.end;
        let len = usize::from(u16::from_be_bytes([
            payloads[at_len.start],
            payloads[at_len.start + 1],
        ]));
        if len < GENERIC_HEADER || at + len > payloads.len() {
            return None;
        }
        if kind == CERT && payloads.get(at + GENERIC_HEADER) == Some(&X509_SIGNATURE) {
            return Some(payloads[at + GENERIC_HEADER + 1..at + len].to_vec());
        }
        kind = next;
        at += len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// vpn.acme.example's certificate (EC P-256), signed by "ACME Root CA"
    /// (O=ACME), for vpn.acme.example, *.vpn.acme.example and
    /// 198.51.100.10, from 1 October 2026 to 1 March 2028; made with
    /// openssl for this test.
    const CERT_B64: &str = "MIIBvDCCAWKgAwIBAgIUeUPjZ+Z600Q0qoepdJrs5KH8ynUwCgYIKoZIzj0EAwIwJjENMAsGA1UECgwEQUNNRTEVMBMGA1UEAwwMQUNNRSBSb290IENBMB4XDTI2MTAwMTAwMDAwMFoXDTI4MDMwMTAwMDAwMFowGzEZMBcGA1UEAwwQdnBuLmFjbWUuZXhhbXBsZTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABCJszzRmFAePi7ejTrjDl3Glipv7MJ68cLdoK+LDyHjXVfJH/wUna/pBnYEs4SyHC2R0KT9o/AwfkDZAVba95lKjeTB3MDUGA1UdEQQuMCyCEHZwbi5hY21lLmV4YW1wbGWCEioudnBuLmFjbWUuZXhhbXBsZYcExjNkCjAdBgNVHQ4EFgQU7r4Lzun41PSCTv3Q14dwN6Um/W0wHwYDVR0jBBgwFoAUAFMwZr1KO68IJahUx/4zoaTpGfQwCgYIKoZIzj0EAwIDSAAwRQIhALdKv2vg8/SsIBuV2PwkFPnfbqDKIeHnwnDkLgQ6/n5WAiASDWc5sAB8ec6zThbHp2h90NQP4HoRHJdM3abyK//jqA==";
    /// Its key's pin, as `openssl x509 -pubkey | openssl dgst -sha256` gives it.
    const PIN: &str = "pin-sha256:B7zmbn3mFMUvAEaYNWwJ69hr7NCqwogSqfTWud715WA=";

    fn der() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(CERT_B64)
            .unwrap()
    }

    #[test]
    fn a_certificate_says_who_it_is_for_and_who_signed_it() {
        let c = parse(&der()).unwrap();
        assert_eq!(c.subject, "vpn.acme.example");
        assert_eq!(c.issuer, "ACME Root CA");
        assert_eq!(
            c.names,
            ["vpn.acme.example", "*.vpn.acme.example", "198.51.100.10"]
        );
        assert_eq!(
            c.not_before,
            time::Date::from_calendar_date(2026, time::Month::October, 1).unwrap()
        );
        assert_eq!(
            c.not_after,
            time::Date::from_calendar_date(2028, time::Month::March, 1).unwrap()
        );
        assert_eq!(c.pin(), PIN);
        assert_eq!(c.names_said(), "vpn.acme.example");
    }

    #[test]
    fn a_name_is_matched_as_rfc_6125_says() {
        let c = parse(&der()).unwrap();
        assert!(c.is_for("vpn.acme.example"));
        assert!(c.is_for("VPN.ACME.example."));
        assert!(c.is_for("gw.vpn.acme.example"), "the wildcard's one label");
        assert!(!c.is_for("a.b.vpn.acme.example"), "not two labels");
        assert!(!c.is_for("acme.example"));
        assert!(c.is_for("198.51.100.10"));
        assert!(!c.is_for("vpn.globex.example"));
    }

    #[test]
    fn what_isnt_a_certificate_is_none() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[SEQUENCE, 0x05, 1, 2]), None);
        let mut cut = der();
        cut.truncate(cut.len() / 2);
        assert_eq!(parse(&cut), None);
        assert_eq!(
            element(&[SEQUENCE, 0x85, 1, 1, 1, 1, 1]),
            None,
            "too long a length"
        );
    }

    #[test]
    fn pem_is_der_in_lines_of_64() {
        let p = pem(&der());
        let lines: Vec<&str> = p.lines().collect();
        assert_eq!(lines[0], "-----BEGIN CERTIFICATE-----");
        assert_eq!(*lines.last().unwrap(), "-----END CERTIFICATE-----");
        assert!(lines[1..lines.len() - 1].iter().all(|l| l.len() <= 64));
        assert_eq!(lines[1..lines.len() - 1].concat(), CERT_B64);
    }

    #[test]
    fn utc_years_turn_at_fifty() {
        assert_eq!(date(UTC_TIME, b"491231000000Z").unwrap().year(), 2049);
        assert_eq!(date(UTC_TIME, b"500101000000Z").unwrap().year(), 1950);
        assert_eq!(
            date(GENERALIZED_TIME, b"20500101000000Z").unwrap().year(),
            2050
        );
        assert_eq!(date(OID, b"20500101"), None);
    }

    /// `bytes` as charon's file log dumps them for the connection `ike`.
    fn dump(ike: &str, label: &str, bytes: &[u8]) -> String {
        let mut s = format!(
            "05[ENC] <{ike}|1> {label} => {} bytes @ 0x7f00\n",
            bytes.len()
        );
        for (i, row) in bytes.chunks(ROW).enumerate() {
            let hex: Vec<String> = row.iter().map(|b| format!("{b:02X}")).collect();
            s += &format!("05[ENC] <{ike}|1> {:4}: {}  ....\n", i * ROW, hex.join(" "));
        }
        s
    }

    /// An IKE_AUTH reply's header, as "assoc" dumps it, for fragment `n`.
    fn header(n: Option<u16>, first: u8) -> Vec<u8> {
        let mut h = vec![0; IKE_HEADER];
        h[EXCHANGE] = IKE_AUTH;
        h[FLAGS] = RESPONSE;
        h[MESSAGE_ID].copy_from_slice(&[0, 0, 0, 1]);
        h[NEXT_PAYLOAD] = if n.is_some() { ENCRYPTED_FRAGMENT } else { 46 };
        h.extend([first, 0, 0, 0]);
        if let Some(n) = n {
            h.extend(n.to_be_bytes());
            h.extend(2u16.to_be_bytes());
        }
        h
    }

    /// A payload: its next payload's type, then its body.
    fn payload(next: u8, body: &[u8]) -> Vec<u8> {
        let len = u16::try_from(body.len() + GENERIC_HEADER).unwrap();
        let mut p = vec![next, 0];
        p.extend(len.to_be_bytes());
        p.extend(body);
        p
    }

    #[test]
    fn the_gateways_certificate_is_read_out_of_the_dump() {
        let cert = der();
        let mut cert_body = vec![X509_SIGNATURE];
        cert_body.extend(&cert);
        // IDr, then CERT, then AUTH: in two fragments, the second first.
        let mut plain = payload(CERT, b"\x02\0\0\0vpn.acme.example");
        plain.extend(payload(39, &cert_body));
        plain.extend(payload(0, b"\x0e\0\0\0auth"));
        let (one, two) = plain.split_at(plain.len() / 2);
        let mut log = String::from("05[IKE] <acme|1> received end entity cert\n");
        log += &dump("acme", "assoc", &header(Some(2), 0));
        log += &dump("acme", "plain", two);
        log += &dump("globex", "assoc", &header(Some(1), 36));
        log += &dump("globex", "plain", b"another connection's");
        log += &dump("acme", "assoc", &header(Some(1), 36));
        log += &dump("acme", "plain", one);
        assert_eq!(from_dump(&log, "acme"), Some(cert.clone()));
        assert_eq!(from_dump(&log, "initech"), None);
        // Whole, not in fragments.
        let mut log = dump("acme", "assoc", &header(None, 36));
        log += &dump("acme", "plain", &plain);
        assert_eq!(from_dump(&log, "acme"), Some(cert));
        // A request isn't the gateway's.
        let mut h = header(None, 36);
        h[FLAGS] = 0;
        let mut log = dump("acme", "assoc", &h);
        log += &dump("acme", "plain", &plain);
        assert_eq!(from_dump(&log, "acme"), None);
    }

    #[test]
    fn a_dump_cut_short_is_left_out() {
        let mut log = dump("acme", "plain", &[1; 40]);
        log = log.lines().take(2).collect::<Vec<_>>().join("\n");
        assert!(dumps(&log, "acme").is_empty());
    }
}
