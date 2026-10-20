//! Networks that block UDP (CLAUDE.md, 7b), with a fake `openconnect`:
//! DTLS failing, the tunnel carried on over TCP on the same sign-in.

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

/// UDP doesn't get through: DTLS never comes up unless told not to try.
const FAKE_NO_UDP: &str = r#"#!/bin/sh
trap 'exit 0' TERM HUP
echo "$*" >> "$CRAKE_TEST_CALLS"
read -r cookie
export TUNDEV=crake-acme INTERNAL_IP4_ADDRESS=10.40.1.7
reason=connect "$CRAKE_TEST_HOOK" || exit 1
case "$*" in *--no-dtls*) ;; *) echo "DTLS handshake timed out" >&2 ;; esac
while :; do sleep 0.1; done
"#;

#[tokio::test]
async fn where_udp_is_blocked_it_carries_on_over_tcp() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE_NO_UDP);
    let calls = dir.path().join("calls");
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    let session =
        craked::auth::parse("COOKIE='s3cret'\nCONNECT_URL='https://vpn.acme.example/'\n").unwrap();
    mgr.connect_with("acme", Some(session)).unwrap();
    // Up, then restarted without DTLS on the same sign-in.
    for _ in 0..100 {
        let n = std::fs::read_to_string(&calls)
            .unwrap_or_default()
            .lines()
            .count();
        if n >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    wait_for(&mgr, "acme", TunnelState::Connected).await;
    let runs = std::fs::read_to_string(&calls).unwrap();
    let runs: Vec<&str> = runs.lines().collect();
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert!(!runs[0].contains("--no-dtls"));
    assert!(runs[1].contains("--no-dtls") && runs[1].contains("--cookie-on-stdin"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("UDP is blocked here. ACME runs over TCP on this network."),
        "{log}"
    );
    assert_eq!(log.matches("ACME is up").count(), 1, "{log}");
    assert!(log.contains("ACME is back."), "{log}");

    // Next time on this network it starts over TCP straight away.
    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, "acme", TunnelState::Off).await;
    if craked::networks::current().is_some() {
        mgr.connect("acme").unwrap();
        wait_for(&mgr, "acme", TunnelState::Connected).await;
        let runs = std::fs::read_to_string(&calls).unwrap();
        assert!(runs.lines().nth(2).unwrap().contains("--no-dtls"), "{runs}");
        mgr.disconnect("acme").unwrap();
        wait_for(&mgr, "acme", TunnelState::Off).await;
    }
}

/// As openconnect on Windows behind a TCP-only link: DTLS fails before its
/// script says "connect", which it still does while it's being stopped.
const FAKE_UDP_FAILS_FIRST: &str = r#"#!/bin/sh
echo "$*" >> "$CRAKE_TEST_CALLS"
read -r cookie
export TUNDEV=crake-acme INTERNAL_IP4_ADDRESS=10.40.1.7
case "$*" in
  *--no-dtls*)
    trap 'exit 0' TERM HUP
    reason=connect "$CRAKE_TEST_HOOK" || exit 1
    while :; do sleep 0.1; done ;;
  *)
    trap 'stopped=1' HUP
    echo "DTLS handshake failed: Error in the push function." >&2
    sleep 0.5
    reason=connect "$CRAKE_TEST_HOOK"
    exit 0 ;;
esac
"#;

/// The openconnect being replaced doesn't make the tunnel connected: only
/// the one over TCP does, and it's "up", not "back" (it never was).
#[tokio::test]
async fn a_connect_from_the_openconnect_being_replaced_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = setup(dir.path(), FAKE_UDP_FAILS_FIRST);
    let calls = dir.path().join("calls");
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    let session =
        craked::auth::parse("COOKIE='s3cret'\nCONNECT_URL='https://vpn.acme.example/'\n").unwrap();
    mgr.connect_with("acme", Some(session)).unwrap();
    let mut seen = Vec::new();
    for _ in 0..100 {
        let runs = std::fs::read_to_string(&calls).unwrap_or_default();
        let state = mgr.get("acme").unwrap().state;
        if !runs.contains("--no-dtls") {
            seen.push(state);
        } else if state == TunnelState::Connected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        !seen.contains(&TunnelState::Connected),
        "connected before the run over TCP: {seen:?}"
    );
    wait_for(&mgr, "acme", TunnelState::Connected).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert_eq!(log.matches("ACME is up").count(), 1, "{log}");
    assert!(!log.contains("ACME is back."), "{log}");
    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, "acme", TunnelState::Off).await;
}
