//! Connection probing against the real kernel, for a throwaway network
//! namespace only. Skipped unless CRAKE_NETNS_TEST=1; run with
//! `scripts/netns-test.sh`.
//!
//! Two TUN devices: `uplink` (the test plays the internet) and `crake-acme`
//! (the test plays everything behind the tunnel). A responder thread drops,
//! refuses or answers TCP handshakes on each, per destination.

#![cfg(target_os = "linux")]

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crake_proto::TunnelState;
use craked::config::{Config, Protocol, TunnelConfig};
use craked::hook_server;
use craked::manager::Manager;

const FAKE: &str = r#"#!/bin/sh
export TUNDEV=crake-acme VPNGATEWAY=192.0.2.10 INTERNAL_IP4_ADDRESS=10.40.255.7 \
  INTERNAL_IP4_NETMASK=255.255.255.255 INTERNAL_IP4_DNS=10.40.0.2 \
  INTERNAL_IP6_ADDRESS=fd40:ff::7 INTERNAL_IP6_NETMASK=fd40:ff::7/128 \
  CISCO_SPLIT_INC=1 CISCO_SPLIT_INC_0_ADDR=10.70.0.0 CISCO_SPLIT_INC_0_MASKLEN=16 \
  CISCO_IPV6_SPLIT_INC=1 CISCO_IPV6_SPLIT_INC_0_ADDR=fd70:: CISCO_IPV6_SPLIT_INC_0_MASKLEN=48
reason=connect "$CRAKE_TEST_HOOK" || exit 1
trap 'exit 0' TERM
while :; do sleep 0.1; done
"#;

fn sh(cmd: &str) -> String {
    let out = Command::new("sh").args(["-c", cmd]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

fn open_tun(name: &str) -> File {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/net/tun")
        .unwrap();
    #[repr(C)]
    struct IfReq {
        name: [u8; 16],
        flags: libc::c_short,
        _pad: [u8; 22],
    }
    let mut req = IfReq {
        name: [0; 16],
        flags: (libc::IFF_TUN | libc::IFF_NO_PI) as libc::c_short,
        _pad: [0; 22],
    };
    req.name[..name.len()].copy_from_slice(name.as_bytes());
    const TUNSETIFF: libc::c_ulong = 0x4004_54ca;
    // SAFETY: TUNSETIFF with a properly sized ifreq.
    let r = unsafe { libc::ioctl(f.as_raw_fd(), TUNSETIFF, &mut req) };
    assert!(r >= 0, "TUNSETIFF: {}", std::io::Error::last_os_error());
    unsafe { libc::fcntl(f.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
    f
}

fn sum16(data: &[u8]) -> u32 {
    data.chunks(2)
        .map(|c| u32::from(u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)])))
        .sum()
}

fn fold(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// The tunnel's own IPv6 address, as the fake openconnect hands it over.
/// The pushed split networks lie inside the private ranges a learn-anything
/// tunnel races (10.0.0.0/8, fc00::/7): the race's sets must merge them.
const TUNNEL_V6: &str = "fd40:ff::7";

/// A TCP segment answering `syn` (IPv4 or IPv6) with the given flags
/// (SYN-ACK or RST-ACK).
fn tcp_reply(syn: &[u8], flags: u8) -> Vec<u8> {
    let v6 = syn[0] >> 4 == 6;
    let ihl = if v6 {
        40
    } else {
        usize::from(syn[0] & 0x0f) * 4
    };
    let t = &syn[ihl..];
    let seq = u32::from_be_bytes([t[4], t[5], t[6], t[7]]);
    let mut tcp = vec![0u8; 20];
    tcp[0..2].copy_from_slice(&t[2..4]);
    tcp[2..4].copy_from_slice(&t[0..2]);
    tcp[4..8].copy_from_slice(&(if flags & 0x04 != 0 { 0 } else { 777_000u32 }).to_be_bytes());
    tcp[8..12].copy_from_slice(&seq.wrapping_add(1).to_be_bytes());
    tcp[12] = 5 << 4;
    tcp[13] = flags;
    tcp[14..16].copy_from_slice(&65535u16.to_be_bytes());
    if v6 {
        let (src, dst) = (&syn[24..40], &syn[8..24]);
        let mut pseudo = Vec::new();
        pseudo.extend_from_slice(src);
        pseudo.extend_from_slice(dst);
        pseudo.extend_from_slice(&[0, 0, 0, 20, 0, 0, 0, 6]);
        let c = fold(sum16(&pseudo) + sum16(&tcp));
        tcp[16..18].copy_from_slice(&c.to_be_bytes());
        let mut ip = vec![0x60, 0, 0, 0, 0, 20, 6, 64];
        ip.extend_from_slice(src);
        ip.extend_from_slice(dst);
        ip.extend_from_slice(&tcp);
        return ip;
    }
    let (src, dst) = (&syn[16..20], &syn[12..16]);
    let mut pseudo = Vec::new();
    pseudo.extend_from_slice(src);
    pseudo.extend_from_slice(dst);
    pseudo.extend_from_slice(&[0, 6, 0, 20]);
    let c = fold(sum16(&pseudo) + sum16(&tcp));
    tcp[16..18].copy_from_slice(&c.to_be_bytes());
    let mut ip = vec![0x45, 0, 0, 40, 0, 0, 0, 0, 64, 6, 0, 0];
    ip.extend_from_slice(src);
    ip.extend_from_slice(dst);
    let c = fold(sum16(&ip));
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    ip.extend_from_slice(&tcp);
    ip
}

/// The router's (192.168.0.1) ICMP "host unreachable" about `syn`.
fn host_unreachable(syn: &[u8]) -> Vec<u8> {
    let ihl = usize::from(syn[0] & 0x0f) * 4;
    let mut icmp = vec![3, 1, 0, 0, 0, 0, 0, 0];
    icmp.extend_from_slice(&syn[..ihl + 8]);
    let c = fold(sum16(&icmp));
    icmp[2..4].copy_from_slice(&c.to_be_bytes());
    let mut ip = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 1, 0, 0, 192, 168, 0, 1];
    ip.extend_from_slice(&syn[12..16]);
    ip[2..4].copy_from_slice(&((20 + icmp.len()) as u16).to_be_bytes());
    let c = fold(sum16(&ip));
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    ip.extend_from_slice(&icmp);
    ip
}

/// The router's (fd00:1a::1) ICMPv6 "address unreachable" about an IPv6
/// `syn`.
fn host_unreachable6(syn: &[u8]) -> Vec<u8> {
    let router: Ipv6Addr = "fd00:1a::1".parse().unwrap();
    let mut icmp = vec![1, 3, 0, 0, 0, 0, 0, 0];
    icmp.extend_from_slice(syn);
    let mut pseudo = Vec::new();
    pseudo.extend_from_slice(&router.octets());
    pseudo.extend_from_slice(&syn[8..24]);
    pseudo.extend_from_slice(&(icmp.len() as u32).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0, 58]);
    let c = fold(sum16(&pseudo) + sum16(&icmp));
    icmp[2..4].copy_from_slice(&c.to_be_bytes());
    let mut ip = vec![0x60, 0, 0, 0];
    ip.extend_from_slice(&(icmp.len() as u16).to_be_bytes());
    ip.extend_from_slice(&[58, 64]);
    ip.extend_from_slice(&router.octets());
    ip.extend_from_slice(&syn[8..24]);
    ip.extend_from_slice(&icmp);
    ip
}

/// A SYN (no ACK) in this packet, IPv4 or IPv6: (source, destination,
/// port).
fn syn_of(p: &[u8]) -> Option<(IpAddr, IpAddr, u16)> {
    let (src, dst, t): (IpAddr, IpAddr, &[u8]) = match p.first()? >> 4 {
        4 if p.len() >= 40 && p[9] == 6 => {
            let ihl = usize::from(p[0] & 0x0f) * 4;
            let src: [u8; 4] = p[12..16].try_into().ok()?;
            let dst: [u8; 4] = p[16..20].try_into().ok()?;
            (src.into(), dst.into(), p.get(ihl..)?)
        }
        6 if p.len() >= 60 && p[6] == 6 => {
            let src: [u8; 16] = p[8..24].try_into().ok()?;
            let dst: [u8; 16] = p[24..40].try_into().ok()?;
            (src.into(), dst.into(), &p[40..])
        }
        _ => return None,
    };
    (t.len() >= 20 && t[13] & 0x12 == 0x02).then(|| (src, dst, u16::from_be_bytes([t[2], t[3]])))
}

#[derive(Default)]
struct World {
    /// Refused on the internet (RST); everything else there is dropped.
    refuse_direct: HashSet<IpAddr>,
    /// The router says it can't reach these (ICMP host unreachable).
    unreachable_direct: HashSet<IpAddr>,
    /// Answered behind the tunnel; everything else there is dropped.
    behind_tunnel: HashSet<IpAddr>,
    /// Every SYN seen leaving through the tunnel.
    tunnel_syns: Vec<(IpAddr, u16)>,
    /// Hosts that echo UDP datagrams behind the tunnel (dropped directly).
    udp_behind: HashSet<IpAddr>,
    /// Answered on the internet, except the first connection to each
    /// (its SYNs lost, as on a flaky Wi-Fi): by its source port.
    flaky_direct: HashSet<IpAddr>,
    first_port: std::collections::HashMap<IpAddr, u16>,
}

/// An IPv4 UDP datagram in this packet: (source, destination).
fn udp_of(p: &[u8]) -> Option<(IpAddr, IpAddr)> {
    if p.len() < 28 || p[0] >> 4 != 4 || p[9] != 17 {
        return None;
    }
    let src: [u8; 4] = p[12..16].try_into().ok()?;
    let dst: [u8; 4] = p[16..20].try_into().ok()?;
    Some((src.into(), dst.into()))
}

/// The same datagram sent back: addresses and ports swapped (no UDP
/// checksum, which IPv4 allows).
fn udp_echo(p: &[u8]) -> Vec<u8> {
    let ihl = usize::from(p[0] & 0x0f) * 4;
    let mut out = p.to_vec();
    out[12..16].copy_from_slice(&p[16..20]);
    out[16..20].copy_from_slice(&p[12..16]);
    out[10..12].copy_from_slice(&[0, 0]);
    let c = fold(sum16(&out[..ihl]));
    out[10..12].copy_from_slice(&c.to_be_bytes());
    out[ihl..ihl + 2].copy_from_slice(&p[ihl + 2..ihl + 4]);
    out[ihl + 2..ihl + 4].copy_from_slice(&p[ihl..ihl + 2]);
    out[ihl + 6..ihl + 8].copy_from_slice(&[0, 0]);
    out
}

fn responder(mut uplink: File, mut tun: File, world: Arc<Mutex<World>>, stop: Arc<AtomicBool>) {
    let mut buf = [0u8; 2048];
    while !stop.load(Ordering::Relaxed) {
        let mut idle = true;
        if let Ok(n) = uplink.read(&mut buf) {
            idle = false;
            if let Some((_, dst, _)) = syn_of(&buf[..n]) {
                let mut w = world.lock().unwrap();
                if w.flaky_direct.contains(&dst) {
                    let ihl = usize::from(buf[0] & 0x0f) * 4;
                    let sport = u16::from_be_bytes([buf[ihl], buf[ihl + 1]]);
                    if *w.first_port.entry(dst).or_insert(sport) != sport {
                        uplink.write_all(&tcp_reply(&buf[..n], 0x12)).unwrap();
                    }
                } else if w.refuse_direct.contains(&dst) {
                    uplink.write_all(&tcp_reply(&buf[..n], 0x14)).unwrap();
                } else if w.unreachable_direct.contains(&dst) {
                    let notice = if dst.is_ipv6() {
                        host_unreachable6(&buf[..n])
                    } else {
                        host_unreachable(&buf[..n])
                    };
                    uplink.write_all(&notice).unwrap();
                }
            }
        }
        if let Ok(n) = tun.read(&mut buf) {
            idle = false;
            if let Some((src, dst, port)) = syn_of(&buf[..n]) {
                let mut w = world.lock().unwrap();
                w.tunnel_syns.push((dst, port));
                // Like a real VPN gateway: only packets from the tunnel's own
                // address get anywhere; anything else (e.g. still carrying the
                // Wi-Fi address) is dropped.
                let from_tunnel = src == IpAddr::V4(Ipv4Addr::new(10, 40, 255, 7))
                    || src == TUNNEL_V6.parse::<IpAddr>().unwrap();
                if from_tunnel && w.behind_tunnel.contains(&dst) {
                    tun.write_all(&tcp_reply(&buf[..n], 0x12)).unwrap();
                }
            } else if let Some((src, dst)) = udp_of(&buf[..n]) {
                let w = world.lock().unwrap();
                let from_tunnel = src == IpAddr::V4(Ipv4Addr::new(10, 40, 255, 7));
                if from_tunnel && w.udp_behind.contains(&dst) {
                    tun.write_all(&udp_echo(&buf[..n])).unwrap();
                }
            }
        }
        if idle {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn connect(ip: &str, port: u16, within: Duration) -> (std::io::Result<TcpStream>, Duration) {
    let addr: SocketAddr = format!("{ip}:{port}").parse().unwrap();
    let start = Instant::now();
    (TcpStream::connect_timeout(&addr, within), start.elapsed())
}

/// A started craked in the namespace: ACME up (fake openconnect), a
/// responder playing the hosts behind the tunnel and the internet. `race`
/// turns the first-SYN race on. The returned handles are kept alive by the
/// caller for the test's duration.
struct Probe {
    mgr: std::sync::Arc<Manager>,
    world: Arc<Mutex<World>>,
    stop: Arc<AtomicBool>,
    responder: std::thread::JoinHandle<()>,
    _dir: tempfile::TempDir,
    _srv: tokio::task::JoinHandle<()>,
}

async fn start_probe(race: bool) -> Probe {
    // Strict reverse-path filtering, as Fedora has it.
    strict_rp_filter();
    let uplink = open_tun("uplink");
    // Dual-stack, so an IPv6 connection has somewhere to go directly.
    sh(
        "ip link set uplink up; ip addr add 192.168.0.242/24 dev uplink; \
        ip route add default via 192.168.0.1 dev uplink; \
        ip -6 addr add fd00:1a::242/64 dev uplink nodad; ip -6 route add default dev uplink",
    );
    let tun = open_tun("crake-acme");
    sh("ip link set crake-acme up");

    let world = Arc::new(Mutex::new(World::default()));
    {
        let mut w = world.lock().unwrap();
        w.behind_tunnel.extend(
            ["10.70.0.5", "10.70.0.6", "10.70.0.8", "fd70::5", "fd70::8"]
                .map(|a| a.parse::<IpAddr>().unwrap()),
        );
        w.refuse_direct.insert("10.70.0.6".parse().unwrap());
        w.unreachable_direct.insert("10.70.0.8".parse().unwrap());
        w.unreachable_direct.insert("fd70::8".parse().unwrap());
        w.udp_behind.insert("10.70.0.12".parse().unwrap());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let responder = {
        let (w, s) = (Arc::clone(&world), Arc::clone(&stop));
        std::thread::spawn(move || responder(uplink, tun, w, s))
    };

    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("openconnect");
    std::fs::write(&fake, FAKE).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    unsafe { std::env::set_var("CRAKE_TEST_HOOK", env!("CARGO_BIN_EXE_crake-hook")) };
    let cfg = Config {
        hook_path: env!("CARGO_BIN_EXE_crake-hook").into(),
        hook_socket: dir.path().join("hook.sock"),
        openconnect: fake,
        openvpn: "openvpn".into(),
        log_path: dir.path().join("log"),
        state_path: dir.path().join("hosts.json"),
        learned_expiry_days: 7,
        probing: true,
        race_first_syn: race,
        probe_wait_secs: 1,
        never: vec!["10.70.9.0/24".parse().unwrap()],
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
            dns_domains: vec!["acme.internal".into()],
            auth_args: vec![],
            extra_args: vec![],
            managed: None,
            own: None,
        }],
    };
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    let _srv = hook_server::serve(mgr.clone(), &socket).unwrap();
    mgr.connect("acme").unwrap();
    for _ in 0..100 {
        if mgr.get("acme").unwrap().state == TunnelState::Connected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Probe {
        mgr,
        world,
        stop,
        responder,
        _dir: dir,
        _srv,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn connections_that_fail_directly_are_tried_through_the_tunnel() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let Probe {
        mgr,
        world,
        stop,
        responder,
        _dir: dir,
        _srv,
    } = start_probe(false).await;
    let (silent, silent6, refused, unreachable, down, lan, never) =
        tokio::task::spawn_blocking(|| {
            // 1. No answer directly: the app's own attempt ends up connected.
            let silent = connect("10.70.0.5", 5432, Duration::from_secs(12));
            // 1b. The same over IPv6.
            let silent6 = connect("[fd70::5]", 5432, Duration::from_secs(12));
            // 2. Refused directly: that attempt fails, the next goes through.
            let refused_first = connect("10.70.0.6", 443, Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1500));
            let refused_again = connect("10.70.0.6", 443, Duration::from_secs(5));
            // 2b. The router says unreachable: tried through the tunnel at once,
            //     sooner than the wait (1 s), so the next attempt goes through.
            let unreachable_first = connect("10.70.0.8", 443, Duration::from_secs(2));
            std::thread::sleep(Duration::from_millis(400));
            let unreachable_again = connect("10.70.0.8", 443, Duration::from_millis(500));
            // 2c. The same over ICMPv6 ("address unreachable").
            let unreachable6_first = connect("[fd70::8]", 443, Duration::from_secs(2));
            std::thread::sleep(Duration::from_millis(400));
            let unreachable6 = (
                unreachable6_first,
                connect("[fd70::8]", 443, Duration::from_millis(500)),
            );
            // 3. Down everywhere. 4. The local network. 5. The never list.
            let down = connect("10.70.0.7", 22, Duration::from_secs(1));
            let lan = connect("192.168.0.50", 80, Duration::from_secs(1));
            let never = connect("10.70.9.9", 80, Duration::from_secs(1));
            std::thread::sleep(Duration::from_secs(5)); // probe wait + probe timeout
            (
                silent,
                silent6,
                (refused_first, refused_again),
                (unreachable_first, unreachable_again, unreachable6),
                down,
                lan,
                never,
            )
        })
        .await
        .unwrap();

    let (ok, took) = silent;
    assert!(
        ok.is_ok(),
        "the stuck connection succeeded through the tunnel: {ok:?}"
    );
    assert!(
        took < Duration::from_secs(8),
        "it recovered on a SYN retry ({took:?})"
    );
    assert!(
        silent6.0.is_ok(),
        "the stuck IPv6 connection succeeded through the tunnel: {:?}\nsyns: {:?}\nlog:\n{}",
        silent6.0,
        world.lock().unwrap().tunnel_syns,
        std::fs::read_to_string(dir.path().join("log")).unwrap_or_default()
    );
    assert!(
        sh("nft list set inet crake learned6_acme").contains("fd70::5"),
        "{}",
        sh("nft list set inet crake learned6_acme")
    );
    let (first, again) = refused;
    assert_eq!(
        first.0.unwrap_err().kind(),
        std::io::ErrorKind::ConnectionRefused
    );
    assert!(
        again.0.is_ok(),
        "after the refusal, the next attempt goes through: {:?}\nsyns: {:?}\nlog:\n{}",
        again.0,
        world.lock().unwrap().tunnel_syns,
        std::fs::read_to_string(dir.path().join("log")).unwrap_or_default()
    );
    let (first, again, v6) = unreachable;
    assert!(v6.0.0.is_err(), "the router said unreachable: {:?}", v6.0.0);
    assert!(
        v6.1.0.is_ok(),
        "ICMPv6: probed at once, before the wait: {:?}",
        v6.1.0
    );
    assert!(
        first.0.is_err(),
        "the router said unreachable: {:?}",
        first.0
    );
    assert!(
        again.0.is_ok(),
        "probed at once, before the wait: {:?}",
        again.0
    );
    assert!(down.0.is_err() && lan.0.is_err() && never.0.is_err());

    let syns = world.lock().unwrap().tunnel_syns.clone();
    let tried = |ip: &str| {
        syns.iter()
            .any(|(d, _)| *d == ip.parse::<IpAddr>().unwrap())
    };
    assert!(tried("10.70.0.7"), "down host was tried through the tunnel");
    assert!(!tried("192.168.0.50"), "the local network is never probed");
    assert!(!tried("10.70.9.9"), "the never list is never probed");

    let set = sh("nft list set inet crake learned_acme");
    assert!(
        set.contains("10.70.0.5") && set.contains("10.70.0.6"),
        "{set}"
    );
    assert!(!set.contains("10.70.0.7"), "{set}");

    // Undo: direct again, and stays direct.
    mgr.undo("acme", &["10.70.0.5"]).await.unwrap();
    assert!(!sh("nft list set inet crake learned_acme").contains("10.70.0.5"));
    let rows = mgr.hosts("acme").await.unwrap();
    assert!(
        rows.iter()
            .any(|r| r.direct && r.target.to_string() == "10.70.0.5")
    );
    mgr.redo("acme", &["10.70.0.5"]).await.unwrap();
    assert!(sh("nft list set inet crake learned_acme").contains("10.70.0.5"));

    forget_is_not_forbid(&mgr).await;

    mgr.shutdown().await;
    stop.store(true, Ordering::Relaxed);
    responder.join().unwrap();

    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    for want in [
        "Sent 10.70.0.5 through ACME.",
        "It didn't answer directly. Through ACME it answered in",
        "Sent fd70::5 through ACME.",
        "Sent 10.70.0.6 through ACME.",
        "Refused directly. Through ACME it answered in",
        "Sent 10.70.0.8 through ACME.",
        "Unreachable directly. Through ACME it answered in",
        "Left 10.70.0.7 alone.",
        "No answer just then, directly or through ACME. Nothing was changed.",
        "10.70.0.5 goes direct again.",
        "Sent 10.70.0.5 through ACME again.",
    ] {
        assert!(log.contains(want), "missing {want:?} in\n{log}");
    }
    assert!(
        !log.contains("192.168.0.50") && !log.contains("10.70.9.9"),
        "{log}"
    );
    assert!(
        log.lines()
            .any(|l| l.contains("fd70::8") && l.contains("Unreachable directly.")),
        "the ICMPv6 notice started the probe, not the wait:\n{log}"
    );
}

/// Forget: gone, and the next visit is a first one again (probed and
/// routed afresh, although it was probed moments ago), not blocked. A
/// connection open to it ends (a browser would wait on it).
async fn forget_is_not_forbid(mgr: &Manager) {
    let held = tokio::task::spawn_blocking(|| connect("10.70.0.6", 443, Duration::from_secs(5)).0)
        .await
        .unwrap()
        .expect("10.70.0.6 answers through ACME");
    mgr.forget("acme", "10.70.0.6").await.unwrap();
    assert!(!sh("nft list set inet crake learned_acme").contains("10.70.0.6"));
    let ended = tokio::task::spawn_blocking(move || {
        let mut held = held;
        held.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = [0u8; 1];
        held.read(&mut buf)
    })
    .await
    .unwrap();
    assert!(
        !matches!(&ended, Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "the open connection was closed: {ended:?}"
    );
    let again = tokio::task::spawn_blocking(|| {
        let first = connect("10.70.0.6", 443, Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1500));
        (first, connect("10.70.0.6", 443, Duration::from_secs(5)))
    })
    .await
    .unwrap();
    assert!(
        again.1.0.is_ok(),
        "forgotten, then routed again on the next visit: {:?}",
        again.1.0
    );
    assert!(sh("nft list set inet crake learned_acme").contains("10.70.0.6"));

    // An address ACME's DNS gave for a name, reached again from an app's
    // own DNS cache after Forget (no lookup to route it before the first
    // packet): tried through ACME after a round trip, not the probe wait,
    // so the app's first SYN retry gets through.
    mgr.learn_from_dns(
        "acme",
        craked::dns::Answer {
            name: "db.acme.internal".into(),
            addrs: vec!["10.70.0.5".parse().unwrap()],
        },
    )
    .await;
    mgr.forget("acme", "10.70.0.5").await.unwrap();
    let (back, took) =
        tokio::task::spawn_blocking(|| connect("10.70.0.5", 5432, Duration::from_secs(5)))
            .await
            .unwrap();
    assert!(back.is_ok(), "routed again: {back:?}");
    assert!(
        took < Duration::from_millis(1600),
        "on the first SYN retry, not after the probe wait ({took:?})"
    );
}

/// rp_filter=1 everywhere, as Fedora has it (scripts/netns-test.sh starts
/// the namespace so): a reply arriving on an interface the main table
/// wouldn't use to reach its sender is dropped, unless craked loosens it
/// for the tunnel's interface.
fn strict_rp_filter() {
    let all = std::fs::read_to_string("/proc/sys/net/ipv4/conf/all/rp_filter").unwrap();
    assert_eq!(all.trim(), "1", "the namespace filters strictly");
}

/// With the first-SYN race on, the very first connection to a host that
/// only a tunnel reaches succeeds on that first attempt, with no probe
/// wait and no retry (CLAUDE.md milestone 4). Without it (the passive test
/// above), the first attempt fails and only a later one gets through.
#[tokio::test(flavor = "multi_thread")]
async fn the_first_connection_is_routed_when_the_syn_is_held() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let Probe {
        mgr,
        world,
        stop,
        responder,
        _dir: dir,
        _srv,
    } = start_probe(true).await;

    // 10.70.0.5 answers only through the tunnel, and was never probed. One
    // connection, held and raced: it succeeds, fast.
    let (first, first6, down) = tokio::task::spawn_blocking(|| {
        let first = connect("10.70.0.5", 5432, Duration::from_secs(4));
        // fd70::5 likewise, over IPv6 (a unique local address: raced for a
        // learn-anything tunnel).
        let first6 = connect("[fd70::5]", 5432, Duration::from_secs(4));
        // 10.70.0.7 answers nowhere: held, raced, let through, fails.
        let down = connect("10.70.0.7", 22, Duration::from_secs(4));
        (first, first6, down)
    })
    .await
    .unwrap();

    assert!(
        first.0.is_ok(),
        "the first connection was routed through the tunnel: {:?}\nsyns: {:?}\nlog:\n{}",
        first.0,
        world.lock().unwrap().tunnel_syns,
        std::fs::read_to_string(dir.path().join("log")).unwrap_or_default()
    );
    assert!(
        first.1 < Duration::from_secs(2),
        "on the first attempt, no probe wait or retry ({:?})",
        first.1
    );
    assert!(down.0.is_err(), "a host nothing answers for still fails");
    assert!(
        first6.0.is_ok() && first6.1 < Duration::from_secs(2),
        "the first IPv6 connection, held and routed: {first6:?}\n{}",
        sh("nft list chain inet crake syn_race; nft list set inet crake syn_watch6")
    );
    // The race knows the tunnel as it came up (its gateway never raced),
    // although its pushed networks overlap the private ranges it races.
    let skip = sh("nft list set inet crake syn_skip");
    assert!(skip.contains("192.0.2.10"), "{skip}");

    // The SYN that got through went out the tunnel, and the host is learned.
    assert!(
        world
            .lock()
            .unwrap()
            .tunnel_syns
            .iter()
            .any(|(d, _)| *d == "10.70.0.5".parse::<IpAddr>().unwrap())
    );
    assert!(sh("nft list set inet crake learned_acme").contains("10.70.0.5"));

    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("Sent 10.70.0.5 through ACME."),
        "the decision is logged:\n{log}"
    );

    mgr.shutdown().await;
    stop.store(true, Ordering::Relaxed);
    responder.join().unwrap();
}

/// The Traffic window's look (Manager::live): this computer's open
/// connections, each with its bytes (conntrack's accounting, switched on
/// when it was off) and the way it takes; the tunnel's device counted.
#[tokio::test(flavor = "multi_thread")]
async fn the_traffic_window_sees_each_connection_and_its_way() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let Probe {
        mgr,
        stop,
        responder,
        _dir: dir,
        _srv,
        ..
    } = start_probe(true).await;
    // Through ACME (raced and routed), and one direct that nothing answers.
    let (routed, _direct) = tokio::task::spawn_blocking(|| {
        let routed = connect("10.70.0.5", 5432, Duration::from_secs(4)).0;
        let direct = connect("198.51.100.7", 443, Duration::from_millis(300)).0;
        (routed, direct)
    })
    .await
    .unwrap();
    let _routed = routed.expect("routed through ACME");
    // Accounting is on here (the container sets it: /proc/sys can't be
    // written in it; switching it on and off is a unit test of
    // conntrack.rs). Another routed one, counted.
    let _ = mgr.live().await;
    let _later =
        tokio::task::spawn_blocking(|| connect("10.70.0.5", 5433, Duration::from_secs(4)).0)
            .await
            .unwrap()
            .expect("routed through ACME again");
    let live = mgr.live().await;
    let conns = live.connections.clone().expect("Linux lists connections");
    let to = |ip: &str, port: u16| {
        conns
            .iter()
            .find(|c| c.dst == ip.parse::<IpAddr>().unwrap() && c.dport == port)
            .unwrap_or_else(|| panic!("{ip}:{port} not in {conns:#?}"))
    };
    assert_eq!(to("10.70.0.5", 5432).tunnel.as_deref(), Some("acme"));
    assert_eq!(to("198.51.100.7", 443).tunnel, None, "direct");
    assert!(
        to("10.70.0.5", 5433).bytes > 0,
        "counted with accounting on ({})",
        sh("cat /proc/sys/net/netfilter/nf_conntrack_acct")
    );
    assert!(
        conns.iter().all(|c| c.dport != 53 && !c.dst.is_loopback()),
        "no DNS questions, nothing to itself"
    );
    // (The tunnel's device counters come from /sys, which is the host's
    // here, not this namespace's: the panel's charts cover them.)
    // On before craked looked: left on when it stops.
    mgr.shutdown().await;
    assert_eq!(
        sh("cat /proc/sys/net/netfilter/nf_conntrack_acct").trim(),
        "1"
    );
    stop.store(true, Ordering::Relaxed);
    responder.join().unwrap();
    drop(dir);
}

/// UDP: a host only the tunnel answers is tried through it with the app's
/// own first datagram, learned, and the app's next datagram gets through.
/// One that answers nowhere (a one-way service) is neither routed nor
/// logged.
#[tokio::test(flavor = "multi_thread")]
async fn udp_that_only_the_tunnel_answers_is_learned() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let Probe {
        mgr,
        world: _world,
        stop,
        responder,
        _dir: dir,
        _srv,
    } = start_probe(false).await;
    let (answered, silent) = tokio::task::spawn_blocking(|| {
        // An app that asks again until answered (as UDP apps do).
        let ask = |to: &str, tries: u32| {
            let sock = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
            sock.connect(to).unwrap();
            sock.set_read_timeout(Some(Duration::from_millis(700)))
                .unwrap();
            let start = Instant::now();
            let mut buf = [0u8; 64];
            for _ in 0..tries {
                sock.send(b"ping").unwrap();
                if let Ok(n) = sock.recv(&mut buf) {
                    return Some((buf[..n].to_vec(), start.elapsed()));
                }
            }
            None
        };
        (ask("10.70.0.12:5000", 10), ask("10.70.0.13:5000", 4))
    })
    .await
    .unwrap();
    let set = sh("nft list set inet crake learned_acme");
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap_or_default();
    assert!(
        answered.as_ref().is_some_and(|(b, _)| b == b"ping"),
        "the app's datagram answered through the tunnel: {answered:?}\n{set}\n{log}"
    );
    assert!(set.contains("10.70.0.12"), "{set}");
    assert!(log.contains("Sent 10.70.0.12 through ACME."), "{log}");
    assert!(silent.is_none());
    std::thread::sleep(Duration::from_secs(3));
    assert!(!sh("nft list set inet crake learned_acme").contains("10.70.0.13"));
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap_or_default();
    assert!(
        !log.contains("10.70.0.13"),
        "silence isn't worth a line: {log}"
    );
    mgr.shutdown().await;
    stop.store(true, Ordering::Relaxed);
    responder.join().unwrap();
}

/// A minute of trouble on the local network (seen 2026-10-15, after a wake
/// from sleep): two hosts answered neither directly nor through a tunnel,
/// and the next public host that didn't answer directly but did through a
/// company's tunnel was learned there for a week. While other hosts answer
/// nowhere, a public address isn't learned (it's tried again a minute
/// later); a private one is, as it can't be reached directly anyway.
#[tokio::test(flavor = "multi_thread")]
async fn a_public_host_isnt_learned_while_the_network_is_in_trouble() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let Probe {
        mgr,
        world,
        stop,
        responder,
        _dir: dir,
        _srv,
    } = start_probe(false).await;
    {
        let mut w = world.lock().unwrap();
        for a in ["198.51.100.20", "198.51.100.23", "10.70.0.9"] {
            w.behind_tunnel.insert(a.parse().unwrap());
        }
    }
    let path = dir.path().join("log");
    tokio::task::spawn_blocking(move || {
        let log = || std::fs::read_to_string(&path).unwrap_or_default();
        let learned = |a: &str| sh("nft list set inet crake learned_acme").contains(a);
        let wait_for = |line: String| {
            let end = Instant::now() + Duration::from_secs(15);
            while !log().contains(&line) {
                assert!(Instant::now() < end, "never: {line}\n{}", log());
                std::thread::sleep(Duration::from_millis(100));
            }
        };
        // A good minute: a public host only the tunnel reaches is learned.
        let _ = connect("198.51.100.23", 443, Duration::from_secs(1));
        wait_for("Sent 198.51.100.23 through ACME.".into());
        // Then two hosts answer nowhere.
        for a in ["198.51.100.21", "198.51.100.22"] {
            let _ = connect(a, 443, Duration::from_secs(1));
        }
        for a in ["198.51.100.21", "198.51.100.22"] {
            wait_for(format!("Left {a} alone."));
        }
        // Now a public host the tunnel answers for isn't learned...
        let _ = connect("198.51.100.20", 443, Duration::from_secs(1));
        wait_for("Left 198.51.100.20 alone.".into());
        assert!(!learned("198.51.100.20"));
        assert!(log().contains("the network itself"), "{}", log());
        // ...and a private one is.
        let _ = connect("10.70.0.9", 443, Duration::from_secs(1));
        wait_for("Sent 10.70.0.9 through ACME.".into());
        assert!(learned("10.70.0.9"));
    })
    .await
    .unwrap();
    mgr.shutdown().await;
    stop.store(true, Ordering::Relaxed);
    responder.join().unwrap();
}

/// One connection to a public host got no answer (its SYNs lost, seen
/// 2026-10-15 with api.anthropic.com on a working Wi-Fi), and a company's
/// tunnel answered: Crake learned it there, and the chat on it broke when
/// that tunnel's session expired. A public host is tried directly once
/// more; answering, it's left alone.
#[tokio::test(flavor = "multi_thread")]
async fn a_public_host_that_answers_directly_on_a_second_try_isnt_learned() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let Probe {
        mgr,
        world,
        stop,
        responder,
        _dir: dir,
        _srv,
    } = start_probe(false).await;
    {
        let mut w = world.lock().unwrap();
        // Answers through the tunnel too, as any public host would.
        w.behind_tunnel.insert("198.51.100.30".parse().unwrap());
        w.flaky_direct.insert("198.51.100.30".parse().unwrap());
    }
    let path = dir.path().join("log");
    tokio::task::spawn_blocking(move || {
        let log = || std::fs::read_to_string(&path).unwrap_or_default();
        let _ = connect("198.51.100.30", 443, Duration::from_secs(1));
        let end = Instant::now() + Duration::from_secs(15);
        while !log().contains("Left 198.51.100.30 alone.") {
            assert!(Instant::now() < end, "never left alone:\n{}", log());
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(log().contains("answered directly"), "{}", log());
        assert!(!sh("nft list set inet crake learned_acme").contains("198.51.100.30"));
    })
    .await
    .unwrap();
    mgr.shutdown().await;
    stop.store(true, Ordering::Relaxed);
    responder.join().unwrap();
}

/// A public host an older Crake learned by mistake (through a company's
/// tunnel, after one lost connection) is tried directly once a new version
/// runs: answering, it's forgotten and said; one that answers only through
/// the tunnel stays.
#[tokio::test(flavor = "multi_thread")]
async fn a_public_host_learned_by_mistake_goes_direct_again() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    let p = start_probe(false).await;
    {
        let mut w = p.world.lock().unwrap();
        // Two public hosts only the tunnel answers for, for now.
        w.behind_tunnel.insert("198.51.100.40".parse().unwrap());
        w.behind_tunnel.insert("198.51.100.41".parse().unwrap());
    }
    let learned = || sh("nft list set inet crake learned_acme");
    tokio::task::spawn_blocking(move || {
        for host in ["198.51.100.40", "198.51.100.41"] {
            let _ = connect(host, 443, Duration::from_secs(1));
        }
        let end = Instant::now() + Duration::from_secs(20);
        while !(learned().contains("198.51.100.40") && learned().contains("198.51.100.41")) {
            assert!(Instant::now() < end, "never learned:\n{}", learned());
            std::thread::sleep(Duration::from_millis(100));
        }
    })
    .await
    .unwrap();
    {
        // .41 answers directly now: its one lost connection is long past.
        let mut w = p.world.lock().unwrap();
        let a: IpAddr = "198.51.100.41".parse().unwrap();
        w.flaky_direct.insert(a);
        w.first_port.insert(a, 1);
    }
    p.mgr.recheck_tunnel("acme", "ACME").await;
    let set = sh("nft list set inet crake learned_acme");
    assert!(
        !set.contains("198.51.100.41"),
        "answers directly: forgotten\n{set}"
    );
    assert!(
        set.contains("198.51.100.40"),
        "only through the tunnel: kept\n{set}"
    );
    let log = std::fs::read_to_string(p._dir.path().join("log")).unwrap();
    assert!(
        log.contains("198.51.100.41 goes direct again.") && log.contains("an older Crake"),
        "{log}"
    );
    p.mgr.shutdown().await;
    p.stop.store(true, Ordering::Relaxed);
    p.responder.join().unwrap();
}
