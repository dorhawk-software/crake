//! Names a tunnel's DNS knows differently: a public name with a private
//! address (split-horizon), and a tunnel's DNS that stops answering (7h).

use std::time::Duration;

use super::platform::{self, ACME_DNS, look_up};
use super::{Up, WAIT};

/// A name ACME's DNS knows, asked before its DNS goes quiet.
const WIKI: (&str, &str) = ("wiki.acme.internal", "10.40.1.11");
/// A name under ACME's domain nobody asked before the outage.
const NEVER_ASKED: &str = "new.acme.internal";
/// How long craked gets to notice its DNS stopped, or answers again: it
/// asks every 30 s and calls two silences in a row an outage.
const NOTICED: Duration = Duration::from_secs(150);

/// A name under ACME's own domain (the gateway's, acme.example) that
/// public DNS gives a public address and ACME's DNS a private one: the
/// private one goes through the tunnel and is the answer the app gets
/// (a company's portal that resets everyone outside it). A public name
/// ACME's DNS has nothing else for keeps its public answer.
#[cfg(target_os = "linux")]
pub(crate) async fn a_public_name_with_a_private_address_goes_through(up: &mut Up) {
    let (name, at) = ("shop.acme.example", "10.40.1.16");
    let found = look_up(name).await;
    assert!(
        found.iter().map(ToString::to_string).eq([at.to_owned()]),
        "{name}: {found:?}\n{}",
        platform::diagnose(&up.device)
    );
    assert_eq!(
        platform::way(at),
        up.device,
        "{at}'s way, before any connection"
    );
    assert!(platform::answers(at).await, "{name} at {at}");
    let (public, public_at) = (
        "www.acme.example",
        platform::PUBLIC.split(':').next().unwrap(),
    );
    let found = look_up(public).await;
    assert!(
        found
            .iter()
            .map(ToString::to_string)
            .eq([public_at.to_owned()]),
        "{public}: {found:?}"
    );
}

#[cfg(not(target_os = "linux"))]
pub(crate) async fn a_public_name_with_a_private_address_goes_through(_up: &mut Up) {
    super::not_here(
        "split-horizon needs the gateway reached by its name and a public DNS the test \
         controls; through Cloudflare the gateway is 127.0.0.1 and public DNS is the internet's",
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
    up.app
        .until("the panel says it", |m| {
            m.tunnels.iter().any(|t| t.name == tunnel && t.dns_down)
        })
        .await;
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

/// Another program makes the tunnel's link every name's DNS (openvpn3
/// does on a reconnect: "~." and the default route, so every name went to
/// the company): craked puts its routing-only setup back within moments.
#[cfg(target_os = "linux")]
pub(crate) async fn another_programs_dns_change_is_put_back(up: &mut Up) {
    let dev = up.device.clone();
    let out = platform::sh(&format!(
        "resolvectl domain {dev} '~.' acme.internal && resolvectl default-route {dev} yes"
    ));
    assert!(out.trim().is_empty(), "rewriting {dev}: {out}");
    let end = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let link = platform::sh(&format!("resolvectl status {dev}"));
        // resolvectl says "Default Route: no", or (older) "-DefaultRoute".
        let back = !link.contains("Default Route: yes")
            && !link.contains("+DefaultRoute")
            && link.contains("~acme.internal")
            && !link.contains("~.");
        if back {
            break;
        }
        assert!(
            std::time::Instant::now() < end,
            "{dev} not put back:\n{link}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) async fn another_programs_dns_change_is_put_back(_up: &mut Up) {
    super::not_here("only Linux's systemd-resolved has per-link DNS another program rewrites");
}
