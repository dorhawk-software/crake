//! IPsec (IKEv2) with strongSwan's charon (CLAUDE.md, 7k): craked runs its
//! own charon (`charon`), drives it over vici (`vici`), and gives each
//! tunnel an XFRM interface of its own, `crake-<name>`, its traffic
//! selectors wide: nothing goes in until Crake's routing sends it. What
//! the gateway handed over comes in through strongSwan's updown, which
//! calls the hook (`env`). Linux only for now.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
pub mod charon;
pub mod env;
#[cfg(target_os = "linux")]
mod unit;
#[cfg(target_os = "linux")]
pub mod vici;

/// What the gateway proves itself with, as IT hands it out (a FortiClient
/// profile's pre-shared key and the gateway's IKE identity). Root's only,
/// never in `tunnels.json` or ListTunnels.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret {
    /// The pre-shared key.
    pub key: String,
    /// The gateway's IKE identity ("vpn.acme.example"); any, when unset.
    #[serde(default)]
    pub gateway_id: Option<String>,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret {{ gateway_id: {:?}, .. }}", self.gateway_id)
    }
}

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
        let gateway_id = gateway_id.map(str::trim).filter(|g| !g.is_empty());
        if let Some(g) = gateway_id
            && (g.contains(char::is_whitespace) || g.starts_with('-'))
        {
            bail!("{g:?} isn't a gateway's identity, like vpn.example.com.");
        }
        Ok(Self {
            key: key.to_owned(),
            gateway_id: gateway_id.map(str::to_owned),
        })
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))
    }
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

/// The XFRM interface ID of the tunnel at `index` (a tunnel's mark and
/// table come from its index too): never 0, which means none.
pub fn if_id(index: usize) -> u32 {
    u32::try_from(index).map_or(u32::MAX, |i| i.saturating_add(1))
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
        assert_eq!(s.key, "hunter2hunter2");
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
    fn interface_ids_are_never_zero() {
        assert_eq!(if_id(0), 1);
        assert_eq!(if_id(7), 8);
    }
}
