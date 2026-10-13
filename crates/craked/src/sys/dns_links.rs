//! Windows and macOS: the links craked has registered with its own DNS
//! server (Linux: systemd-resolved's links), so the server knows whose a
//! name is: a tunnel's (asked through it), or craked's own `crake-hush`
//! (refused) and `crake-keep` (answered from memory). Each system sends
//! the names there its own way (`resolved`: NRPT rules, /etc/resolver).

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// A link's DNS servers and domains, and whether every other name goes
/// to it too (a full tunnel's DNS).
type Link = (Vec<IpAddr>, Vec<String>, bool);

/// Each link craked registered, by device.
static LINKS: Mutex<BTreeMap<String, Link>> = Mutex::new(BTreeMap::new());

/// Public names come to craked's server too (while a tunnel is up): it
/// asks the system's own DNS servers and can rescue what they don't know.
static PUBLIC: AtomicBool = AtomicBool::new(false);
/// The system has its own DNS back for a while (its servers stopped answering).
static GIVEN_BACK: AtomicBool = AtomicBool::new(false);

fn links() -> std::sync::MutexGuard<'static, BTreeMap<String, Link>> {
    LINKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn set(device: &str, servers: &[IpAddr], domains: &[String], everything: bool) {
    links().insert(
        device.to_owned(),
        (servers.to_vec(), domains.to_vec(), everything),
    );
}

pub fn remove(device: &str) {
    links().remove(device);
}

/// Whether public names come to craked's server now: wanted, and not
/// given back to the system for a while (`stand_in::Watch`).
pub fn public() -> bool {
    PUBLIC.load(Ordering::Relaxed) && !GIVEN_BACK.load(Ordering::Relaxed)
}

/// The system's own servers stopped answering craked: it has its DNS back
/// for now (true), or craked stands in again (false).
pub fn set_given_back(on: bool) {
    GIVEN_BACK.store(on, Ordering::Relaxed);
}

/// Public names come to craked's server (Windows: its NRPT root rule;
/// macOS: its server in the primary service's DNS settings).
pub fn set_public(on: bool) {
    PUBLIC.store(on, Ordering::Relaxed);
}

/// A full tunnel's link is registered: every name is its.
#[cfg(target_os = "macos")]
pub fn everything() -> bool {
    links().values().any(|(_, _, everything)| *everything)
}

/// The link that answers for `name` (its most specific domain; failing
/// that, a full tunnel's), with that link's servers. On a tie a tunnel
/// that's up wins over craked's own links: a domain kept private for a
/// tunnel that's off may be one a tunnel that's up has too.
pub fn owner(name: &str) -> Option<(String, Vec<IpAddr>)> {
    links()
        .iter()
        .filter_map(|(device, (servers, domains, everything))| {
            let best = domains
                .iter()
                .filter(|d| crate::dns::in_domains(name, std::slice::from_ref(*d)))
                .map(|d| d.len() + 1)
                .max()
                .or(everything.then_some(0))?;
            let own = device == crate::hush::DEVICE || device == crate::keep::DEVICE;
            Some(((best, !own), device, servers))
        })
        .max_by_key(|(rank, _, _)| *rank)
        .map(|(_, device, servers)| (device.clone(), servers.clone()))
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    fn owner(name: &str) -> Option<String> {
        super::owner(name).map(|(d, _)| d)
    }

    fn dns(n: u8) -> Vec<std::net::IpAddr> {
        vec![Ipv4Addr::new(10, 0, 0, n).into()]
    }

    // One test: the links are one table for the whole process.
    #[test]
    fn whose_a_name_is() {
        // The most specific domain wins; a full tunnel takes the rest.
        super::set("crake-acme", &dns(1), &["acme.internal".into()], false);
        super::set("crake-dev", &dns(2), &["dev.acme.internal".into()], false);
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

        // ACME's names kept private while it's off; Nordbank, up, has the
        // same domain: Nordbank answers.
        super::set(crate::hush::DEVICE, &[], &["acme.internal".into()], false);
        assert_eq!(
            owner("wiki.acme.internal").as_deref(),
            Some(crate::hush::DEVICE)
        );
        super::set("crake-nordbank", &dns(4), &["acme.internal".into()], false);
        assert_eq!(
            owner("wiki.acme.internal").as_deref(),
            Some("crake-nordbank")
        );
        super::remove("crake-nordbank");
        super::remove(crate::hush::DEVICE);
    }
}
