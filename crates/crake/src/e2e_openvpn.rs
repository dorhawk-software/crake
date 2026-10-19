//! End to end against a real OpenVPN gateway (CLAUDE.md, 7i): openvpn as a
//! server in the e2e gateway's container, beside ocserv, with ACME's
//! internal network behind it and a browser sign-in like the user's real
//! gateway's (scripts/e2e/sso.py). craked runs the client's openvpn with
//! its hook and management interface, real nftables and policy routing.
//!
//! Skipped unless CRAKE_E2E=1; run it with `scripts/e2e-test.sh`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crake_proto::TunnelState;
use craked::manager::Manager;

use crate::client::Command;
use crate::e2e_tests::{config, look_up_and_get, sh};
use crate::test_app::{enabled, get};
use crate::wire_tests;

/// The gateway's address on the wire, and the folder it shares.
const GATEWAY: &str = "198.51.100.10";
const SHARED: &str = "/mnt/link";
/// How long the gateway, and the tunnel, get to do each thing.
const WAIT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(100);

/// A browser sign-in profile, as the user's real gateway's: no client
/// certificate, the gateway's CA inline, placeholder credentials.
fn profile() -> String {
    let ca = std::fs::read_to_string(format!("{SHARED}/openvpn-ca.crt")).unwrap();
    format!(
        "client\ndev tun\nproto udp\nremote {GATEWAY} 1194\nnobind\npersist-tun\n\
         remote-cert-tls server\nverb 3\n<ca>\n{ca}</ca>\n\
         <auth-user-pass>\nsso-placeholder\nsso-placeholder\n</auth-user-pass>\n"
    )
}

/// A password profile: nothing inline but the gateway's CA.
fn password_profile() -> String {
    let ca = std::fs::read_to_string(format!("{SHARED}/openvpn-ca.crt")).unwrap();
    format!(
        "client\ndev tun\nproto udp\nremote {GATEWAY} 1194\nnobind\nauth-user-pass\n\
         remote-cert-tls server\n<ca>\n{ca}</ca>\n"
    )
}

fn fields() -> HashMap<String, String> {
    fields_for("ACME OpenVPN", profile())
}

fn fields_for(display: &str, profile: String) -> HashMap<String, String> {
    [
        ("display_name", display.to_owned()),
        ("protocol", "openvpn".to_owned()),
        ("profile", profile),
        ("mode", "learn".to_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
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
    let gateway =
        std::fs::read_to_string(format!("{SHARED}/openvpn-server.log")).unwrap_or_default();
    let tail: Vec<&str> = gateway.lines().rev().take(GATEWAY_LINES).collect();
    panic!(
        "never: {what}\nthe gateway said, last first:\n{}",
        tail.join("\n")
    );
}

/// How much of the gateway's log a failure shows.
const GATEWAY_LINES: usize = 25;

/// The tunnel, and why it's in trouble, for a failure's message.
fn why(mgr: &Manager, name: &str) -> String {
    let s = mgr.get(name).unwrap();
    format!("{:?}: {}", s.state, s.why.unwrap_or_default())
}

fn state(mgr: &Manager, name: &str) -> TunnelState {
    mgr.get(name).unwrap().state
}

/// The page, as the browser would open it.
fn open(url: &str) -> String {
    let rest = url.strip_prefix("http://").unwrap();
    let (host, path) = rest.split_once('/').unwrap();
    let mut s = TcpStream::connect(host).unwrap();
    s.set_read_timeout(Some(WAIT)).unwrap();
    write!(s, "GET /{path} HTTP/1.0\r\n\r\n").unwrap();
    let mut body = String::new();
    s.read_to_string(&mut body).unwrap();
    body
}

// Two threads: craked's tasks run while the test's blocking calls wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_browser_sign_in_then_the_gateways_token() {
    if !enabled() {
        return;
    }
    // craked's lines and openvpn's, beside a failure.
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path());
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    let hooks = craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let name = mgr.add_tunnel(&fields()).unwrap();
    assert_eq!(mgr.get(&name).unwrap().config.gateway, GATEWAY);

    // Switched on: openvpn asks craked, craked asks the user to open the
    // gateway's page.
    mgr.connect(&name).unwrap();
    let ask = until("the gateway's sign-in page", || {
        assert_ne!(
            state(&mgr, &name),
            TunnelState::Trouble,
            "{}",
            why(&mgr, &name)
        );
        mgr.asking(&name)
    })
    .await;
    assert_eq!(ask["kind"], "browser");
    let url = ask["url"].clone();
    assert!(
        url.starts_with("http://198.51.100.10:8080/sign-in?cid="),
        "{url}"
    );
    assert!(open(&url).contains("Signed in"));
    until("connected", || {
        (state(&mgr, &name) == TunnelState::Connected).then_some(())
    })
    .await;
    assert!(mgr.asking(&name).is_none(), "nothing more to ask");
    let info = mgr.get(&name).unwrap().info.unwrap();
    assert!(
        info.address.unwrap().to_string().starts_with("10.40.254."),
        "{info:?}"
    );
    assert_eq!(
        info.dns,
        ["10.40.0.2".parse::<std::net::Ipv4Addr>().unwrap()]
    );
    assert_eq!(info.domains, ["acme.internal"]);
    assert_eq!(info.split_include, ["10.40.0.0/16"]);

    // The internet still answers direct (the gateway pushed a default
    // route; nothing of it was applied): 203.0.113.80 drops what comes
    // through a tunnel.
    let direct = get("203.0.113.80", Duration::from_secs(2)).unwrap();
    assert!(direct.contains("crex"), "{direct}");
    // A name only its DNS knows: routed before the first packet.
    let git = look_up_and_get("git.acme.internal", "10.40.1.10").unwrap();
    assert!(git.contains("crex"), "{git}");
    // A host that doesn't answer directly: learned by probing.
    let wiki = until("wiki through the tunnel", || {
        get("10.40.1.11", Duration::from_secs(1)).ok()
    })
    .await;
    assert!(wiki.contains("crex"), "{wiki}");
    let direct = get("203.0.113.80", Duration::from_secs(2)).unwrap();
    assert!(direct.contains("crex"), "still direct: {direct}");

    // Off, keeping the sign-in; on again: the gateway's token, no page.
    mgr.disconnect(&name).unwrap();
    until("off", || {
        (state(&mgr, &name) == TunnelState::Off).then_some(())
    })
    .await;
    assert!(
        mgr.get(&name).unwrap().signed_in,
        "the gateway's token is kept"
    );
    mgr.connect(&name).unwrap();
    until("connected with the token", || {
        assert!(mgr.asking(&name).is_none(), "asked again");
        (state(&mgr, &name) == TunnelState::Connected).then_some(())
    })
    .await;
    // The gateway checks its own token: its sign-in was asked once, for
    // the first connect.
    let seen = std::fs::read_to_string(format!("{SHARED}/openvpn-seen")).unwrap();
    let seen: Vec<&str> = seen.lines().collect();
    assert_eq!(seen.len(), 1, "signed in once: {seen:?}");
    assert!(
        seen[0].contains("IV_SSO=webauth,crtext"),
        "craked says it can open a page: {seen:?}"
    );

    // A package upgrade: the old craked hands it over (postinst's flag),
    // the new one brings it back with the gateway's token, asking nothing.
    std::fs::write(dir.path().join("handover"), "").unwrap();
    mgr.shutdown().await;
    hooks.abort();
    let mgr = Manager::new(config(dir.path())).unwrap();
    mgr.init().await.unwrap();
    craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    mgr.resume_after_update().await;
    until("back after the upgrade", || {
        assert!(mgr.asking(&name).is_none(), "asked after the upgrade");
        (state(&mgr, &name) == TunnelState::Connected).then_some(())
    })
    .await;
    let seen = std::fs::read_to_string(format!("{SHARED}/openvpn-seen")).unwrap();
    assert_eq!(seen.lines().count(), 1, "still signed in once: {seen}");

    // Off: nothing of it is left.
    mgr.disconnect(&name).unwrap();
    until("off", || {
        (state(&mgr, &name) == TunnelState::Off).then_some(())
    })
    .await;
    let links = sh("ip -o link");
    assert!(!links.contains(&format!("crake-{name}")), "{links}");
    // Its own openvpn (the chroot sees every process on the machine, the
    // gateway's openvpn too): known by the config craked wrote for it.
    let left = sh(&format!(
        "pgrep -a -x openvpn | grep -F openvpn-{name}.conf"
    ));
    assert!(left.trim().is_empty(), "openvpn stopped: {left}");
}

/// The app's side of a password sign-in, over D-Bus as the sign-in window
/// does it: switched on, craked asks for the username and the password; a
/// wrong one is asked again, saying so.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_password_sign_in_through_the_app() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path());
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let mut app = wire_tests::serve(mgr.clone(), dir.path());
    app.send(Command::AddTunnel(fields_for(
        "ACME Password",
        password_profile(),
    )));
    app.until("added", |m| {
        m.tunnels.iter().any(|t| t.protocol == "openvpn")
    })
    .await;
    let name = app
        .model
        .tunnels
        .iter()
        .find(|t| t.protocol == "openvpn")
        .unwrap()
        .name
        .clone();

    // The panel's switch: an OpenVPN tunnel just connects.
    app.send(Command::Connect(name.clone()));
    let answer = |ask: &HashMap<String, String>, text: &str| {
        Command::Answer(name.clone(), ask["id"].parse().unwrap(), text.to_owned())
    };
    let ask = app.asked("the username").await;
    let prompt = crate::openvpn::prompt(&ask).unwrap();
    assert_eq!(
        (prompt.ask, prompt.label.as_str()),
        (crate::signin::Ask::Text, "Username")
    );
    app.send(answer(&ask, "alice"));
    let ask = app.asked("the password").await;
    assert_eq!(ask["kind"], "password");
    app.send(answer(&ask, "wrong"));
    // Refused: asked again, from the username, saying so.
    let ask = app.asked("the username again").await;
    assert_eq!(
        (ask["kind"].as_str(), ask["retry"].as_str()),
        ("user", "true")
    );
    app.send(answer(&ask, "alice"));
    let ask = app.asked("the password again").await;
    app.send(answer(&ask, "hunter2"));
    app.within(WAIT, "connected", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == name && t.state == "connected")
    })
    .await;
    app.send(Command::AddHosts(name.clone(), vec!["10.40.1.10".into()]));
    let git = until("git through the tunnel", || {
        get("10.40.1.10", Duration::from_secs(1)).ok()
    })
    .await;
    assert!(git.contains("crex"), "{git}");
    app.send(Command::Disconnect(name.clone()));
    app.within(WAIT, "off", |m| {
        m.tunnels.iter().any(|t| t.name == name && t.state == "off")
    })
    .await;
}

/// An OpenVPN tunnel IT set up (`managed.d`: its file names a profile
/// beside it): craked reads it, the user signs in with a password, and IT's
/// scope holds: a host inside goes through, one outside is refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_openvpn_tunnel_it_set_up() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let managed = dir.path().join("managed.d");
    std::fs::create_dir_all(&managed).unwrap();
    std::fs::write(managed.join("initech.ovpn"), password_profile()).unwrap();
    std::fs::write(
        managed.join("initech.toml"),
        "display_name = \"Initech\"\nprotocol = \"openvpn\"\nprofile = \"initech.ovpn\"\n\
         learn = [\"10.40.1.10/32\"]\n",
    )
    .unwrap();
    let mut cfg = craked::config::Config {
        managed_dir: managed,
        ..config(dir.path())
    };
    // As craked.toml's loading does.
    cfg.add_managed();
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let mut app = wire_tests::serve(mgr.clone(), dir.path());
    app.until("IT's tunnel", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.protocol == "openvpn" && t.managed)
    })
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
    app.send(answer(&ask, "alice"));
    let ask = app.asked("the password").await;
    app.send(answer(&ask, "hunter2"));
    app.within(WAIT, "connected", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "connected")
    })
    .await;
    // Inside IT's scope: through. Outside: refused, in IT's words.
    app.send(Command::AddHosts(
        "initech".into(),
        vec!["10.40.1.10".into()],
    ));
    let git = until("git through the tunnel", || {
        get("10.40.1.10", Duration::from_secs(1)).ok()
    })
    .await;
    assert!(git.contains("crex"), "{git}");
    app.send(Command::AddHosts(
        "initech".into(),
        vec!["10.40.1.11".into()],
    ));
    let refused = tokio::time::timeout(WAIT, async {
        loop {
            match app.updates.recv().await {
                Ok(crate::client::Update::Failed(text)) => return text,
                Ok(u) => {
                    app.model.apply(u);
                }
                Err(_) => panic!("craked went away"),
            }
        }
    })
    .await
    .expect("refused outside the scope");
    assert!(
        refused.starts_with("Initech IT only allows 10.40.1.10")
            && refused.ends_with("through Initech."),
        "{refused}"
    );
    app.send(Command::Disconnect("initech".into()));
    app.within(WAIT, "off", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "off")
    })
    .await;
    mgr.shutdown().await;
}

/// A gateway that can't take data channel offload (the user's Azure
/// profile, CLAUDE.local.md): openvpn 2.6+ says "Use --disable-dco" and
/// restarts forever. This container's gateway always offers what DCO
/// needs, so a stand-in for openvpn says what openvpn says against such a
/// gateway unless it's started with --disable-dco, then runs the real
/// openvpn either way, noting each start's arguments. What's Crake's is
/// real: it notices, starts again without, connects to the real gateway,
/// says so once, and starts without from then on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gateway_without_offload_runs_without_it() {
    use std::os::unix::fs::PermissionsExt as _;
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let starts = dir.path().join("starts");
    let openvpn = dir.path().join("openvpn");
    std::fs::write(
        &openvpn,
        format!(
            "#!/bin/sh\necho \"$*\" >> {starts:?}\n\
             for a; do [ \"$a\" = --disable-dco ] && exec /usr/sbin/openvpn \"$@\"; done\n\
             echo \"$(date '+%F %T') Server did not request DATA_V2 packet format required \
             for data channel offload. Use --disable-dco to connect to this server.\"\n\
             exec /usr/sbin/openvpn \"$@\"\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&openvpn, std::fs::Permissions::from_mode(0o755)).unwrap();
    let managed = dir.path().join("managed.d");
    std::fs::create_dir_all(&managed).unwrap();
    std::fs::write(managed.join("initech.ovpn"), password_profile()).unwrap();
    std::fs::write(
        managed.join("initech.toml"),
        "display_name = \"Initech\"\nprotocol = \"openvpn\"\nprofile = \"initech.ovpn\"\n\
         learn = [\"10.40.1.10/32\"]\n",
    )
    .unwrap();
    let mut cfg = craked::config::Config {
        managed_dir: managed,
        openvpn: openvpn.clone(),
        charon: "charon".into(),
        ..config(dir.path())
    };
    cfg.add_managed();
    let hook_socket = cfg.hook_socket.clone();
    let log = cfg.log_path.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let mut app = wire_tests::serve(mgr.clone(), dir.path());
    app.until("IT's tunnel", |m| {
        m.tunnels.iter().any(|t| t.name == "initech")
    })
    .await;
    let on = |m: &crate::model::Model, s: &str| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == s)
    };
    for round in ["first", "again"] {
        app.send(Command::Connect("initech".into()));
        // Whatever it asks, as often as it asks: the restart without
        // offload may come between two questions.
        tokio::time::timeout(WAIT, async {
            while !on(&app.model, "connected") {
                match app.updates.recv().await {
                    Ok(crate::client::Update::Asking(_, ask)) => {
                        let text = if ask["kind"] == "user" {
                            "alice"
                        } else {
                            "hunter2"
                        };
                        app.send(Command::Answer(
                            "initech".into(),
                            ask["id"].parse().unwrap(),
                            text.into(),
                        ));
                    }
                    Ok(u) => {
                        app.model.apply(u);
                    }
                    Err(_) => panic!("craked went away"),
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("never connected ({round}): {}", why(&mgr, "initech")));
        let git = until("git through the tunnel", || {
            get("10.40.1.10", Duration::from_secs(1)).ok()
        })
        .await;
        assert!(git.contains("crex"), "{git}");
        app.send(Command::Disconnect("initech".into()));
        app.within(WAIT, "off", |m| on(m, "off")).await;
    }
    let starts = std::fs::read_to_string(&starts).unwrap();
    let with: Vec<bool> = starts
        .lines()
        .map(|l| l.split(' ').any(|a| a == "--disable-dco"))
        .collect();
    assert_eq!(
        with,
        [false, true, true],
        "with offload once, then without, and without from the start next time:\n{starts}"
    );
    let logged = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        logged
            .matches("Initech starts without data channel offload.")
            .count(),
        1,
        "{logged}"
    );
    mgr.shutdown().await;
}
