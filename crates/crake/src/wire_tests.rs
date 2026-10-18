//! The app and craked together, over real D-Bus: craked's manager serves its
//! API on a private socket (as `--dev` does), and the app's own client
//! (`client.rs`) drives it, its updates applied to the app's model. What
//! crosses the wire is checked from the app's side: requests become D-Bus
//! calls, craked's signals become model changes.
//!
//! Tunnels come up through OpenVPN 3's attach (no openconnect needed); the
//! openconnect path is covered by craked's lifecycle tests.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::time::{Duration, Instant};

use craked::config::{Config, Protocol, TunnelConfig};
use craked::manager::Manager;

use crate::client::{self, Command, Update};
use crate::test_app::App;

fn config(dir: &std::path::Path) -> Config {
    Config {
        hook_path: "/bin/true".into(),
        hook_socket: dir.join("hook.sock"),
        openconnect: "/bin/false".into(),
        openvpn: "openvpn".into(),
        log_path: dir.join("log"),
        state_path: dir.join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        never: vec![],
        use_resolved: false,
        apply_network: false,
        managed_dir: dir.join("managed.d"),
        tunnels: vec![TunnelConfig {
            name: "acme".into(),
            display_name: Some("ACME".into()),
            gateway: "vpn.acme.example".into(),
            protocol: Protocol::Anyconnect,
            user: None,
            passwd_file: None,
            dns_domains: vec![],
            auth_args: vec![],
            extra_args: vec![],
            managed: None,
            own: None,
        }],
    }
}

async fn start(dir: &std::path::Path) -> App {
    let mgr = Manager::new(config(dir)).unwrap();
    mgr.init().await.unwrap();
    serve(mgr, dir)
}

/// craked serving on a private socket in `dir`, and the app's client
/// connected to it.
pub(crate) fn serve(mgr: std::sync::Arc<Manager>, dir: &std::path::Path) -> App {
    let socket = dir.join("crake.sock");
    let owner = std::fs::metadata(dir).unwrap().uid();
    craked::dbus::serve_p2p(mgr, &socket, owner).unwrap();
    let (tx, updates) = async_channel::unbounded();
    App::new(client::start_on(socket, tx), updates)
}

fn fields(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_app_and_craked_work_together_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = start(dir.path()).await;

    // Reached, with craked's tunnels and settings.
    app.until("craked reached, with ACME", |m| {
        m.reachable == Some(true) && m.tunnels.iter().any(|t| t.name == "acme")
    })
    .await;
    assert_eq!(app.state("acme").as_deref(), Some("off"));
    app.until("the settings", |m| !m.settings.is_empty()).await;

    // New tunnel's "Add and connect": an OpenVPN 3 profile.
    app.send(Command::AddTunnel(fields(&[
        ("display_name", "Initech"),
        ("gateway", "initech"),
        ("protocol", "openvpn3"),
    ])));
    app.until("Initech added", |m| {
        m.tunnels.iter().any(|t| t.name == "initech")
    })
    .await;
    let initech = app
        .model
        .tunnels
        .iter()
        .find(|t| t.name == "initech")
        .unwrap();
    assert_eq!(
        initech.source, "app",
        "added in the app: the app may change it"
    );
    assert_eq!(initech.protocol, "openvpn3");

    // Its session connects; craked routes through its device.
    app.send(Command::Attach(
        "initech".into(),
        fields(&[
            ("device", "tun0"),
            ("dns", "10.8.0.1"),
            ("domains", "initech.internal"),
            ("transport", "udp"),
        ]),
    ));
    app.until("Initech connected", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "connected" && t.transport == "UDP")
    })
    .await;
    app.until("its up line in the log", |m| {
        m.log
            .iter()
            .any(|e| e.message.starts_with("Initech is up."))
    })
    .await;

    // "Copy for helpdesk" carries craked's view of the computer: what a
    // helpdesk would otherwise ask the user to look up by hand.
    app.until("craked's view in the helpdesk copy", |m| {
        let copy = m.helpdesk(false);
        copy.contains("Crake's view of this computer")
            && copy.contains("Initech: tun0")
            && copy.contains("10.8.0.1")
    })
    .await;

    // A host sent through it from tunnel detail's "Add a host".
    app.send(Command::AddHosts(
        "initech".into(),
        vec!["10.81.1.4".into()],
    ));
    app.until("the decision in the log", |m| {
        m.log
            .iter()
            .any(|e| e.message == "Sent 10.81.1.4 through Initech.")
    })
    .await;
    assert!(app.logged("Sent 10.81.1.4 through Initech."));

    // Tunnel detail's host list: the host, and how it was learned.
    app.send(Command::Hosts("initech".into()));
    app.until("the host list", |m| m.host_rows.contains_key("initech"))
        .await;
    let rows = &app.model.host_rows["initech"];
    let row = rows.iter().find(|r| r.target == "10.81.1.4").unwrap();
    assert_eq!(row.kind, "learned");
    assert_eq!(row.learned.as_ref().map(|l| l.1.as_str()), Some("you"));

    // Tunnel detail's "Forget all…": what was learned goes, and the list
    // comes back without it.
    app.send(Command::ForgetLearned("initech".into()));
    app.until("forgotten in the log", |m| {
        m.log
            .iter()
            .any(|e| e.message == "Forgot the 1 host Crake learned for Initech.")
    })
    .await;
    app.send(Command::Hosts("initech".into()));
    app.until("an empty host list", |m| {
        m.host_rows
            .get("initech")
            .is_some_and(|rows| rows.iter().all(|r| r.target != "10.81.1.4"))
    })
    .await;

    // The Traffic window's counters: two looks give rates; this craked
    // stands in for routing, so it lists no connections (it says so).
    app.send(Command::Live);
    app.send(Command::Live);
    app.until("the Traffic window's rates", |m| m.live.rates.is_some())
        .await;
    assert_eq!(app.model.live.lists_connections(), Some(false));

    // A setting changed in Settings comes back changed.
    app.send(Command::SetSetting("probe_wait_secs".into(), "2".into()));
    app.until("the new wait", |m| m.setting("probe_wait_secs") == "2")
        .await;

    // Something craked refuses comes back as its own sentence.
    app.send(Command::AddHosts("nosuch".into(), vec!["10.81.1.5".into()]));
    let end = Instant::now() + Duration::from_secs(5);
    let refused = loop {
        match tokio::time::timeout(end - Instant::now(), app.updates.recv()).await {
            Ok(Ok(Update::Failed(text))) => break text,
            Ok(Ok(u)) => {
                app.model.apply(u);
            }
            _ => panic!("no refusal came back"),
        }
    };
    assert!(!refused.is_empty());

    // The session ends; then the tunnel is removed.
    app.send(Command::Detach("initech".into(), String::new()));
    app.until("Initech off", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "off")
    })
    .await;
    app.until("switched off in the log", |m| {
        m.log.iter().any(|e| e.message == "Switched Initech off.")
    })
    .await;

    // Up again, then its session expires: the app learns why, and the
    // panel says it.
    app.send(Command::Attach(
        "initech".into(),
        fields(&[("device", "tun0"), ("dns", "10.8.0.1")]),
    ));
    app.until("Initech connected again", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "connected")
    })
    .await;
    let expired = format!(
        "{}: the server ends sessions after a while. {}",
        crake_proto::trouble::EXPIRED,
        crake_proto::trouble::SIGN_IN_AGAIN
    );
    app.send(Command::Detach("initech".into(), expired));
    app.until("Initech's session expired", |m| {
        m.tunnels.iter().any(|t| {
            t.name == "initech"
                && t.state == "trouble"
                && crate::model::tunnel_state(t) == "Session expired"
        })
    })
    .await;
    // Seen, in the panel: plain Off, nothing logged, and the reason kept
    // for tunnel detail.
    app.until("why it stopped, logged", |m| {
        m.log
            .iter()
            .any(|e| e.message == "Initech stopped unexpectedly.")
    })
    .await;
    let lines = app.model.log.len();
    app.send(Command::DismissTrouble("initech".into()));
    app.until("Initech dismissed", |m| {
        m.tunnels.iter().any(|t| {
            t.name == "initech" && t.state == "off" && t.why.contains("the server ends sessions")
        })
    })
    .await;
    app.settle(std::time::Duration::from_millis(300)).await;
    assert_eq!(app.model.log.len(), lines, "nothing logged");
    // An OpenVPN 3 session's process that outlived its session: said in
    // the log, under its tunnel, with how to stop it.
    app.send(Command::LeftRunning(
        "initech".into(),
        "initech".into(),
        4242,
    ));
    app.until("the leftover process, logged", |m| {
        m.log.iter().any(|e| {
            e.is_left_running()
                && e.tunnel.as_deref() == Some("initech")
                && e.reason
                    .as_deref()
                    .is_some_and(|r| r.ends_with("sudo kill -9 4242"))
        })
    })
    .await;
    app.send(Command::RemoveTunnel("initech".into()));
    app.until("Initech gone", |m| {
        !m.tunnels.iter().any(|t| t.name == "initech")
    })
    .await;
    assert_eq!(
        app.state("acme").as_deref(),
        Some("off"),
        "ACME untouched throughout"
    );
}
