//! Real gateways, reached through Cloudflare (CLAUDE.md, 7j), on CI's Mac:
//! the e2e gateway's container runs in a Linux job (ocserv, and openvpn
//! over TCP, with ACME's internal network behind them), Cloudflare Tunnel
//! carries each to this Mac (`cloudflared access tcp`, on 127.0.0.1), and
//! craked runs here as root, the app's own client and sign-in driving it.
//! Connected, a host only the tunnel reaches answers, a name only its DNS
//! knows is routed before the first packet, and the internet still answers
//! directly; off, the internal network is gone again.
//!
//! Skipped unless CRAKE_E2E=1, with CRAKE_E2E_GATEWAY (AnyConnect's
//! address here), CRAKE_GATEWAY_OPENVPN ("host port", over TCP),
//! CRAKE_GATEWAY_CA (the OpenVPN gateway's CA) and CRAKE_E2E_HOOK
//! (craked's crake-hook): see .github/workflows/ci.yml.

use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs as _};
use std::process::Command as Run;
use std::time::Duration;

use craked::config::{Config, Protocol, TunnelConfig};
use craked::manager::Manager;

use crate::client::Command;
use crate::test_app::{App, connected, enabled, gateway, get, off, sign_in};

/// How long a tunnel, and a host through it, get.
const WAIT: Duration = Duration::from_secs(60);
/// A public address that answers anywhere: it must answer directly.
const PUBLIC: &str = "1.1.1.1:443";

fn sh(cmd: &str) -> String {
    let out = Run::new("sh").args(["-c", cmd]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// craked, as root on this Mac, with ACME's AnyConnect gateway configured.
async fn start(dir: &std::path::Path) -> (std::sync::Arc<Manager>, App) {
    let hook = std::env::var("CRAKE_E2E_HOOK").expect("CRAKE_E2E_HOOK: craked's crake-hook");
    let cfg = Config {
        hook_path: hook.into(),
        hook_socket: dir.join("hook.sock"),
        log_path: dir.join("log"),
        state_path: dir.join("hosts.json"),
        probe_wait_secs: 1,
        race_first_syn: false,
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
            managed: None,
            own: None,
        }],
        ..Config::default()
    };
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let mut app = crate::test_app::serve(mgr.clone(), dir);
    app.within(WAIT, "craked reached, with ACME", |m| {
        m.reachable == Some(true) && m.tunnels.iter().any(|t| t.name == "acme")
    })
    .await;
    (mgr, app)
}

/// An HTTP GET of `ip`, retried until it answers (the first try may wait
/// for the probe), or None after `WAIT`.
async fn get_within(ip: &'static str) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        let end = std::time::Instant::now() + WAIT;
        while std::time::Instant::now() < end {
            if let Ok(body) = get(ip, Duration::from_secs(5)) {
                return Some(body);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        None
    })
    .await
    .unwrap()
}

/// The device macOS sends `ip` out of.
fn route_device(ip: &str) -> String {
    sh(&format!("route -n get {ip}"))
        .lines()
        .find_map(|l| l.trim().strip_prefix("interface: ").map(str::to_owned))
        .unwrap_or_default()
}

/// What a tunnel that's up must give: ACME's network through it, its names
/// before the first packet, and the internet directly.
async fn the_split_holds(mgr: &Manager, tunnel: &str) {
    let device = mgr.get(tunnel).unwrap().info.unwrap().device;
    // 1. A host that doesn't answer directly: probed, then through.
    let probed = get_within("10.40.1.10").await;
    assert!(
        probed.is_some_and(|b| b.contains("crex")),
        "git.acme.internal through {tunnel}"
    );
    assert_eq!(route_device("10.40.1.10"), device, "10.40.1.10's way");
    // 2. A name only ACME's DNS knows, asked as an app asks macOS.
    let named = tokio::task::spawn_blocking(|| {
        let addr: Vec<SocketAddr> = ("wiki.acme.internal", 80).to_socket_addrs()?.collect();
        assert!(
            addr.iter().any(|a| a.ip().to_string() == "10.40.1.11"),
            "wiki.acme.internal: {addr:?}"
        );
        get("10.40.1.11", Duration::from_secs(5))
    })
    .await
    .unwrap();
    assert!(
        named.is_ok_and(|b| b.contains("crex")),
        "wiki.acme.internal"
    );
    // 3. The internet, directly: it answers, and not through the tunnel
    //    (the OpenVPN gateway pushes redirect-gateway; Crake leaves it).
    let public = tokio::task::spawn_blocking(|| {
        TcpStream::connect_timeout(&PUBLIC.parse().unwrap(), Duration::from_secs(10))
    })
    .await
    .unwrap();
    assert!(public.is_ok(), "{PUBLIC} answered: {public:?}");
    let way = route_device("1.1.1.1");
    assert!(
        !way.is_empty() && way != device,
        "1.1.1.1 goes {way}, not through {device}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anyconnect_through_cloudflare() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (mgr, mut app) = start(dir.path()).await;
    let before = get("10.40.1.10", Duration::from_secs(2));
    assert!(before.is_err(), "10.40.1.10 answered without a tunnel");

    let session = sign_in().await;
    app.send(Command::ConnectWithSession("acme".into(), session));
    app.within(WAIT, "ACME connected", connected).await;
    the_split_holds(&mgr, "acme").await;

    app.send(Command::Disconnect("acme".into()));
    app.within(WAIT, "ACME off", off).await;
    assert!(get("10.40.1.10", Duration::from_secs(2)).is_err());
    mgr.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openvpn_through_cloudflare() {
    if !enabled() {
        return;
    }
    let remote = std::env::var("CRAKE_GATEWAY_OPENVPN").expect("CRAKE_GATEWAY_OPENVPN");
    let ca_file = std::env::var("CRAKE_GATEWAY_CA").expect("CRAKE_GATEWAY_CA");
    let ca = std::fs::read_to_string(&ca_file).unwrap();
    let profile = format!(
        "client\ndev tun\nproto tcp-client\nremote {remote}\nnobind\nauth-user-pass\n\
         remote-cert-tls server\n<ca>\n{ca}</ca>\n"
    );
    let dir = tempfile::tempdir().unwrap();
    let (mgr, mut app) = start(dir.path()).await;
    let fields: HashMap<String, String> = [
        ("display_name", "Initech"),
        ("protocol", "openvpn"),
        ("profile", profile.as_str()),
    ]
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();
    app.send(Command::AddTunnel(fields));
    app.until("added", |m| m.tunnels.iter().any(|t| t.name == "initech"))
        .await;

    app.send(Command::Connect("initech".into()));
    let answer = |ask: &HashMap<String, String>, text: &str| {
        Command::Answer(
            "initech".into(),
            ask["id"].parse().unwrap(),
            text.to_owned(),
        )
    };
    let ask = app.asked("the username").await;
    assert_eq!(ask["kind"], "user");
    app.send(answer(&ask, "alice"));
    let ask = app.asked("the password").await;
    assert_eq!(ask["kind"], "password");
    app.send(answer(&ask, "hunter2"));
    app.within(WAIT, "Initech connected", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "connected")
    })
    .await;
    the_split_holds(&mgr, "initech").await;

    app.send(Command::Disconnect("initech".into()));
    app.within(WAIT, "Initech off", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "off")
    })
    .await;
    assert!(get("10.40.1.10", Duration::from_secs(2)).is_err());
    mgr.shutdown().await;
}
