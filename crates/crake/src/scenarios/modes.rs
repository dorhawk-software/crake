//! A tunnel's routing mode, as New tunnel or Edit sets it: "Ask me
//! first" and "Only what I list".

use std::time::{Duration, Instant};

use super::platform::{answers, look_up, way};
use super::{Up, WAIT};
use crate::client::Command;
use crate::test_app::get;

const GIT: &str = "10.40.1.10";
const WIKI: (&str, &str) = ("wiki.acme.internal", "10.40.1.11");
const UNTOUCHED: &str = "10.40.1.12";

/// The tunnel's mode changed as Edit changes it (its own fields, sent
/// back with the new mode and list), applied at once.
async fn set_mode(up: &mut Up, mode: &str, list: &str) {
    let name = up.name;
    let mut own = up
        .app
        .model
        .tunnels
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.own.clone())
        .expect("the tunnel's own fields");
    own.insert("mode".into(), mode.into());
    own.insert("list".into(), list.into());
    up.app.send(Command::UpdateTunnel(name.into(), own));
    let (mode, list) = (mode.to_owned(), list.to_owned());
    up.app
        .within(WAIT, "the new mode", |m| {
            m.tunnels.iter().any(|t| {
                t.name == name
                    && t.own.get("mode") == Some(&mode)
                    && t.own.get("list").map(String::as_str).unwrap_or("") == list
            })
        })
        .await;
}

/// Until `ip` goes through the tunnel.
async fn until_through(up: &Up, ip: &str) {
    let end = Instant::now() + WAIT;
    while way(ip) != up.device {
        assert!(
            Instant::now() < end,
            "{ip} goes {:?}, not {}",
            way(ip),
            up.device
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// `ip` asked for twice, longer than any probe waits: it never answers.
async fn stays_unanswered(ip: &'static str, why: &str) {
    for _ in 0..2 {
        let tried = tokio::task::spawn_blocking(move || get(ip, Duration::from_secs(6)))
            .await
            .unwrap();
        assert!(tried.is_err(), "{ip} answered: {why}");
    }
}

/// "Ask me first": a host that needs the tunnel isn't sent through it;
/// the panel says it needs the tunnel, and "Send through ACME" sends it.
pub(crate) async fn asking_first_sends_a_host_only_when_told(up: &mut Up) {
    set_mode(up, "ask", "").await;
    stays_unanswered(GIT, "sent through without asking").await;
    let said = format!("{GIT} needs {}.", up.label);
    up.app
        .within(WAIT, "the question", |m| {
            m.log.iter().any(|e| {
                e.message == said && e.kind == crake_proto::LogKind::Asked && e.hosts == [GIT]
            })
        })
        .await;
    assert_ne!(way(GIT), up.device, "{GIT} went through before it was sent");
    // The panel's "Send through ACME".
    up.app
        .send(Command::AddHosts(up.name.into(), vec![GIT.into()]));
    until_through(up, GIT).await;
    assert!(answers(GIT).await, "{GIT} once sent");
}

/// "Only what I list": a listed host and a listed name go through, by
/// probing and before the first packet; anything else stays direct until
/// it's added by hand (the user's own list: only IT's scope refuses).
pub(crate) async fn only_what_is_listed_goes_through(up: &mut Up) {
    let (wiki, at) = WIKI;
    set_mode(up, "list", &format!("{wiki}\n{GIT}")).await;
    let end = Instant::now() + WAIT;
    while !answers(GIT).await {
        assert!(Instant::now() < end, "{GIT}, listed, never answered");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    until_through(up, GIT).await;
    let found = look_up(wiki).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{wiki}: {found:?}"
    );
    assert_eq!(way(at), up.device, "{wiki}, listed");
    stays_unanswered(UNTOUCHED, "it isn't listed").await;
    assert_ne!(
        way(UNTOUCHED),
        up.device,
        "{UNTOUCHED} went through unlisted"
    );
    // Added by hand, it's the user's own choice (only IT's scope refuses).
    up.app
        .send(Command::AddHosts(up.name.into(), vec![UNTOUCHED.into()]));
    until_through(up, UNTOUCHED).await;
    assert!(answers(UNTOUCHED).await, "{UNTOUCHED} added by hand");
}
