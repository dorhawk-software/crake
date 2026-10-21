//! Tunnels the user adds in the app (Settings → Tunnels, New tunnel), kept by
//! craked in `tunnels.json` next to the host store. Unlike the config file's
//! and IT's, the user can change and remove them.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// A tunnel's name, as people see it.
const MAX_DISPLAY_NAME: usize = 40;
/// An interface name holds 15 characters; "crake-" takes six.
const NAME_ROOM: usize = 9;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::{Protocol, TunnelConfig};
use crate::hosts::Target;
use crate::policy::Learning;

/// What should go through it (new-tunnel.html, step 3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Learn as it goes: hosts that don't answer directly.
    #[default]
    Learn,
    /// Ask first: Crake finds what needs it, and asks before routing.
    Ask,
    /// Only the names and networks listed.
    List,
    /// Everything (full tunnel).
    Full,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Learn => "learn",
            Self::Ask => "ask",
            Self::List => "list",
            Self::Full => "full",
        }
    }
}

/// How the user signs in (step 2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignIn {
    #[default]
    Browser,
    Password,
    Certificate,
}

impl SignIn {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Password => "password",
            Self::Certificate => "certificate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnTunnel {
    pub name: String,
    pub display_name: String,
    pub gateway: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub sign_in: SignIn,
    #[serde(default)]
    pub user: Option<String>,
    /// A file, a `pkcs11:` URI, or "keyring" (kept by the app in the keyring).
    #[serde(default)]
    pub certificate: Option<String>,
    #[serde(default)]
    pub mode: Mode,
    /// For "only what I list": names ("*.nordbank.example") and networks.
    #[serde(default)]
    pub list: Vec<String>,
    /// Domains whose names belong to it, on top of what its gateway hands
    /// over: an IPsec gateway's can't reach Crake (strongSwan passes on no
    /// INTERNAL_DNS_DOMAIN), so whoever sets it up names them.
    #[serde(default)]
    pub domains: Vec<String>,
    /// Forget learned hosts unused for this many days: None for the
    /// Settings value, Some(None) never.
    #[serde(default)]
    pub forget_after_days: Option<Option<u64>>,
    /// Advanced: very old gateways.
    #[serde(default)]
    pub weak_ciphers: bool,
    /// Advanced: report a current AnyConnect version.
    #[serde(default)]
    pub client_version: bool,
    /// Keep the sign-in when switched off, so switching on again needs none
    /// (as long as the gateway allows). Off: switching off logs off.
    #[serde(default = "yes")]
    pub keep_signed_in: bool,
    /// IPsec: the gateway proves itself with its certificate, not a key
    /// (the key and IT's authority are craked's alone, `ipsec::Secret`;
    /// these say only which, for Edit).
    #[serde(default)]
    pub gateway_certificate: bool,
    /// IPsec: IT's certificate authority is kept for it.
    #[serde(default)]
    pub gateway_ca: bool,
}

fn yes() -> bool {
    true
}

/// The AnyConnect version reported when asked to (Advanced).
const CLIENT_VERSION: &str = "4.10.08029";

impl OwnTunnel {
    /// From the D-Bus API's text form (keys as in `to_map`).
    pub fn from_map(name: &str, m: &HashMap<String, String>) -> Result<Self> {
        let get = |k: &str| m.get(k).map(|v| v.trim()).filter(|v| !v.is_empty());
        let display_name = get("display_name").unwrap_or("").to_owned();
        if display_name.is_empty() || display_name.chars().count() > MAX_DISPLAY_NAME {
            bail!("Give it a name of up to {MAX_DISPLAY_NAME} characters.");
        }
        let gateway = get("gateway").unwrap_or("").to_owned();
        if gateway.is_empty() || gateway.starts_with('-') || gateway.contains(char::is_whitespace) {
            bail!("The gateway is a name or an address, like vpn.example.com.");
        }
        let protocol: Protocol = get("protocol").unwrap_or("anyconnect").parse()?;
        let flag = |k: &str| get(k) == Some("true");
        let safe = |k: &str| -> Result<Option<String>> {
            match get(k) {
                Some(v) if v.starts_with('-') || v.contains('\n') => bail!("{v:?} can't be used"),
                Some(v) => Ok(Some(v.to_owned())),
                None => Ok(None),
            }
        };
        let sign_in = match get("sign_in").unwrap_or("browser") {
            "browser" => SignIn::Browser,
            "password" => SignIn::Password,
            "certificate" => SignIn::Certificate,
            other => bail!("{other:?} isn't a way to sign in"),
        };
        let mode = match get("mode").unwrap_or("learn") {
            "learn" => Mode::Learn,
            "ask" => Mode::Ask,
            "list" => Mode::List,
            "full" => Mode::Full,
            other => bail!("{other:?} isn't a routing mode"),
        };
        let list: Vec<String> = get("list")
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        for item in &list {
            if item.parse::<Target>().is_err()
                && !crate::dns::valid_domain(item.strip_prefix("*.").unwrap_or(item))
            {
                bail!("{item:?} is neither a name nor a network.");
            }
        }
        let mut domains: Vec<String> = Vec::new();
        for d in get("domains").unwrap_or("").split([',', '\n', ' ']) {
            let d = crate::dns::normalize(d.trim());
            if d.is_empty() {
                continue;
            }
            if !crate::dns::valid_domain(&d) {
                bail!("{d:?} isn't a domain, like acme.internal.");
            }
            if !domains.contains(&d) {
                domains.push(d);
            }
        }
        let forget_after_days = match get("forget_after_days") {
            None => None,
            Some("never") => Some(None),
            Some(d) => match d.parse::<u64>() {
                Ok(n) if (1..=crate::config::MAX_DAYS).contains(&n) => Some(Some(n)),
                _ => bail!("{d:?} isn't a number of days"),
            },
        };
        Ok(Self {
            name: name.to_owned(),
            display_name,
            gateway,
            protocol,
            group: safe("group")?,
            sign_in,
            user: safe("user")?,
            certificate: safe("certificate")?,
            mode,
            list,
            domains,
            forget_after_days,
            weak_ciphers: flag("weak_ciphers"),
            client_version: flag("client_version"),
            keep_signed_in: get("keep_signed_in") != Some("false"),
            gateway_certificate: false,
            gateway_ca: false,
        })
    }

    /// What its secret says about the gateway's proof (`ipsec::Secret`).
    pub fn with_secret(mut self, secret: Option<&crate::ipsec::Secret>) -> Self {
        self.gateway_certificate = secret.is_some_and(|s| s.key.is_none());
        self.gateway_ca = secret.is_some_and(|s| s.ca.is_some());
        self
    }

    /// The text form, for the app to edit it.
    pub fn to_map(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("display_name".into(), self.display_name.clone());
        m.insert("gateway".into(), self.gateway.clone());
        m.insert("protocol".into(), self.protocol.as_str().into());
        m.insert("group".into(), self.group.clone().unwrap_or_default());
        m.insert("sign_in".into(), self.sign_in.as_str().into());
        m.insert("user".into(), self.user.clone().unwrap_or_default());
        m.insert(
            "certificate".into(),
            self.certificate.clone().unwrap_or_default(),
        );
        m.insert("mode".into(), self.mode.as_str().into());
        m.insert("list".into(), self.list.join("\n"));
        m.insert("domains".into(), self.domains.join("\n"));
        m.insert(
            "forget_after_days".into(),
            match self.forget_after_days {
                None => String::new(),
                Some(None) => "never".into(),
                Some(Some(d)) => d.to_string(),
            },
        );
        m.insert("weak_ciphers".into(), self.weak_ciphers.to_string());
        m.insert("client_version".into(), self.client_version.to_string());
        m.insert("keep_signed_in".into(), self.keep_signed_in.to_string());
        if self.protocol == Protocol::Ipsec {
            let proof = if self.gateway_certificate {
                "certificate"
            } else {
                "key"
            };
            m.insert("gateway_proof".into(), proof.into());
            m.insert(
                "gateway_ca".into(),
                if self.gateway_ca { "kept" } else { "" }.into(),
            );
        }
        m
    }

    /// What may go through it.
    pub fn learning(&self) -> Learning {
        match self.mode {
            Mode::Learn | Mode::Ask => Learning::Anything,
            Mode::Full => Learning::FullTunnel,
            Mode::List => {
                let (mut names, mut nets) = (Vec::new(), Vec::new());
                for item in &self.list {
                    match item.parse::<Target>() {
                        Ok(t) => nets.push(t),
                        Err(_) => names.push(item.to_ascii_lowercase()),
                    }
                }
                Learning::Only { names, nets }
            }
        }
    }

    pub fn to_config(&self) -> TunnelConfig {
        let mut auth_args = Vec::new();
        if let Some(g) = &self.group {
            auth_args.push(format!("--authgroup={g}"));
        }
        match (self.sign_in, &self.certificate) {
            (SignIn::Browser, _) => {
                auth_args.push(format!("--external-browser={}", crake_proto::BROWSER))
            }
            // The app writes a keyring copy for each sign-in itself.
            (SignIn::Certificate, Some(c)) if c != "keyring" => {
                auth_args.push(format!("--certificate={c}"))
            }
            _ => {}
        }
        let mut extra_args = Vec::new();
        if self.weak_ciphers {
            extra_args.push("--allow-insecure-crypto".into());
        }
        if self.client_version {
            extra_args.push(format!("--version-string={CLIENT_VERSION}"));
        }
        TunnelConfig {
            name: self.name.clone(),
            display_name: Some(self.display_name.clone()),
            gateway: self.gateway.clone(),
            protocol: self.protocol,
            user: self
                .user
                .clone()
                .filter(|_| self.sign_in == SignIn::Password),
            passwd_file: None,
            dns_domains: self.domains.clone(),
            auth_args,
            extra_args,
            profile: None,
            gateway_key_file: None,
            gateway_id: None,
            managed: None,
            own: Some(self.clone()),
        }
    }
}

/// A free tunnel name (becomes the interface `crake-<name>`, at most 9
/// characters of a-z, 0-9 and '-') from its display name.
pub fn name_for(display: &str, taken: &[String]) -> String {
    let base: String = display
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base: String = base.chars().take(NAME_ROOM).collect();
    let base = base.trim_end_matches('-');
    let base = if base.is_empty() { "tunnel" } else { base };
    if !taken.iter().any(|t| t == base) {
        return base.to_owned();
    }
    for n in 2.. {
        let suffix = n.to_string();
        let keep = NAME_ROOM - suffix.len();
        let candidate = format!("{}{suffix}", &base[..base.len().min(keep)]);
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// The store: `tunnels.json` next to the host store.
pub fn path_for(state_path: &Path) -> PathBuf {
    state_path.with_file_name("tunnels.json")
}

pub fn load(path: &Path) -> Vec<OwnTunnel> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, tunnels: &[OwnTunnel]) {
    if let Ok(json) = serde_json::to_string_pretty(tunnels) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn an_ipsec_tunnel_names_its_domains() {
        let t = OwnTunnel::from_map(
            "acme",
            &map(&[
                ("display_name", "ACME"),
                ("gateway", "vpn.acme.example"),
                ("protocol", "ipsec"),
                ("domains", "ACME.internal, acme.example\nacme.internal"),
            ]),
        )
        .unwrap();
        assert_eq!(t.protocol, Protocol::Ipsec);
        assert_eq!(t.to_config().dns_domains, ["acme.internal", "acme.example"]);
        assert_eq!(t.to_map()["domains"], "acme.internal\nacme.example");
        let bad = map(&[
            ("display_name", "ACME"),
            ("gateway", "vpn.acme.example"),
            ("domains", "not a domain!"),
        ]);
        assert!(OwnTunnel::from_map("acme", &bad).is_err());
    }

    #[test]
    fn a_tunnel_from_the_new_tunnel_window() {
        let t = OwnTunnel::from_map(
            "nordbank",
            &map(&[
                ("display_name", "Nordbank"),
                ("gateway", "gw.nordbank.example"),
                ("protocol", "gp"),
                ("group", "Contractors"),
                ("sign_in", "certificate"),
                ("certificate", "keyring"),
                ("mode", "list"),
                ("list", "*.nordbank.example\n172.22.0.0/16"),
                ("forget_after_days", "never"),
                ("weak_ciphers", "true"),
            ]),
        )
        .unwrap();
        let c = t.to_config();
        assert_eq!(c.auth_args, ["--authgroup=Contractors"]);
        assert_eq!(c.extra_args, ["--allow-insecure-crypto"]);
        assert!(matches!(c.learning(), Learning::Only { .. }));
        let back = OwnTunnel::from_map("nordbank", &t.to_map().into_iter().collect()).unwrap();
        assert_eq!(back, t);
        assert!(
            OwnTunnel::from_map(
                "x",
                &map(&[("display_name", "X"), ("gateway", "--script=/x")])
            )
            .is_err()
        );
        let empty = OwnTunnel::from_map(
            "x",
            &map(&[("display_name", "X"), ("gateway", "g"), ("mode", "list")]),
        )
        .unwrap();
        assert_eq!(
            empty.learning(),
            Learning::Only {
                names: vec![],
                nets: vec![]
            },
            "only what I list, nothing listed yet: nothing goes through"
        );
        assert!(empty.keep_signed_in, "kept unless said otherwise");
    }

    #[test]
    fn names_fit_an_interface() {
        let taken = vec!["home-lab".to_owned()];
        assert_eq!(name_for("Nordbank", &taken), "nordbank");
        assert_eq!(name_for("Home lab", &taken), "home-lab2");
        assert_eq!(name_for("Nordbank Payments Team", &taken), "nordbank");
        assert_eq!(name_for("Ž!", &taken), "tunnel");
    }
}
