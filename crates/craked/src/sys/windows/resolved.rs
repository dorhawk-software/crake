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
/// Room for a registry key's name (they're at most 255 characters).
const NAME_LEN: usize = 256;

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, REG_DWORD, REG_MULTI_SZ,
    REG_OPTION_VOLATILE, REG_SZ, RRF_RT_REG_MULTI_SZ, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW,
    RegDeleteTreeW, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};

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

    /// What another program registered for the device (OpenVPN 3 on Linux).
    /// Its own DNS server holds the tunnels' domains here: nothing else
    /// rewrites them.
    pub async fn link_as_set(&self, _device: &str, _domains: &[String], _everything: bool) -> bool {
        true
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
            for rule in subkeys(root, KEY_READ) {
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

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// One rule: the domains (each with and without a leading dot: the name
/// itself and everything under it) to craked's server; with `everything`,
/// the root (".", every name) too.
fn write_rule(device: &str, domains: &[String], everything: bool) -> Result<()> {
    let path = wide(&format!("{POLICY}\\{PREFIX}{device}"));
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: a NUL-terminated path; `key` is closed below.
    let err = unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_VOLATILE,
            KEY_ALL_ACCESS,
            std::ptr::null(),
            &raw mut key,
            std::ptr::null_mut(),
        )
    };
    if err != ERROR_SUCCESS {
        bail!("writing the DNS rule for {device}: error {err}");
    }
    let mut names: Vec<u16> = Vec::new();
    let root = everything.then(|| ".".to_owned());
    let each = domains.iter().flat_map(|d| [d.clone(), format!(".{d}")]);
    for n in each.chain(root) {
        names.extend(n.encode_utf16());
        names.push(0);
    }
    names.push(0);
    let server = wide(&super::dns_queue::ADDR.to_string());
    let restriction = wide("");
    let results = [
        set(key, "Version", REG_DWORD, &2u32.to_le_bytes()),
        // Generic DNS servers: ours.
        set(
            key,
            "ConfigOptions",
            REG_DWORD,
            &GENERIC_DNS_SERVERS.to_le_bytes(),
        ),
        set(key, "GenericDNSServers", REG_SZ, bytes(&server)),
        set(key, "IPSECCARestriction", REG_SZ, bytes(&restriction)),
        set(key, "Name", REG_MULTI_SZ, bytes(&names)),
    ];
    // SAFETY: the key opened above.
    unsafe { RegCloseKey(key) };
    if let Some(err) = results.into_iter().find(|e| *e != ERROR_SUCCESS) {
        bail!("writing the DNS rule for {device}: error {err}");
    }
    Ok(())
}

fn bytes(w: &[u16]) -> &[u8] {
    // SAFETY: the same memory, as bytes.
    unsafe { std::slice::from_raw_parts(w.as_ptr().cast(), std::mem::size_of_val(w)) }
}

fn set(key: HKEY, name: &str, kind: u32, data: &[u8]) -> u32 {
    let name = wide(name);
    // SAFETY: an open key, a NUL-terminated name, `data` as long as given.
    unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            kind,
            data.as_ptr(),
            data.len() as u32,
        )
    }
}

fn delete(path: &str) {
    let path = wide(path);
    // SAFETY: a NUL-terminated path.
    unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, path.as_ptr()) };
}

/// Crake's rules, whoever left them.
fn rule_keys() -> Vec<String> {
    subkeys(POLICY, KEY_ALL_ACCESS)
        .into_iter()
        .filter(|name| name.starts_with(PREFIX))
        .map(|name| format!("{POLICY}\\{name}"))
        .collect()
}

/// The names of the keys under `path` (in HKLM), opened with `access`.
fn subkeys(path: &str, access: u32) -> Vec<String> {
    let path = wide(path);
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: a NUL-terminated path; `key` is closed below.
    if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, access, &raw mut key) }
        != ERROR_SUCCESS
    {
        return Vec::new();
    }
    let mut found = Vec::new();
    for i in 0.. {
        let mut name = [0u16; NAME_LEN];
        let mut len = name.len() as u32;
        // SAFETY: `name` holds `len` characters.
        let err = unsafe {
            RegEnumKeyExW(
                key,
                i,
                name.as_mut_ptr(),
                &raw mut len,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if err != ERROR_SUCCESS {
            break;
        }
        found.push(String::from_utf16_lossy(&name[..len as usize]));
    }
    // SAFETY: the key opened above.
    unsafe { RegCloseKey(key) };
    found
}

/// A REG_MULTI_SZ (or REG_SZ) value under `path` in HKLM, as UTF-16.
fn multi_sz(path: &str, value: &str) -> Vec<u16> {
    let (path, value) = (wide(path), wide(value));
    let flags = RRF_RT_REG_MULTI_SZ | RRF_RT_REG_SZ;
    let mut size: u32 = 0;
    // SAFETY: NUL-terminated names; a size query, then a buffer that size.
    unsafe {
        if RegGetValueW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            value.as_ptr(),
            flags,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        ) != ERROR_SUCCESS
        {
            return Vec::new();
        }
        let mut data = vec![0u16; (size as usize).div_ceil(2)];
        if RegGetValueW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            value.as_ptr(),
            flags,
            std::ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &raw mut size,
        ) != ERROR_SUCCESS
        {
            return Vec::new();
        }
        data.truncate((size as usize) / 2);
        data
    }
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
