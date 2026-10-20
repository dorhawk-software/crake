//! IT's tunnels (managed.d): its scope holds, and the user can switch it
//! on and off but not change or remove it.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::platform::{self, answers, look_up, way};
use super::{Up, WAIT, state};
use crate::client::{Command, Update};
use crate::test_app::{App, get};

/// What IT lets through: ACME's internal names and git's address.
pub(crate) const SCOPE: &str = r#"learn = ["*.acme.internal", "10.40.1.10/32"]"#;
const GIT: &str = "10.40.1.10";
const WIKI: (&str, &str) = ("wiki.acme.internal", "10.40.1.11");
/// Inside ACME's network, outside IT's scope.
const OUTSIDE: &str = "10.40.1.12";

/// The next refusal craked sends, applying what comes before it.
async fn refused(app: &mut App, what: &str) -> String {
    let end = Instant::now() + WAIT;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match tokio::time::timeout(left, app.updates.recv()).await {
            Ok(Ok(Update::Failed(text))) => return text,
            Ok(Ok(u)) => {
                app.model.apply(u);
            }
            _ => panic!("{what} wasn't refused"),
        }
    }
}

/// Inside IT's scope a host is learned by probing and a name is routed
/// before its first packet; outside it, nothing goes through, by probing
/// or by hand ("ACME IT only allows … through ACME.").
pub(crate) async fn it_s_tunnel_keeps_to_its_scope(up: &mut Up) {
    let (name, label) = (up.name, up.label);
    let policy = up
        .app
        .model
        .tunnels
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.policy.clone())
        .unwrap_or_default();
    assert!(
        policy.contains("*.acme.internal"),
        "the policy shown: {policy:?}"
    );
    let end = Instant::now() + WAIT;
    while !answers(GIT).await {
        assert!(
            Instant::now() < end,
            "{GIT} never answered through {label}\n{}",
            platform::diagnose(&up.device)
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(way(GIT), up.device, "{GIT}, inside IT's scope");
    let (wiki, at) = WIKI;
    let found = look_up(wiki).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{wiki}: {found:?}"
    );
    assert_eq!(way(at), up.device, "{wiki}, a name inside IT's scope");
    up.app
        .send(Command::AddHosts(name.into(), vec![OUTSIDE.into()]));
    let text = refused(&mut up.app, "a host outside IT's scope").await;
    assert!(
        text.starts_with(&format!("{label} IT only allows "))
            && text.ends_with(&format!("through {label}.")),
        "{text}"
    );
    for _ in 0..2 {
        let tried = tokio::task::spawn_blocking(|| get(OUTSIDE, Duration::from_secs(6)))
            .await
            .unwrap();
        assert!(tried.is_err(), "{OUTSIDE} answered: it went through");
    }
    assert_ne!(
        way(OUTSIDE),
        up.device,
        "{OUTSIDE} was learned outside IT's scope\n{}",
        platform::diagnose(&up.device)
    );
}

/// The user switches IT's tunnel off and on, but can't change or remove
/// it: craked says so, and it stays as IT set it.
pub(crate) async fn it_s_tunnel_cant_be_changed_or_removed(up: &mut Up) {
    let (name, label) = (up.name, up.label);
    up.app.send(Command::RemoveTunnel(name.into()));
    let text = refused(&mut up.app, "removing IT's tunnel").await;
    assert_eq!(
        text,
        format!("{label} isn't one you added here, so it can't be removed here.")
    );
    let fields = HashMap::from([("display_name".to_owned(), "Mine".to_owned())]);
    up.app.send(Command::UpdateTunnel(name.into(), fields));
    let text = refused(&mut up.app, "changing IT's tunnel").await;
    assert_eq!(
        text,
        format!("{label} isn't one you added here, so it can't be changed here.")
    );
    assert_eq!(state(&up.app.model, name), "connected", "{label} stays up");
    up.off().await;
    up.app.send(Command::Connect(name.into()));
    up.app
        .within(WAIT, "on again", |m| state(m, name) == "connected")
        .await;
    up.device = up.crake.device(name);
}
