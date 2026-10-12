//! DNS learning against the real kernel, for a throwaway network namespace only.
//! Skipped unless CRAKE_NETNS_TEST=1; run with `scripts/netns-test.sh`.
//!
//! The test owns a TUN device named like a tunnel and plays the tunnel's DNS
//! server: it reads the query that leaves through the tunnel and writes the
//! answer back in. The answer then passes craked's queue on its way in.

#![cfg(target_os = "linux")]

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::{Duration, Instant};

use crake_proto::TunnelState;
use craked::config::{Config, Protocol, TunnelConfig};
use craked::hook_server;
use craked::manager::Manager;

const FAKE: &str = r#"#!/bin/sh
export TUNDEV=crake-acme VPNGATEWAY=192.0.2.10 INTERNAL_IP4_ADDRESS=10.40.255.7 \
  INTERNAL_IP4_NETMASK=255.255.255.255 INTERNAL_IP4_DNS=10.40.0.2 \
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
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query(name), "192.168.0.1:53").unwrap();
    let q = read_query(uplink);
    match public_knows {
        Some(a) => uplink.write_all(&answer(&q, a)).unwrap(),
        None => uplink.write_all(&nxdomain(&q)).unwrap(),
    }
    let tunnel_asked = match try_read_query(tun, Duration::from_millis(1500)) {
        Some(tq) => {
            match tunnel_knows {
                Some(a) => tun.write_all(&answer(&tq, a)).unwrap(),
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
            let _ = sock.send_to(b"first", (*a, 5432));
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
    public: Ipv4Addr,
    tunnel_knows: Ipv4Addr,
    private_answers: bool,
) -> Public {
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.send_to(&query(name), "192.168.0.1:53").unwrap();
    let q = read_query(uplink);
    uplink.write_all(&answer(&q, public)).unwrap();
    let tunnel_asked = match try_read_query(tun, Duration::from_millis(1500)) {
        Some(tq) => {
            tun.write_all(&answer(&tq, tunnel_knows)).unwrap();
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
            let _ = sock.send_to(b"first", (*a, 5432));
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

/// A SYN in this packet: (destination, port).
fn syn_to(p: &[u8]) -> Option<(Ipv4Addr, u16)> {
    if p.len() < 40 || p[0] >> 4 != 4 || p[9] != 6 {
        return None;
    }
    let ihl = usize::from(p[0] & 0x0f) * 4;
    let t = p.get(ihl..)?;
    (t.len() >= 20 && t[13] & 0x12 == 0x02).then(|| {
        (
            Ipv4Addr::new(p[16], p[17], p[18], p[19]),
            u16::from_be_bytes([t[2], t[3]]),
        )
    })
}

/// The SYN-ACK answering `syn`.
fn tcp_synack(syn: &[u8]) -> Vec<u8> {
    let sum16 = |d: &[u8]| -> u32 {
        d.chunks(2)
            .map(|c| u32::from(u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)])))
            .sum()
    };
    let fold = |mut s: u32| -> u16 {
        while s >> 16 != 0 {
            s = (s & 0xffff) + (s >> 16);
        }
        !(s as u16)
    };
    let ihl = usize::from(syn[0] & 0x0f) * 4;
    let t = &syn[ihl..];
    let seq = u32::from_be_bytes([t[4], t[5], t[6], t[7]]);
    let mut tcp = vec![0u8; 20];
    tcp[0..2].copy_from_slice(&t[2..4]);
    tcp[2..4].copy_from_slice(&t[0..2]);
    tcp[4..8].copy_from_slice(&777_000u32.to_be_bytes());
    tcp[8..12].copy_from_slice(&seq.wrapping_add(1).to_be_bytes());
    tcp[12] = 5 << 4;
    tcp[13] = 0x12;
    tcp[14..16].copy_from_slice(&65535u16.to_be_bytes());
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

fn checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = header
        .chunks(2)
        .map(|c| u32::from(u16::from_be_bytes([c[0], c[1]])))
        .sum();
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// Turn the query packet into an answer packet with one A record.
fn answer(query: &[u8], addr: Ipv4Addr) -> Vec<u8> {
    reply(query, 0, &[addr])
}

fn nxdomain(query: &[u8]) -> Vec<u8> {
    reply(query, 3, &[])
}

/// Turn the query packet into an answer packet: rcode plus A records.
fn reply(query: &[u8], rcode: u8, addrs: &[Ipv4Addr]) -> Vec<u8> {
    let ihl = usize::from(query[0] & 0x0f) * 4;
    let udp = &query[ihl..];
    let dns = answer_message(&udp[8..], rcode, addrs);
    let mut udp_out = Vec::new();
    udp_out.extend_from_slice(&udp[2..4]); // sport = their dport (53)
    udp_out.extend_from_slice(&udp[0..2]); // dport = their sport
    udp_out.extend_from_slice(&((8 + dns.len()) as u16).to_be_bytes());
    udp_out.extend_from_slice(&[0, 0]); // no UDP checksum
    udp_out.extend_from_slice(&dns);

    let mut ip = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0];
    ip.extend_from_slice(&query[16..20]); // src = the DNS server
    ip.extend_from_slice(&query[12..16]); // dst = us
    let total = (20 + udp_out.len()) as u16;
    ip[2..4].copy_from_slice(&total.to_be_bytes());
    let c = checksum(&ip);
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    ip.extend_from_slice(&udp_out);
    ip
}

/// The answer to a DNS query message: rcode plus A records.
fn answer_message(query: &[u8], rcode: u8, addrs: &[Ipv4Addr]) -> Vec<u8> {
    let mut dns = query.to_vec();
    dns[2] = 0x81; // response, recursion desired
    dns[3] = 0x80 | rcode; // recursion available
    dns[7] = addrs.len() as u8;
    dns[10] = 0; // drop EDNS from additional count
    dns[11] = 0;
    // Cut the additional section: question ends after the name plus 4 bytes.
    let mut i = 12;
    while dns[i] != 0 {
        i += usize::from(dns[i]) + 1;
    }
    dns.truncate(i + 5);
    for addr in addrs {
        dns.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4]);
        dns.extend_from_slice(&addr.octets());
    }
    dns
}

/// A reply segment to the client's `seg` (PSH|ACK) from the server at `seq`,
/// acknowledging what `seg` carried.
fn tcp_reply(seg: &[u8], seq: u32, data: &[u8]) -> Vec<u8> {
    let ihl = usize::from(seg[0] & 0x0f) * 4;
    let t = &seg[ihl..];
    let carried = seg.len() - ihl - usize::from(t[12] >> 4) * 4;
    let their_seq = u32::from_be_bytes([t[4], t[5], t[6], t[7]]);
    let mut tcp = vec![0u8; 20];
    tcp[0..2].copy_from_slice(&t[2..4]);
    tcp[2..4].copy_from_slice(&t[0..2]);
    tcp[4..8].copy_from_slice(&seq.to_be_bytes());
    tcp[8..12].copy_from_slice(&their_seq.wrapping_add(carried as u32).to_be_bytes());
    tcp[12] = 5 << 4;
    tcp[13] = 0x18;
    tcp[14..16].copy_from_slice(&65535u16.to_be_bytes());
    tcp.extend_from_slice(data);
    let (src, dst) = (&seg[16..20], &seg[12..16]);
    let mut pseudo = Vec::new();
    pseudo.extend_from_slice(src);
    pseudo.extend_from_slice(dst);
    pseudo.extend_from_slice(&[0, 6]);
    pseudo.extend_from_slice(&(tcp.len() as u16).to_be_bytes());
    let sum = |d: &[u8]| -> u32 {
        d.chunks(2)
            .map(|c| u32::from(u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)])))
            .sum()
    };
    let mut s = sum(&pseudo) + sum(&tcp);
    while s >> 16 != 0 {
        s = (s & 0xffff) + (s >> 16);
    }
    tcp[16..18].copy_from_slice(&(!(s as u16)).to_be_bytes());
    let mut ip = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 6, 0, 0];
    ip.extend_from_slice(src);
    ip.extend_from_slice(dst);
    ip[2..4].copy_from_slice(&((20 + tcp.len()) as u16).to_be_bytes());
    let c = checksum(&ip);
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    ip.extend_from_slice(&tcp);
    ip
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
        syn_to(p) == Some(("10.40.0.2".parse().unwrap(), 53))
    });
    tun.write_all(&tcp_synack(&syn)).unwrap();
    // The client's segment carrying the query (after its bare ACK).
    let seg = read_tcp(tun, |p| {
        let ihl = usize::from(p[0] & 0x0f) * 4;
        p.len() > ihl + usize::from(p[ihl + 12] >> 4) * 4
    });
    let ihl = usize::from(seg[0] & 0x0f) * 4;
    let data = &seg[ihl + usize::from(seg[ihl + 12] >> 4) * 4..];
    let reply = answer_message(&data[2..], 0, &[addr]);
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

fn query(name: &str) -> Vec<u8> {
    let mut m = vec![0xab, 0xcd, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        m.push(label.len() as u8);
        m.extend_from_slice(label.as_bytes());
    }
    m.extend_from_slice(&[0, 0, 1, 0, 1]);
    m
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
    sh(
        "ip link set uplink up; ip addr add 192.168.0.242/24 dev uplink; \
        ip route add default via 192.168.0.1 dev uplink",
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
        );
        let r = (
            ask(&mut tun, "git.acme.internal", git),
            ask(&mut tun, "wiki.corp.example", wiki),
            ask(&mut tun, "www.example.com", public),
            ask(&mut tun, "vpn.acme.internal", gw),
            ask_tcp(&mut tun, "big.acme.internal", big),
        );
        (tun, r, publics, splits)
    })
    .await
    .unwrap();
    let (rescued, unknown, unknown_again, known) = publics;
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

    let (fe, office, www) = splits;
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

    let (git_first, wiki_first, public_routed, gw_routed, tcp_first) = results;
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
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(
        log.contains("Sent git.acme.internal through ACME."),
        "{log}"
    );
    assert!(log.contains("Routed before the first packet."), "{log}");
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
