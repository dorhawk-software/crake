//! DNS learning against the real kernel, for a throwaway network namespace only.
//! Skipped unless CRAKE_NETNS_TEST=1; run with `scripts/netns-test.sh`.
//!
//! The test owns a TUN device named like a tunnel and plays the tunnel's DNS
//! server: it reads the query that leaves through the tunnel and writes the
//! answer back in. The answer then passes craked's queue on its way in.

#![cfg(target_os = "linux")]

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::{Duration, Instant};

use crake_proto::TunnelState;
use craked::config::{Config, Protocol, TunnelConfig};
use craked::hook_server;
use craked::manager::Manager;

#[path = "dns_netns/wire.rs"]
mod wire;
use wire::*;

const FAKE: &str = r#"#!/bin/sh
export TUNDEV=crake-acme VPNGATEWAY=192.0.2.10 INTERNAL_IP4_ADDRESS=10.40.255.7 \
  INTERNAL_IP4_NETMASK=255.255.255.255 INTERNAL_IP4_DNS='10.40.0.2 fd40::53' \
  INTERNAL_IP6_ADDRESS=fd40:ff::7 INTERNAL_IP6_NETMASK=fd40:ff::7/64 \
  CISCO_DEF_DOMAIN=acme.internal
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

/// What the app got when it asked public DNS.
#[derive(Debug)]
struct Public {
    rcode: u8,
    addrs: Vec<IpAddr>,
    tunnel_asked: bool,
    first_through_tunnel: bool,
}

/// The app asks public DNS (the test plays it on `uplink`). If public DNS
/// fails, craked may ask the tunnel's DNS (the test plays that on `tun`).
fn ask_public(
    uplink: &mut File,
    tun: &mut File,
    name: &str,
    public_knows: Option<Ipv4Addr>,
    tunnel_knows: Option<Ipv4Addr>,
) -> Public {
    ask_public_for(
        uplink,
        tun,
        name,
        1,
        public_knows.map(Into::into),
        tunnel_knows.map(Into::into),
    )
}

/// The same for a question of type `kind` (1: A, 28: AAAA).
fn ask_public_for(
    uplink: &mut File,
    tun: &mut File,
    name: &str,
    kind: u8,
    public_knows: Option<IpAddr>,
    tunnel_knows: Option<IpAddr>,
) -> Public {
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query_for(name, kind), "192.168.0.1:53")
        .unwrap();
    let q = read_query(uplink);
    match public_knows {
        Some(a) => uplink.write_all(&reply(&q, 0, &[a])).unwrap(),
        None => uplink.write_all(&nxdomain(&q)).unwrap(),
    }
    let tunnel_asked = match try_read_query(tun, Duration::from_millis(1500)) {
        Some(tq) => {
            match tunnel_knows {
                Some(a) => tun.write_all(&reply(&tq, 0, &[a])).unwrap(),
                None => tun.write_all(&nxdomain(&tq)).unwrap(),
            }
            true
        }
        None => false,
    };
    let mut buf = [0u8; 1500];
    let (n, _) = sock.recv_from(&mut buf).expect("the app got an answer");
    let rcode = buf[3] & 0x0f;
    let addrs = craked::dns::parse_message(&buf[..n])
        .map(|a| a.addrs)
        .unwrap_or_default();
    let first_through_tunnel = match addrs.first() {
        Some(a) => {
            let app = UdpSocket::bind(if a.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }).unwrap();
            let _ = app.send_to(b"first", (*a, 5432));
            first_packet_through_tunnel(tun, *a)
        }
        None => false,
    };
    Public {
        rcode,
        addrs,
        tunnel_asked,
        first_through_tunnel,
    }
}

/// A name under the organisation's domain (the gateway's): public DNS knows
/// it, and the tunnel's DNS may know a private address for it, which may or
/// may not answer through the tunnel.
fn ask_split(
    uplink: &mut File,
    tun: &mut File,
    name: &str,
    public: impl Into<IpAddr>,
    tunnel_knows: impl Into<IpAddr>,
    private_answers: bool,
) -> Public {
    let (public, tunnel_knows) = (public.into(), tunnel_knows.into());
    let kind = if public.is_ipv6() { 28 } else { 1 };
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query_for(name, kind), "192.168.0.1:53")
        .unwrap();
    let q = read_query(uplink);
    uplink.write_all(&reply(&q, 0, &[public])).unwrap();
    let tunnel_asked = match try_read_query(tun, Duration::from_millis(1500)) {
        Some(tq) => {
            tun.write_all(&reply(&tq, 0, &[tunnel_knows])).unwrap();
            true
        }
        None => false,
    };
    // craked checks the private address answers (443) through the tunnel.
    if private_answers {
        let start = Instant::now();
        let mut buf = [0u8; 2048];
        while start.elapsed() < Duration::from_millis(1500) {
            match tun.read(&mut buf) {
                Ok(n) if syn_to(&buf[..n]) == Some((tunnel_knows, 443)) => {
                    tun.write_all(&tcp_synack(&buf[..n])).unwrap();
                    break;
                }
                Ok(_) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(2)),
            }
        }
    }
    let mut buf = [0u8; 1500];
    let (n, _) = sock.recv_from(&mut buf).expect("the app got an answer");
    let addrs = craked::dns::parse_message(&buf[..n])
        .map(|a| a.addrs)
        .unwrap_or_default();
    let first_through_tunnel = match addrs.first() {
        Some(a) => {
            let app = UdpSocket::bind(if a.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }).unwrap();
            let _ = app.send_to(b"first", (*a, 5432));
            first_packet_through_tunnel(tun, *a)
        }
        None => false,
    };
    Public {
        rcode: buf[3] & 0x0f,
        addrs,
        tunnel_asked,
        first_through_tunnel,
    }
}

fn try_read_query(tun: &mut File, wait: Duration) -> Option<Vec<u8>> {
    let start = Instant::now();
    let mut buf = [0u8; 2048];
    while start.elapsed() < wait {
        match tun.read(&mut buf) {
            Ok(n) if n > 28 && buf[0] >> 4 == 4 && buf[9] == 17 => {
                let ihl = usize::from(buf[0] & 0x0f) * 4;
                if u16::from_be_bytes([buf[ihl + 2], buf[ihl + 3]]) == 53 {
                    return Some(buf[..n].to_vec());
                }
            }
            _ => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    None
}

/// Wait for a UDP packet to port 53 leaving through the TUN.
fn read_query(tun: &mut File) -> Vec<u8> {
    let start = Instant::now();
    let mut buf = [0u8; 2048];
    while start.elapsed() < Duration::from_secs(5) {
        match tun.read(&mut buf) {
            Ok(n) if n > 28 && buf[0] >> 4 == 4 && buf[9] == 17 => {
                let ihl = usize::from(buf[0] & 0x0f) * 4;
                if u16::from_be_bytes([buf[ihl + 2], buf[ihl + 3]]) == 53 {
                    return buf[..n].to_vec();
                }
            }
            _ => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("no DNS query left through the tunnel");
}

/// Ask the tunnel's DNS server reached over IPv6 (fd40::53) for `name`;
/// the test answers `addr`. Returns whether the app's first packet to
/// `addr` left through the tunnel.
fn ask_over_ipv6(tun: &mut File, name: &str, addr: Ipv4Addr) -> bool {
    let sock = UdpSocket::bind("[::]:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query(name), "[fd40::53]:53").unwrap();
    let q = read_query6(tun);
    tun.write_all(&reply6(&q, 0, &[addr.into()])).unwrap();
    let mut answer = [0u8; 512];
    sock.recv_from(&mut answer)
        .expect("the answer reached the app");
    let _ = sock.send_to(b"first", (addr, 443));
    first_packet_through_tunnel(tun, IpAddr::V4(addr))
}

/// Wait for an IPv6 UDP packet to port 53 leaving through `dev`.
fn read_query6(dev: &mut File) -> Vec<u8> {
    let start = Instant::now();
    let mut buf = [0u8; 2048];
    loop {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "no IPv6 DNS query left"
        );
        match dev.read(&mut buf) {
            Ok(n) if n > 48 && buf[0] >> 4 == 6 && buf[6] == 17 && buf[42..44] == [0, 53] => {
                return buf[..n].to_vec();
            }
            _ => std::thread::sleep(Duration::from_millis(2)),
        }
    }
}

/// The app asks public DNS reached over IPv6 (the test plays it at
/// fd00:1a::1 on `uplink`), which doesn't know `name`; the tunnel's DNS
/// knows it as `addr`. Returns what the app got and whether its first
/// packet went through the tunnel.
fn ask_public_over_ipv6(
    uplink: &mut File,
    tun: &mut File,
    name: &str,
    addr: Ipv4Addr,
) -> (Vec<IpAddr>, bool) {
    let sock = UdpSocket::bind("[::]:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query(name), "[fd00:1a::1]:53").unwrap();
    let q = read_query6(uplink);
    uplink.write_all(&reply6(&q, 3, &[])).unwrap();
    if let Some(tq) = try_read_query(tun, Duration::from_millis(1500)) {
        tun.write_all(&answer(&tq, addr)).unwrap();
    }
    let mut buf = [0u8; 1500];
    let (n, _) = sock.recv_from(&mut buf).expect("the app got an answer");
    let addrs = craked::dns::parse_message(&buf[..n])
        .map(|a| a.addrs)
        .unwrap_or_default();
    let app = UdpSocket::bind("0.0.0.0:0").unwrap();
    let _ = app.send_to(b"first", (addr, 5432));
    (addrs, first_packet_through_tunnel(tun, IpAddr::V4(addr)))
}

/// A TCP packet leaving through the TUN that matches `want`, within 5 s.
fn read_tcp(tun: &mut File, want: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    let start = Instant::now();
    let mut buf = [0u8; 2048];
    while start.elapsed() < Duration::from_secs(5) {
        match tun.read(&mut buf) {
            Ok(n) if n >= 40 && buf[0] >> 4 == 4 && buf[9] == 6 && want(&buf[..n]) => {
                return buf[..n].to_vec();
            }
            _ => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    panic!("no such TCP packet left through the tunnel");
}

/// Ask the tunnel's DNS for `name` over TCP (as resolved does when an answer
/// is too big for UDP); the test answers `addr` in one segment. Returns
/// whether the app's first packet to `addr` left through the tunnel.
fn ask_tcp(tun: &mut File, name: &str, addr: Ipv4Addr) -> bool {
    let msg = query(name);
    let app = std::thread::spawn(move || {
        let mut s = std::net::TcpStream::connect_timeout(
            &"10.40.0.2:53".parse().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut framed = (msg.len() as u16).to_be_bytes().to_vec();
        framed.extend_from_slice(&msg);
        s.write_all(&framed).unwrap();
        let mut len = [0u8; 2];
        s.read_exact(&mut len).unwrap();
        let mut answer = vec![0u8; usize::from(u16::from_be_bytes(len))];
        s.read_exact(&mut answer).unwrap();
        // The app's first packet, sent the moment it has the answer.
        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"first", (addr, 443));
        answer
    });
    let syn = read_tcp(tun, |p| {
        syn_to(p) == Some(("10.40.0.2".parse::<IpAddr>().unwrap(), 53))
    });
    tun.write_all(&tcp_synack(&syn)).unwrap();
    // The client's segment carrying the query (after its bare ACK).
    let seg = read_tcp(tun, |p| {
        let ihl = usize::from(p[0] & 0x0f) * 4;
        p.len() > ihl + usize::from(p[ihl + 12] >> 4) * 4
    });
    let ihl = usize::from(seg[0] & 0x0f) * 4;
    let data = &seg[ihl + usize::from(seg[ihl + 12] >> 4) * 4..];
    let reply = answer_message(&data[2..], 0, &[addr.into()]);
    let mut framed = (reply.len() as u16).to_be_bytes().to_vec();
    framed.extend_from_slice(&reply);
    tun.write_all(&tcp_reply(&seg, 777_001, &framed)).unwrap();
    let got = app.join().unwrap();
    assert!(
        got.windows(4).any(|w| w == addr.octets()),
        "the app got the answer"
    );
    first_packet_through_tunnel(tun, IpAddr::V4(addr))
}

/// Ask the tunnel's DNS (over IPv4) for `name`'s IPv6 address; the test
/// answers `addr`. Returns whether the app's first packet to `addr` left
/// through the tunnel.
fn ask_aaaa(tun: &mut File, name: &str, addr: Ipv6Addr) -> bool {
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query_for(name, 28), "10.40.0.2:53").unwrap();
    let q = read_query(tun);
    tun.write_all(&reply(&q, 0, &[addr.into()])).unwrap();
    let mut buf = [0u8; 512];
    let (n, _) = sock
        .recv_from(&mut buf)
        .expect("the answer reached the app");
    assert!(buf[..n].windows(16).any(|w| w == addr.octets()));
    let app = UdpSocket::bind("[::]:0").unwrap();
    let _ = app.send_to(b"first", (addr, 443));
    first_packet_through_tunnel(tun, IpAddr::V6(addr))
}

/// Ask the tunnel's DNS for `name`; the test answers `addr`. Returns whether
/// the app's first packet to `addr` left through the tunnel.
fn ask(tun: &mut File, name: &str, addr: Ipv4Addr) -> bool {
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query(name), "10.40.0.2:53").unwrap();
    let q = read_query(tun);
    tun.write_all(&answer(&q, addr)).unwrap();
    let mut buf = [0u8; 512];
    let (n, _) = sock
        .recv_from(&mut buf)
        .expect("the answer reached the app");
    assert!(n > 12);
    // The app's first packet, sent the moment it has the answer.
    let _ = sock.send_to(b"first", (addr, 443));
    first_packet_through_tunnel(tun, IpAddr::V4(addr))
}

fn first_packet_through_tunnel(tun: &mut File, addr: IpAddr) -> bool {
    let start = Instant::now();
    let mut buf = [0u8; 2048];
    while start.elapsed() < Duration::from_millis(300) {
        let hit = match tun.read(&mut buf) {
            Ok(n) => match addr {
                IpAddr::V4(a) => n >= 20 && buf[0] >> 4 == 4 && buf[16..20] == a.octets(),
                IpAddr::V6(a) => n >= 40 && buf[0] >> 4 == 6 && buf[24..40] == a.octets(),
            },
            Err(_) => {
                std::thread::sleep(Duration::from_millis(2));
                false
            }
        };
        if hit {
            return true;
        }
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn names_only_the_tunnel_knows_are_routed_before_the_first_packet() {
    if std::env::var("CRAKE_NETNS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CRAKE_NETNS_TEST=1 inside a throwaway network namespace");
        return;
    }
    // Strict reverse-path filtering, as Fedora has it.
    strict_rp_filter();
    // A stand-in for the Wi-Fi, with public DNS at 192.168.0.1 (played by the test).
    let mut uplink = open_tun("uplink");
    // Dual-stack: an unrouted IPv6 destination goes this way first.
    sh(
        "ip link set uplink up; ip addr add 192.168.0.242/24 dev uplink; \
        ip route add default via 192.168.0.1 dev uplink; \
        ip -6 addr add fd00:1a::242/64 dev uplink nodad; ip -6 route add default dev uplink",
    );
    let mut tun = open_tun("crake-acme");
    sh("ip link set crake-acme up");

    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("openconnect");
    std::fs::write(&fake, FAKE).unwrap();
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
        use_resolved: false, // no systemd in the namespace
        apply_network: true,
        managed_dir: dir.path().join("managed.d"),
        tunnels: vec![TunnelConfig {
            name: "acme".into(),
            display_name: Some("ACME".into()),
            gateway: "vpn.acme.example".into(),
            protocol: Protocol::Anyconnect,
            user: None,
            passwd_file: None,
            dns_domains: vec!["corp.example".into()],
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
    assert!(sh("nft list chain inet crake dns_acme").contains("queue"));

    let git: Ipv4Addr = "10.33.7.7".parse().unwrap();
    let wiki: Ipv4Addr = "10.33.8.8".parse().unwrap();
    let big: Ipv4Addr = "10.33.9.9".parse().unwrap();
    let public: Ipv4Addr = "93.184.215.14".parse().unwrap();
    let gw: Ipv4Addr = "192.0.2.10".parse().unwrap();
    let git6: Ipv6Addr = "fd40:1::7".parse().unwrap();
    let db6: Ipv6Addr = "fd40:1::5".parse().unwrap();
    let fe6_private: Ipv6Addr = "fd40:9::30".parse().unwrap();
    let db_v6net: Ipv4Addr = "10.58.70.120".parse().unwrap();
    let over6: Ipv4Addr = "10.33.6.6".parse().unwrap();

    // Run the blocking socket work off the runtime; the queue thread needs it.
    let db: Ipv4Addr = "10.58.70.119".parse().unwrap();
    let site: Ipv4Addr = "93.184.215.20".parse().unwrap();
    let fe_public: Ipv4Addr = "198.51.100.32".parse().unwrap();
    let fe_private: Ipv4Addr = "10.40.9.30".parse().unwrap();
    let office_public: Ipv4Addr = "198.51.100.40".parse().unwrap();
    let office_private: Ipv4Addr = "10.58.9.9".parse().unwrap();
    let www_public: Ipv4Addr = "198.51.100.50".parse().unwrap();
    let (tun_back, results, publics, splits) = tokio::task::spawn_blocking(move || {
        let publics = (
            // Public DNS doesn't know it; the tunnel does.
            ask_public(&mut uplink, &mut tun, "db.example.cloud", None, Some(db)),
            // Nobody knows it: the failure reaches the app unchanged.
            ask_public(&mut uplink, &mut tun, "typo.example.cloud", None, None),
            // Asked again soon: not asked of the tunnel again.
            ask_public(&mut uplink, &mut tun, "typo.example.cloud", None, None),
            // Public DNS knows it: untouched, the tunnel is never asked.
            ask_public(&mut uplink, &mut tun, "www.example.org", Some(site), None),
            // Asked for its IPv6 address: the tunnel's, routed.
            ask_public_for(
                &mut uplink,
                &mut tun,
                "db6.example.cloud",
                28,
                None,
                Some(db6.into()),
            ),
            // Public DNS reached over IPv6 (a dual-stack network's router
            // often hands out one): rescued the same way.
            ask_public_over_ipv6(&mut uplink, &mut tun, "db.v6net.example", db_v6net),
        );
        // Under the organisation's own domain (gateway vpn.acme.example):
        // public DNS knows it, and the tunnel's DNS knows better.
        let splits = (
            // A private address that answers through the tunnel: used.
            ask_split(
                &mut uplink,
                &mut tun,
                "fe.acme.example",
                fe_public,
                fe_private,
                true,
            ),
            // One that doesn't answer (office-only): the public one stays.
            ask_split(
                &mut uplink,
                &mut tun,
                "office.acme.example",
                office_public,
                office_private,
                false,
            ),
            // The tunnel says the same public address: nothing changes.
            ask_split(
                &mut uplink,
                &mut tun,
                "www.acme.example",
                www_public,
                www_public,
                false,
            ),
            // Its IPv6 address (AAAA) the same way.
            ask_split(
                &mut uplink,
                &mut tun,
                "fe6.acme.example",
                "2001:db8:5::32".parse::<Ipv6Addr>().unwrap(),
                fe6_private,
                true,
            ),
        );
        let r = (
            ask(&mut tun, "git.acme.internal", git),
            ask(&mut tun, "wiki.corp.example", wiki),
            ask(&mut tun, "www.example.com", public),
            ask(&mut tun, "vpn.acme.internal", gw),
            ask_tcp(&mut tun, "big.acme.internal", big),
            ask_aaaa(&mut tun, "v6.acme.internal", git6),
            ask_over_ipv6(&mut tun, "dns6.acme.internal", over6),
        );
        (tun, r, publics, splits)
    })
    .await
    .unwrap();
    let (p0, p1, p2, p3, p4, over_v6) = publics;
    assert_eq!(
        over_v6.0,
        vec![IpAddr::V4(db_v6net)],
        "public DNS over IPv6 didn't know it; the tunnel's answer replaced it"
    );
    assert!(over_v6.1, "and it was routed first");
    publics_handled((p0, p1, p2, p3, p4), db, db6, site);

    splits_handled(splits, fe_private, office_public, www_public, fe6_private);

    let (git_first, wiki_first, public_routed, gw_routed, tcp_first, v6_first, over6_first) =
        results;
    assert!(
        over6_first,
        "an answer from the tunnel's DNS reached over IPv6: routed first\n{}",
        sh("nft list chain inet crake dns_acme; ip -6 route")
    );
    assert!(
        v6_first,
        "an AAAA answer: the first IPv6 packet went through the tunnel\n{}",
        sh(
            "nft list set inet crake learned6_acme; ip -6 addr; ip -6 rule; ip -6 route show table all"
        )
    );
    assert!(
        tcp_first,
        "an answer over TCP: the first packet went through the tunnel"
    );
    assert!(
        git_first,
        "pushed domain: the first packet went through the tunnel"
    );
    assert!(
        wiki_first,
        "configured domain: the first packet went through the tunnel"
    );
    assert!(
        !public_routed,
        "names outside the tunnel's domains are left alone"
    );
    assert!(!gw_routed, "never the gateway");

    let rows = mgr.hosts("acme").await.unwrap();
    assert!(
        rows.iter()
            .any(|r| r.name.as_deref() == Some("git.acme.internal"))
    );

    mgr.shutdown().await;
    drop(tun_back);
    logged_what_it_did(dir.path());
}

/// What the app got from public DNS: rescued by the tunnel when public
/// DNS didn't know the name (A and AAAA), unchanged otherwise.
fn publics_handled(
    publics: (Public, Public, Public, Public, Public),
    db: Ipv4Addr,
    db6: Ipv6Addr,
    site: Ipv4Addr,
) {
    let (rescued, unknown, unknown_again, known, rescued6) = publics;
    assert_eq!(rescued6.addrs, vec![IpAddr::V6(db6)], "{rescued6:?}");
    assert!(
        rescued6.tunnel_asked && rescued6.first_through_tunnel,
        "an AAAA rescued from the tunnel is routed: {rescued6:?}"
    );
    assert_eq!(rescued.rcode, 0, "{rescued:?}");
    assert_eq!(
        rescued.addrs,
        vec![IpAddr::V4(db)],
        "the tunnel's answer replaced the failure"
    );
    assert!(
        rescued.tunnel_asked && rescued.first_through_tunnel,
        "{rescued:?}"
    );
    assert_eq!(unknown.rcode, 3, "unknown everywhere: still NXDOMAIN");
    assert!(
        unknown.tunnel_asked && !unknown_again.tunnel_asked,
        "{unknown_again:?}"
    );
    assert_eq!(unknown_again.rcode, 3);
    assert_eq!(
        (known.rcode, known.addrs.clone()),
        (0, vec![IpAddr::V4(site)])
    );
    assert!(
        !known.tunnel_asked,
        "public successes never go to the tunnel"
    );
    assert!(!known.first_through_tunnel, "and stay direct");
}

/// Names under the organisation's domain: the tunnel's private address
/// (IPv4 or IPv6) wins when it answers through the tunnel, else the
/// public one stands.
fn splits_handled(
    splits: (Public, Public, Public, Public),
    fe_private: Ipv4Addr,
    office_public: Ipv4Addr,
    www_public: Ipv4Addr,
    fe6_private: Ipv6Addr,
) {
    let (fe, office, www, fe6) = splits;
    assert_eq!(
        fe6.addrs,
        vec![IpAddr::V6(fe6_private)],
        "the private IPv6 address won: {fe6:?}"
    );
    assert!(fe6.tunnel_asked && fe6.first_through_tunnel, "{fe6:?}");
    assert_eq!(
        fe.addrs,
        vec![IpAddr::V4(fe_private)],
        "the private address won: {fe:?}"
    );
    assert!(fe.tunnel_asked && fe.first_through_tunnel, "{fe:?}");
    assert_eq!(office.addrs, vec![IpAddr::V4(office_public)], "{office:?}");
    assert!(
        office.tunnel_asked && !office.first_through_tunnel,
        "{office:?}"
    );
    assert_eq!(www.addrs, vec![IpAddr::V4(www_public)], "{www:?}");
    assert!(!www.first_through_tunnel, "{www:?}");
}

/// What the log says about the test's names.
fn logged_what_it_did(dir: &std::path::Path) {
    let log = std::fs::read_to_string(dir.join("log")).unwrap();
    assert!(
        log.contains("Sent git.acme.internal through ACME."),
        "{log}"
    );
    assert!(log.contains("Routed before the first packet."), "{log}");
    assert!(log.contains("Sent v6.acme.internal through ACME."), "{log}");
    assert!(log.contains("Left vpn.acme.internal alone."), "{log}");
    assert!(
        log.contains("Sent db.example.cloud through ACME.")
            && log.contains("Public DNS didn't know it; ACME's DNS did (10.58.70.119)."),
        "{log}"
    );
    assert!(
        log.contains("Sent fe.acme.example through ACME.")
            && log.contains("Public name, private address: ACME's DNS knows it as 10.40.9.30."),
        "{log}"
    );
    assert!(!log.contains("office.acme.example"), "{log}");
}

/// rp_filter=1 everywhere, as Fedora has it (scripts/netns-test.sh starts
/// the namespace so): a reply arriving on an interface the main table
/// wouldn't use to reach its sender is dropped, unless craked loosens it
/// for the tunnel's interface.
fn strict_rp_filter() {
    let all = std::fs::read_to_string("/proc/sys/net/ipv4/conf/all/rp_filter").unwrap();
    assert_eq!(all.trim(), "1", "the namespace filters strictly");
}
