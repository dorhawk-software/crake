//! Real gateways, reached through Cloudflare (CLAUDE.md, 7j), on CI's Mac
//! and Windows: the e2e gateway's container runs in a Linux job (ocserv,
//! and openvpn over TCP, with ACME's internal network behind them),
//! Cloudflare Tunnel carries each here (`cloudflared access tcp`, on
//! 127.0.0.1), and craked (macOS: in the test, as root; Windows: the
//! installed service) is driven by the app's own client and sign-in.
//! What a tunnel that's up does is the scenario suite's (`scenarios/`,
//! run here as on Linux); this holds what only these systems do: craked
//! ended and started again by the system, OpenVPN's browser sign-in
//! through Cloudflare (`sso.rs`), and the Doctor workflow's tunnel.
//!
//! Skipped unless CRAKE_E2E=1, with CRAKE_E2E_GATEWAY (AnyConnect's
//! address here), CRAKE_GATEWAY_OPENVPN ("host port", over TCP) and
//! CRAKE_GATEWAY_CA (the OpenVPN gateway's CA); on macOS CRAKE_E2E_HOOK
//! (craked's crake-hook). See scripts/ci-through-cloudflare.sh.

use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs as _};
use std::process::Command as Run;
use std::time::Duration;

use crate::client::Command;
use crate::test_app::{App, connected, enabled, gateway, get, sign_in};

mod sso;

/// How long a tunnel, and a host through it, get.
const WAIT: Duration = Duration::from_secs(60);
/// A public name: "my internet stopped" is almost always names that stop
/// resolving (craked stands in for the system's DNS while a tunnel is up).
const PUBLIC_NAME: (&str, u16) = ("one.one.one.one", 443);
/// How long the system gets to have its DNS back after craked's end.
const BACK: Duration = Duration::from_secs(45);
/// The test tunnels, gone before and after.
const TUNNELS: [&str; 2] = ["acme", "initech"];

/// craked and the app's client connected to it, ACME's AnyConnect gateway
/// added as New tunnel adds one.
struct Crake {
    /// craked started as the scenario suite starts it.
    craked: crate::scenarios::Craked,
    app: App,
}

impl Crake {
    async fn craked() -> Self {
        let (craked, app) = crate::scenarios::Craked::start_with(&[], false).await;
        Self { craked, app }
    }

    async fn start() -> Self {
        let mut c = Self::craked().await;
        c.app
            .within(WAIT, "craked reached", |m| {
                m.reachable == Some(true) && !m.settings.is_empty()
            })
            .await;
        c.remove_tunnels().await;
        add_acme(&mut c.app).await;
        c
    }

    async fn add(&mut self, fields: &[(&str, &str)]) {
        add(&mut self.app, fields).await;
    }

    async fn remove_tunnels(&mut self) {
        remove_tunnels(&mut self.app).await;
    }

    async fn stop(mut self) {
        self.remove_tunnels().await;
        self.craked.stop().await;
    }
}

/// A tunnel added as New tunnel adds one.
async fn add(app: &mut App, fields: &[(&str, &str)]) {
    let fields: HashMap<String, String> = fields
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let name = fields["display_name"].to_lowercase();
    app.send(Command::AddTunnel(fields));
    app.until("added", |m| m.tunnels.iter().any(|t| t.name == name))
        .await;
}

/// ACME's AnyConnect gateway.
async fn add_acme(app: &mut App) {
    let gateway = gateway();
    add(
        app,
        &[
            ("display_name", "ACME"),
            ("gateway", &gateway),
            ("protocol", "anyconnect"),
            ("user", "alice"),
        ],
    )
    .await;
}

/// No test tunnel left (the service keeps them from run to run).
async fn remove_tunnels(app: &mut App) {
    for old in TUNNELS {
        if app.model.tunnels.iter().any(|t| t.name == old) {
            app.send(Command::RemoveTunnel(old.into()));
            app.until("an old one gone", |m| {
                !m.tunnels.iter().any(|t| t.name == old)
            })
            .await;
        }
    }
}

fn sh(cmd: &str) -> String {
    #[cfg(windows)]
    let out = Run::new("powershell")
        .args(["-NoProfile", "-Command", cmd])
        .output()
        .unwrap();
    #[cfg(target_os = "macos")]
    let out = Run::new("sh").args(["-c", cmd]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// macOS: no NAT rule of Crake's left in pf, `when` (the Doctor saw one
/// stay with the tunnel off and craked gone, and pf on, 2026-10-18).
fn no_nat_left(when: &str) {
    #[cfg(target_os = "macos")]
    {
        let nat = sh("pfctl -a com.apple/250.Crake -s nat 2>/dev/null");
        assert!(
            !nat.contains("nat on"),
            "{when}, Crake's NAT stayed:\n{nat}"
        );
    }
    #[cfg(windows)]
    let _ = when;
}

/// The internet works by name within `wait`: a public name looked up as
/// an app looks it up, and its address answering.
async fn names_work(what: &str, wait: Duration) {
    let answers = || {
        PUBLIC_NAME
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.find(SocketAddr::is_ipv4))
            .is_some_and(|a| TcpStream::connect_timeout(&a, Duration::from_secs(5)).is_ok())
    };
    let end = std::time::Instant::now() + wait;
    while !tokio::task::spawn_blocking(answers).await.unwrap() {
        assert!(
            std::time::Instant::now() < end,
            "public names don't resolve: {what}"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
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

/// macOS: craked as a process of its own, serving the app where the
/// installed one does (crake_proto::APP_SOCKET), its files in `dir`.
#[cfg(target_os = "macos")]
pub(crate) fn own_craked(dir: &std::path::Path) -> std::process::Child {
    let hook = std::env::var("CRAKE_E2E_HOOK").expect("CRAKE_E2E_HOOK: craked's crake-hook");
    let bin = std::env::var("CRAKE_E2E_CRAKED").expect("CRAKE_E2E_CRAKED: craked itself");
    let cfg = dir.join("craked.toml");
    std::fs::write(
        &cfg,
        format!(
            "hook_path = {hook:?}\nhook_socket = {:?}\nlog_path = {:?}\nstate_path = {:?}\n\
             managed_dir = {:?}\nprobe_wait_secs = 1\nrace_first_syn = false\n",
            dir.join("hook.sock"),
            dir.join("log"),
            dir.join("hosts.json"),
            dir.join("managed.d"),
        ),
    )
    .unwrap();
    Run::new(bin)
        .arg("--config")
        .arg(&cfg)
        .stderr(std::fs::File::create(dir.join("craked.err")).unwrap())
        .spawn()
        .unwrap()
}

/// macOS: the app socket's file, as made by the craked serving it now.
#[cfg(target_os = "macos")]
fn socket_made() -> Option<u64> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata(crake_proto::APP_SOCKET)
        .ok()
        .map(|m| m.ino())
}

/// macOS: a new craked has started, within BACK: it makes its app socket
/// anew once its start has put the system right (`Manager::init`).
#[cfg(target_os = "macos")]
async fn started_again(before: Option<u64>) {
    let end = std::time::Instant::now() + BACK;
    while socket_made().is_none_or(|now| Some(now) == before) {
        assert!(
            std::time::Instant::now() < end,
            "craked didn't start again within {} s",
            BACK.as_secs()
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// craked ending mid-connection (a crash, or killed): once it's started
/// again (launchd and Windows' service manager restart it), the system's
/// DNS is its own again and the internet works by name. macOS: craked as
/// a process of its own, killed with SIGKILL and started again; Windows:
/// the service's process killed, and Windows restarting it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_internet_survives_crakeds_end() {
    if !enabled() {
        return;
    }
    #[cfg(target_os = "macos")]
    let dir = tempfile::tempdir().unwrap();
    #[cfg(target_os = "macos")]
    let mut craked = own_craked(dir.path());
    let (tx, updates) = async_channel::unbounded();
    let mut app = App::new(crate::client::start(tx), updates);
    let reached = |m: &crate::model::Model| m.reachable == Some(true) && !m.settings.is_empty();
    app.within(WAIT, "craked reached", reached).await;
    remove_tunnels(&mut app).await;
    add_acme(&mut app).await;
    let session = sign_in().await;
    app.send(Command::ConnectWithSession("acme".into(), session));
    app.within(WAIT, "ACME connected", connected).await;
    names_work("connected", WAIT).await;

    // The end, and the start after it. openconnect, left without craked,
    // is ended too: the start that follows knows nothing of it.
    #[cfg(target_os = "macos")]
    {
        let before = socket_made();
        let _ = craked.kill();
        let _ = craked.wait();
        sh("pkill -9 openconnect");
        craked = own_craked(dir.path());
        // The app hasn't noticed the end yet, and a name just looked up
        // answers from macOS' cache: neither says the new craked started.
        started_again(before).await;
    }
    #[cfg(windows)]
    {
        sh("Stop-Process -Name craked -Force");
        sh("Stop-Process -Name openconnect -Force -ErrorAction SilentlyContinue");
    }
    names_work("after craked's end, started again", BACK).await;
    #[cfg(target_os = "macos")]
    {
        // The system's own resolver (#1, every name), not the ones per
        // domain: those may rightly be craked's (an off tunnel's domains,
        // kept private, go to its server through /etc/resolver).
        let dns = sh("scutil --dns");
        let primary = dns
            .split("resolver #2")
            .next()
            .unwrap_or_default()
            .to_owned();
        assert!(
            !primary.contains("127.53.0.1"),
            "macOS's own DNS still points at craked's server:\n{dns}"
        );
    }
    // Nor pf's NAT for a tunnel: the new craked emptied the anchor and gave
    // back the token the one that died held.
    no_nat_left("after craked's end, started again");

    app.within(BACK, "craked reached again", reached).await;
    remove_tunnels(&mut app).await;
    #[cfg(target_os = "macos")]
    {
        sh(&format!("kill -TERM {}", craked.id()));
        let _ = craked.wait();
    }
}

/// The Doctor workflow (.github/workflows/doctor.yml): the tunnel it was
/// asked for (CRAKE_DOCTOR_TUNNEL: anyconnect, openvpn or none) up for
/// CRAKE_DOCTOR_MINUTES while scripts/doctor-snapshot.sh looks at the
/// system, each phase said in CRAKE_DOCTOR_PHASE (a file): "connected",
/// "off", "done". Skipped unless the workflow runs it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doctor() {
    let (Ok(tunnel), Ok(minutes), Ok(phase)) = (
        std::env::var("CRAKE_DOCTOR_TUNNEL"),
        std::env::var("CRAKE_DOCTOR_MINUTES"),
        std::env::var("CRAKE_DOCTOR_PHASE"),
    ) else {
        return;
    };
    let say = |what: &str| std::fs::write(&phase, what).unwrap();
    let hold = Duration::from_secs(minutes.parse::<u64>().unwrap_or(1) * 60);
    let mut crake = Crake::start().await;
    let name = match tunnel.as_str() {
        "anyconnect" => {
            let session = sign_in().await;
            crake
                .app
                .send(Command::ConnectWithSession("acme".into(), session));
            crake.app.within(WAIT, "ACME connected", connected).await;
            Some("acme")
        }
        "openvpn" => {
            let remote = std::env::var("CRAKE_GATEWAY_OPENVPN").expect("CRAKE_GATEWAY_OPENVPN");
            let ca = std::fs::read_to_string(std::env::var("CRAKE_GATEWAY_CA").unwrap()).unwrap();
            let profile = format!(
                "client\ndev tun\nproto tcp-client\nremote {remote}\nnobind\nauth-user-pass\n\
                 remote-cert-tls server\n<ca>\n{ca}</ca>\n"
            );
            crake
                .add(&[
                    ("display_name", "Initech"),
                    ("protocol", "openvpn"),
                    ("profile", profile.as_str()),
                ])
                .await;
            let app = &mut crake.app;
            app.send(Command::Connect("initech".into()));
            for (kind, text) in [("user", "alice"), ("password", "hunter2")] {
                let ask = app.asked(kind).await;
                app.send(Command::Answer(
                    "initech".into(),
                    ask["id"].parse().unwrap(),
                    text.to_owned(),
                ));
            }
            app.within(WAIT, "Initech connected", |m| {
                m.tunnels
                    .iter()
                    .any(|t| t.name == "initech" && t.state == "connected")
            })
            .await;
            Some("initech")
        }
        _ => None,
    };
    // A host through it, as a person's first visit would go.
    if name.is_some() {
        let _ = get_within("10.40.1.10").await;
    }
    say("connected");
    crake.app.settle(hold).await;
    if let Some(name) = name {
        crake.app.send(Command::Disconnect(name.into()));
        crake
            .app
            .within(WAIT, "off", |m| {
                m.tunnels.iter().any(|t| t.name == name && t.state == "off")
            })
            .await;
    }
    say("off");
    crake.app.settle(Duration::from_secs(20)).await;
    crake.stop().await;
    say("done");
}
