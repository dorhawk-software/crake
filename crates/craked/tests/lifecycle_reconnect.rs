//! A tunnel that loses its gateway and comes back, with a fake
//! `openconnect`: the log says what can't be reached meanwhile, and for how
//! long it was gone (design/screens/tray-panel.html?moment=back).

#![cfg(target_os = "linux")]

use std::time::Duration;

use crake_proto::TunnelState;
use craked::hook_server;
use craked::manager::Manager;

#[path = "common/manager.rs"]
mod manager;
#[path = "common/script.rs"]
mod script;
use manager::{setup, wait_for};

/// Up, then the gateway goes away for a while, then it's back.
const FAKE_RECONNECTS: &str = r#"#!/bin/sh
trap 'exit 0' TERM HUP
read -r cookie
export TUNDEV=crake-acme INTERNAL_IP4_ADDRESS=10.40.1.7
reason=connect "$CRAKE_TEST_HOOK" || exit 1
sleep 1
reason=attempt-reconnect "$CRAKE_TEST_HOOK"
sleep 1.2
reason=reconnect "$CRAKE_TEST_HOOK"
while :; do sleep 0.1; done
"#;

#[tokio::test]
async fn a_tunnel_that_lost_its_gateway_says_what_waits_and_for_how_long() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE_RECONNECTS);
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    let session =
        craked::auth::parse("COOKIE='s3cret'\nCONNECT_URL='https://vpn.acme.example/'\n").unwrap();
    mgr.connect_with("acme", Some(session)).unwrap();
    wait_for(&mgr, "acme", TunnelState::Connected).await;
    let since = mgr.get("acme").unwrap().since;
    mgr.pin("acme", "10.40.1.20").await.unwrap();

    // Reconnecting: still "since" the first connection (the panel's
    // "Lost the gateway", not "Connecting…").
    wait_for(&mgr, "acme", TunnelState::Connecting).await;
    assert!(since.is_some());
    assert_eq!(mgr.get("acme").unwrap().since, since);
    wait_for(&mgr, "acme", TunnelState::Connected).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("ACME lost the gateway. Trying again."),
        "{log}"
    );
    assert!(
        log.contains(
            "Its 1 host can\u{2019}t be reached until it\u{2019}s back. Everything else goes direct as usual."
        ),
        "{log}"
    );
    assert!(log.contains("ACME is back."), "{log}");
    assert!(log.contains("Gone for "), "{log}");
    assert!(
        log.contains(" s. Its 1 host goes through it again."),
        "{log}"
    );
    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, "acme", TunnelState::Off).await;
}
