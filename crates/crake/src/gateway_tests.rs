//! Real gateways, reached through Cloudflare (CLAUDE.md, 7j), on CI's Mac
//! and Windows: the e2e gateway's container runs in a Linux job (ocserv,
//! and openvpn over TCP, with ACME's internal network behind them),
//! Cloudflare Tunnel carries each here (`cloudflared access tcp`, on
//! 127.0.0.1), and craked (macOS: in the test, as root; Windows: the
//! installed service) is driven by the app's own client and sign-in.
//! Connected, a host only the tunnel reaches answers, a name only its DNS
//! knows is routed before the first packet, and the internet still answers
//! directly; off, the internal network is gone again.
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
use crate::test_app::{App, connected, enabled, gateway, get, off, sign_in};

/// How long a tunnel, and a host through it, get.
const WAIT: Duration = Duration::from_secs(60);
/// A public address that answers anywhere: it must answer directly.
const PUBLIC: &str = "1.1.1.1:443";
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
    #[cfg(target_os = "macos")]
    _dir: tempfile::TempDir,
    #[cfg(target_os = "macos")]
    mgr: std::sync::Arc<craked::manager::Manager>,
    app: App,
    /// The socket craked serves the app on: the test's own (macOS), or
    /// none, the installed service's (Windows).
    socket: Option<std::path::PathBuf>,
}

impl Crake {
    /// macOS: craked in this test, as root, with a folder of its own.
    #[cfg(target_os = "macos")]
    async fn craked() -> Self {
        use craked::config::Config;
        let dir = tempfile::tempdir().unwrap();
        let hook = std::env::var("CRAKE_E2E_HOOK").expect("CRAKE_E2E_HOOK: craked's crake-hook");
        let cfg = Config {
            hook_path: hook.into(),
            hook_socket: dir.path().join("hook.sock"),
            log_path: dir.path().join("log"),
            state_path: dir.path().join("hosts.json"),
            probe_wait_secs: 1,
            race_first_syn: false,
            managed_dir: dir.path().join("managed.d"),
            ..Config::default()
        };
        let hook_socket = cfg.hook_socket.clone();
        let mgr = craked::manager::Manager::new(cfg).unwrap();
        mgr.init().await.unwrap();
        craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
        let app = crate::test_app::serve(mgr.clone(), dir.path());
        Self {
            socket: Some(dir.path().join("crake.sock")),
            _dir: dir,
            mgr,
            app,
        }
    }

    /// Windows: the craked service the installer put there.
    #[cfg(windows)]
    async fn craked() -> Self {
        let (tx, updates) = async_channel::unbounded();
        Self {
            app: App::new(crate::client::start(tx), updates),
            socket: None,
        }
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

    /// The device `tunnel` runs on.
    #[cfg(target_os = "macos")]
    fn device(&self, tunnel: &str) -> String {
        self.mgr.get(tunnel).unwrap().info.unwrap().device
    }

    #[cfg(windows)]
    fn device(&self, tunnel: &str) -> String {
        format!("crake-{tunnel}")
    }

    async fn stop(mut self) {
        self.remove_tunnels().await;
        #[cfg(target_os = "macos")]
        self.mgr.shutdown().await;
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

/// The device the system sends `ip` out of.
#[cfg(target_os = "macos")]
fn route_device(ip: &str) -> String {
    sh(&format!("route -n get {ip}"))
        .lines()
        .find_map(|l| l.trim().strip_prefix("interface: ").map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(windows)]
fn route_device(ip: &str) -> String {
    sh(&format!(
        "(Find-NetRoute -RemoteIPAddress {ip} | Select-Object -First 1).InterfaceAlias"
    ))
    .trim()
    .to_owned()
}

/// A public name looked up as an app looks it up, and its address
/// answering: the internet works by name.
fn public_name_answers() -> bool {
    PUBLIC_NAME
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.find(SocketAddr::is_ipv4))
        .is_some_and(|a| TcpStream::connect_timeout(&a, Duration::from_secs(5)).is_ok())
}

/// The internet works by name within `wait`.
async fn names_work(what: &str, wait: Duration) {
    let end = std::time::Instant::now() + wait;
    while !tokio::task::spawn_blocking(public_name_answers)
        .await
        .unwrap()
    {
        assert!(
            std::time::Instant::now() < end,
            "{}: {what}",
            "public names don't resolve"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// What the system makes of ACME's host and the tunnel's device, and
/// what craked decided: for a failure's message.
fn diagnose(device: &str) -> String {
    #[cfg(target_os = "macos")]
    let seen = sh(&format!(
        "route -n get 10.40.1.10; ifconfig {device}; netstat -rn -f inet | head -40"
    ));
    #[cfg(windows)]
    let seen = sh(&format!(
        "Find-NetRoute -RemoteIPAddress 10.40.1.10 | Format-List | Out-String; \
         Get-NetIPAddress -InterfaceAlias {device} -ErrorAction SilentlyContinue | Format-Table | Out-String; \
         Get-NetRoute -InterfaceAlias {device} -ErrorAction SilentlyContinue | Format-Table | Out-String; \
         'ACME DNS through the tunnel:'; Resolve-DnsName git.acme.internal -Server 10.40.0.2 -DnsOnly -QuickTimeout -ErrorAction Continue 2>&1 | Out-String; \
         'decisions:'; Get-Content C:\\ProgramData\\Crake\\log -Tail 15 | Out-String; \
         'craked:'; Get-Content C:\\ProgramData\\Crake\\craked.log | Select-String -Pattern 'probe|flow|stuck|SYN|10.40.1.10|WARN|ERROR' | Select-Object -Last 40 | Out-String"
    ));
    format!("where 10.40.1.10 goes and what craked decided:\n{seen}")
}

/// Where wiki.acme.internal's answer came from, and what craked was told
/// of the tunnel's DNS: for a failure's message.
fn diagnose_name(device: &str) -> String {
    #[cfg(target_os = "macos")]
    let seen = sh(&format!(
        "scutil --dns | head -60; route -n get 10.40.1.11; ifconfig {device}; cat /etc/resolver/* 2>/dev/null"
    ));
    #[cfg(windows)]
    let seen = sh(&format!(
        "'cache:'; Get-DnsClientCache -Entry wiki.acme.internal -ErrorAction SilentlyContinue | Format-List | Out-String; \
         'NRPT:'; Get-DnsClientNrptRule | Format-Table Namespace,NameServers -AutoSize | Out-String; \
         'route:'; Find-NetRoute -RemoteIPAddress 10.40.1.11 | Select-Object -Last 1 | Format-List InterfaceAlias,DestinationPrefix | Out-String; \
         'decisions:'; Get-Content C:\\ProgramData\\Crake\\log -Tail 8 | Out-String; \
         'the hook:'; Get-Content C:\\ProgramData\\Crake\\craked.log | Select-String -Pattern \"the hook's\" | Select-Object -Last 3 | Out-String; \
         '{device}'"
    ));
    format!("where wiki.acme.internal's answer came from:\n{seen}")
}

/// What a tunnel that's up must give: ACME's network through it, its names
/// before the first packet, and the internet directly.
async fn the_split_holds(crake: &Crake, tunnel: &str) {
    let device = crake.device(tunnel);
    // 1. A host that doesn't answer directly: probed, then through.
    let probed = get_within("10.40.1.10").await;
    assert!(
        probed.is_some_and(|b| b.contains("crex")),
        "git.acme.internal through {tunnel}\n{}",
        diagnose(&device)
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
        "wiki.acme.internal through {tunnel}\n{}",
        diagnose_name(&device)
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
    // 4. And by name: craked's DNS answers public names too.
    names_work("while a tunnel is up", WAIT).await;
    // 5. A report (the log's "Save a report…") says so, part by part.
    report_holds(crake, tunnel).await;
}

/// The report while `tunnel` is up: the internet answered directly, ACME's
/// name through the tunnel, this system's DNS with craked's server in it,
/// its routes and Crake's rules. Printed whole for the run's log.
async fn report_holds(crake: &Crake, tunnel: &str) {
    let label = if tunnel == "acme" { "ACME" } else { "Initech" };
    let report = crate::report::gather_from(crake.socket.clone()).await;
    let text = crate::report::text(&report, false, &jiff::Zoned::now());
    println!("{text}");
    let computer = report.computer.as_ref().expect("craked's side");
    let check = |start: &str| {
        computer
            .checks
            .iter()
            .find(|c| c.starts_with(start))
            .cloned()
            .unwrap_or_else(|| panic!("{start}…\n{text}"))
    };
    let internet = check("The internet: one.one.one.one is ");
    assert!(internet.ends_with(", direct."), "{internet}");
    let names = check(&format!("{label}'s names: "));
    assert!(names.ends_with(&format!("through {label}.")), "{names}");
    let part = |title: &str| {
        computer
            .parts
            .iter()
            .find(|p| p.title.starts_with(title))
            .map(|p| p.lines.join("\n"))
            .unwrap_or_else(|| panic!("{title}…\n{text}"))
    };
    assert!(part("DNS").contains("127.53.0.1"), "{text}");
    assert!(part("Routes").contains("\u{2190} Crake: "), "{text}");
    assert!(!part("Crake's rules").is_empty(), "{text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anyconnect_through_cloudflare() {
    if !enabled() {
        return;
    }
    let mut crake = Crake::start().await;
    let before = get("10.40.1.10", Duration::from_secs(2));
    assert!(before.is_err(), "10.40.1.10 answered without a tunnel");

    let session = sign_in().await;
    crake
        .app
        .send(Command::ConnectWithSession("acme".into(), session));
    crake.app.within(WAIT, "ACME connected", connected).await;
    the_split_holds(&crake, "acme").await;

    crake.app.send(Command::Disconnect("acme".into()));
    crake.app.within(WAIT, "ACME off", off).await;
    assert!(get("10.40.1.10", Duration::from_secs(2)).is_err());
    names_work("switched off", WAIT).await;
    no_nat_left("switched off");
    crake.stop().await;
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
    let mut crake = Crake::start().await;
    crake
        .add(&[
            ("display_name", "Initech"),
            ("protocol", "openvpn"),
            ("profile", profile.as_str()),
        ])
        .await;
    let app = &mut crake.app;
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
    the_split_holds(&crake, "initech").await;

    let app = &mut crake.app;
    app.send(Command::Disconnect("initech".into()));
    app.within(WAIT, "Initech off", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "off")
    })
    .await;
    assert!(get("10.40.1.10", Duration::from_secs(2)).is_err());
    names_work("switched off", WAIT).await;
    no_nat_left("switched off");
    crake.stop().await;
}

/// macOS: craked as a process of its own, serving the app where the
/// installed one does (crake_proto::APP_SOCKET), its files in `dir`.
#[cfg(target_os = "macos")]
fn own_craked(dir: &std::path::Path) -> std::process::Child {
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
        let _ = craked.kill();
        let _ = craked.wait();
        sh("pkill -9 openconnect");
        craked = own_craked(dir.path());
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
