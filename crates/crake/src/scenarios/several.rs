//! Two tunnels at once: one that can't be up beside the first, and two
//! companies' that can (Globex's beside each kind of gateway).

use std::time::{Duration, Instant};

use super::platform::{self, ACME_DNS, look_up, way};
use super::{Gateway, Up, WAIT, add, sign_in, sign_in_and_connect, state};
use crate::client::Command;

/// ACME's git, and Globex's (scripts/e2e/companies.sh).
const ACME_GIT: &str = "10.40.1.10";
const GLOBEX_GIT: &str = "10.41.1.10";
/// A host both companies' networks reach.
const SHARED: &str = "10.42.0.10";

/// A second company's tunnel that uses the same DNS server as the one
/// that's up (here: another kind of gateway into ACME's own network)
/// doesn't half-work: craked says which to switch off and why, leaves the
/// first as it was, and the app knows it before the next switch-on.
pub(crate) async fn a_second_tunnel_that_clashes_is_refused(up: &mut Up) {
    let other = up.gateway.other();
    let (name, label) = other.names();
    add(&mut up.app, other).await;
    sign_in(&mut up.app, other, &["hunter2"]).await;
    let said = format!("Turn {} off to use {label}.", up.label);
    let why = format!("Both use {ACME_DNS} for DNS.");
    up.app
        .within(WAIT, "the clash said", |m| {
            m.log
                .iter()
                .any(|e| e.message == said && e.reason.as_deref() == Some(why.as_str()))
        })
        .await;
    up.app
        .within(WAIT, "the second one off", |m| state(m, name) == "off")
        .await;
    let first = up.name;
    assert_eq!(
        state(&up.app.model, first),
        "connected",
        "{} stays up",
        up.label
    );
    let (wiki, at) = ("wiki.acme.internal", "10.40.1.11");
    let found = look_up(wiki).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{wiki} through {}: {found:?}\n{}",
        up.label,
        platform::diagnose(&up.device)
    );
    assert_eq!(way(at), up.device, "{at} goes through {}", up.label);
    let before = format!("{said} {why}");
    up.app
        .within(WAIT, "the clash known before switching on", |m| {
            m.tunnels
                .iter()
                .any(|t| t.name == name && t.clash == before)
        })
        .await;
}

/// Globex's tunnel (the AnyConnect gateway's other company) up beside the
/// one that's up: its device.
async fn globex_beside(up: &mut Up) -> String {
    add(&mut up.app, Gateway::Globex).await;
    sign_in_and_connect(&mut up.app, Gateway::Globex, &["hunter2"]).await;
    assert_eq!(
        state(&up.app.model, up.name),
        "connected",
        "{} stays up",
        up.label
    );
    up.crake.device("globex")
}

/// `ip`'s page, retried until it answers (the first try may wait for the
/// probe).
async fn reached(ip: &'static str) -> bool {
    let end = Instant::now() + WAIT;
    while Instant::now() < end {
        if platform::answers(ip).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

/// Two companies' tunnels up at once: each one's names and hosts go
/// through its own, the internet through neither.
pub(crate) async fn two_companies_each_through_their_own(up: &mut Up) {
    let globex = globex_beside(up).await;
    let (wiki, at) = ("wiki.acme.internal", "10.40.1.11");
    let found = look_up(wiki).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{wiki}: {found:?}"
    );
    assert_eq!(way(at), up.device, "{wiki} through {}", up.label);
    let (git, git_at) = ("git.globex.internal", GLOBEX_GIT);
    let found = look_up(git).await;
    assert!(
        found.iter().any(|a| a.to_string() == git_at),
        "{git}: {found:?}\n{}",
        platform::diagnose(&globex)
    );
    assert_eq!(way(git_at), globex, "{git} through Globex");
    assert!(platform::answers(git_at).await, "{git} at {git_at}");
    assert!(reached(ACME_GIT).await, "{ACME_GIT} probed");
    assert_eq!(way(ACME_GIT), up.device, "{ACME_GIT} through {}", up.label);
    let public = platform::PUBLIC.split(':').next().unwrap();
    let out = way(public);
    assert!(out != up.device && out != globex, "{public} goes {out:?}");
}

/// A host both companies' tunnels reach goes through the first in order,
/// the log says the other answers for it too, and "Use Globex" moves it.
pub(crate) async fn a_host_both_answer_for_can_be_moved(up: &mut Up) {
    let globex = globex_beside(up).await;
    assert!(reached(SHARED).await, "{SHARED} probed");
    assert_eq!(way(SHARED), up.device, "{SHARED}: the first in order");
    let too = format!("Globex answers for {SHARED} too.");
    up.app
        .within(WAIT, "the other said", |m| {
            m.log
                .iter()
                .any(|e| e.message == too && e.others == ["globex"])
        })
        .await;
    up.app.send(Command::Move(
        up.name.into(),
        "globex".into(),
        vec![SHARED.into()],
    ));
    let end = Instant::now() + WAIT;
    while way(SHARED) != globex {
        assert!(
            Instant::now() < end,
            "{SHARED} goes {:?}, not Globex",
            way(SHARED)
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(platform::answers(SHARED).await, "{SHARED} through Globex");
}

/// Globex's DNS gives one of its names the address ACME's git has, which
/// already goes through the tunnel that's up: routing goes by address, so
/// it stays there, and craked says the name can't go through Globex.
pub(crate) async fn one_address_goes_through_one_tunnel(up: &mut Up) {
    let _globex = globex_beside(up).await;
    assert!(reached(ACME_GIT).await, "{ACME_GIT} probed");
    assert_eq!(way(ACME_GIT), up.device, "{ACME_GIT} through {}", up.label);
    let dup = "dup.globex.internal";
    let _ = look_up(dup).await;
    let said = format!("{dup} can't go through Globex.");
    up.app
        .within(WAIT, "the shared address said", |m| {
            m.log
                .iter()
                .any(|e| e.message == said && e.others == ["globex"])
        })
        .await;
    assert_eq!(
        way(ACME_GIT),
        up.device,
        "{ACME_GIT} stays with {}",
        up.label
    );
}
