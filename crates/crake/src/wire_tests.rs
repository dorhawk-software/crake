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
use crate::model::Model;

/// The app's side: its model, fed by the client's updates.
pub(crate) struct App {
    pub model: Model,
    pub updates: async_channel::Receiver<Update>,
    commands: async_channel::Sender<Command>,
}

impl App {
    pub fn send(&self, c: Command) {
        self.commands.send_blocking(c).unwrap();
    }

    /// Apply updates until `done` holds for the model (5 s at most).
    pub async fn until(&mut self, what: &str, done: impl Fn(&Model) -> bool) {
        self.within(Duration::from_secs(5), what, done).await;
    }

    pub async fn within(&mut self, wait: Duration, what: &str, done: impl Fn(&Model) -> bool) {
        let end = Instant::now() + wait;
        while !done(&self.model) {
            let left = end.saturating_duration_since(Instant::now());
            match tokio::time::timeout(left, self.updates.recv()).await {
                Ok(Ok(Update::Failed(text))) => {
                    panic!("craked refused while waiting for {what}: {text}")
                }
                Ok(Ok(u)) => {
                    self.model.apply(u);
                }
                _ => panic!("never saw {what}"),
            }
        }
    }

    /// Apply whatever arrives for `wait`.
    pub async fn settle(&mut self, wait: Duration) {
        let end = Instant::now() + wait;
        while let Ok(Ok(u)) = tokio::time::timeout(
            end.saturating_duration_since(Instant::now()),
            self.updates.recv(),
        )
        .await
        {
            self.model.apply(u);
        }
    }

    pub fn state(&self, tunnel: &str) -> Option<String> {
        self.model
            .tunnels
            .iter()
            .find(|t| t.name == tunnel)
            .map(|t| t.state.clone())
    }

    fn logged(&self, message: &str) -> bool {
        self.model.log.iter().any(|e| e.message == message)
    }
}

fn config(dir: &std::path::Path) -> Config {
    Config {
        hook_path: "/bin/true".into(),
        hook_socket: dir.join("hook.sock"),
        openconnect: "/bin/false".into(),
        log_path: dir.join("log"),
        state_path: dir.join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
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
    let commands = client::start_on(socket, tx);
    App {
        model: Model::default(),
        updates,
        commands,
    }
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
