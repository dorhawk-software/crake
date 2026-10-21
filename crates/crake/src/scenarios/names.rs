//! Names a tunnel's DNS knows differently: a public name with a private
//! address (split-horizon), and a tunnel's DNS that stops answering (7h).

use std::net::Ipv6Addr;
use std::time::Duration;

use super::platform::{self, ACME_DNS, look_up, look_up6};
use super::{Up, WAIT};

/// A name ACME's DNS knows, asked before its DNS goes quiet.
const WIKI: (&str, &str) = ("wiki.acme.internal", "10.40.1.11");
/// A name under ACME's domain nobody asked before the outage.
const NEVER_ASKED: &str = "new.acme.internal";
/// How long craked gets to notice its DNS stopped, or answers again: it
/// asks every 30 s and calls two silences in a row an outage.
const NOTICED: Duration = Duration::from_secs(150);

/// A name under ACME's own domain (its gateway's: acme.example on Linux,
/// dorhawk.software through Cloudflare) that public DNS gives a public
/// address and ACME's DNS a private one: the private one goes through the
/// tunnel and is the answer the app gets (a company's portal that resets
/// everyone outside it), and its public IPv6 addresses aren't given. A public name ACME's DNS has nothing else for
/// keeps its public answer.
pub(crate) async fn a_public_name_with_a_private_address_goes_through(up: &mut Up) {
    let (name, at) = platform::SPLIT;
    let found = look_up(name).await;
    if !found.iter().map(ToString::to_string).eq([at.to_owned()]) {
        up.app.settle(Duration::from_secs(1)).await;
        panic!(
            "{name}: {found:?}\n{}\n{}\ncraked said:\n{}",
            platform::name_diagnosis(name),
            platform::diagnose(&up.device),
            up.said()
        );
    }
    assert_eq!(
        platform::way(at),
        up.device,
        "{at}'s way, before any connection"
    );
    assert!(platform::answers(at).await, "{name} at {at}");
    // Nor its public IPv6 addresses, which an app that tries IPv6 first
    // would take around the tunnel (public DNS gives it both).
    let six = look_up6(name).await;
    assert!(six.iter().all(Ipv6Addr::is_unique_local), "{name}: {six:?}");
    let public = platform::PUBLIC_UNDER_ORG;
    let found = look_up(public).await;
    assert!(
        !found.is_empty() && found.iter().all(|a| !a.is_private()),
        "{public}: {found:?}"
    );
}

/// ACME's DNS stops answering while the tunnel is up: craked notices and
/// says so, the panel says so, a name it has seen is answered with its
/// last address (which still answers through the tunnel), one it hasn't
/// fails; when the DNS answers again, that's said and names work again.
pub(crate) async fn a_tunnel_dns_that_stops_answering_is_stood_in_for(up: &mut Up) {
    let (name, at) = WIKI;
    let found = look_up(name).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{name}: {found:?}"
    );
    let label = up.label;
    let tunnel = up.name;
    let silenced = platform::silence(ACME_DNS);
    let asked = tokio::task::spawn_blocking(move || platform::acme_dns_answers(name, at))
        .await
        .unwrap();
    assert!(
        !asked,
        "{ACME_DNS} still answers: the test's block didn't take"
    );
    let said = format!("{label}'s DNS ({ACME_DNS}) doesn't answer.");
    up.app
        .within(NOTICED, "the outage said", |m| {
            m.log.iter().any(|e| e.message == said)
        })
        .await;
    up.wait_for("the panel say its DNS doesn't answer", |m| {
        m.tunnels.iter().any(|t| t.name == tunnel && t.dns_down)
    })
    .await;
    // Asked afresh, as after the system's cache let it go: noticed within
    // seconds, the system could still answer from its own cache (CI's Mac).
    platform::flush_dns_cache();
    let found = look_up(name).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{name} during the outage: {found:?}\n{}\n{}",
        platform::diagnose(&up.device),
        platform::name_diagnosis(name)
    );
    let from_memory =
        format!("{label}'s DNS doesn't answer. Using the address {name} had last time.");
    up.app
        .within(WAIT, "the answer from memory said", |m| {
            m.log.iter().any(|e| e.message == from_memory)
        })
        .await;
    let unknown = look_up(NEVER_ASKED).await;
    assert!(
        unknown.is_empty(),
        "{NEVER_ASKED} during the outage: {unknown:?}"
    );
    drop(silenced);
    let again = format!("{label}'s DNS answers again.");
    up.app
        .within(NOTICED, "the DNS back", |m| {
            m.log.iter().any(|e| e.message == again)
        })
        .await;
    up.app
        .until("the panel no longer says it", |m| {
            m.tunnels.iter().any(|t| t.name == tunnel && !t.dns_down)
        })
        .await;
}

/// Another program rewrites the tunnel's DNS setup (openvpn3 on Linux makes
/// its link every name's DNS on a reconnect; a VPN client clears macOS'
/// /etc/resolver or Windows' NRPT rules): craked puts its own back within
/// moments, and the tunnel's names work.
pub(crate) async fn another_programs_dns_change_is_put_back(up: &mut Up) {
    let dev = up.device.clone();
    let out = platform::rewrite_tunnel_dns(&dev);
    assert!(out.trim().is_empty(), "rewriting {dev}'s DNS: {out}");
    let end = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let (back, how) = platform::tunnel_dns_is_crakes(&dev);
        if back {
            break;
        }
        assert!(
            std::time::Instant::now() < end,
            "{dev}'s DNS not put back:\n{how}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let (name, at) = WIKI;
    let end = std::time::Instant::now() + WAIT;
    while !look_up(name).await.iter().any(|a| a.to_string() == at) {
        assert!(
            std::time::Instant::now() < end,
            "{name} after its DNS was put back"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Another VPN client serving ACME's domain (on Linux a link of its own in
/// resolved, `othervpn0`, whose DNS is the network's public DNS stand-in,
/// which logs what it's asked), removed again when the scenario ends.
#[cfg(target_os = "linux")]
struct OtherClient;

#[cfg(target_os = "linux")]
impl OtherClient {
    fn serving(domain: &str) -> Self {
        let said = platform::sh(&format!(
            "ip link add othervpn0 type dummy && ip link set othervpn0 up && \
             resolvectl dns othervpn0 203.0.113.53 && resolvectl domain othervpn0 '~{domain}' && \
             resolvectl domain othervpn0"
        ));
        assert!(said.contains(domain), "the other client's link: {said}");
        Self
    }
}

#[cfg(target_os = "linux")]
impl Drop for OtherClient {
    fn drop(&mut self) {
        platform::sh("ip link del othervpn0");
    }
}

/// With another VPN client serving ACME's domain (a colleague's own
/// AnyConnect, 2026-10-14), switching ACME off leaves those names to it:
/// they go to its DNS instead of being refused at once (Linux; macOS'
/// tests/macos.rs and Windows' VM test play theirs: /etc/resolver files
/// and NRPT rules not Crake's).
pub(crate) async fn another_clients_names_are_left_to_it(up: &mut Up) {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = up;
        super::not_here("another client is played on Linux here (macOS: tests/macos.rs)");
    }
    #[cfg(target_os = "linux")]
    {
        let _other = OtherClient::serving("acme.internal");
        up.off().await;
        let before = platform::public_questions().unwrap_or_default();
        let name = WIKI.0;
        let end = std::time::Instant::now() + WAIT;
        loop {
            // craked looks at others' links every 2 s; the other client's
            // DNS says it doesn't know the name, but it was asked.
            let _ = look_up(name).await;
            let after = platform::public_questions().unwrap_or_default();
            let since = after.strip_prefix(before.as_str()).unwrap_or(&after);
            if since.contains(name) {
                break;
            }
            assert!(
                std::time::Instant::now() < end,
                "{name} never reached the other client's DNS:\n{}\ncraked said:\n{}",
                platform::sh("resolvectl domain"),
                up.said()
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        // resolved asks every link whose domain fits: the question reaching
        // the other client's DNS doesn't say Crake left it alone. Its own
        // refusing link mustn't carry the domain.
        let hush = platform::sh("resolvectl domain crake-hush");
        assert!(
            !hush.contains("acme.internal"),
            "Crake keeps acme.internal private though another client serves it: {hush}"
        );
    }
}
