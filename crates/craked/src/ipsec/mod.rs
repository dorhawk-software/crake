//! IPsec (IKEv2) with strongSwan's charon (CLAUDE.md, 7k): craked runs its
//! own charon (`charon`), drives it over vici (`vici`), and gives each
//! tunnel an XFRM interface of its own, `crake-<name>`, its traffic
//! selectors wide: nothing goes in until Crake's routing sends it. What
//! the gateway handed over comes in through strongSwan's updown, which
//! calls the hook (`env`). Linux only for now.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub mod certificate;
#[cfg(target_os = "linux")]
pub mod charon;
pub mod credential;
pub mod env;
#[cfg(target_os = "linux")]
mod unit;
#[cfg(target_os = "linux")]
pub mod vici;

/// What the gateway proves itself with, as IT hands it out (a FortiClient
/// profile's pre-shared key, or its certificate, with IT's authority when
/// it isn't one the system knows) and the gateway's IKE identity. Root's
/// only, never in `tunnels.json` or ListTunnels.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret {
    /// The pre-shared key; none when the gateway proves itself with its
    /// certificate (design/proposals/ipsec-certificates.html).
    #[serde(default)]
    pub key: Option<String>,
    /// The gateway's IKE identity ("vpn.acme.example"); with a key, any
    /// when unset; with a certificate, the gateway as typed.
    #[serde(default)]
    pub gateway_id: Option<String>,
    /// IT's certificate authority (PEM), the only one trusted for this
    /// gateway when given; else the system's.
    #[serde(default)]
    pub ca: Option<String>,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Secret {{ key: {}, gateway_id: {:?}, ca: {} }}",
            if self.key.is_some() { "(kept)" } else { "none" },
            self.gateway_id,
            if self.ca.is_some() {
                "IT's"
            } else {
                "the system's"
            },
        )
    }
}

/// How a certificate starts in PEM.
const PEM_CERTIFICATE: &str = "-----BEGIN CERTIFICATE-----";

impl Secret {
    /// From AddTunnel's fields: `gateway_key` and `gateway_id`.
    pub fn from_fields(key: &str, gateway_id: Option<&str>) -> Result<Self> {
        let key = key.trim();
        if key.is_empty() {
            bail!("An IPsec tunnel needs the gateway's key (its pre-shared key).");
        }
        if key.contains('\n') {
            bail!("The gateway's key is one line.");
        }
        Ok(Self {
            key: Some(key.to_owned()),
            gateway_id: identity(gateway_id)?,
            ca: None,
        })
    }

    /// A gateway that proves itself with its certificate, signed by IT's
    /// authority (`ca`, PEM) or one the system knows.
    pub fn certificate(gateway_id: Option<&str>, ca: Option<&str>) -> Result<Self> {
        let ca = ca.map(str::trim).filter(|c| !c.is_empty());
        if let Some(c) = ca
            && !c.contains(PEM_CERTIFICATE)
        {
            bail!("IT's certificate authority is a certificate file (.crt, .pem or .cer).");
        }
        Ok(Self {
            key: None,
            gateway_id: identity(gateway_id)?,
            ca: ca.map(str::to_owned),
        })
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))
    }
}

/// A gateway's identity as typed: none when empty.
fn identity(gateway_id: Option<&str>) -> Result<Option<String>> {
    let gateway_id = gateway_id.map(str::trim).filter(|g| !g.is_empty());
    if let Some(g) = gateway_id
        && (g.contains(char::is_whitespace) || g.starts_with('-'))
    {
        bail!("{g:?} isn't a gateway's identity, like vpn.example.com.");
    }
    Ok(gateway_id.map(str::to_owned))
}

/// The secret from a key file IT (or craked.toml) names: one line, root's
/// alone (a group key others on the computer can read would let them pose
/// as the gateway's clients), and the gateway's identity beside it.
pub fn key_from_file(path: &Path, gateway_id: Option<&str>) -> Result<Secret> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        /// Readable or writable by the group or others.
        const SHARED: u32 = 0o077;
        let mode = std::fs::metadata(path)
            .with_context(|| format!("reading {}", path.display()))?
            .permissions()
            .mode();
        if mode & SHARED != 0 {
            bail!(
                "{} can be read by others: the gateway's key must be root's alone (chmod 600)",
                path.display()
            );
        }
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Secret::from_fields(text.trim(), gateway_id).with_context(|| path.display().to_string())
}

/// Where craked keeps a tunnel's secret: root's only, beside the OpenVPN
/// profiles.
pub fn secret_path(state_path: &Path, tunnel: &str) -> PathBuf {
    state_path
        .with_file_name("profiles")
        .join(format!("{tunnel}.ipsec"))
}

/// A gateway's certificate the user trusted (the trust question), kept
/// beside the tunnel's secret: IT's tunnels have one too, their secret
/// being IT's files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    /// The certificate, DER in base64.
    pub der: String,
    /// The day it was trusted ("2026-10-21"), for "the one you trusted on".
    pub trusted_on: String,
}

impl Pin {
    pub fn new(der: &[u8], today: time::Date) -> Self {
        use base64::Engine as _;
        Self {
            der: base64::engine::general_purpose::STANDARD.encode(der),
            trusted_on: today.to_string(),
        }
    }

    pub fn der(&self) -> Option<Vec<u8>> {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(&self.der)
            .ok()
    }

    pub fn read(path: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
}

/// Where craked keeps the certificate trusted for a tunnel's gateway.
pub fn pin_path(state_path: &Path, tunnel: &str) -> PathBuf {
    state_path
        .with_file_name("profiles")
        .join(format!("{tunnel}.ipsec-pin"))
}

/// The XFRM interface ID of the tunnel at `index` (a tunnel's mark and
/// table come from its index too): never 0, which means none.
pub fn if_id(index: usize) -> u32 {
    u32::try_from(index).map_or(u32::MAX, |i| i.saturating_add(1))
}

/// Where distributions keep the certificate authorities they trust, as
/// one file of PEM certificates: Debian, Ubuntu, Arch and NixOS; Fedora.
const AUTHORITIES: [&str; 2] = [
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
];

/// The system's certificate authorities, each as PEM, and their file.
pub fn system_authorities() -> Option<(PathBuf, Vec<String>)> {
    AUTHORITIES.iter().find_map(|p| {
        let text = std::fs::read_to_string(p).ok()?;
        let pems = pem_certificates(&text);
        (!pems.is_empty()).then(|| (PathBuf::from(p), pems))
    })
}

/// Each certificate in a PEM file, from its BEGIN line to its END line.
pub fn pem_certificates(text: &str) -> Vec<String> {
    const END: &str = "-----END CERTIFICATE-----";
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(PEM_CERTIFICATE) {
        let Some(len) = rest[start..].find(END) else {
            break;
        };
        let end = start + len + END.len();
        out.push(format!("{}\n", &rest[start..end]));
        rest = &rest[end..];
    }
    out
}

/// The tunnel's interface MTU: the wire's 1500 less ESP's overhead over
/// UDP (NAT-T), with room to spare, so nothing is fragmented.
pub const MTU: u32 = 1400;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_is_one_line_and_never_shown() {
        let s = Secret::from_fields(" hunter2hunter2 \n", Some(" vpn.acme.example ")).unwrap();
        assert_eq!(s.key.as_deref(), Some("hunter2hunter2"));
        assert_eq!(s.gateway_id.as_deref(), Some("vpn.acme.example"));
        assert!(!format!("{s:?}").contains("hunter2"));
        assert!(Secret::from_fields("  ", None).is_err());
        assert!(Secret::from_fields("a\nb", None).is_err());
        assert!(Secret::from_fields("k", Some("-x")).is_err());
        assert_eq!(
            Secret::from_fields("k", Some(" ")).unwrap().gateway_id,
            None
        );
    }

    #[test]
    fn a_gateway_with_a_certificate_has_no_key() {
        let s = Secret::certificate(Some("vpn.acme.example"), None).unwrap();
        assert_eq!(s.key, None);
        assert_eq!(s.ca, None);
        let ca = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        let s = Secret::certificate(None, Some(ca)).unwrap();
        assert_eq!(s.ca.as_deref(), Some(ca.trim()));
        assert_eq!(Secret::certificate(None, Some(" ")).unwrap().ca, None);
        assert!(Secret::certificate(None, Some("not a certificate")).is_err());
        assert!(format!("{s:?}").contains("IT's"));
    }

    #[test]
    fn a_secret_kept_before_certificates_still_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("acme.ipsec");
        std::fs::write(&path, r#"{"key":"hunter2hunter2","gateway_id":null}"#).unwrap();
        let s = Secret::read(&path).unwrap();
        assert_eq!(s.key.as_deref(), Some("hunter2hunter2"));
        assert_eq!(s.ca, None);
    }

    #[test]
    fn a_secret_is_kept_beside_the_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let path = secret_path(&dir.path().join("hosts.json"), "acme");
        assert!(path.ends_with("profiles/acme.ipsec"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let s = Secret::from_fields("k", None).unwrap();
        std::fs::write(&path, serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(Secret::read(&path).unwrap(), s);
    }

    #[test]
    fn a_bundle_of_authorities_is_split_into_certificates() {
        let one = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----";
        let text = format!("# ACME Root\n{one}\n\n# Globex Root\n{one}\ntrailing");
        let pems = pem_certificates(&text);
        assert_eq!(pems, [format!("{one}\n"), format!("{one}\n")]);
        assert!(pem_certificates("-----BEGIN CERTIFICATE-----\ncut").is_empty());
    }

    #[test]
    fn a_pin_keeps_the_certificate_and_its_day() {
        let day = time::Date::from_calendar_date(2026, time::Month::October, 21).unwrap();
        let pin = Pin::new(&[1, 2, 3], day);
        assert_eq!(pin.trusted_on, "2026-10-21");
        assert_eq!(pin.der(), Some(vec![1, 2, 3]));
        let dir = tempfile::tempdir().unwrap();
        let path = pin_path(&dir.path().join("hosts.json"), "acme");
        assert!(path.ends_with("profiles/acme.ipsec-pin"));
        assert_eq!(Pin::read(&path), None);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string(&pin).unwrap()).unwrap();
        assert_eq!(Pin::read(&path), Some(pin));
    }

    #[test]
    fn interface_ids_are_never_zero() {
        assert_eq!(if_id(0), 1);
        assert_eq!(if_id(7), 8);
    }
}
