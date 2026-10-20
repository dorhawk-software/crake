//! IPv6 inside the tunnel (AnyConnect: ocserv hands out an IPv6 address
//! and ACME's IPv6 network; it rides in the tunnel's TLS, so through
//! Cloudflare as well).

use std::time::{Duration, Instant};

use super::platform::{self, look_up6, way};
use super::{Up, WAIT};
use crate::test_app::get;

/// ACME's IPv6 host, and the name only ACME's DNS gives it.
const V6: &str = "fd40:1::10";
const V6_NAME: &str = "v6.acme.internal";

/// An IPv6 host that doesn't answer directly but does through the
/// tunnel: probed and learned, and the app's own retry gets through
/// (Windows can't reset another program's IPv6 connection: the first
/// runs out, the next goes through).
pub(crate) async fn an_ipv6_host_is_learned_by_probing(up: &mut Up) {
    // An app's IPv6 connection on a network without IPv6 fails at once,
    // with no packet to probe from (CLAUDE.md, Not now).
    if !platform::network_has_ipv6() {
        super::not_here("this network has no IPv6 of its own: an app's first try sends nothing");
        return;
    }
    let reached = tokio::task::spawn_blocking(|| {
        let end = Instant::now() + WAIT;
        while Instant::now() < end {
            if get(V6, Duration::from_secs(5)).is_ok_and(|b| b.contains("crex")) {
                return true;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        false
    })
    .await
    .unwrap();
    if !reached {
        up.app.settle(Duration::from_secs(1)).await;
        panic!(
            "{V6} through {}\n{}\ncraked said:\n{}",
            up.label,
            platform::diagnose(&up.device),
            up.said()
        );
    }
    assert_eq!(way(V6), up.device, "{V6}'s way");
}

/// A name whose only address is IPv6, which only the tunnel's DNS knows:
/// routed before the app's first packet.
pub(crate) async fn an_ipv6_name_is_routed_before_its_first_packet(up: &mut Up) {
    // macOS doesn't ask for IPv6 addresses at all on a network without
    // IPv6 of its own (CLAUDE.md, Not now).
    if platform::NAMES_NEED_NETWORK_IPV6 && !platform::network_has_ipv6() {
        super::not_here(
            "this network has no IPv6 of its own, so the system asks no IPv6 addresses",
        );
        return;
    }
    let found = look_up6(V6_NAME).await;
    assert!(
        found.iter().any(|a| a.to_string() == V6),
        "{V6_NAME}: {found:?}\n{}",
        platform::diagnose(&up.device)
    );
    assert_eq!(way(V6), up.device, "{V6}'s way, before any connection");
    let page = tokio::task::spawn_blocking(|| get(V6, Duration::from_secs(5)))
        .await
        .unwrap();
    assert!(page.is_ok_and(|b| b.contains("crex")), "{V6_NAME}");
}
