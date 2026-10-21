//! The user's own certificate for an IPsec tunnel
//! (design/proposals/ipsec-certificates.html, 2): a file's bytes, as the
//! app keeps them in the keyring (a .p12 or .pfx, or PEM with the key),
//! opened here, in craked, with its password when it has one, and handed
//! to charon over vici, which can't take a PKCS#12 file itself (swanctl
//! opens those before it loads them).

use base64::Engine as _;

use super::certificate::{self, Certificate};

/// PEM's labels this takes: the certificate, and an unencrypted key
/// (PKCS#8, or the older RSA and EC forms charon reads too).
const CERTIFICATE: &str = "CERTIFICATE";
const KEYS: [&str; 3] = ["PRIVATE KEY", "RSA PRIVATE KEY", "EC PRIVATE KEY"];
const ENCRYPTED_KEY: &str = "ENCRYPTED PRIVATE KEY";

/// A certificate and its key, ready for charon (PEM, both).
#[derive(Clone)]
pub struct Credential {
    pub cert_pem: String,
    pub key_pem: String,
    pub cert: Certificate,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Credential {{ subject: {:?}, .. }}", self.cert.subject)
    }
}

/// What opening a certificate file came to.
#[derive(Debug)]
pub enum Opened {
    Ready(Credential),
    /// It needs a password, or the one given didn't open it.
    Password,
    /// Not one Crake can use, and why, in the sign-in window's words.
    Unusable(String),
}

/// The contents of PEM's first block labelled `label`.
fn pem_block(text: &str, label: &str) -> Option<Vec<u8>> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = text.find(&begin)? + begin.len();
    let stop = start + text[start..].find(&end)?;
    let body: String = text[start..stop].split_whitespace().collect();
    base64::engine::general_purpose::STANDARD.decode(body).ok()
}

/// `der` as PEM labelled `label`.
fn pem(label: &str, der: &[u8]) -> String {
    /// PEM's line length (RFC 7468).
    const LINE: usize = 64;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for line in b64.as_bytes().chunks(LINE) {
        out.push_str(&String::from_utf8_lossy(line));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

fn credential(cert_der: &[u8], key_pem: String) -> Opened {
    match certificate::parse(cert_der) {
        Some(cert) => Opened::Ready(Credential {
            cert_pem: pem(CERTIFICATE, cert_der),
            key_pem,
            cert,
        }),
        None => Opened::Unusable(NOT_A_CERTIFICATE.into()),
    }
}

const NOT_A_CERTIFICATE: &str =
    "It isn\u{2019}t a certificate file Crake can open (a .p12, .pfx or .pem file).";

/// Open the user's certificate file; `password` when it asked for one.
pub fn open(bytes: &[u8], password: Option<&str>) -> Opened {
    if let Ok(text) = std::str::from_utf8(bytes)
        && text.contains(&format!("-----BEGIN {CERTIFICATE}-----"))
    {
        let Some(cert) = pem_block(text, CERTIFICATE) else {
            return Opened::Unusable(NOT_A_CERTIFICATE.into());
        };
        if text.contains(&format!("-----BEGIN {ENCRYPTED_KEY}-----")) {
            return Opened::Unusable(
                "Its key has a password Crake can\u{2019}t open in a .pem file yet. Your IT team can give it as a .p12 file."
                    .into(),
            );
        }
        let Some(key) = KEYS
            .iter()
            .find_map(|label| pem_block(text, label).map(|der| pem(label, &der)))
        else {
            return Opened::Unusable(
                "It has the certificate but not its private key. Your IT team can give both, as a .p12 file."
                    .into(),
            );
        };
        return credential(&cert, key);
    }
    let store = p12_keystore::KeyStore::from_pkcs12(
        bytes,
        password.unwrap_or(""),
        p12_keystore::Pkcs12ImportPolicy::Strict,
    );
    match store {
        Ok(store) => {
            let Some((_, chain)) = store.private_key_chain() else {
                return Opened::Unusable(
                    "It has no private key with its certificate. Your IT team can give you one that has."
                        .into(),
                );
            };
            let Some(cert) = chain.certs().first() else {
                return Opened::Unusable(NOT_A_CERTIFICATE.into());
            };
            credential(cert.as_der(), pem(KEYS[0], chain.key().as_der()))
        }
        // The password (none, or the one given) didn't open it: its MAC,
        // or its contents' decryption, failed.
        Err(
            p12_keystore::error::Error::MacError(_)
            | p12_keystore::error::Error::UnpadError
            | p12_keystore::error::Error::Pkcs5Error(_),
        ) => Opened::Password,
        Err(_) => Opened::Unusable(NOT_A_CERTIFICATE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P12: &[u8] = include_bytes!("testdata/alice.p12");
    const LEGACY: &[u8] = include_bytes!("testdata/alice-legacy.p12");
    const NO_PASSWORD: &[u8] = include_bytes!("testdata/alice-nopass.p12");
    const PEM: &[u8] = include_bytes!("testdata/alice.pem");

    fn ready(o: Opened) -> Credential {
        match o {
            Opened::Ready(c) => c,
            other => panic!("not opened: {other:?}"),
        }
    }

    #[test]
    fn a_p12_asks_for_its_password_and_opens_with_it() {
        assert!(matches!(open(P12, None), Opened::Password));
        assert!(matches!(open(P12, Some("hunter3")), Opened::Password));
        let c = ready(open(P12, Some("hunter2")));
        assert_eq!(c.cert.subject, "alice");
        assert!(c.cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(c.key_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(!format!("{c:?}").contains("PRIVATE"), "the key isn't shown");
    }

    #[test]
    fn an_older_p12_opens_too() {
        let c = ready(open(LEGACY, Some("hunter2")));
        assert_eq!(c.cert.subject, "alice");
    }

    #[test]
    fn a_p12_without_a_password_opens_at_once() {
        assert_eq!(ready(open(NO_PASSWORD, None)).cert.subject, "alice");
    }

    #[test]
    fn a_pem_file_needs_its_key_unencrypted() {
        let c = ready(open(PEM, None));
        assert_eq!(c.cert.subject, "alice");
        assert!(c.key_pem.contains("PRIVATE KEY"));
        let text = std::str::from_utf8(PEM).unwrap();
        let cert_only = &text[..text.find("-----BEGIN PRIVATE KEY").unwrap()];
        assert!(
            matches!(open(cert_only.as_bytes(), None), Opened::Unusable(w) if w.contains("not its private key"))
        );
        let encrypted = format!(
            "{cert_only}-----BEGIN ENCRYPTED PRIVATE KEY-----\nMII=\n-----END ENCRYPTED PRIVATE KEY-----\n"
        );
        assert!(
            matches!(open(encrypted.as_bytes(), None), Opened::Unusable(w) if w.contains(".p12"))
        );
    }

    #[test]
    fn anything_else_isnt_a_certificate() {
        assert!(matches!(
            open(b"client\nremote vpn 1194\n", None),
            Opened::Unusable(_)
        ));
        assert!(matches!(open(&[], None), Opened::Unusable(_)));
    }
}
