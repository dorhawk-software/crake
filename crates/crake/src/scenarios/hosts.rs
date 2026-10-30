//! The scenarios of what a person does with hosts: Undo and Redo, adding
//! a host or a name by hand, pins, Forget and Forget all, the never list,
//! and tunnel detail's list.

use std::time::{Duration, Instant};

use super::platform::{answers, way};
use super::{Up, WAIT};
use crate::client::Command;
use crate::test_app::get;

const GIT: &str = "10.40.1.10";
const WIKI: (&str, &str) = ("wiki.acme.internal", "10.40.1.11");
const UNTOUCHED: &str = "10.40.1.12";
/// ACME's hosts' network, for a pin.
const HOSTS_NET: &str = "10.40.1.0/24";
/// How long a change made from the app gets to reach the system.
const APPLIED: Duration = Duration::from_secs(10);

/// Until `ip` goes through the tunnel (`through`) or not.
async fn until_way(up: &Up, ip: &str, through: bool) {
    let end = Instant::now() + APPLIED;
    loop {
        let now = way(ip);
        if (now == up.device) == through {
            return;
        }
        assert!(
            Instant::now() < end,
            "{ip} goes {now:?}, {} {}",
            if through {
                "not through"
            } else {
                "still through"
            },
            up.device
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// GIT learned by probing, as a person's first visit learns it.
async fn learn_git(up: &Up) {
    let end = Instant::now() + WAIT;
    while !answers(GIT).await {
        assert!(
            Instant::now() < end,
            "{GIT} never answered through the tunnel"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    until_way(up, GIT, true).await;
}

/// Undo sends a learned host direct and keeps it there, even when it's
/// asked for again; Redo sends it through again.
pub(crate) async fn undo_sends_a_host_direct_and_redo_brings_it_back(up: &mut Up) {
    learn_git(up).await;
    up.app.send(Command::Undo(up.name.into(), vec![GIT.into()]));
    until_way(up, GIT, false).await;
    // Asked again, longer than any probe waits: it stays direct.
    let again = tokio::task::spawn_blocking(|| get(GIT, Duration::from_secs(6)))
        .await
        .unwrap();
    assert!(again.is_err(), "{GIT} answered after Undo");
    assert_ne!(way(GIT), up.device, "{GIT} learned again after Undo");
    up.app.send(Command::Redo(up.name.into(), vec![GIT.into()]));
    until_way(up, GIT, true).await;
    assert!(answers(GIT).await, "{GIT} after Redo");
}

/// A host added by hand (tunnel detail's "Add a host") goes through at
/// once, before anyone connects to it.
pub(crate) async fn a_host_added_by_hand_goes_through(up: &mut Up) {
    up.app
        .send(Command::AddHosts(up.name.into(), vec![UNTOUCHED.into()]));
    until_way(up, UNTOUCHED, true).await;
    assert!(answers(UNTOUCHED).await, "{UNTOUCHED} after adding it");
}

/// A name added by hand is asked of the tunnel's DNS, and its address
/// goes through.
pub(crate) async fn a_name_added_by_hand_goes_through(up: &mut Up) {
    let (name, at) = WIKI;
    up.app
        .send(Command::AddHosts(up.name.into(), vec![name.into()]));
    until_way(up, at, true).await;
    assert!(answers(at).await, "{name} after adding it");
}

/// A short name ("git", typed on its own as in http://git/) ACME's DNS
/// knows under its domain: Crake asks which tunnel's it means; chosen, it
/// is ACME's git.acme.internal, through the tunnel, until forgotten.
pub(crate) async fn a_short_name_means_the_tunnels_once_chosen(up: &mut Up) {
    const SHORT: &str = "git";
    const FULL: &str = "git.acme.internal";
    assert!(
        super::platform::look_up(SHORT).await.is_empty(),
        "{SHORT} before"
    );
    let name = up.name;
    up.wait_for("the question about git", |m| {
        m.log.iter().any(|e| {
            e.short
                .as_ref()
                .is_some_and(|s| s.name == SHORT && s.full == [(name.to_owned(), FULL.to_owned())])
        })
    })
    .await;
    up.app.send(Command::UseShortName(
        name.into(),
        SHORT.into(),
        FULL.into(),
    ));
    up.wait_for("the choice", |m| {
        m.log
            .iter()
            .any(|e| e.message == "git is git.acme.internal now.")
    })
    .await;
    let found = super::platform::look_up(SHORT).await;
    if !found.iter().map(ToString::to_string).eq([GIT.to_owned()]) {
        up.fail(&format!("{SHORT} once chosen: {found:?}")).await;
    }
    until_way(up, GIT, true).await;
    assert!(answers(GIT).await, "{SHORT} through {}", up.label);
    up.app.send(Command::Forget(name.into(), SHORT.into()));
    up.wait_for("forgotten", |m| {
        m.log.iter().any(|e| e.message == "Forgot git.")
    })
    .await;
    assert!(
        super::platform::look_up(SHORT).await.is_empty(),
        "{SHORT} forgotten"
    );
}

/// A pinned network goes through as a whole; unpinned, it doesn't.
pub(crate) async fn a_pinned_network_goes_through_until_unpinned(up: &mut Up) {
    up.app
        .send(Command::Pin(up.name.into(), vec![HOSTS_NET.into()]));
    until_way(up, UNTOUCHED, true).await;
    assert!(
        answers(UNTOUCHED).await,
        "{UNTOUCHED} in the pinned network"
    );
    up.app
        .send(Command::Unpin(up.name.into(), HOSTS_NET.into()));
    until_way(up, UNTOUCHED, false).await;
}

/// Forgetting a learned host sends it direct again.
pub(crate) async fn forgetting_a_host_sends_it_direct(up: &mut Up) {
    learn_git(up).await;
    up.app.send(Command::Forget(up.name.into(), GIT.into()));
    until_way(up, GIT, false).await;
}

/// Forget all (tunnel detail's Learned tab) empties what was learned and
/// keeps what's pinned.
pub(crate) async fn forget_all_keeps_the_pins(up: &mut Up) {
    up.app
        .send(Command::Pin(up.name.into(), vec![UNTOUCHED.into()]));
    until_way(up, UNTOUCHED, true).await;
    learn_git(up).await;
    up.app.send(Command::ForgetLearned(up.name.into()));
    until_way(up, GIT, false).await;
    assert_eq!(way(UNTOUCHED), up.device, "the pin stays");
}

/// A host on the never list isn't learned, however it answers through
/// the tunnel.
pub(crate) async fn the_never_list_keeps_a_host_direct(up: &mut Up) {
    up.app.send(Command::SetSetting("never".into(), GIT.into()));
    up.app
        .until("the never list", |m| {
            m.settings.get("never").map(String::as_str) == Some(GIT)
        })
        .await;
    for _ in 0..2 {
        let tried = tokio::task::spawn_blocking(|| get(GIT, Duration::from_secs(6)))
            .await
            .unwrap();
        assert!(tried.is_err(), "{GIT} answered while on the never list");
    }
    assert_ne!(way(GIT), up.device, "{GIT} was learned");
}

/// Tunnel detail's list: hosts learned by probing and by name, each as
/// learned.
pub(crate) async fn tunnel_detail_lists_what_was_learned(up: &mut Up) {
    learn_git(up).await;
    let (name, at) = WIKI;
    assert!(!super::platform::look_up(name).await.is_empty(), "{name}");
    until_way(up, at, true).await;
    let tunnel = up.name;
    up.app.send(Command::Hosts(tunnel.into()));
    up.app
        .within(WAIT, "both hosts in the list", |m| {
            m.host_rows.get(tunnel).is_some_and(|rows| {
                [GIT, at]
                    .iter()
                    .all(|h| rows.iter().any(|r| r.target == *h && r.kind == "learned"))
            })
        })
        .await;
}
