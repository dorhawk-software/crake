//! End to end against a real IPsec (IKEv2) gateway (CLAUDE.md, 7k):
//! strongSwan as the gateway in the e2e gateway's container, beside ocserv
//! and openvpn, signing users in as a FortiGate does (its key, then the
//! user's password over EAP), with ACME's internal network behind it.
//! craked runs its own charon, real nftables and policy routing.
//!
//! Skipped unless CRAKE_E2E=1; run it with `scripts/e2e-test.sh`.

use std::collections::HashMap;
use std::time::Duration;

use crake_proto::TunnelState;
use craked::manager::Manager;

use crate::e2e_tests::{config, look_up_and_get, sh};
use crate::test_app::{enabled, get};

/// The gateway's address on the wire, and the folder it shares.
const GATEWAY: &str = "198.51.100.10";
const SHARED: &str = "/mnt/link";
/// How long the gateway, and the tunnel, get to do each thing.
const WAIT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(100);
/// How much of the gateway's log a failure shows.
const GATEWAY_LINES: usize = 25;

fn fields() -> HashMap<String, String> {
    [
        ("display_name", "ACME IPsec"),
        ("protocol", "ipsec"),
        ("gateway", GATEWAY),
        ("gateway_key", "hunter2hunter2"),
        ("gateway_id", "vpn.acme.example"),
        ("sign_in", "password"),
        // Its gateway's domain can't reach Crake over IKEv2: named here.
        ("domains", "acme.internal"),
        ("mode", "learn"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

async fn until<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + WAIT;
    while std::time::Instant::now() < deadline {
        if let Some(v) = f() {
            return v;
        }
        tokio::time::sleep(POLL).await;
    }
    let gateway = std::fs::read_to_string(format!("{SHARED}/ipsec-server.log")).unwrap_or_default();
    let tail: Vec<&str> = gateway.lines().rev().take(GATEWAY_LINES).collect();
    panic!(
        "never: {what}\nthe gateway said, last first:\n{}",
        tail.join("\n")
    );
}

/// How the client routes, for a failure's message.
fn routing() -> String {
    sh(
        "ip rule; ip route get 10.40.1.10 mark 0; ip -o addr; ip -s link show; \
        ip xfrm policy; ip xfrm state; nft list ruleset",
    )
}

fn state(mgr: &Manager, name: &str) -> TunnelState {
    mgr.get(name).unwrap().state
}

fn why(mgr: &Manager, name: &str) -> String {
    let s = mgr.get(name).unwrap();
    format!("{:?}: {}", s.state, s.why.unwrap_or_default())
}

/// The next question, answered with `text`; its fields, to look at.
async fn answer(mgr: &Manager, name: &str, kind: &str, text: &str) -> HashMap<String, String> {
    let ask = until(&format!("asked for the {kind}"), || {
        assert_ne!(state(mgr, name), TunnelState::Trouble, "{}", why(mgr, name));
        mgr.asking(name).filter(|a| a["kind"] == kind)
    })
    .await;
    let id = ask["id"].parse().unwrap();
    mgr.answer(name, id, text.to_owned()).unwrap();
    ask.into_iter().collect()
}

// Two threads: craked's tasks run while the test's blocking calls wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_password_sign_in_over_ikev2() {
    if !enabled() {
        return;
    }
    // craked's lines and charon's, beside a failure.
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info,charon=debug")
        .with_test_writer()
        .try_init();
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path());
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    let _hooks = craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let name = mgr.add_tunnel(&fields()).unwrap();
    // The gateway's key is craked's alone: never with the tunnel's fields.
    let listed = format!("{:?}", mgr.get(&name).unwrap().config);
    assert!(!listed.contains("hunter2hunter2"), "{listed}");

    // Switched on: the user's name and password are asked; a wrong
    // password is turned down by the gateway and asked again, saying so.
    mgr.connect(&name).unwrap();
    let first = answer(&mgr, &name, "user", "alice").await;
    assert_eq!(first["retry"], "false");
    answer(&mgr, &name, "password", "hunter3").await;
    let again = answer(&mgr, &name, "user", "alice").await;
    assert_eq!(again["retry"], "true", "the refusal is said");
    answer(&mgr, &name, "password", "hunter2").await;
    until("connected", || {
        assert_ne!(
            state(&mgr, &name),
            TunnelState::Trouble,
            "{}",
            why(&mgr, &name)
        );
        (state(&mgr, &name) == TunnelState::Connected).then_some(())
    })
    .await;
    let info = mgr.get(&name).unwrap().info.unwrap();
    assert_eq!(info.device, format!("crake-{name}"));
    assert!(
        info.address.unwrap().to_string().starts_with("10.40.9."),
        "{info:?}"
    );
    assert_eq!(
        info.dns,
        ["10.40.0.2".parse::<std::net::Ipv4Addr>().unwrap()]
    );
    assert_eq!(info.split_include, ["10.40.0.0/16"]);

    // The internet still answers direct: 203.0.113.80 drops what comes
    // through a tunnel.
    let direct = get("203.0.113.80", Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("203.0.113.80: {e}\n{}", routing()));
    assert!(direct.contains("crex"), "{direct}");
    // A name only its DNS knows: routed before the first packet.
    let git = look_up_and_get("git.acme.internal", "10.40.1.10")
        .unwrap_or_else(|e| panic!("git.acme.internal: {e}\n{}", routing()));
    assert!(git.contains("crex"), "{git}");
    // A host that doesn't answer directly: learned by probing.
    let wiki = until("wiki through the tunnel", || {
        get("10.40.1.11", Duration::from_secs(1)).ok()
    })
    .await;
    assert!(wiki.contains("crex"), "{wiki}");

    // Off: nothing of it is left, not its interface nor its SAs.
    mgr.disconnect(&name).unwrap();
    until("off", || {
        (state(&mgr, &name) == TunnelState::Off).then_some(())
    })
    .await;
    let links = sh("ip -o link");
    assert!(!links.contains(&format!("crake-{name}")), "{links}");
    let sas = sh("ip xfrm state");
    assert!(!sas.contains(GATEWAY), "{sas}");

    // craked leaving stops its charon.
    mgr.shutdown().await;
    let left = sh("pgrep -a -x charon");
    assert!(
        !left.contains(&format!("{}", dir.path().display())),
        "{left}"
    );
}

/// A key that doesn't match the gateway is said apart from a wrong
/// password (only IT can help with it), and changed in Edit; a username
/// set in the tunnel isn't asked again (ipsec-tunnel.html, 3 and 5).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_key_then_the_right_one() {
    if !enabled() {
        return;
    }
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path());
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    let _hooks = craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let mut f = fields();
    f.insert("gateway_key".into(), "not-the-key".into());
    f.insert("user".into(), "alice".into());
    let name = mgr.add_tunnel(&f).unwrap();

    mgr.connect(&name).unwrap();
    // The username is the tunnel's: only the password is asked.
    answer(&mgr, &name, "password", "hunter2").await;
    let why = until("refused for its key", || {
        (state(&mgr, &name) == TunnelState::Trouble).then(|| why(&mgr, &name))
    })
    .await;
    assert!(why.contains(crake_proto::trouble::KEY_MISMATCH), "{why}");
    assert_eq!(
        crake_proto::trouble::label(why.split_once(": ").map_or(why.as_str(), |(_, w)| w)),
        Some("Key doesn\u{2019}t match"),
        "the panel's few words"
    );

    // Edit: the right key; switched on, it connects.
    f.insert("gateway_key".into(), "hunter2hunter2".into());
    mgr.update_tunnel(&name, &f).await.unwrap();
    mgr.connect(&name).unwrap();
    answer(&mgr, &name, "password", "hunter2").await;
    until("connected with the right key", || {
        (state(&mgr, &name) == TunnelState::Connected).then_some(())
    })
    .await;
    mgr.disconnect(&name).unwrap();
    until("off", || {
        (state(&mgr, &name) == TunnelState::Off).then_some(())
    })
    .await;
    mgr.shutdown().await;
}
