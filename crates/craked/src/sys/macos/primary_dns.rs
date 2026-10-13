//! macOS: every name to craked's own DNS server, as Windows' NRPT rule
//! for the root does. An /etc/resolver file names a resolver for a domain,
//! never for every name (`resolved`), so while craked wants them (a tunnel
//! is up: the public-name rescue; a full tunnel: every name to its DNS)
//! craked's server takes the place of the primary network service's DNS
//! servers in the dynamic store, as vpnc-script and other VPN clients do:
//! `State:/Network/Service/<id>/DNS` (DHCP's), and `Setup:`'s when the user
//! typed servers in System Settings (they win over DHCP's). The servers it
//! displaced are the ones craked asks for public names (`upstream`).
//! macOS writes its own again on a DHCP renewal or a change of network: a
//! watch on those keys takes their place again, and the new servers are
//! asked from then on. What was displaced is kept in a file as well, so a
//! craked that stopped without putting it back does so when it starts.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, Once};

use serde::{Deserialize, Serialize};
use system_configuration::core_foundation::array::CFArray;
use system_configuration::core_foundation::base::{CFType, TCFType};
use system_configuration::core_foundation::dictionary::CFDictionary;
use system_configuration::core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use system_configuration::core_foundation::string::CFString;
use system_configuration::dynamic_store::{
    SCDynamicStore, SCDynamicStoreBuilder, SCDynamicStoreCallBackContext,
};

use super::dns_queue::ADDR;

/// What craked displaced, while it is displaced.
const SAVED: &str = "/var/db/crake/system-dns.json";
/// The keys watched: which service is primary, and each service's DNS.
const GLOBAL: &str = "State:/Network/Global/IPv[46]";
const SERVICE_DNS: &str = "(State|Setup):/Network/Service/[^/]+/DNS";
const PRIMARY_KEYS: [&str; 2] = ["State:/Network/Global/IPv4", "State:/Network/Global/IPv6"];
const PRIMARY_SERVICE: &str = "PrimaryService";
const SERVERS: &str = "ServerAddresses";
const SETUP: &str = "Setup:";
const STATE: &str = "State:";

/// What a key held before craked's server took its place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Was {
    /// There was no such key.
    Absent,
    /// The key, naming these servers (none: it named none).
    Servers(Vec<String>),
}

/// Each key craked's server stands in, with what it held.
static DISPLACED: Mutex<BTreeMap<String, Was>> = Mutex::new(BTreeMap::new());
/// Whether craked wants every name now (the watch asks).
static WANTED: AtomicBool = AtomicBool::new(false);
static WATCH: Once = Once::new();

fn displaced() -> MutexGuard<'static, BTreeMap<String, Was>> {
    DISPLACED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Every name to craked's server, or each service's own servers back.
pub fn want(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
    if on {
        watch();
    }
    apply(on);
}

/// What a craked before this one left displaced, put back.
pub fn put_back_left_behind() {
    let Ok(text) = std::fs::read_to_string(SAVED) else {
        return;
    };
    if let Ok(left) = serde_json::from_str::<BTreeMap<String, Was>>(&text) {
        displaced().extend(left);
    }
    apply(false);
}

/// Whether craked's server stands in for the system's DNS servers now.
pub fn standing_in() -> bool {
    !displaced().is_empty()
}

/// The servers craked's server stands in for, asked for public names:
/// the ones the user typed, or else DHCP's.
pub fn upstream() -> Vec<IpAddr> {
    let displaced = displaced();
    let named = |prefix: &str| {
        displaced.iter().find_map(|(key, was)| match was {
            Was::Servers(s) if key.starts_with(prefix) => Some(s.clone()),
            _ => None,
        })
    };
    let (typed, dhcp) = (named(SETUP), named(STATE));
    crate::stand_in::servers_to_ask(typed.as_deref(), dhcp.as_deref(), IpAddr::V4(ADDR))
}

/// Bring the dynamic store in line with what's wanted: the primary
/// service's DNS keys stood in for (`on`), every other key craked stood in
/// for given back what it had.
fn apply(on: bool) {
    let mut displaced = displaced();
    let store = SCDynamicStoreBuilder::new("Crake").build();
    let mut keys: Vec<String> = on
        .then(|| primary_service(&store))
        .flatten()
        .map(|id| {
            [STATE, SETUP]
                .map(|kind| format!("{kind}/Network/Service/{id}/DNS"))
                .to_vec()
        })
        .unwrap_or_default();
    // Only with a server to ask in their stead (what's displaced already
    // counts, under the key's own name): otherwise every name would fail.
    if !keys.is_empty() {
        let servers = |key: &String| match displaced.get(key) {
            Some(Was::Servers(s)) => Some(s.clone()),
            Some(Was::Absent) => None,
            None => dictionary(&store, key)
                .as_ref()
                .and_then(servers_of)
                .filter(|s| s[..] != [ADDR.to_string()]),
        };
        let (dhcp, typed) = (servers(&keys[0]), servers(&keys[1]));
        if !crate::stand_in::may_stand_in(typed.as_deref(), dhcp.as_deref(), IpAddr::V4(ADDR)) {
            tracing::warn!(
                "not standing in for the system's DNS: no server of its to ask (DHCP's {dhcp:?}, typed {typed:?})"
            );
            keys.clear();
        }
    }
    let mut changed = false;
    let gone: Vec<String> = displaced
        .keys()
        .filter(|k| !keys.contains(k))
        .cloned()
        .collect();
    for key in gone {
        if let Some(was) = displaced.remove(&key) {
            changed |= put_back(&store, &key, &was);
        }
    }
    let ours = [ADDR.to_string()];
    for key in &keys {
        let current = dictionary(&store, key);
        let servers = current.as_ref().and_then(servers_of);
        if servers.as_deref() == Some(&ours[..]) {
            continue;
        }
        // Setup's servers matter only when the user typed some.
        if key.starts_with(SETUP) && servers.as_ref().is_none_or(Vec::is_empty) {
            displaced.remove(key);
            continue;
        }
        let was = match &current {
            None => Was::Absent,
            Some(_) => Was::Servers(servers.unwrap_or_default()),
        };
        if store.set(key.as_str(), with_servers(current.as_ref(), Some(&ours))) {
            displaced.insert(key.clone(), was);
            changed = true;
        } else {
            tracing::warn!("couldn't point {key} at craked's DNS server");
        }
    }
    if changed {
        keep(&displaced);
        flush();
    }
}

/// `key` given back what it had, if it's still craked's (macOS may have
/// written its own since).
fn put_back(store: &SCDynamicStore, key: &str, was: &Was) -> bool {
    let current = dictionary(store, key);
    let ours = [ADDR.to_string()];
    if current.as_ref().and_then(servers_of).as_deref() != Some(&ours[..]) {
        return false;
    }
    match was {
        Was::Absent => store.remove(key),
        Was::Servers(s) => store.set(
            key,
            with_servers(current.as_ref(), (!s.is_empty()).then_some(s)),
        ),
    }
}

/// What's displaced, in the file (gone when nothing is).
fn keep(displaced: &BTreeMap<String, Was>) {
    if displaced.is_empty() {
        let _ = std::fs::remove_file(SAVED);
        return;
    }
    if let Some(dir) = std::path::Path::new(SAVED).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(displaced)
        && let Err(e) = std::fs::write(SAVED, text)
    {
        tracing::warn!("keeping the system's DNS servers in {SAVED}: {e}");
    }
}

/// macOS' DNS cache, emptied: names it looked up before are asked again.
fn flush() {
    let _ = std::process::Command::new("dscacheutil")
        .arg("-flushcache")
        .status();
    let _ = std::process::Command::new("killall")
        .args(["-HUP", "mDNSResponder"])
        .status();
}

/// Each change to the watched keys brings the store in line again.
fn watch() {
    WATCH.call_once(|| {
        let started = std::thread::Builder::new()
            .name("crake-primary-dns".into())
            .spawn(|| {
                let store = SCDynamicStoreBuilder::new("Crake")
                    .callback_context(SCDynamicStoreCallBackContext {
                        callout: changed,
                        info: (),
                    })
                    .build();
                let keys: CFArray<CFString> = CFArray::from_CFTypes(&[]);
                let patterns = CFArray::from_CFTypes(&[
                    CFString::from_static_string(GLOBAL),
                    CFString::from_static_string(SERVICE_DNS),
                ]);
                if !store.set_notification_keys(&keys, &patterns) {
                    tracing::warn!("couldn't watch the system's DNS settings");
                    return;
                }
                let source = store.create_run_loop_source();
                // SAFETY: CoreFoundation's constant, only read.
                CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
                CFRunLoop::run_current();
            });
        if let Err(e) = started {
            tracing::warn!("watching the system's DNS settings: {e}");
        }
    });
}

fn changed(_store: SCDynamicStore, _keys: CFArray<CFString>, _info: &mut ()) {
    apply(WANTED.load(Ordering::Relaxed));
}

/// The primary network service's id (the IPv4 one's, else the IPv6 one's).
fn primary_service(store: &SCDynamicStore) -> Option<String> {
    PRIMARY_KEYS.iter().find_map(|key| {
        let global = dictionary(store, key)?;
        value(&global, PRIMARY_SERVICE)?
            .downcast::<CFString>()
            .map(|s| s.to_string())
    })
}

fn dictionary(store: &SCDynamicStore, key: &str) -> Option<CFDictionary> {
    store.get(key)?.downcast_into::<CFDictionary>()
}

/// A dictionary's value under a string key.
fn value(dict: &CFDictionary, key: &'static str) -> Option<CFType> {
    let key = CFString::from_static_string(key);
    let item = dict.find(key.as_CFTypeRef())?;
    // SAFETY: a value the dictionary holds; the wrapper retains it.
    Some(unsafe { CFType::wrap_under_get_rule(*item) })
}

/// The servers a service's DNS dictionary names.
fn servers_of(dict: &CFDictionary) -> Option<Vec<String>> {
    let list = value(dict, SERVERS)?.downcast::<CFArray>()?;
    Some(
        list.iter()
            .filter_map(|item| {
                // SAFETY: an element the array holds; the wrapper retains it.
                unsafe { CFType::wrap_under_get_rule(*item) }
                    .downcast::<CFString>()
                    .map(|s| s.to_string())
            })
            .collect(),
    )
}

/// `base` (its search domains and the rest kept) naming `servers`
/// instead (none: naming none).
fn with_servers(base: Option<&CFDictionary>, servers: Option<&[String]>) -> CFDictionary {
    let name = CFString::from_static_string(SERVERS);
    let mut pairs: Vec<(CFType, CFType)> = Vec::new();
    if let Some(base) = base {
        let (keys, values) = base.get_keys_and_values();
        for (k, v) in keys.into_iter().zip(values) {
            // SAFETY: keys and values the dictionary holds; the wrappers
            // retain them.
            let (k, v) = unsafe {
                (
                    CFType::wrap_under_get_rule(k),
                    CFType::wrap_under_get_rule(v),
                )
            };
            if k.downcast::<CFString>().is_some_and(|s| s == name) {
                continue;
            }
            pairs.push((k, v));
        }
    }
    if let Some(servers) = servers {
        let list: Vec<CFString> = servers.iter().map(|s| CFString::new(s)).collect();
        pairs.push((name.as_CFType(), CFArray::from_CFTypes(&list).as_CFType()));
    }
    CFDictionary::from_CFType_pairs(&pairs).into_untyped()
}
