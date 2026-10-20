//! End to end against a real gateway: ocserv (AnyConnect) in a container,
//! with ACME's internal network behind it (its DNS at 10.40.0.2, web servers
//! git.acme.internal at 10.40.1.10 and wiki.acme.internal at 10.40.1.11).
//! The app signs in with its own sign-in (`signin.rs`) and the real
//! openconnect, hands the session to craked over D-Bus, and craked runs
//! openconnect with its hook, real nftables and policy routing.
//!
//! Skipped unless CRAKE_E2E=1; run it with `scripts/e2e-test.sh`, which
//! gives it a container with no network but its wire to the gateway's.

use std::net::UdpSocket;
use std::process::Command as Run;
use std::time::Duration;

use craked::config::{Config, Protocol, TunnelConfig};
use craked::manager::Manager;

use crate::client::Command;
use crate::test_app::{App, connected, enabled, gateway, get, off, sign_in};
use crate::wire_tests;

pub(crate) fn sh(cmd: &str) -> String {
    let out = Run::new("sh").args(["-c", cmd]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

pub(crate) fn config(dir: &std::path::Path) -> Config {
    let hook = std::env::var("CRAKE_E2E_HOOK").expect("CRAKE_E2E_HOOK: craked's crake-hook");
    Config {
        hook_path: hook.into(),
        hook_socket: dir.join("hook.sock"),
        openconnect: "openconnect".into(),
        openvpn: "openvpn".into(),
        // Where this system keeps it (Debian's, Fedora's, Arch's).
        charon: Config::default().charon,
        charon_unit: None,
        log_path: dir.join("log"),
        state_path: dir.join("hosts.json"),
        learned_expiry_days: 7,
        probing: true,
        race_first_syn: false,
        probe_wait_secs: 1,
        never: vec![],
        use_resolved: false, // no systemd in the container
        apply_network: true,
        managed_dir: dir.join("managed.d"),
        tunnels: vec![TunnelConfig {
            name: "acme".into(),
            display_name: Some("ACME".into()),
            gateway: gateway(),
            protocol: Protocol::Anyconnect,
            user: Some("alice".into()),
            passwd_file: None,
            dns_domains: vec![],
            auth_args: vec![],
            extra_args: vec![],
            profile: None,
            managed: None,
            own: None,
        }],
    }
}

/// A DNS query for `name`'s address, as resolved would send it.
fn query(name: &str) -> Vec<u8> {
    let mut m = vec![0xab, 0xcd, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        m.push(u8::try_from(label.len()).unwrap());
        m.extend_from_slice(label.as_bytes());
    }
    m.extend_from_slice(&[0, 0, 1, 0, 1]);
    m
}

/// Ask the tunnel's DNS for `name`; then the app's first connection to the
/// address it gave, at once.
pub(crate) fn look_up_and_get(name: &str, addr: &str) -> std::io::Result<String> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_read_timeout(Some(Duration::from_secs(5)))?;
    sock.send_to(&query(name), "10.40.0.2:53")?;
    let mut buf = [0u8; 512];
    let (n, _) = sock.recv_from(&mut buf)?;
    let ip: std::net::Ipv4Addr = addr.parse().unwrap();
    assert!(
        buf[..n].windows(4).any(|w| w == ip.octets()),
        "the tunnel's DNS answered {addr}"
    );
    // Shorter than the probe wait: only a route in place before the first
    // packet gets through this fast.
    get(addr, Duration::from_millis(800))
}

/// craked, with its hook's socket and its D-Bus API, and the app's client.
struct Crake {
    _dir: tempfile::TempDir,
    mgr: std::sync::Arc<Manager>,
    app: App,
}

async fn start() -> Crake {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path());
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let mut app = wire_tests::serve(mgr.clone(), dir.path());
    app.until("craked reached, with ACME", |m| {
        m.reachable == Some(true) && m.tunnels.iter().any(|t| t.name == "acme")
    })
    .await;
    Crake {
        _dir: dir,
        mgr,
        app,
    }
}

fn transport(m: &crate::model::Model) -> String {
    m.tunnels
        .iter()
        .find(|t| t.name == "acme")
        .map(|t| t.transport.clone())
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_real_gateway_end_to_end() {
    if !enabled() {
        return;
    }
    let Crake { _dir, mgr, mut app } = start().await;

    // Nothing reaches ACME's network before the tunnel.
    let before = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_secs(1)))
        .await
        .unwrap();
    assert!(before.is_err(), "10.40.1.10 answered without a tunnel");

    // New tunnel's check: an AnyConnect gateway, and when its certificate
    // runs out, read in a handshake (the image's is valid for ten years).
    let found = tokio::task::spawn_blocking(|| {
        crate::detect::gateway(&crate::test_app::gateway(), Duration::from_secs(15))
    })
    .await
    .unwrap();
    let crate::detect::Found::AnyConnect {
        expires: Some(expires),
        ..
    } = found
    else {
        panic!("not an AnyConnect gateway with a certificate: {found:?}");
    };
    let year = jiff::SignedDuration::from_hours(365 * 24);
    assert!(
        expires > jiff::Timestamp::now() + year,
        "valid for years: {expires}"
    );

    // Sign in as the app does, then switch on with that session.
    let session = sign_in().await;
    assert!(
        session.fingerprint.is_some(),
        "the trusted certificate goes along"
    );
    app.send(Command::ConnectWithSession("acme".into(), session));
    app.within(Duration::from_secs(20), "ACME connected", connected)
        .await;

    // Its address on its own interface; routes for nothing but its DNS,
    // although the gateway pushes 10.40.0.0/16.
    let addr = sh("ip -4 -o addr show crake-acme");
    assert!(addr.contains("inet 10.40.255."), "{addr}");
    let routes = sh("ip -4 route show table all");
    assert!(!routes.contains("10.40.0.0/16"), "{routes}");
    assert!(routes.contains("10.40.0.2 dev crake-acme"), "{routes}");

    // 1. A host that doesn't answer directly: probed, and the app's own
    //    connection gets through on a retry.
    let probed = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_secs(12)))
        .await
        .unwrap();
    assert!(probed.unwrap().contains("crex"), "git.acme.internal's page");
    app.until("the probe's decision", |m| {
        m.log
            .iter()
            .any(|e| e.message == "Sent 10.40.1.10 through ACME.")
    })
    .await;

    // 2. A name only ACME's DNS knows: routed before the first packet.
    let named = tokio::task::spawn_blocking(|| look_up_and_get("wiki.acme.internal", "10.40.1.11"))
        .await
        .unwrap();
    assert!(named.unwrap().contains("crex"), "wiki.acme.internal's page");

    // Tunnel detail's list: both, learned by Crake.
    app.send(Command::Hosts("acme".into()));
    app.until("both hosts in the list", |m| {
        m.host_rows.get("acme").is_some_and(|rows| {
            ["10.40.1.10", "10.40.1.11"]
                .iter()
                .all(|h| rows.iter().any(|r| r.target == *h && r.kind == "learned"))
        })
    })
    .await;

    // A report (Settings → Diagnostics → "Save a report…"), from this craked: ACME's
    // name tried through ACME, its DNS route marked as Crake's, its
    // nftables table; hidden, the hosts go and ACME's plumbing stays.
    let report = crate::report::gather_from(Some(_dir.path().join("crake.sock"))).await;
    let computer = report.computer.as_ref().expect("craked's side");
    let acme = computer
        .checks
        .iter()
        .find(|c| c.starts_with("ACME's names: wiki.acme.internal"))
        .unwrap_or_else(|| panic!("ACME's name tried: {:?}", computer.checks));
    assert!(
        acme.contains("took") && acme.ends_with("through ACME."),
        "{acme}"
    );
    let part = |title: &str| {
        computer
            .parts
            .iter()
            .find(|p| p.title.starts_with(title))
            .unwrap_or_else(|| panic!("{title}: {:?}", computer.parts))
            .lines
            .join("\n")
    };
    let routes = part("Routes");
    assert!(
        routes.contains("10.40.0.2 dev crake-acme")
            && routes.contains("\u{2190} Crake: ACME's DNS"),
        "{routes}"
    );
    assert!(part("Crake's rules").contains("table inet crake"));
    let now = jiff::Zoned::now();
    let hidden = crate::report::text(&report, true, &now);
    assert!(
        !hidden.contains("wiki.acme.internal") && !hidden.contains("10.40.1.11"),
        "{hidden}"
    );
    assert!(
        hidden.contains("10.40.0.2 dev crake-acme"),
        "ACME's DNS stays: {hidden}"
    );

    // Switched off: the interface and its DNS route go.
    app.send(Command::Disconnect("acme".into()));
    app.within(Duration::from_secs(10), "ACME off", off).await;
    let after = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_secs(1)))
        .await
        .unwrap();
    assert!(after.is_err(), "10.40.1.10 still answered with ACME off");
    assert!(!sh("ip -4 route show table all").contains("crake-acme"));

    // craked stopping leaves nothing behind.
    mgr.shutdown().await;
    assert!(!sh("nft list tables").contains("crake"));
    assert!(!sh("ip link").contains("crake-"));
}

/// A network that blocks UDP (CLAUDE.md, 7b): DTLS never comes up, craked
/// notices, restarts the tunnel over TCP on the same session, and remembers
/// the network: switched on again there, it starts over TCP at once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_network_that_blocks_udp() {
    if !enabled() {
        return;
    }
    // The wire drops UDP on port 443 while this file exists (relay.py).
    struct Blocked(std::path::PathBuf);
    impl Drop for Blocked {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let flag = Blocked(std::env::temp_dir().join("block-udp"));
    std::fs::write(&flag.0, "").unwrap();

    let Crake { _dir, mgr, mut app } = start().await;
    let session = sign_in().await;
    app.send(Command::ConnectWithSession("acme".into(), session));
    app.within(Duration::from_secs(90), "UDP noticed", |m| {
        m.log
            .iter()
            .any(|e| e.message == "UDP is blocked here. ACME runs over TCP on this network.")
    })
    .await;
    app.within(Duration::from_secs(20), "connected, over TCP", |m| {
        connected(m) && transport(m) == "TLS"
    })
    .await;
    app.settle(Duration::from_secs(1)).await;
    let said: Vec<&str> = app
        .model
        .log
        .iter()
        .map(|e| e.message.as_str())
        .filter(|m| m.starts_with("ACME is") || m.starts_with("UDP is"))
        .collect();
    // DTLS fails before the first openconnect is up, so the one over TCP
    // is the tunnel coming up, not back (until 2026-10-20 the first one's
    // late "connect" made it connected while its device was going).
    assert_eq!(
        said,
        [
            "UDP is blocked here. ACME runs over TCP on this network.",
            "ACME is up. Routing nothing yet."
        ]
    );
    let works = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_secs(12)))
        .await
        .unwrap();
    assert!(works.unwrap().contains("crex"), "hosts go through over TCP");

    // Remembered: switched on again here, it starts over TCP, even now that
    // UDP would get through (DTLS comes up within a second when tried).
    app.send(Command::Disconnect("acme".into()));
    app.within(Duration::from_secs(10), "ACME off", off).await;
    drop(flag);
    app.send(Command::Connect("acme".into()));
    app.within(Duration::from_secs(20), "ACME on again", connected)
        .await;
    app.settle(Duration::from_secs(3)).await;
    assert_eq!(transport(&app.model), "TLS", "DTLS wasn't tried");
    let said = |m: &crate::model::Model| {
        m.log
            .iter()
            .filter(|e| e.message.starts_with("UDP is blocked here."))
            .count()
    };
    assert_eq!(said(&app.model), 1, "said once");

    mgr.shutdown().await;
}
