//! macOS: a link's DNS and domains (Linux: systemd-resolved's per-link
//! DNS) as /etc/resolver files, one per domain, which send the domain's
//! names to craked's own DNS server (`dns_queue`); it looks up whose a name
//! is in `sys::dns_links`. Each file says it's Crake's and whose; files
//! someone else wrote are left alone. Every other name (the public-name
//! rescue, a full tunnel's DNS) can't be sent by a file: craked's server
//! stands in for the primary service's DNS servers then (`primary_dns`).

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::dns_queue::{ADDR, PORT};

const DIR: &str = "/etc/resolver";
/// The first line of every file Crake writes, then the link's device.
const MARK: &str = "# Written by Crake for ";

/// Public names to craked's server (while a tunnel is up), or not.
pub fn set_public(on: bool) {
    super::super::dns_links::set_public(on);
    every_name();
}

/// craked's server gets every name while public names are wanted or a
/// full tunnel is up.
fn every_name() {
    let links = super::super::dns_links::public() || super::super::dns_links::everything();
    super::primary_dns::want(links);
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

    /// Files a craked before this one left behind go first, and the
    /// system's DNS servers it stood in for come back.
    pub fn new(enabled: bool) -> Self {
        if enabled {
            for (path, _) in ours() {
                let _ = std::fs::remove_file(path);
            }
            super::primary_dns::put_back_left_behind();
            Self::Real
        } else {
            Self::Noop
        }
    }

    /// `domains` go to craked's server, which asks `servers` (or answers
    /// itself for its own links).
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
        every_name();
        std::fs::create_dir_all(DIR)?;
        // Domains it no longer has go.
        for (path, owner) in ours() {
            let domain = path.file_name().map(|n| n.to_string_lossy().into_owned());
            if owner == device && !domain.is_some_and(|d| domains.contains(&d)) {
                let _ = std::fs::remove_file(path);
            }
        }
        for domain in domains.iter().filter(|d| file_name(d)) {
            let path = Path::new(DIR).join(domain);
            if path.exists() && owner(&path).is_none() {
                tracing::warn!("{} isn't Crake's: left as it is", path.display());
                continue;
            }
            std::fs::write(&path, contents(device))?;
        }
        Ok(())
    }

    /// What another program registered for the device (OpenVPN 3 on Linux).
    pub async fn link(&self, _device: &str) -> Result<(Vec<Ipv4Addr>, Vec<String>)> {
        Ok((Vec::new(), Vec::new()))
    }

    /// macOS' DNS cache, emptied.
    pub async fn flush(&self) {
        if let Self::Real = self {
            let _ = super::cmd::run("dscacheutil", &["-flushcache"]).await;
            let _ = super::cmd::run("killall", &["-HUP", "mDNSResponder"]).await;
        }
    }

    /// The link's files removed: its domains go back to the usual DNS.
    pub async fn revert(&self, device: &str) {
        super::super::dns_links::remove(device);
        if let Self::Real = self {
            every_name();
            for (path, owner) in ours() {
                if owner == device {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

/// A file's contents: Crake's mark, then craked's server.
fn contents(device: &str) -> String {
    format!("{MARK}{device}\nnameserver {ADDR}\nport {PORT}\n")
}

/// A domain that can be a file's name.
fn file_name(domain: &str) -> bool {
    !domain.is_empty() && !domain.starts_with('.') && !domain.contains('/')
}

/// Whose a file is, if Crake wrote it.
fn owner(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.lines().next()?.strip_prefix(MARK)?.to_owned())
}

/// Crake's files, with whose each is.
fn ours() -> Vec<(PathBuf, String)> {
    let Ok(dir) = std::fs::read_dir(DIR) else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| Some((e.path(), owner(&e.path())?)))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_file_points_at_crakes_server_and_says_whose_it_is() {
        let text = super::contents("utun5");
        assert!(text.starts_with("# Written by Crake for utun5\n"));
        assert!(text.contains("nameserver 127.53.0.1\nport 53\n"));
        assert!(super::file_name("acme.internal"));
        assert!(!super::file_name("../hosts"));
        assert!(!super::file_name(".acme.internal"));
    }
}
