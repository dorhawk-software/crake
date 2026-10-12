//! New tunnel's "Its certificate is valid until March 2027.": the gateway's
//! certificate, read in a TLS handshake that checks nothing and sends
//! nothing after it (sign-in decides whether to trust the gateway), and the
//! one date wanted from it, its validity's "not after" (RFC 5280).

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

/// HTTPS, unless the gateway's address says otherwise.
const HTTPS: u16 = 443;

/// When the certificate `gateway` shows stops being valid, if it shows one
/// within `wait`.
pub fn expiry(gateway: &str, wait: Duration) -> Option<jiff::Timestamp> {
    let (host, port) = host_and_port(gateway)?;
    let addr = (host.as_str(), port).to_socket_addrs().ok()?.next()?;
    let mut tcp = TcpStream::connect_timeout(&addr, wait).ok()?;
    tcp.set_read_timeout(Some(wait)).ok()?;
    tcp.set_write_timeout(Some(wait)).ok()?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .ok()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(LookOnly(provider)))
        .with_no_client_auth();
    let name = ServerName::try_from(host).ok()?;
    let mut tls = rustls::ClientConnection::new(Arc::new(config), name).ok()?;
    while tls.is_handshaking() {
        tls.complete_io(&mut tcp).ok()?;
    }
    let cert = tls.peer_certificates()?.first()?.clone();
    let _ = tcp.flush();
    let _ = tcp.read(&mut []);
    not_after(&cert)
}

/// `vpn.acme.example`, `https://vpn.acme.example:8443/employees`.
fn host_and_port(gateway: &str) -> Option<(String, u16)> {
    let rest = gateway
        .trim()
        .split_once("://")
        .map_or(gateway.trim(), |(_, r)| r);
    let authority = rest.split('/').next()?;
    if authority.is_empty() {
        return None;
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Some((host.to_owned(), port.parse().ok()?)),
        _ => Some((authority.to_owned(), HTTPS)),
    }
}

/// Accepts whatever the gateway shows: only its dates are read here.
#[derive(Debug)]
struct LookOnly(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for LookOnly {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// DER's tags as read here.
const SEQUENCE: u8 = 0x30;
/// tbsCertificate's optional `[0] version`.
const EXPLICIT_VERSION: u8 = 0xA0;
const UTC_TIME: u8 = 0x17;
const GENERALIZED_TIME: u8 = 0x18;
/// A long DER length: the low bits say how many bytes follow.
const LONG_LENGTH: u8 = 0x80;
const LENGTH_BYTES: u8 = 0x7F;
/// tbsCertificate's fields before its validity: serialNumber, signature,
/// issuer.
const BEFORE_VALIDITY: usize = 3;
/// GeneralizedTime's four-digit year; then MMDDHHMMSS, two digits each.
const YEAR_DIGITS: usize = 4;
const MONTH: usize = 0;
const DAY: usize = 2;
const HOUR: usize = 4;
const MINUTE: usize = 6;
const SECOND: usize = 8;
/// UTCTime's two-digit years: 50 and up are 19xx, the rest 20xx.
const UTC_PIVOT: i16 = 50;
const CENTURY_19: i16 = 1900;
const CENTURY_20: i16 = 2000;

/// The certificate's validity's "not after": Certificate ::= SEQUENCE {
/// tbsCertificate SEQUENCE { [0] version?, serialNumber, signature,
/// issuer, validity SEQUENCE { notBefore, notAfter }, ... }, ... }.
pub fn not_after(cert: &[u8]) -> Option<jiff::Timestamp> {
    let (tag, certificate, _) = element(cert)?;
    let (tbs_tag, tbs, _) = element(certificate).filter(|_| tag == SEQUENCE)?;
    if tbs_tag != SEQUENCE {
        return None;
    }
    let mut rest = tbs;
    let (first, _, after) = element(rest)?;
    if first == EXPLICIT_VERSION {
        rest = after;
    }
    // serialNumber, signature, issuer.
    for _ in 0..BEFORE_VALIDITY {
        rest = element(rest)?.2;
    }
    let (validity_tag, validity, _) = element(rest)?;
    if validity_tag != SEQUENCE {
        return None;
    }
    let after_not_before = element(validity)?.2;
    let (time_tag, time, _) = element(after_not_before)?;
    time_of(time_tag, std::str::from_utf8(time).ok()?)
}

/// One DER element: (tag, contents, what follows it).
fn element(der: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *der.first()?;
    let first = *der.get(1)?;
    let (len, header) = if first & LONG_LENGTH == 0 {
        (usize::from(first), 2)
    } else {
        let n = usize::from(first & LENGTH_BYTES);
        let bytes = der.get(2..2 + n)?;
        let len = bytes.iter().try_fold(0usize, |acc, b| {
            acc.checked_mul(usize::from(u8::MAX) + 1)?
                .checked_add(usize::from(*b))
        })?;
        (len, 2 + n)
    };
    let contents = der.get(header..header.checked_add(len)?)?;
    Some((tag, contents, &der[header + len..]))
}

/// UTCTime `YYMMDDHHMMSSZ` or GeneralizedTime `YYYYMMDDHHMMSSZ`.
fn time_of(tag: u8, text: &str) -> Option<jiff::Timestamp> {
    let text = text.strip_suffix('Z')?;
    let (year, rest) = match tag {
        UTC_TIME => {
            let yy: i16 = text.get(..2)?.parse().ok()?;
            let century = if yy >= UTC_PIVOT {
                CENTURY_19
            } else {
                CENTURY_20
            };
            (century + yy, text.get(2..)?)
        }
        GENERALIZED_TIME => (
            text.get(..YEAR_DIGITS)?.parse().ok()?,
            text.get(YEAR_DIGITS..)?,
        ),
        _ => return None,
    };
    let two = |i: usize| -> Option<i8> { rest.get(i..i + 2)?.parse().ok() };
    let civil = jiff::civil::date(year, two(MONTH)?, two(DAY)?).at(
        two(HOUR)?,
        two(MINUTE)?,
        two(SECOND)?,
        0,
    );
    civil
        .to_zoned(jiff::tz::TimeZone::UTC)
        .ok()
        .map(|z| z.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_where_the_gateway_is() {
        assert_eq!(
            host_and_port("vpn.acme.example"),
            Some(("vpn.acme.example".into(), 443))
        );
        assert_eq!(
            host_and_port("https://vpn.acme.example:8443/employees"),
            Some(("vpn.acme.example".into(), 8443))
        );
        assert_eq!(host_and_port(""), None);
    }

    #[test]
    fn reads_the_end_of_a_certificates_validity() {
        // tbsCertificate with a version, a serial, an algorithm, an issuer
        // and a validity of UTCTime 260101000000Z to GeneralizedTime
        // 20270315120000Z (not a whole certificate: only what's read).
        let mut validity = vec![UTC_TIME, 13];
        validity.extend_from_slice(b"260101000000Z");
        validity.extend_from_slice(&[GENERALIZED_TIME, 15]);
        validity.extend_from_slice(b"20270315120000Z");
        let mut tbs = vec![EXPLICIT_VERSION, 3, 0x02, 1, 2]; // [0] { INTEGER 2 }
        tbs.extend_from_slice(&[0x02, 1, 7]); // serialNumber
        tbs.extend_from_slice(&[SEQUENCE, 2, 0x05, 0]); // signature { NULL }
        tbs.extend_from_slice(&[SEQUENCE, 0]); // issuer
        tbs.push(SEQUENCE);
        tbs.push(validity.len() as u8);
        tbs.extend_from_slice(&validity);
        let mut cert = vec![SEQUENCE, LONG_LENGTH | 1, (tbs.len() + 2) as u8, SEQUENCE];
        cert.push(tbs.len() as u8);
        cert.extend_from_slice(&tbs);
        assert_eq!(
            not_after(&cert),
            Some("2027-03-15T12:00:00Z".parse().unwrap())
        );
        assert_eq!(not_after(b"\x30\x05junk"), None);
        assert_eq!(
            time_of(UTC_TIME, "491231235959Z"),
            Some("2049-12-31T23:59:59Z".parse().unwrap())
        );
        assert_eq!(
            time_of(UTC_TIME, "991231235959Z"),
            Some("1999-12-31T23:59:59Z".parse().unwrap())
        );
    }
}
