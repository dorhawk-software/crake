//! The certificates this user trusts in the Keychain, for openconnect.
//! Homebrew's GnuTLS reads only its own bundle, never the Keychain, so a
//! company's own root (trusted there, as IT installs it) left every
//! sign-in asking "Crake can't vouch for…" (the user's Mac, 2026-10-27:
//! twenty times; on Linux the root is in the system's store, which
//! openconnect reads, and nothing was asked).

use std::path::PathBuf;

use base64::Engine as _;
use security_framework::trust_settings::{Domain, TrustSettings, TrustSettingsForCertificate};

/// A PEM file's line length.
const PEM_LINE: usize = 64;

/// The certificates the user or an administrator trusts for TLS in the
/// Keychain, as a file for `--cafile` (written afresh each time, so a root
/// IT adds or takes away counts at the next sign-in); None: there are none.
/// The system's own roots aren't here: Homebrew's bundle has them.
pub fn trusted_cafile() -> Option<PathBuf> {
    let mut pem = String::new();
    for domain in [Domain::User, Domain::Admin] {
        let settings = TrustSettings::new(domain);
        let Ok(certificates) = settings.iter() else {
            continue;
        };
        for certificate in certificates {
            // No setting for TLS in particular: trusted, as the Keychain
            // takes it (rustls-native-certs reads it so too).
            let denied = matches!(
                settings.tls_trust_settings_for_certificate(&certificate),
                Ok(Some(TrustSettingsForCertificate::Deny)) | Err(_)
            );
            if !denied {
                pem.push_str(&pem_of(&certificate.to_der()));
            }
        }
    }
    if pem.is_empty() {
        return None;
    }
    let path = super::private_dir().ok()?.join("trusted.pem");
    crate::sys::write_private(&path, pem.as_bytes()).ok()?;
    Some(path)
}

/// One certificate as PEM.
fn pem_of(der: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for line in encoded.as_bytes().chunks(PEM_LINE) {
        pem.push_str(&String::from_utf8_lossy(line));
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    pem
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_certificate_reads_as_pem() {
        let pem = super::pem_of(&[0xAB; 100]);
        let lines: Vec<&str> = pem.lines().collect();
        assert_eq!(lines.first(), Some(&"-----BEGIN CERTIFICATE-----"));
        assert_eq!(lines.last(), Some(&"-----END CERTIFICATE-----"));
        assert_eq!(lines[1].len(), super::PEM_LINE);
        assert!(
            lines[1..lines.len() - 1]
                .iter()
                .all(|l| l.len() <= super::PEM_LINE)
        );
    }
}
