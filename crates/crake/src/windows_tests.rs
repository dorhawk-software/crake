//! Windows, end to end: the installed craked service (Program Files\Crake,
//! as a person would have it), the app's own client and sign-in with the
//! openconnect beside it, against a real gateway: ocserv with ACME's
//! network behind it, as in `e2e_tests.rs`.
//!
//! Skipped unless CRAKE_E2E=1; run it with `scripts/windows-vm-test.sh`,
//! which installs Crake in a throwaway Windows VM wired only to a gateway VM.

use std::collections::HashMap;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::client::{self, Command};
use crate::test_app::{App, connected, enabled, get, off, sign_in};

/// The craked service's own log.
const LOG: &str = r"C:\ProgramData\Crake\craked.log";

/// PowerShell's answer to `cmd`.
fn ps(cmd: &str) -> String {
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", cmd])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// The addresses Windows' own resolver gives `name` (as any app asks), or
/// why it gave none.
fn resolve(name: &str) -> Result<Vec<IpAddr>, String> {
    (name, 80)
        .to_socket_addrs()
        .map(|a| a.map(|a| a.ip()).collect())
        .map_err(|e| e.to_string())
}

/// `get` until it answers or `wait` is over: an app retrying.
fn get_within(ip: &str, wait: Duration) -> (Option<String>, u32) {
    let end = Instant::now() + wait;
    let mut tries = 0;
    while Instant::now() < end {
        tries += 1;
        match get(ip, Duration::from_secs(4)) {
            Ok(body) => return (Some(body), tries),
            Err(e) => eprintln!("  {ip}: {e}"),
        }
    }
    (None, tries)
}

/// The routes on an interface, one prefix a line.
fn routes(alias: &str) -> String {
    ps(&format!(
        "Get-NetRoute -InterfaceAlias {alias} -AddressFamily IPv4 -ErrorAction SilentlyContinue | \
         ForEach-Object {{ $_.DestinationPrefix }}"
    ))
}

/// The app connected to the craked service, with a tunnel `name` freshly
/// added (New tunnel's "Add") to the test gateway, with `more` fields, and
/// no other test tunnel.
async fn added(name: &str, display: &str, more: &[(&str, &str)]) -> App {
    let (tx, updates) = async_channel::unbounded();
    let mut app = App::new(client::start(tx), updates);
    app.within(Duration::from_secs(30), "the craked service", |m| {
        m.reachable == Some(true) && !m.settings.is_empty()
    })
    .await;
    // No test tunnel left from before (one that's up would clash).
    for old in ["acme", "globex", name] {
        if app.state(old).is_some() {
            app.send(Command::RemoveTunnel(old.into()));
            app.until("an old one gone", |m| {
                !m.tunnels.iter().any(|t| t.name == old)
            })
            .await;
        }
    }
    let gateway = crate::test_app::gateway();
    let fields: HashMap<String, String> = [
        ("display_name", display),
        ("gateway", &gateway),
        ("protocol", "anyconnect"),
        ("user", "alice"),
    ]
    .iter()
    .chain(more)
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();
    app.send(Command::AddTunnel(fields));
    app.until("added", |m| m.tunnels.iter().any(|t| t.name == name))
        .await;
    app
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_installed_service_end_to_end() {
    if !enabled() {
        return;
    }
    let log_from = std::fs::metadata(LOG).map_or(0, |m| m.len());
    let mut app = added("acme", "ACME", &[]).await;

    // Nothing reaches ACME's network before the tunnel.
    let before = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_secs(1)))
        .await
        .unwrap();
    assert!(before.is_err(), "10.40.1.10 answered without a tunnel");

    // Sign in as the app does, then switch on with that session.
    let session = sign_in().await;
    app.send(Command::ConnectWithSession("acme".into(), session));
    app.within(Duration::from_secs(30), "ACME connected", connected)
        .await;

    // Its address on its own adapter; nothing routed but its DNS, although
    // the gateway pushes 10.40.0.0/16.
    let addr = ps("(Get-NetIPAddress -InterfaceAlias crake-acme -AddressFamily IPv4).IPAddress");
    assert!(addr.contains("10.40.255."), "crake-acme's address: {addr}");
    let routes = routes("crake-acme");
    assert!(!routes.contains("10.40.0.0/16"), "routes: {routes}");
    assert!(routes.contains("10.40.0.2/32"), "routes: {routes}");

    // 1. A host that doesn't answer directly: probed, and the app gets
    //    through on a retry at the latest.
    let (probed, tries) =
        tokio::task::spawn_blocking(|| get_within("10.40.1.10", Duration::from_secs(20)))
            .await
            .unwrap();
    assert!(
        probed.is_some_and(|b| b.contains("crex")),
        "git.acme.internal's page"
    );
    eprintln!("  10.40.1.10 answered on try {tries}");
    app.until("the probe's decision", |m| {
        m.log
            .iter()
            .any(|e| e.message == "Sent 10.40.1.10 through ACME.")
    })
    .await;

    // 2. A name only ACME's DNS knows, asked of Windows' resolver (NRPT →
    //    craked's DNS): routed before the first packet.
    let named = tokio::task::spawn_blocking(|| {
        let addrs = resolve("wiki.acme.internal")?;
        assert!(
            addrs.contains(&IpAddr::from([10, 40, 1, 11])),
            "wiki.acme.internal is {addrs:?}"
        );
        // Shorter than the probe wait: only a route in place before the
        // first packet gets through this fast.
        get("10.40.1.11", Duration::from_millis(800)).map_err(|e| e.to_string())
    })
    .await
    .unwrap();
    assert!(
        named.is_ok_and(|b| b.contains("crex")),
        "wiki.acme.internal's page at once"
    );

    // 2b. IPv6: the tunnel has the IPv6 address the gateway gave, and a
    //     name ACME's DNS knows by its IPv6 address is routed before the
    //     first packet.
    let addr6 = ps("(Get-NetIPAddress -InterfaceAlias crake-acme -AddressFamily IPv6).IPAddress");
    assert!(
        addr6.contains("fd40:ff::"),
        "crake-acme's IPv6 address: {addr6}"
    );
    // ACME's DNS reached over IPv6, the gateway's first: routed through
    //     the tunnel and asked (only it knows dns6.acme.internal).
    let routes6 = ps(
        "Get-NetRoute -InterfaceAlias crake-acme -AddressFamily IPv6 | ForEach-Object { $_.DestinationPrefix }",
    );
    assert!(routes6.contains("fd40::2/128"), "IPv6 routes: {routes6}");
    let by_dns6 = tokio::task::spawn_blocking(|| resolve("dns6.acme.internal"))
        .await
        .unwrap();
    assert!(
        by_dns6
            .as_ref()
            .is_ok_and(|a| a.contains(&IpAddr::from([10, 40, 1, 10]))),
        "dns6.acme.internal, from ACME's IPv6 DNS: {by_dns6:?}"
    );
    let named6 = tokio::task::spawn_blocking(|| {
        let addrs = resolve("v6.acme.internal")?;
        let want: IpAddr = "fd40:1::10".parse().unwrap();
        assert!(addrs.contains(&want), "v6.acme.internal is {addrs:?}");
        get("[fd40:1::10]", Duration::from_millis(800)).map_err(|e| e.to_string())
    })
    .await
    .unwrap();
    assert!(
        named6.is_ok_and(|b| b.contains("crex")),
        "v6.acme.internal's page over IPv6 at once; routes: {}",
        ps(
            "Get-NetRoute -InterfaceAlias crake-acme -AddressFamily IPv6 | Format-Table -AutoSize | Out-String"
        )
    );

    // 3. A name public DNS doesn't know but ACME's does (outside its
    //    domains): rescued, routed before the first packet.
    let rescued = tokio::task::spawn_blocking(|| {
        let addrs = resolve("files.acme.example")?;
        assert!(
            addrs.contains(&IpAddr::from([10, 40, 1, 12])),
            "files.acme.example is {addrs:?}"
        );
        get("10.40.1.12", Duration::from_millis(800)).map_err(|e| e.to_string())
    })
    .await
    .unwrap();
    assert!(
        rescued.is_ok_and(|b| b.contains("crex")),
        "files.acme.example's page at once"
    );
    app.until("the rescue's decision", |m| {
        m.log
            .iter()
            .any(|e| e.message == "Sent files.acme.example through ACME.")
    })
    .await;

    // Over TCP too (Windows asks again over TCP when an answer was cut).
    let tcp = ps("(Resolve-DnsName -Name git.acme.internal -Type A -TcpOnly -DnsOnly).IPAddress");
    assert!(
        tcp.contains("10.40.1.10"),
        "git.acme.internal over TCP: {tcp}"
    );

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

    // The charts' numbers (GetIfTable2's counters, sampled every 10 s):
    // bytes through ACME for tunnel detail, direct ones for the panel.
    let end = Instant::now() + Duration::from_secs(40);
    let through = |m: &crate::model::Model| {
        m.tunnel_traffic
            .get("acme")
            .is_some_and(|t| t.iter().map(|(_, b)| b).sum::<u64>() > 0)
    };
    while !through(&app.model) && Instant::now() < end {
        app.send(Command::TunnelTraffic("acme".into()));
        app.settle(Duration::from_secs(3)).await;
    }
    assert!(through(&app.model), "no bytes through ACME in its chart");
    app.within(Duration::from_secs(45), "direct bytes in the chart", |m| {
        m.traffic.iter().map(|(_, b)| b).sum::<u64>() > 0
    })
    .await;

    // Switched off: the hosts go direct again, and ACME's names are refused
    // at once by craked's DNS (kept private), never asked publicly.
    app.send(Command::Disconnect("acme".into()));
    app.within(Duration::from_secs(15), "ACME off", off).await;
    let after = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_secs(1)))
        .await
        .unwrap();
    assert!(after.is_err(), "10.40.1.10 still answered with ACME off");
    let (refused, took) = tokio::task::spawn_blocking(|| {
        let start = Instant::now();
        (resolve("git.acme.internal"), start.elapsed())
    })
    .await
    .unwrap();
    assert!(refused.is_err(), "git.acme.internal resolved: {refused:?}");
    assert!(
        took < Duration::from_millis(1500),
        "refused at once, not after a timeout ({took:?})"
    );

    app.send(Command::RemoveTunnel("acme".into()));
    app.until("ACME removed", |m| {
        !m.tunnels.iter().any(|t| t.name == "acme")
    })
    .await;
    // What craked logged meanwhile: no errors.
    let log = std::fs::read(LOG).unwrap_or_default();
    let log = String::from_utf8_lossy(
        log.get(usize::try_from(log_from).unwrap()..)
            .unwrap_or_default(),
    );
    let trouble: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("ERROR") || l.contains("panicked"))
        .collect();
    assert!(trouble.is_empty(), "craked.log: {trouble:#?}");
}

/// A full tunnel (New tunnel's "Everything"): all traffic through it but
/// its own gateway and the LAN, every name asked of its DNS.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_tunnel() {
    if !enabled() {
        return;
    }
    let mut app = added("globex", "Globex", &[("mode", "full")]).await;
    let session = sign_in().await;
    app.send(Command::ConnectWithSession("globex".into(), session));
    let up = |m: &crate::model::Model| {
        m.tunnels
            .iter()
            .any(|t| t.name == "globex" && t.state == "connected")
    };
    app.within(Duration::from_secs(30), "Globex connected", up)
        .await;

    // The two halves through the tunnel; its gateway kept on the LAN.
    let through = routes("crake-globex");
    assert!(
        through.contains("0.0.0.0/1") && through.contains("128.0.0.0/1"),
        "routes: {through}"
    );
    let gateway = ps("(Get-NetRoute -DestinationPrefix 198.51.100.10/32).InterfaceAlias");
    assert!(
        !gateway.trim().is_empty() && !gateway.contains("crake-"),
        "the gateway's own route: {gateway}"
    );

    // Everything goes through at once: no probing needed.
    let at_once = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_millis(800)))
        .await
        .unwrap();
    assert!(
        at_once.is_ok_and(|b| b.contains("crex")),
        "10.40.1.10 at once"
    );
    // Every name to its DNS: the root's rule.
    let rule = ps(
        r"(Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig\Crake crake-globex').Name",
    );
    assert!(
        rule.lines().any(|l| l.trim() == "."),
        "the NRPT rule: {rule}"
    );
    let named = tokio::task::spawn_blocking(|| resolve("wiki.acme.internal"))
        .await
        .unwrap();
    assert!(
        named.is_ok_and(|a| a.contains(&IpAddr::from([10, 40, 1, 11]))),
        "wiki.acme.internal through Globex's DNS"
    );
    // IPv6 too (the gateway gave the tunnel an IPv6 address): its halves
    // through the tunnel, and the internet's IPv6 host reached that way.
    let through6 = ps(
        "Get-NetRoute -InterfaceAlias crake-globex -AddressFamily IPv6 | ForEach-Object { $_.DestinationPrefix }",
    );
    assert!(
        through6.contains("::/1") && through6.contains("8000::/1"),
        "IPv6 routes: {through6}"
    );
    let way6 = ps(
        "(Find-NetRoute -RemoteIPAddress 2001:db8:77::1 | Select-Object -First 1).InterfaceAlias",
    );
    assert!(
        way6.contains("crake-globex"),
        "2001:db8:77::1 goes by {way6}"
    );
    let v6 = tokio::task::spawn_blocking(|| get("[2001:db8:77::1]", Duration::from_secs(3)))
        .await
        .unwrap();
    assert!(v6.is_ok_and(|b| b.contains("crex")), "IPv6 through Globex");
    // Still up a while later: its own packets didn't loop into it.
    app.settle(Duration::from_secs(5)).await;
    assert_eq!(app.state("globex").as_deref(), Some("connected"));

    app.send(Command::Disconnect("globex".into()));
    app.within(Duration::from_secs(15), "Globex off", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "globex" && t.state == "off")
    })
    .await;
    let left = ps("Get-NetRoute -DestinationPrefix 198.51.100.10/32 -ErrorAction SilentlyContinue");
    assert!(left.trim().is_empty(), "the gateway's route stayed: {left}");
    app.send(Command::RemoveTunnel("globex".into()));
    app.until("Globex removed", |m| {
        !m.tunnels.iter().any(|t| t.name == "globex")
    })
    .await;
    drop(app);
    a_full_tunnel_without_ipv6().await;
}

/// A full tunnel whose gateway gives no IPv6 address (the second gateway,
/// on 8443): IPv6 is refused at once, never sent around the tunnel.
async fn a_full_tunnel_without_ipv6() {
    let gateway = format!("{}:8443", crate::test_app::gateway());
    let direct = tokio::task::spawn_blocking(|| get("[2001:db8:77::1]", Duration::from_secs(3)))
        .await
        .unwrap();
    assert!(direct.is_ok(), "IPv6 direct before: {direct:?}");
    let mut app = added(
        "initech",
        "Initech",
        &[("mode", "full"), ("gateway", &gateway)],
    )
    .await;
    let session = crate::test_app::sign_in_to(&gateway).await;
    app.send(Command::ConnectWithSession("initech".into(), session));
    let state = |want: &'static str| {
        move |m: &crate::model::Model| {
            m.tunnels
                .iter()
                .any(|t| t.name == "initech" && t.state == want)
        }
    };
    app.within(
        Duration::from_secs(30),
        "Initech connected",
        state("connected"),
    )
    .await;
    let (refused, took) = tokio::task::spawn_blocking(|| {
        let start = Instant::now();
        (
            get("[2001:db8:77::1]", Duration::from_secs(5)),
            start.elapsed(),
        )
    })
    .await
    .unwrap();
    assert!(refused.is_err(), "IPv6 went around Initech: {refused:?}");
    assert!(
        took < Duration::from_secs(2),
        "refused at once ({took:?}): {refused:?}; routes: {}",
        ps("Get-NetRoute -AddressFamily IPv6 | Format-Table -AutoSize | Out-String")
    );
    // IPv4 still goes through.
    let v4 = tokio::task::spawn_blocking(|| get("10.40.1.10", Duration::from_secs(3)))
        .await
        .unwrap();
    assert!(v4.is_ok_and(|b| b.contains("crex")), "IPv4 through Initech");
    app.send(Command::Disconnect("initech".into()));
    app.within(Duration::from_secs(15), "Initech off", state("off"))
        .await;
    let back = tokio::task::spawn_blocking(|| get("[2001:db8:77::1]", Duration::from_secs(3)))
        .await
        .unwrap();
    assert!(back.is_ok(), "IPv6 direct again: {back:?}");
    app.send(Command::RemoveTunnel("initech".into()));
    app.until("Initech removed", |m| {
        !m.tunnels.iter().any(|t| t.name == "initech")
    })
    .await;
}

/// OpenVPN on Windows (CLAUDE.md, 7i, slice 4): the installed service runs
/// the openvpn beside it against ACME's OpenVPN gateway, on an adapter of
/// its own (ovpn-dco, made with tapctl), with a password sign-in answered
/// as the app's sign-in window does; a host that doesn't answer directly
/// gets through it; off, its adapter goes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_openvpn_tunnel() {
    if !enabled() {
        return;
    }
    let ca = tokio::task::spawn_blocking(|| {
        let addr: std::net::SocketAddr = "198.51.100.10:80".parse().unwrap();
        let mut s = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(5)).unwrap();
        use std::io::{Read, Write};
        s.write_all(b"GET /openvpn-ca.crt HTTP/1.0\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).unwrap();
        body.split_once("\r\n\r\n")
            .map(|(_, b)| b.to_owned())
            .unwrap_or_default()
    })
    .await
    .unwrap();
    assert!(ca.contains("BEGIN CERTIFICATE"), "the gateway's CA: {ca}");
    let profile = format!(
        "client\ndev tun\nproto udp\nremote 198.51.100.10 1194\nnobind\nauth-user-pass\n\
         remote-cert-tls server\n<ca>\n{ca}</ca>\n"
    );
    let (tx, updates) = async_channel::unbounded();
    let mut app = App::new(client::start(tx), updates);
    app.within(Duration::from_secs(30), "the craked service", |m| {
        m.reachable == Some(true) && !m.settings.is_empty()
    })
    .await;
    for old in ["acme", "globex", "initech"] {
        if app.state(old).is_some() {
            app.send(Command::RemoveTunnel(old.into()));
            app.until("an old one gone", |m| {
                !m.tunnels.iter().any(|t| t.name == old)
            })
            .await;
        }
    }
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
    app.within(Duration::from_secs(60), "Initech connected", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "connected")
    })
    .await;
    let addr = ps("(Get-NetIPAddress -InterfaceAlias crake-initech -AddressFamily IPv4).IPAddress");
    assert!(
        addr.contains("10.40.254."),
        "crake-initech's address: {addr}"
    );
    let (through, tries) =
        tokio::task::spawn_blocking(|| get_within("10.40.1.12", Duration::from_secs(30)))
            .await
            .unwrap();
    assert!(
        through.is_some_and(|b| b.contains("crex")),
        "10.40.1.12 through Initech ({tries} tries)"
    );

    app.send(Command::Disconnect("initech".into()));
    app.within(Duration::from_secs(30), "Initech off", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "off")
    })
    .await;
    let left = ps("Get-NetAdapter -Name crake-initech -ErrorAction SilentlyContinue");
    assert!(left.trim().is_empty(), "its adapter went: {left}");
    app.send(Command::RemoveTunnel("initech".into()));
    app.until("removed", |m| {
        !m.tunnels.iter().any(|t| t.name == "initech")
    })
    .await;
}
