//! Real nftables and policy routing, for a throwaway network namespace only.
//! Skipped unless CRAKE_NETNS_TEST=1; run it with `scripts/netns-test.sh`,
//! which uses a disposable container with no network.

#![cfg(target_os = "linux")]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::Duration;

use crake_proto::TunnelState;
use craked::config::{Config, Protocol, TunnelConfig};
use craked::hook_server;
use craked::manager::Manager;

const FAKE: &str = r#"#!/bin/sh
ip link add crake-acme type dummy 2>/dev/null
ip link set crake-acme up
export TUNDEV=crake-acme VPNGATEWAY=192.0.2.10 INTERNAL_IP4_ADDRESS=10.40.255.7 \
  INTERNAL_IP4_NETMASK=255.255.255.255 INTERNAL_IP4_DNS=10.40.0.2
reason=connect "$CRAKE_TEST_HOOK" || exit 1
trap 'ip link del crake-acme; exit 0' TERM
while :; do sleep 0.1; done
"#;

// Like FAKE, but the tunnel device also gets an IPv6 address, so IPv6
// destinations routed to it have a source to leave with.
const FAKE6: &str = r#"#!/bin/sh
ip link add crake-acme type dummy 2>/dev/null
ip link set crake-acme up
ip -6 addr add fd00:ac::7/64 dev crake-acme nodad
export TUNDEV=crake-acme VPNGATEWAY=192.0.2.10 INTERNAL_IP4_ADDRESS=10.40.255.7 \
  INTERNAL_IP4_NETMASK=255.255.255.255 INTERNAL_IP4_DNS=10.40.0.2
reason=connect "$CRAKE_TEST_HOOK" || exit 1
trap 'ip link del crake-acme; exit 0' TERM
while :; do sleep 0.1; done
"#;

// Like FAKE, but openconnect hands over an IPv6 address too, which craked
// gives the device.
const FAKE_IP6: &str = r#"#!/bin/sh
ip link add crake-acme type dummy 2>/dev/null
ip link set crake-acme up
export TUNDEV=crake-acme VPNGATEWAY=192.0.2.10 INTERNAL_IP4_ADDRESS=10.40.255.7 \
  INTERNAL_IP4_NETMASK=255.255.255.255 INTERNAL_IP4_DNS=10.40.0.2 \
  INTERNAL_IP6_ADDRESS=fd00:ac::7 INTERNAL_IP6_NETMASK=fd00:ac::7/64
reason=connect "$CRAKE_TEST_HOOK" || exit 1
trap 'ip link del crake-acme; exit 0' TERM
while :; do sleep 0.1; done
"#;

/// A TCP connection to `addr` within half a second: how it ended.
fn tcp_to(addr: &str) -> std::io::ErrorKind {
    let addr: std::net::SocketAddr = addr.parse().unwrap();
    match std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
        Ok(_) => std::io::ErrorKind::Other,
        Err(e) => e.kind(),
    }
}

fn enabled() -> bool {
    std::env::var("CRAKE_NETNS_TEST").as_deref() == Ok("1")
}

fn sh(cmd: &str) -> String {
    let out = Command::new("sh").args(["-c", cmd]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// Packets the tunnel interface has sent.
fn tx() -> u64 {
    let out = sh("ip -j -s link show crake-acme");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    v[0]["stats64"]["tx"]["packets"].as_u64().unwrap()
}

/// Did a packet to `ip` leave through the tunnel?
fn goes_through_tunnel(ip: &str) -> bool {
    let before = tx();
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
    let _ = sock.send_to(b"crex", format!("{ip}:9"));
    std::thread::sleep(Duration::from_millis(50));
    tx() > before
}

/// The same for an IPv6 destination.
fn goes_through_tunnel6(ip: &str) -> bool {
    let before = tx();
    let Ok(sock) = std::net::UdpSocket::bind("[::]:0") else {
        return false;
    };
    let _ = sock.send_to(b"crex", format!("[{ip}]:9"));
    std::thread::sleep(Duration::from_millis(50));
    tx() > before
}

async fn wait_for(mgr: &Manager, want: TunnelState) {
    for _ in 0..100 {
        if mgr.get("acme").unwrap().state == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("never reached {want:?}");
}

#[tokio::test]
async fn routes_only_what_it_is_told() {
    if !enabled() {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    // Strict reverse-path filtering, as Fedora has it.
    strict_rp_filter();
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("openconnect");
    std::fs::write(&fake, FAKE).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    unsafe { std::env::set_var("CRAKE_TEST_HOOK", env!("CARGO_BIN_EXE_crake-hook")) };
    let state_path = dir.path().join("hosts.json");
    let cfg = Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.path().join("hook.sock"),
        openconnect: fake,
        log_path: dir.path().join("log"),
        state_path: state_path.clone(),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        never: vec![],
        use_resolved: false,
        apply_network: true,
        managed_dir: dir.path().join("managed.d"),
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
    };
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    // A stand-in for the Wi-Fi: everything goes direct by default.
    sh("ip link add uplink type dummy; ip link set uplink up; \
        ip addr add 192.168.0.242/24 dev uplink; ip route add default via 192.168.0.1 dev uplink");
    // Leftovers from a crashed run are cleared at startup.
    sh("nft add table inet crake; ip rule add fwmark 0x43520005 lookup 7305 priority 7305");
    mgr.init().await.unwrap();
    assert!(!sh("nft list tables").contains("crake"));
    assert!(!sh("ip rule").contains("7305"));

    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    // Pinned while off: stored, applied on connect.
    mgr.pin("acme", "10.40.0.0/16").await.unwrap();
    mgr.connect("acme").unwrap();
    wait_for(&mgr, TunnelState::Connected).await;
    assert!(sh("ip rule").contains("lookup 7301"), "{}", sh("ip rule"));
    assert!(goes_through_tunnel("10.40.1.1"), "pinned network");
    assert!(!goes_through_tunnel("10.99.1.1"), "nothing else");

    // Add, then forget, a single host.
    mgr.add_host("acme", "10.33.1.1").await.unwrap();
    assert!(goes_through_tunnel("10.33.1.1"), "added host");
    assert!(!goes_through_tunnel("10.33.1.2"), "only that host");
    let rows = mgr.hosts("acme").await.unwrap();
    assert_eq!(rows.len(), 2);
    let left = rows[1].expires_in.unwrap();
    assert!(left > 6 * 86_400 && left <= 7 * 86_400, "{left}");
    mgr.forget("acme", "10.33.1.1").await.unwrap();
    assert!(!goes_through_tunnel("10.33.1.1"), "forgotten host");

    // Pin a learned host: it stops expiring.
    mgr.add_host("acme", "10.33.1.3").await.unwrap();
    mgr.pin("acme", "10.33.1.3").await.unwrap();
    assert!(goes_through_tunnel("10.33.1.3"));
    assert!(!sh("nft list set inet crake learned_acme").contains("10.33.1.3"));

    // A host pinned inside a pinned network (nft's interval sets refuse
    // overlaps: it once failed this pin and every later connect).
    mgr.pin("acme", "10.40.5.5").await.unwrap();
    assert!(goes_through_tunnel("10.40.5.5"));

    // Never the gateway itself.
    let err = mgr.pin("acme", "192.0.2.0/24").await.unwrap_err();
    assert!(err.to_string().contains("gateway"), "{err}");
    assert!(mgr.add_host("acme", "127.0.0.1").await.is_err());

    // Masquerade: the tunnel's address is the source.
    assert!(sh("nft list chain inet crake nat_acme").contains("masquerade"));

    // Off: everything of this tunnel leaves the kernel; decisions stay on disk.
    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, TunnelState::Off).await;
    assert!(!sh("ip rule").contains("7301"), "{}", sh("ip rule"));
    let ruleset = sh("nft list ruleset");
    assert!(!ruleset.contains("acme"), "{ruleset}");
    let saved = std::fs::read_to_string(&state_path).unwrap();
    assert!(
        saved.contains("10.40.0.0/16") && saved.contains("10.33.1.3"),
        "{saved}"
    );

    // On again: restored from disk.
    mgr.connect("acme").unwrap();
    wait_for(&mgr, TunnelState::Connected).await;
    assert!(goes_through_tunnel("10.40.7.7"));
    assert!(goes_through_tunnel("10.33.1.3"));
    assert!(!goes_through_tunnel("10.33.1.1"));
    // The network unpinned: the host pinned inside it stays.
    mgr.unpin("acme", "10.40.0.0/16").await.unwrap();
    assert!(!goes_through_tunnel("10.40.7.7"));
    assert!(goes_through_tunnel("10.40.5.5"));
    mgr.shutdown().await;
    // Off is gone: nothing of Crake's is left in the network setup.
    assert!(
        !sh("nft list ruleset").contains("crake"),
        "{}",
        sh("nft list ruleset")
    );
    let rules = sh("ip rule show");
    assert!(
        !rules.contains("lookup 7"),
        "Crake's policy rules stay: {rules}"
    );
    for table in 7301..=7364 {
        let left = sh(&format!("ip route show table {table} 2>/dev/null"));
        assert!(left.trim().is_empty(), "table {table} keeps {left}");
    }
    let links = sh("ip -br link");
    assert!(
        !links.contains("crake-hush") && !links.contains("crake-keep"),
        "{links}"
    );

    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    for want in [
        "Pinned 10.40.0.0/16 to ACME.",
        "Sent 10.33.1.1 through ACME.",
        "Stopped sending 10.33.1.1 through ACME.",
        "Unpinned 10.40.0.0/16 from ACME.",
        "ACME is up. Sending 3 known destinations through it.",
    ] {
        assert!(log.contains(want), "missing {want:?} in\n{log}");
    }
}

/// A pinned or learned IPv6 host is routed through the tunnel, the same way
/// as IPv4: its own nftables sets, its own `ip -6` rule and route.
#[tokio::test]
async fn routes_an_ipv6_host() {
    if !enabled() {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    strict_rp_filter();
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("openconnect");
    std::fs::write(&fake, FAKE6).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    unsafe { std::env::set_var("CRAKE_TEST_HOOK", env!("CARGO_BIN_EXE_crake-hook")) };
    let cfg = Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.path().join("hook.sock"),
        openconnect: fake,
        log_path: dir.path().join("log"),
        state_path: dir.path().join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        never: vec![],
        use_resolved: false,
        apply_network: true,
        managed_dir: dir.path().join("managed.d"),
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
    };
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    // A dual-stack "LAN": a v4 and a v6 default, so an unrouted destination
    // has somewhere to go first (and is then re-routed to the tunnel by its
    // mark, as on a real machine).
    sh("ip link add uplink type dummy; ip link set uplink up; \
        ip addr add 192.168.0.242/24 dev uplink; ip route add default via 192.168.0.1 dev uplink; \
        ip -6 addr add fd00:1a::242/64 dev uplink nodad; ip -6 route add default via fd00:1a::1 dev uplink");
    mgr.init().await.unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    // Pin a v6 network, connect, and it routes.
    mgr.pin("acme", "fd40::/32").await.unwrap();
    mgr.connect("acme").unwrap();
    wait_for(&mgr, TunnelState::Connected).await;
    assert!(
        sh("ip -6 rule").contains("lookup 7301"),
        "{}",
        sh("ip -6 rule")
    );
    assert!(
        sh("nft list set inet crake pinned6_acme").contains("fd40::/32"),
        "{}",
        sh("nft list set inet crake pinned6_acme")
    );
    assert!(goes_through_tunnel6("fd40::1:1"), "pinned v6 network");
    assert!(!goes_through_tunnel6("fd99::1"), "nothing else");

    // A single learned v6 host, then forgotten.
    mgr.add_host("acme", "fd50::9").await.unwrap();
    assert!(
        sh("nft list set inet crake learned6_acme").contains("fd50::9"),
        "{}",
        sh("nft list set inet crake learned6_acme")
    );
    assert!(goes_through_tunnel6("fd50::9"), "added v6 host");
    mgr.forget("acme", "fd50::9").await.unwrap();
    assert!(!goes_through_tunnel6("fd50::9"), "forgotten v6 host");

    // Off: the v6 rule and sets leave with everything else.
    mgr.shutdown().await;
    assert!(!sh("ip -6 rule").contains("7301"), "{}", sh("ip -6 rule"));
    assert!(
        !sh("nft list ruleset").contains("pinned6_acme"),
        "{}",
        sh("nft list ruleset")
    );
}

/// IT requires a full tunnel: everything goes through, except the tunnel's
/// own gateway and the local network.
#[tokio::test]
async fn a_full_tunnel_takes_everything_but_the_gateway_and_the_lan() {
    if !enabled() {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    // Strict reverse-path filtering, as Fedora has it.
    strict_rp_filter();
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("openconnect");
    std::fs::write(&fake, FAKE).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    unsafe { std::env::set_var("CRAKE_TEST_HOOK", env!("CARGO_BIN_EXE_crake-hook")) };
    let managed_dir = dir.path().join("managed.d");
    std::fs::create_dir_all(&managed_dir).unwrap();
    std::fs::write(
        managed_dir.join("acme.toml"),
        "display_name = \"ACME\"\ngateway = \"vpn.acme.example\"\nfull_tunnel = true\n",
    )
    .unwrap();
    let mut cfg = Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.path().join("hook.sock"),
        openconnect: fake,
        log_path: dir.path().join("log"),
        state_path: dir.path().join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        never: vec![],
        use_resolved: false,
        apply_network: true,
        managed_dir,
        tunnels: vec![],
    };
    cfg.add_managed();
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    sh("ip link add uplink type dummy; ip link set uplink up; \
        ip addr add 192.168.0.242/24 dev uplink; ip route add default via 192.168.0.1 dev uplink; \
        ip -6 addr add fd00:1a::242/64 dev uplink nodad; ip -6 route add default via fd00:1a::1 dev uplink");
    mgr.init().await.unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();

    mgr.connect("acme").unwrap();
    wait_for(&mgr, TunnelState::Connected).await;
    assert!(
        goes_through_tunnel("10.99.1.1"),
        "{}",
        sh("nft list ruleset")
    );
    assert!(goes_through_tunnel("203.0.113.40"), "public addresses too");
    assert!(!goes_through_tunnel("192.0.2.10"), "never the gateway");
    assert!(!goes_through_tunnel("192.168.0.50"), "never the LAN");
    let err = mgr.add_host("acme", "10.99.1.1").await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Everything already goes through ACME (full tunnel)."
    );
    // No IPv6 address on the tunnel: IPv6 is refused at once, never sent
    // around it; the LAN's IPv6 isn't touched.
    assert_eq!(
        tcp_to("[2001:db8::40]:443"),
        std::io::ErrorKind::ConnectionRefused,
        "{}",
        sh("nft list ruleset")
    );
    assert_eq!(tcp_to("[fd00:1a::50]:443"), std::io::ErrorKind::TimedOut);

    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, TunnelState::Off).await;
    assert_eq!(
        tcp_to("[2001:db8::40]:443"),
        std::io::ErrorKind::TimedOut,
        "off: IPv6 goes its own way again"
    );

    // With an IPv6 address from the gateway: craked gives it to the
    // device, and IPv6 goes through too (but the LAN's).
    std::fs::write(dir.path().join("openconnect"), FAKE_IP6).unwrap();
    mgr.connect("acme").unwrap();
    wait_for(&mgr, TunnelState::Connected).await;
    assert!(
        sh("ip -6 addr show dev crake-acme").contains("fd00:ac::7/64"),
        "{}",
        sh("ip -6 addr show dev crake-acme")
    );
    assert!(
        goes_through_tunnel6("2001:db8::40"),
        "{}",
        sh("nft list ruleset")
    );
    assert!(!goes_through_tunnel6("fd00:1a::50"), "never the LAN");
    assert!(goes_through_tunnel("203.0.113.40"));

    mgr.disconnect("acme").unwrap();
    wait_for(&mgr, TunnelState::Off).await;
    assert!(
        !sh("nft list ruleset").contains("acme"),
        "off: nothing goes through"
    );
    mgr.shutdown().await;
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("ACME is up. Everything goes through it (full tunnel)."),
        "{log}"
    );
}

/// "Keep internal names private": with the tunnel off, its domains get a
/// link of their own whose DNS answers REFUSED at once, so the names never
/// reach the internet provider's DNS.
#[tokio::test]
async fn internal_names_stay_private_while_the_tunnel_is_off() {
    if !enabled() {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    // Strict reverse-path filtering, as Fedora has it.
    strict_rp_filter();
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.path().join("hook.sock"),
        openconnect: dir.path().join("openconnect"),
        log_path: dir.path().join("log"),
        state_path: dir.path().join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        never: vec![],
        // No systemd-resolved in the namespace: its calls just fail; the
        // link and the answers are what's tested.
        use_resolved: true,
        apply_network: true,
        managed_dir: dir.path().join("managed.d"),
        tunnels: vec![TunnelConfig {
            name: "acme".into(),
            display_name: Some("ACME".into()),
            gateway: "vpn.acme.example".into(),
            protocol: Protocol::Anyconnect,
            user: None,
            passwd_file: None,
            dns_domains: vec!["acme.internal".into()],
            auth_args: vec![],
            extra_args: vec![],
            managed: None,
            own: None,
        }],
    };
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    assert!(
        sh("ip -br addr show crake-hush").contains("169.254.53.53"),
        "{}",
        sh("ip -br addr")
    );

    // Asked as resolved asks a link's DNS: bound to that link.
    let ask = || async {
        let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap();
        sock.bind_device(Some(b"crake-hush")).unwrap();
        sock.connect("169.254.53.53:53").await.unwrap();
        let q = craked::dns::query(7, "git.acme.internal", hickory_proto::rr::RecordType::A);
        sock.send(&q).await.unwrap();
        let mut buf = [0u8; 512];
        let start = std::time::Instant::now();
        tokio::time::timeout(Duration::from_secs(1), sock.recv(&mut buf))
            .await
            .expect("an answer within a second")
            .unwrap();
        (buf[3] & 0x0f, start.elapsed())
    };
    let (rcode, took) = ask().await;
    assert_eq!(rcode, 5, "REFUSED");
    assert!(took < Duration::from_millis(100), "{took:?}");

    mgr.set_setting("keep_private", "false").await.unwrap();
    assert!(!sh("ip link").contains("crake-hush"), "off: the link goes");
    mgr.set_setting("keep_private", "true").await.unwrap();
    assert!(sh("ip link").contains("crake-hush"));
    mgr.shutdown().await;
    assert!(!sh("ip link").contains("crake-hush"), "nothing left behind");
}

/// OpenVPN 3: the session ended without the app saying so (the app quit
/// and openvpn3 took the session down with it). Its device is gone, so the
/// tunnel is down: not "Connected, its DNS doesn't answer" for good.
#[tokio::test]
async fn an_openvpn3_tunnel_whose_device_is_gone_is_down() {
    use std::collections::HashMap;
    if !enabled() {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.path().join("hook.sock"),
        openconnect: dir.path().join("openconnect"),
        log_path: dir.path().join("log"),
        state_path: dir.path().join("hosts.json"),
        learned_expiry_days: 7,
        probing: false,
        race_first_syn: false,
        probe_wait_secs: 3,
        never: vec![],
        use_resolved: false,
        apply_network: true,
        managed_dir: dir.path().join("managed.d"),
        tunnels: vec![],
    };
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    // What openvpn3 makes: a tun device with the session's address.
    sh("ip tuntap add dev tun7 mode tun && ip link set tun7 up \
        && ip addr add 10.8.0.6/24 dev tun7");
    let name = mgr
        .add_tunnel(&HashMap::from([
            ("display_name".to_owned(), "Initech".to_owned()),
            ("gateway".to_owned(), "initech".to_owned()),
            ("protocol".to_owned(), "openvpn3".to_owned()),
        ]))
        .unwrap();
    mgr.attach(
        &name,
        &HashMap::from([
            ("device".to_owned(), "tun7".to_owned()),
            ("dns".to_owned(), "10.8.0.1".to_owned()),
            ("domains".to_owned(), "initech.internal".to_owned()),
        ]),
    )
    .await
    .unwrap();
    mgr.check_devices().await;
    assert_eq!(
        mgr.get(&name).unwrap().state,
        TunnelState::Connected,
        "still there"
    );

    sh("ip link del tun7");
    mgr.check_devices().await;
    let snap = mgr.get(&name).unwrap();
    assert_eq!(snap.state, TunnelState::Trouble);
    assert!(snap.info.is_none());
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("Initech stopped unexpectedly.") && log.contains("tun7 is gone"),
        "{log}"
    );
    mgr.shutdown().await;
}

/// rp_filter=1 everywhere, as Fedora has it (scripts/netns-test.sh starts
/// the namespace so): a reply arriving on an interface the main table
/// wouldn't use to reach its sender is dropped, unless craked loosens it
/// for the tunnel's interface.
fn strict_rp_filter() {
    let all = std::fs::read_to_string("/proc/sys/net/ipv4/conf/all/rp_filter").unwrap();
    assert_eq!(all.trim(), "1", "the namespace filters strictly");
}
