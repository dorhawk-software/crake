//! Whether Windows completes short names with the tunnel adapters' suffix
//! (`dns::SHORT_SUFFIX`) yet. CI (2026-11-04): "git", asked again and
//! again from just after the tunnel was up, never reached craked in the
//! scenario's minute, though the suffix and craked's server were on the
//! adapter; asked once more just after, it did. Set-DnsClient returns at
//! once, and Windows' DNS client seems to take the suffix later, keeping
//! a short name asked meanwhile as unknown. So craked asks a throwaway
//! short name of its own, as an app would, until its server sees it under
//! the suffix, then empties Windows' DNS cache, and says in its log how
//! long that took. craked answers the throwaway names itself, under any
//! suffix it's sent them with, and never asks a tunnel about them. With
//! Crake's root rule Windows sends it every name; without it (the system
//! given its DNS back), the network's DNS is asked one under the
//! network's suffix, as with any short name typed.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The throwaway names (crake-probe-1, crake-probe-2…), each asked once:
/// asked again, Windows would answer from its cache. Never a host's
/// (craked answers them itself, and no tunnel is asked).
const PROBE: &str = "crake-probe-";
/// How long craked keeps asking.
const FOR: Duration = Duration::from_secs(120);
/// Between two asks.
const EVERY: Duration = Duration::from_millis(500);

/// The last throwaway name asked.
static ASKED: AtomicU64 = AtomicU64::new(0);
/// The last throwaway name craked's server was asked under the suffix.
static SEEN: AtomicU64 = AtomicU64::new(0);
/// A probe is running (another adapter's suffix needs none of its own:
/// one adapter's is enough for Windows to ask craked).
static RUNNING: AtomicBool = AtomicBool::new(false);

/// craked's server: `name` is one of the throwaway names (answered "no
/// such name"); under the suffix, it's noted as seen.
pub fn is_probe(name: &str) -> bool {
    let (first, rest) = name.split_once('.').unwrap_or((name, ""));
    let Some(number) = first
        .strip_prefix(PROBE)
        .and_then(|n| n.parse::<u64>().ok())
    else {
        return false;
    };
    if rest == crate::dns::SHORT_SUFFIX {
        SEEN.fetch_max(number, Ordering::Relaxed);
    }
    true
}

/// A tunnel adapter just took the suffix: ask until Windows uses it, then
/// empty its DNS cache. In the background; the tunnel doesn't wait.
pub fn start() {
    if RUNNING.swap(true, Ordering::Relaxed) {
        return;
    }
    tokio::spawn(async {
        let started = Instant::now();
        let before = ASKED.load(Ordering::Relaxed);
        let used = loop {
            let number = ASKED.fetch_add(1, Ordering::Relaxed) + 1;
            let name = format!("{PROBE}{number}");
            // Unknown everywhere: only whether craked was asked counts.
            let _ = tokio::net::lookup_host((name.as_str(), 0)).await;
            if SEEN.load(Ordering::Relaxed) > before {
                break true;
            }
            if started.elapsed() > FOR {
                break false;
            }
            tokio::time::sleep(EVERY).await;
        };
        if used {
            let _ = super::host::powershell("emptying the DNS cache", "Clear-DnsClientCache").await;
            tracing::info!(
                "Windows completes short names with {} after {:?}",
                crate::dns::SHORT_SUFFIX,
                started.elapsed()
            );
        } else {
            tracing::warn!(
                "Windows didn't complete short names with {} in {FOR:?}",
                crate::dns::SHORT_SUFFIX
            );
        }
        RUNNING.store(false, Ordering::Relaxed);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_throwaway_names_are_answered_and_counted_under_the_suffix() {
        assert!(is_probe("crake-probe-7.crake.internal"));
        assert!(SEEN.load(Ordering::Relaxed) >= 7);
        // The network's suffix: answered too, not counted.
        assert!(is_probe("crake-probe-9000.example.internal"));
        assert!(SEEN.load(Ordering::Relaxed) < 9000);
        assert!(!is_probe("git.crake.internal"));
        assert!(!is_probe("crake-probe-x.crake.internal"));
    }
}
