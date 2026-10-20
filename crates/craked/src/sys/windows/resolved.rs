//! Windows: a link's DNS and domains (Linux: systemd-resolved's per-link
//! DNS) as NRPT rules, which send the link's domains to craked's own DNS
//! server (`dns_queue::ADDR`), which looks up whose a name is in
//! `sys::dns_links`. The rules are volatile registry keys: gone at the
//! latest when Windows restarts.

use std::net::Ipv4Addr;

/// A child process without a console window of its own.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// An NRPT rule's ConfigOptions: it names generic DNS servers.
const GENERIC_DNS_SERVERS: u32 = 8;

use anyhow::{Result, bail};
use crake_os::windows::registry::{self, Key, Root, Value};

/// Where Windows keeps local NRPT rules.
const POLICY: &str = r"SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig";
/// Where group policy's NRPT rules are.
const GROUP_POLICY: &str = r"SOFTWARE\Policies\Microsoft\Windows NT\DNSClient\DnsPolicyConfig";
/// Crake's rules are named after it and the link.
const PREFIX: &str = "Crake ";

/// The public rule's name (`Crake public`).
const PUBLIC_RULE: &str = "public";

pub use super::super::dns_links::{owner, public};

/// Send every name that isn't a link's to craked's server (Linux watches
/// public answers in nftables instead), or stop.
pub fn set_public(on: bool) -> Result<()> {
    super::super::dns_links::set_public(on);
    refresh_public()
}

/// The root rule there or gone, as `dns_links::public` now says (wanted,
/// and not given back to the system for a while).
pub fn refresh_public() -> Result<()> {
    if super::super::dns_links::public() {
        write_rule(PUBLIC_RULE, &[], true)
    } else {
        delete(&format!("{POLICY}\\{PREFIX}{PUBLIC_RULE}"));
        Ok(())
    }
}

pub enum Resolved {
    Real,
    Noop,
}

impl Resolved {
    /// DNS over TLS is systemd-resolved's (Linux): here craked's own server
    /// answers, and sees every public answer.
    pub async fn encrypts_dns(&self) -> bool {
        false
    }

    /// Rules a craked before this one left behind go first.
    pub fn new(enabled: bool) -> Self {
        if enabled {
            for key in rule_keys() {
                delete(&key);
            }
            Self::Real
        } else {
            Self::Noop
        }
    }

    /// `domains` go to craked's server, which asks `servers` (or answers
    /// itself for its own links); with `everything` (a full tunnel's DNS),
    /// every other name too.
    pub async fn set_link(
        &self,
        device: &str,
        servers: &[std::net::IpAddr],
        domains: &[String],
        everything: bool,
    ) -> Result<()> {
        if let Self::Noop = self {
            return Ok(());
        }
        super::super::dns_links::set(device, servers, domains, everything);
        write_rule(device, domains, everything)
    }

    /// The link's NRPT rule still names what craked wrote: one another
    /// program removed or changed (a VPN client cleaning NRPT up, a tool)
    /// sent the tunnel's names to the usual DNS until it reconnected
    /// (found 2026-10-20).
    pub async fn link_as_set(&self, device: &str, domains: &[String], everything: bool) -> bool {
        // A tunnel going away: its rule goes, and stays gone (until
        // 2026-10-20 this put a removed tunnel's rule back).
        if matches!(self, Self::Noop) || !super::super::dns_links::registered(device) {
            return true;
        }
        let mut want = rule_names(domains, everything);
        let mut have: Vec<String> = multi_sz(&format!("{POLICY}\\{PREFIX}{device}"), "Name")
            .split(|&c| c == 0)
            .map(String::from_utf16_lossy)
            .filter(|n| !n.is_empty())
            .collect();
        want.sort();
        have.sort();
        want == have
    }

    /// The domains other programs serve: the NRPT rules that aren't
    /// Crake's (another VPN client's, group policy's) and the suffixes of
    /// the adapters that are up and aren't Crake's (AnyConnect's, the
    /// network's own). Crake's own adapters are all crake-*.
    pub async fn served_elsewhere(&self, _ours: &[String]) -> Vec<String> {
        if let Self::Noop = self {
            return Vec::new();
        }
        let mut found = super::adapter_domains();
        for root in [POLICY, GROUP_POLICY] {
            for rule in subkeys(root, false) {
                if !rule.starts_with(PREFIX) {
                    found.extend(namespaces(&multi_sz(&format!("{root}\\{rule}"), "Name")));
                }
            }
        }
        found
    }

    pub async fn link(&self, _device: &str) -> Result<(Vec<Ipv4Addr>, Vec<String>)> {
        Ok((Vec::new(), Vec::new()))
    }

    /// Windows' DNS cache, emptied.
    pub async fn flush(&self) {
        if let Self::Real = self {
            let mut cmd = tokio::process::Command::new("ipconfig");
            cmd.arg("/flushdns")
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let _ = cmd.status().await;
        }
    }

    /// The link's rule removed: its domains go back to the usual DNS.
    pub async fn revert(&self, device: &str) {
        super::super::dns_links::remove(device);
        if let Self::Real = self {
            delete(&format!("{POLICY}\\{PREFIX}{device}"));
        }
    }
}

/// One rule: the domains (each with and without a leading dot: the name
/// itself and everything under it) to craked's server; with `everything`,
/// the root (".", every name) too.
fn write_rule(device: &str, domains: &[String], everything: bool) -> Result<()> {
    let names = rule_names(domains, everything);
    let server = super::dns_queue::ADDR.to_string();
    let written = (|| {
        let key = Key::create_volatile(Root::LocalMachine, &format!("{POLICY}\\{PREFIX}{device}"))?;
        key.set("Version", &Value::Dword(2))?;
        // Generic DNS servers: ours.
        key.set("ConfigOptions", &Value::Dword(GENERIC_DNS_SERVERS))?;
        key.set("GenericDNSServers", &Value::Sz(&server))?;
        key.set("IPSECCARestriction", &Value::Sz(""))?;
        key.set("Name", &Value::MultiSz(&names))
    })();
    if let Err(e) = written {
        bail!("writing the DNS rule for {device}: {e}");
    }
    Ok(())
}

/// A rule's namespaces: each domain with and without a leading dot, and
/// the root with `everything`.
fn rule_names(domains: &[String], everything: bool) -> Vec<String> {
    let root = everything.then(|| ".".to_owned());
    domains
        .iter()
        .flat_map(|d| [d.clone(), format!(".{d}")])
        .chain(root)
        .collect()
}

fn delete(path: &str) {
    registry::delete_tree(Root::LocalMachine, path);
}

/// Crake's rules, whoever left them.
fn rule_keys() -> Vec<String> {
    subkeys(POLICY, true)
        .into_iter()
        .filter(|name| name.starts_with(PREFIX))
        .map(|name| format!("{POLICY}\\{name}"))
        .collect()
}

/// The names of the keys under `path` (in HKLM).
fn subkeys(path: &str, write: bool) -> Vec<String> {
    Key::open(Root::LocalMachine, path, write).map_or_else(Vec::new, |k| k.subkeys())
}

/// A REG_MULTI_SZ (or REG_SZ) value under `path` in HKLM, as UTF-16.
fn multi_sz(path: &str, value: &str) -> Vec<u16> {
    registry::multi_sz(Root::LocalMachine, path, value)
}

/// An NRPT rule's namespaces as domains: ".corp.example" (a suffix) and
/// "vpn.corp.example" (a name) alike; never the root, or a wildcard's.
fn namespaces(multi: &[u16]) -> Vec<String> {
    multi
        .split(|&c| c == 0)
        .map(|s| {
            String::from_utf16_lossy(s)
                .trim()
                .trim_start_matches(['*', '.'])
                .to_lowercase()
        })
        .filter(|d| !d.is_empty())
        .collect()
}

/// Who answers the system's DNS, for helpdesk copies (`Manager::diagnostics`).
pub fn view() -> Vec<String> {
    let links = super::super::dns_links::public();
    let line = if super::super::dns_links::given_back() {
        "Public names: given back to Windows's own DNS for now (it stopped answering Crake)."
            .to_owned()
    } else if links {
        let servers: Vec<String> = super::system_dns()
            .iter()
            .map(ToString::to_string)
            .collect();
        format!(
            "Public names: Crake answers Windows's DNS, asking {}.",
            if servers.is_empty() {
                "nothing (no server known)".to_owned()
            } else {
                servers.join(", ")
            }
        )
    } else {
        "Public names: Windows's own DNS answers them.".to_owned()
    };
    vec![line]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_clients_rule_names_its_domains() {
        let multi: Vec<u16> = ".Corp.Example\0vpn.globex.example\0*.initech.example\0.\0\0"
            .encode_utf16()
            .collect();
        assert_eq!(
            namespaces(&multi),
            ["corp.example", "vpn.globex.example", "initech.example"]
        );
    }
}
