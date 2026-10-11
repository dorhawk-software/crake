//! Windows and macOS: the links craked has registered with its own DNS
//! server (Linux: systemd-resolved's links), so the server knows whose a
//! name is: a tunnel's (asked through it), or craked's own `crake-hush`
//! (refused) and `crake-keep` (answered from memory). Each system sends
//! the names there its own way (`resolved`: NRPT rules, /etc/resolver).

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// A link's DNS servers and domains, and whether every other name goes
/// to it too (a full tunnel's DNS).
type Link = (Vec<Ipv4Addr>, Vec<String>, bool);

/// Each link craked registered, by device.
static LINKS: Mutex<BTreeMap<String, Link>> = Mutex::new(BTreeMap::new());

/// Public names come to craked's server too (while a tunnel is up): it
/// asks the system's own DNS servers and can rescue what they don't know.
static PUBLIC: AtomicBool = AtomicBool::new(false);

fn links() -> std::sync::MutexGuard<'static, BTreeMap<String, Link>> {
    LINKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn set(device: &str, servers: &[Ipv4Addr], domains: &[String], everything: bool) {
    links().insert(
        device.to_owned(),
        (servers.to_vec(), domains.to_vec(), everything),
    );
}

pub fn remove(device: &str) {
    links().remove(device);
}

/// Whether public names come to craked's server now.
pub fn public() -> bool {
    PUBLIC.load(Ordering::Relaxed)
}

/// Windows sends public names to craked's server (its NRPT root rule);
/// macOS doesn't yet.
#[cfg(windows)]
pub fn set_public(on: bool) {
    PUBLIC.store(on, Ordering::Relaxed);
}

/// The link that answers for `name` (its most specific domain; failing
/// that, a full tunnel's), with that link's servers.
pub fn owner(name: &str) -> Option<(String, Vec<Ipv4Addr>)> {
    links()
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    #[test]
    fn the_most_specific_domain_wins_and_a_full_tunnel_takes_the_rest() {
        let dns = |n| vec![Ipv4Addr::new(10, 0, 0, n)];
        super::set("crake-acme", &dns(1), &["acme.internal".into()], false);
        super::set("crake-dev", &dns(2), &["dev.acme.internal".into()], false);
        let owner = |name| super::owner(name).map(|(d, _)| d);
        assert_eq!(owner("git.acme.internal").as_deref(), Some("crake-acme"));
        assert_eq!(owner("ci.dev.acme.internal").as_deref(), Some("crake-dev"));
        assert_eq!(owner("example.org"), None);
        super::set("crake-globex", &dns(3), &[], true);
        assert_eq!(owner("example.org").as_deref(), Some("crake-globex"));
        assert_eq!(owner("git.acme.internal").as_deref(), Some("crake-acme"));
        for d in ["crake-acme", "crake-dev", "crake-globex"] {
            super::remove(d);
        }
        assert_eq!(owner("git.acme.internal"), None);
    }
}
