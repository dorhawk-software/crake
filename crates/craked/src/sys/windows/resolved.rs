//! Windows: a link's DNS and domains (Linux: systemd-resolved's per-link
//! DNS) as NRPT rules, which send the link's domains to craked's own DNS
//! server (`dns_queue::ADDR`). The server looks up here which link a name
//! belongs to: a tunnel (asked through it), or craked's own `crake-hush`
//! (refused) and `crake-keep` (answered from memory). The rules are
//! volatile registry keys: gone at the latest when Windows restarts.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, REG_DWORD, REG_MULTI_SZ, REG_OPTION_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegEnumKeyExW, RegOpenKeyExW, RegSetValueExW,
};

/// Where Windows keeps local NRPT rules.
const POLICY: &str = r"SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig";
/// Crake's rules are named after it and the link.
const PREFIX: &str = "Crake ";

/// A link's DNS servers and domains, and whether every other name goes
/// to it too (a full tunnel's DNS).
type Link = (Vec<Ipv4Addr>, Vec<String>, bool);

/// Each link craked registered, by device.
static LINKS: Mutex<BTreeMap<String, Link>> = Mutex::new(BTreeMap::new());

/// The link that answers for `name` (its most specific domain; failing
/// that, a full tunnel's), with that link's servers.
pub fn owner(name: &str) -> Option<(String, Vec<Ipv4Addr>)> {
    let links = LINKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    links
        .iter()
        .filter_map(|(device, (servers, domains, everything))| {
            let best = domains
                .iter()
                .filter(|d| crate::dns::in_domains(name, std::slice::from_ref(*d)))
                .map(|d| d.len() + 1)
                .max()
                .or(everything.then_some(0))?;
            Some((best, device, servers))
        })
        .max_by_key(|(len, _, _)| *len)
        .map(|(_, device, servers)| (device.clone(), servers.clone()))
}

pub enum Resolved {
    Real,
    Noop,
}

impl Resolved {
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
        servers: &[Ipv4Addr],
        domains: &[String],
        everything: bool,
    ) -> Result<()> {
        if let Self::Noop = self {
            return Ok(());
        }
        LINKS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                device.to_owned(),
                (servers.to_vec(), domains.to_vec(), everything),
            );
        write_rule(device, domains, everything)
    }

    /// What another program registered for the device (OpenVPN 3 on Linux).
    pub async fn link(&self, _device: &str) -> Result<(Vec<Ipv4Addr>, Vec<String>)> {
        Ok((Vec::new(), Vec::new()))
    }

    /// Windows' DNS cache, emptied.
    pub async fn flush(&self) {
        if let Self::Real = self {
            let mut cmd = tokio::process::Command::new("ipconfig");
            cmd.arg("/flushdns")
                .creation_flags(0x0800_0000)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let _ = cmd.status().await;
        }
    }

    /// The link's rule removed: its domains go back to the usual DNS.
    pub async fn revert(&self, device: &str) {
        LINKS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(device);
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
        set(key, "ConfigOptions", REG_DWORD, &8u32.to_le_bytes()),
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
    let path = wide(POLICY);
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: a NUL-terminated path; `key` is closed below.
    if unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            0,
            KEY_ALL_ACCESS,
            &raw mut key,
        )
    } != ERROR_SUCCESS
    {
        return Vec::new();
    }
    let mut found = Vec::new();
    for i in 0.. {
        let mut name = [0u16; 256];
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
        let name = String::from_utf16_lossy(&name[..len as usize]);
        if name.starts_with(PREFIX) {
            found.push(format!("{POLICY}\\{name}"));
        }
    }
    // SAFETY: the key opened above.
    unsafe { RegCloseKey(key) };
    found
}
