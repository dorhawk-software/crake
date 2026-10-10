use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use crake_proto::{DEFAULT_HOOK_PATH, DEFAULT_HOOK_SOCKET, valid_tunnel_name};
use serde::Deserialize;

pub const DEFAULT_CONFIG: &str = "/etc/crake/craked.toml";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_hook_path")]
    pub hook_path: PathBuf,
    #[serde(default = "default_hook_socket")]
    pub hook_socket: PathBuf,
    #[serde(default = "default_openconnect")]
    pub openconnect: PathBuf,
    /// Append-only JSON lines.
    #[serde(default = "default_log_path")]
    pub log_path: PathBuf,
    /// Pinned and learned hosts per tunnel.
    #[serde(default = "default_state_path")]
    pub state_path: PathBuf,
    /// A learned host goes direct again after this many days without traffic.
    #[serde(default = "default_expiry_days")]
    pub learned_expiry_days: u64,
    /// Try connections that get no answer (or are refused) through the tunnels.
    #[serde(default = "yes")]
    pub probing: bool,
    /// Seconds to wait for an answer before probing: 1, 2, 3 or 5.
    #[serde(default = "default_probe_wait")]
    pub probe_wait_secs: u64,
    /// Never send these through any tunnel (hosts or networks).
    #[serde(default)]
    pub never: Vec<crate::hosts::Target>,
    /// Register tunnel DNS with systemd-resolved (off where there is none, e.g. tests).
    #[serde(default = "yes")]
    pub use_resolved: bool,
    /// Set to false to run without touching interfaces (development, tests).
    #[serde(default = "yes")]
    pub apply_network: bool,
    /// Read-only tunnels from IT (`policy`), one `*.toml` each.
    #[serde(default = "default_managed_dir")]
    pub managed_dir: PathBuf,
    #[serde(default, rename = "tunnel")]
    pub tunnels: Vec<TunnelConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Anyconnect,
    Gp,
    Pulse,
    Nc,
    Fortinet,
    F5,
    Array,
    /// An OpenVPN 3 profile (the gateway field is its name): the app runs
    /// the session; craked routes through its device.
    Openvpn3,
}

impl std::str::FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "anyconnect" => Self::Anyconnect,
            "gp" => Self::Gp,
            "pulse" => Self::Pulse,
            "nc" => Self::Nc,
            "fortinet" => Self::Fortinet,
            "f5" => Self::F5,
            "array" => Self::Array,
            "openvpn3" => Self::Openvpn3,
            other => bail!("{other:?} isn't a protocol openconnect speaks"),
        })
    }
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anyconnect => "anyconnect",
            Self::Gp => "gp",
            Self::Pulse => "pulse",
            Self::Nc => "nc",
            Self::Fortinet => "fortinet",
            Self::F5 => "f5",
            Self::Array => "array",
            Self::Openvpn3 => "openvpn3",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunnelConfig {
    /// `[a-z0-9-]`, at most 9 characters. Becomes the interface `crake-<name>`.
    pub name: String,
    pub display_name: Option<String>,
    pub gateway: String,
    #[serde(default = "default_protocol")]
    pub protocol: Protocol,
    pub user: Option<String>,
    /// Root-only file whose first line is fed to openconnect's password prompt.
    /// Sign-in proper (browser, OTP, keyring) arrives with the UI.
    pub passwd_file: Option<PathBuf>,
    /// Extra domains whose names belong to this tunnel, on top of what the
    /// gateway pushes.
    #[serde(default)]
    pub dns_domains: Vec<String>,
    /// Arguments for sign-in only, e.g. `--certificate=...`.
    #[serde(default)]
    pub auth_args: Vec<String>,
    /// Arguments for both sign-in and the connection, e.g. `--useragent=...`.
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Set for tunnels from `managed_dir`: who decides, and what may go through.
    #[serde(skip)]
    pub managed: Option<crate::policy::Managed>,
    /// Set for tunnels added in the app (`own`): the user may change them.
    #[serde(skip)]
    pub own: Option<crate::own::OwnTunnel>,
}

impl TunnelConfig {
    pub fn display(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }

    /// What may go through this tunnel.
    pub fn learning(&self) -> crate::policy::Learning {
        if let Some(m) = &self.managed {
            return m.learning.clone();
        }
        self.own.as_ref().map(|o| o.learning()).unwrap_or_default()
    }

    /// "Ask me first": Crake finds what needs it, then asks before routing.
    pub fn asks(&self) -> bool {
        self.own
            .as_ref()
            .is_some_and(|o| o.mode == crate::own::Mode::Ask)
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        cfg.add_managed();
        cfg.validate()?;
        Ok(cfg)
    }

    /// IT's tunnels come first (their order is the try order); one of the
    /// user's own with the same name gives way.
    pub fn add_managed(&mut self) {
        let mut managed = Vec::new();
        for t in crate::policy::load_dir(&self.managed_dir) {
            match t {
                Ok(t) if valid_tunnel_name(&t.name) => managed.push(t),
                Ok(t) => tracing::error!(
                    "managed tunnel {:?}: names are 1-9 characters of a-z, 0-9 and '-'",
                    t.name
                ),
                Err(e) => tracing::error!("skipping a managed profile: {e:#}"),
            }
        }
        self.tunnels.retain(|t| {
            let shadowed = managed.iter().any(|m| m.name == t.name);
            if shadowed {
                tracing::warn!(
                    "tunnel {:?} is managed by IT; your own is set aside",
                    t.name
                );
            }
            !shadowed
        });
        managed.append(&mut self.tunnels);
        self.tunnels = managed;
    }

    pub fn validate(&self) -> Result<()> {
        if self.tunnels.len() > crate::routing::MAX_TUNNELS {
            bail!("at most {} tunnels", crate::routing::MAX_TUNNELS);
        }
        if ![1, 2, 3, 5].contains(&self.probe_wait_secs) {
            bail!("probe_wait_secs must be 1, 2, 3 or 5");
        }
        if !(1..=365).contains(&self.learned_expiry_days) {
            bail!("learned_expiry_days must be between 1 and 365");
        }
        let mut seen = std::collections::HashSet::new();
        for t in &self.tunnels {
            if !valid_tunnel_name(&t.name) {
                bail!(
                    "tunnel name {:?} must be 1-9 characters of a-z, 0-9 and '-'",
                    t.name
                );
            }
            if !seen.insert(&t.name) {
                bail!("tunnel {:?} is defined twice", t.name);
            }
            if let Some(d) = t.dns_domains.iter().find(|d| !crate::dns::valid_domain(d)) {
                bail!("tunnel {:?}: {d:?} is not a domain", t.name);
            }
            if t.gateway.is_empty() || t.gateway.starts_with('-') {
                bail!("tunnel {:?} has an invalid gateway", t.name);
            }
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hook_path: default_hook_path(),
            hook_socket: default_hook_socket(),
            openconnect: default_openconnect(),
            log_path: default_log_path(),
            state_path: default_state_path(),
            learned_expiry_days: default_expiry_days(),
            probing: true,
            probe_wait_secs: default_probe_wait(),
            never: Vec::new(),
            use_resolved: true,
            apply_network: true,
            managed_dir: default_managed_dir(),
            tunnels: Vec::new(),
        }
    }
}

fn default_hook_path() -> PathBuf {
    DEFAULT_HOOK_PATH.into()
}
fn default_hook_socket() -> PathBuf {
    DEFAULT_HOOK_SOCKET.into()
}
fn default_openconnect() -> PathBuf {
    "openconnect".into()
}
fn default_log_path() -> PathBuf {
    "/var/lib/crake/log".into()
}
fn default_state_path() -> PathBuf {
    "/var/lib/crake/hosts.json".into()
}
fn default_managed_dir() -> PathBuf {
    "/etc/crake/managed.d".into()
}
fn default_expiry_days() -> u64 {
    7
}
fn default_probe_wait() -> u64 {
    3
}
fn default_protocol() -> Protocol {
    Protocol::Anyconnect
}
fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates() {
        let cfg: Config = toml::from_str(
            r#"
            [[tunnel]]
            name = "acme"
            display_name = "ACME"
            gateway = "vpn.acme.example"
            "#,
        )
        .unwrap();
        cfg.validate().unwrap();
        assert_eq!(cfg.tunnels[0].protocol, Protocol::Anyconnect);
        assert_eq!(cfg.tunnels[0].display(), "ACME");
    }

    #[test]
    fn rejects_bad_names_and_unknown_keys() {
        let bad: Config = toml::from_str("[[tunnel]]\nname = \"ACME\"\ngateway = \"x\"").unwrap();
        assert!(bad.validate().is_err());
        assert!(toml::from_str::<Config>("bogus = 1").is_err());
    }
}
