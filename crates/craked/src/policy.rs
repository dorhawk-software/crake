//! Managed (IT) profiles: read-only tunnels from `/etc/crake/managed.d/*.toml`
//! and the learning policy that comes with them (CLAUDE.md, Managed profiles).
//!
//! ```toml
//! display_name = "ACME"
//! organisation = "ACME IT"          # who decides; default "<name> IT"
//! gateway = "vpn.acme.example"
//! protocol = "anyconnect"
//! group = "Employees"               # optional, --authgroup
//! sign_in = "browser"               # browser | password | certificate
//! learn = ["*.acme.internal", "10.40.0.0/16"]
//! # or: full_tunnel = true
//! ```
//!
//! The user can switch a managed tunnel on and off, but can't edit it or
//! widen its scope: hosts outside `learn` are never sent through it, by
//! learning or by hand.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::config::{Protocol, TunnelConfig};
use crate::hosts::Target;

/// What may go through a tunnel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Learning {
    /// The user's own tunnel (or IT left it open): anything that needs it.
    #[default]
    Anything,
    /// Only names matching these patterns ("*.acme.internal", "git.acme.com")
    /// and addresses inside these networks.
    Only {
        names: Vec<String>,
        nets: Vec<Target>,
    },
    /// Everything goes through; nothing to learn.
    FullTunnel,
}

impl Learning {
    /// May `addr` (known as `name`, if a name is known) go through?
    pub fn allows(&self, name: Option<&str>, addr: Ipv4Addr) -> bool {
        match self {
            Self::Anything => true,
            Self::FullTunnel => false,
            Self::Only { names, nets } => {
                nets.iter().any(|n| n.contains(addr))
                    || name.is_some_and(|n| names.iter().any(|p| name_matches(p, n)))
            }
        }
    }

    /// May a whole network (or host) be pinned? Only inside the scope's networks.
    pub fn allows_target(&self, t: Target) -> bool {
        match self {
            Self::Anything => true,
            Self::FullTunnel => false,
            Self::Only { nets, .. } => nets
                .iter()
                .any(|n| n.prefix_len() <= t.prefix_len() && n.contains(t.addr())),
        }
    }

    /// The scope in a few words: "*.acme.internal and 10.40.0.0/16".
    pub fn scope(&self) -> String {
        match self {
            Self::Only { names, nets } => {
                let all: Vec<String> = names
                    .iter()
                    .cloned()
                    .chain(nets.iter().map(Target::to_string))
                    .collect();
                match all.as_slice() {
                    [] => "nothing".into(),
                    [one] => one.clone(),
                    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
                }
            }
            _ => String::new(),
        }
    }
}

/// "*.acme.internal" matches names under acme.internal (not acme.internal
/// itself); anything else matches that one name.
fn name_matches(pattern: &str, name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    let pattern = pattern.trim_end_matches('.').to_ascii_lowercase();
    match pattern.strip_prefix("*.") {
        Some(domain) => name.ends_with(&format!(".{domain}")),
        None => name == pattern,
    }
}

/// A managed tunnel: who decides, what they allow, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Managed {
    pub file: PathBuf,
    /// "ACME IT".
    pub organisation: String,
    pub learning: Learning,
    /// How IT expects sign-in to go: "browser", "password" or "certificate".
    pub sign_in: Option<String>,
}

impl Managed {
    /// The panel's policy line (design/screens/tray-panel.html).
    pub fn describe(&self, label: &str) -> String {
        let org = &self.organisation;
        match &self.learning {
            Learning::Anything => format!("{org} lets Crake learn what needs {label}"),
            Learning::Only { .. } => {
                format!("{org} allows learning for {}", self.learning.scope())
            }
            Learning::FullTunnel => {
                format!("Full tunnel required by {org}. Crake won\u{2019}t learn here.")
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedFile {
    /// Defaults to the file's name ("acme.toml" → "acme").
    name: Option<String>,
    display_name: Option<String>,
    organisation: Option<String>,
    gateway: String,
    #[serde(default = "anyconnect")]
    protocol: Protocol,
    group: Option<String>,
    user: Option<String>,
    sign_in: Option<String>,
    /// A certificate for sign-in (a file or a pkcs11: URI).
    certificate: Option<String>,
    #[serde(default)]
    dns_domains: Vec<String>,
    #[serde(default)]
    learn: Vec<String>,
    #[serde(default)]
    full_tunnel: bool,
    /// Passed to openconnect as they are, e.g. `--useragent=...`.
    #[serde(default)]
    extra_args: Vec<String>,
}

fn anyconnect() -> Protocol {
    Protocol::Anyconnect
}

/// One managed profile.
pub fn load_file(path: &Path) -> Result<TunnelConfig> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let f: ManagedFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let name = match f.name {
        Some(n) => n,
        None => path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned(),
    };
    let learning = if f.full_tunnel {
        if !f.learn.is_empty() {
            bail!(
                "{}: full_tunnel and learn can't both be set",
                path.display()
            );
        }
        Learning::FullTunnel
    } else if f.learn.is_empty() {
        Learning::Anything
    } else {
        let (mut names, mut nets) = (Vec::new(), Vec::new());
        for item in &f.learn {
            if let Ok(t) = item.parse::<Target>() {
                nets.push(t);
            } else if crate::dns::valid_domain(item.strip_prefix("*.").unwrap_or(item)) {
                names.push(item.to_ascii_lowercase());
            } else {
                bail!(
                    "{}: {item:?} is neither a name nor a network",
                    path.display()
                );
            }
        }
        Learning::Only { names, nets }
    };
    if let Some(s) = &f.sign_in
        && !["browser", "password", "certificate"].contains(&s.as_str())
    {
        bail!(
            "{}: sign_in must be browser, password or certificate",
            path.display()
        );
    }
    let display = f.display_name.clone().unwrap_or_else(|| name.clone());
    let mut auth_args = Vec::new();
    if let Some(g) = &f.group {
        auth_args.push(format!("--authgroup={g}"));
    }
    if let Some(c) = &f.certificate {
        auth_args.push(format!("--certificate={c}"));
    }
    if f.sign_in.as_deref() == Some("browser") {
        auth_args.push(format!("--external-browser={}", crake_proto::BROWSER));
    }
    Ok(TunnelConfig {
        name,
        display_name: f.display_name,
        gateway: f.gateway,
        protocol: f.protocol,
        user: f.user,
        passwd_file: None,
        dns_domains: f.dns_domains,
        auth_args,
        extra_args: f.extra_args,
        managed: Some(Managed {
            file: path.to_owned(),
            organisation: f.organisation.unwrap_or_else(|| format!("{display} IT")),
            learning,
            sign_in: f.sign_in,
        }),
        own: None,
    })
}

/// Every `*.toml` in the directory, in name order. A broken file is skipped
/// (and reported); it doesn't take the others down.
pub fn load_dir(dir: &Path) -> Vec<Result<TunnelConfig>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    files.iter().map(|p| load_file(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn reads_a_managed_profile() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(
            dir.path(),
            "acme.toml",
            r#"
            display_name = "ACME"
            gateway = "vpn.acme.example"
            group = "Employees"
            sign_in = "browser"
            learn = ["*.acme.internal", "10.40.0.0/16"]
            "#,
        );
        let t = load_file(&p).unwrap();
        assert_eq!(t.name, "acme");
        assert_eq!(
            t.auth_args,
            [
                "--authgroup=Employees".to_owned(),
                format!("--external-browser={}", crake_proto::BROWSER)
            ]
        );
        let m = t.managed.unwrap();
        assert_eq!(
            m.describe("ACME"),
            "ACME IT allows learning for *.acme.internal and 10.40.0.0/16"
        );
        let full = write(
            dir.path(),
            "globex.toml",
            "display_name = \"Globex\"\ngateway = \"vpn.globex.example\"\nfull_tunnel = true\n",
        );
        assert_eq!(
            load_file(&full)
                .unwrap()
                .managed
                .unwrap()
                .describe("Globex"),
            "Full tunnel required by Globex IT. Crake won\u{2019}t learn here."
        );
        write(dir.path(), "broken.toml", "gateway = 3\n");
        write(dir.path(), "notes.txt", "not a profile\n");
        let all = load_dir(dir.path());
        assert_eq!(all.len(), 3);
        assert!(all[1].is_err(), "broken.toml, in name order");
        assert!(
            load_file(&write(
                dir.path(),
                "both.toml",
                "gateway = \"g\"\nfull_tunnel = true\nlearn = [\"*.a.b\"]\n"
            ))
            .is_err()
        );
    }

    #[test]
    fn the_scope_decides_what_may_go_through() {
        let l = Learning::Only {
            names: vec!["*.acme.internal".into(), "git.acme.com".into()],
            nets: vec!["10.40.0.0/16".parse().unwrap()],
        };
        let ip = |s: &str| s.parse::<Ipv4Addr>().unwrap();
        assert!(l.allows(Some("jira.acme.internal"), ip("172.16.0.9")));
        assert!(l.allows(Some("GIT.acme.com."), ip("172.16.0.9")));
        assert!(!l.allows(Some("acme.internal"), ip("172.16.0.9")));
        assert!(!l.allows(Some("evil-acme.internal"), ip("172.16.0.9")));
        assert!(l.allows(None, ip("10.40.3.4")));
        assert!(!l.allows(None, ip("10.41.0.1")));
        assert!(l.allows_target("10.40.8.0/24".parse().unwrap()));
        assert!(
            !l.allows_target("10.0.0.0/8".parse().unwrap()),
            "no widening"
        );
        assert!(!Learning::FullTunnel.allows(None, ip("10.40.3.4")));
        assert!(Learning::Anything.allows(None, ip("10.40.3.4")));
    }
}
