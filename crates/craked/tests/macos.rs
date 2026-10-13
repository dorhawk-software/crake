//! macOS, as root (CI job `macos`, GitHub's runner): craked's macOS parts
//! on a real utun device this test makes itself (openconnect makes one the
//! same way): the tunnel's address, its DNS route, learned and pinned hosts
//! as routes, /etc/resolver files sending a domain's names to craked's
//! server (a stand-in that answers every A question with 10.40.1.10), and
//! that server standing in for the system's DNS servers (public names).
//! Skipped unless CRAKE_MACOS_ROOT=1. Not a full tunnel: routing
//! everything into a device nobody reads would cut the runner off.
#![cfg(target_os = "macos")]

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::os::fd::{FromRawFd, OwnedFd};
use std::process::Command;

use craked::hosts::Target;
use craked::net::Net;
use craked::resolved::Resolved;
use craked::routing::{Plan, Routing};

fn enabled() -> bool {
    if std::env::var("CRAKE_MACOS_ROOT").as_deref() == Ok("1") {
        return true;
    }
    eprintln!("skipped: needs root (CRAKE_MACOS_ROOT=1)");
    false
}

/// A utun device of our own: gone when the socket closes.
fn utun() -> (OwnedFd, String) {
    // SAFETY: the kernel control API as openconnect uses it; every buffer
    // is the size the call is told.
    unsafe {
        let fd = libc::socket(libc::PF_SYSTEM, libc::SOCK_DGRAM, libc::SYSPROTO_CONTROL);
        assert!(fd >= 0, "a kernel control socket");
        let fd = OwnedFd::from_raw_fd(fd);
        let mut info: libc::ctl_info = std::mem::zeroed();
        for (i, b) in b"com.apple.net.utun_control".iter().enumerate() {
            info.ctl_name[i] = *b as libc::c_char;
        }
        assert_eq!(
            libc::ioctl(
                std::os::fd::AsRawFd::as_raw_fd(&fd),
                libc::CTLIOCGINFO,
                &raw mut info
            ),
            0,
            "utun's control id"
        );
        let addr = libc::sockaddr_ctl {
            sc_len: size_of::<libc::sockaddr_ctl>() as u8,
            sc_family: libc::AF_SYSTEM as u8,
            ss_sysaddr: libc::AF_SYS_CONTROL as u16,
            sc_id: info.ctl_id,
            sc_unit: 0,
            sc_reserved: [0; 5],
        };
        assert_eq!(
            libc::connect(
                std::os::fd::AsRawFd::as_raw_fd(&fd),
                (&raw const addr).cast(),
                size_of::<libc::sockaddr_ctl>() as u32,
            ),
            0,
            "a new utun device"
        );
        let mut name = [0u8; 32];
        let mut len = name.len() as libc::socklen_t;
        assert_eq!(
            libc::getsockopt(
                std::os::fd::AsRawFd::as_raw_fd(&fd),
                libc::SYSPROTO_CONTROL,
                libc::UTUN_OPT_IFNAME,
                name.as_mut_ptr().cast(),
                &raw mut len,
            ),
            0,
            "its name"
        );
        let name = std::ffi::CStr::from_bytes_until_nul(&name)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        (fd, name)
    }
}

fn sh(program: &str, args: &[&str]) -> String {
    let out = Command::new(program).args(args).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// The interface `route -n get` sends `addr` out of.
fn way(addr: &str) -> String {
    let mut args = vec!["-n", "get"];
    if addr.contains(':') {
        args.push("-inet6");
    }
    args.push(addr);
    sh("route", &args)
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("interface:")
                .map(|s| s.trim().to_owned())
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn a_tunnel_on_a_utun_device() {
    if !enabled() {
        return;
    }
    let (_fd, dev) = utun();
    assert!(craked::sys::tunnel_device("acme", &dev), "{dev}");

    // Its address, and the one route Crake adds up front: its DNS.
    let net = Net::new(true);
    net.assign(&dev, Ipv4Addr::new(10, 40, 255, 5), 32, Some(1400))
        .await
        .unwrap();
    let shown = sh("ifconfig", &[&dev]);
    assert!(shown.contains("inet 10.40.255.5"), "{shown}");
    assert!(shown.contains("mtu 1400"), "{shown}");
    net.route_dns(&dev, Ipv4Addr::new(10, 40, 0, 2))
        .await
        .unwrap();
    assert_eq!(way("10.40.0.2"), dev);
    // The IPv6 address the gateway gave, as well.
    net.assign6(&dev, "fd40:ff::5".parse().unwrap(), 128)
        .await
        .unwrap();
    let shown = sh("ifconfig", &[&dev]);
    assert!(shown.contains("inet6 fd40:ff::5"), "{shown}");

    // Pinned and learned hosts: routes through it; nothing else.
    let routing = Routing::new(true);
    let pinned: Vec<Target> = vec![
        "10.40.2.0/24".parse().unwrap(),
        "fd40:2::/64".parse().unwrap(),
    ];
    let learned = [
        (IpAddr::V4(Ipv4Addr::new(10, 40, 1, 10)), 600),
        ("fd40:1::10".parse().unwrap(), 600),
    ];
    routing
        .up(&Plan {
            name: "acme",
            index: 0,
            device: &dev,
            timeout: 600,
            dns: &[Ipv4Addr::new(10, 40, 0, 2)],
            dns6: &[],
            pinned: &pinned,
            learned: &learned,
            full_except: None,
            ipv6: true,
        })
        .await
        .unwrap();
    assert_eq!(way("10.40.1.10"), dev);
    assert_eq!(way("10.40.2.7"), dev);
    assert_ne!(way("10.40.1.11"), dev, "not learned: not routed");
    // IPv6 hosts and networks likewise.
    assert_eq!(way("fd40:1::10"), dev);
    assert_eq!(way("fd40:2::7"), dev);
    assert_ne!(way("fd40:1::11"), dev, "not learned: not routed");
    routing
        .add_learned("acme", IpAddr::V4(Ipv4Addr::new(10, 40, 1, 11)), 600)
        .await
        .unwrap();
    assert_eq!(way("10.40.1.11"), dev);
    routing
        .remove_learned("acme", IpAddr::V4(Ipv4Addr::new(10, 40, 1, 11)))
        .await
        .unwrap();
    assert_ne!(way("10.40.1.11"), dev, "forgotten");
    // A connection that started before its host was routed: its next SYN
    // goes through the tunnel, NATed to the tunnel's address (pf). Its
    // first SYN vanishes (a blackhole, as a firewall that drops it: the
    // runner's own network may refuse it at once instead).
    sh(
        "route",
        &[
            "-n",
            "add",
            "-net",
            "10.40.1.0/24",
            "127.0.0.1",
            "-blackhole",
        ],
    );
    let early = tokio::net::TcpSocket::new_v4().unwrap();
    let connecting = tokio::spawn(early.connect("10.40.1.99:80".parse().unwrap()));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    routing
        .add_learned("acme", IpAddr::V4(Ipv4Addr::new(10, 40, 1, 99)), 600)
        .await
        .unwrap();
    let syn = tokio::task::spawn_blocking({
        let fd = std::os::fd::AsRawFd::as_raw_fd(&_fd);
        move || syn_to(fd, Ipv4Addr::new(10, 40, 1, 99))
    })
    .await
    .unwrap();
    assert_eq!(
        syn,
        Some(Ipv4Addr::new(10, 40, 255, 5)),
        "the retried SYN, from the tunnel's address"
    );
    connecting.abort();
    sh("route", &["-n", "delete", "-net", "10.40.1.0/24"]);
    // A socket bound to the tunnel finds its way out through it (probes).
    let probe = tokio::net::TcpSocket::new_v4().unwrap();
    craked::sys::mark(&probe, craked::routing::ids(0).mark).unwrap();
    let probe6 = tokio::net::TcpSocket::new_v6().unwrap();
    craked::sys::mark(&probe6, craked::routing::ids(0).mark).unwrap();

    // DNS: acme.internal's names go to craked's server (here a stand-in).
    craked::dns_queue::add_address().unwrap();
    let server = UdpSocket::bind((craked::dns_queue::ADDR, craked::dns_queue::PORT)).unwrap();
    std::thread::spawn(move || {
        let mut buf = [0u8; 512];
        while let Ok((n, from)) = server.recv_from(&mut buf) {
            let _ = server.send_to(&answer(&buf[..n]), from);
        }
    });
    let resolved = Resolved::new(true);
    resolved
        .set_link(
            &dev,
            &[Ipv4Addr::new(10, 40, 0, 2).into()],
            &["acme.internal".into()],
            false,
        )
        .await
        .unwrap();
    let file = std::fs::read_to_string("/etc/resolver/acme.internal").unwrap();
    assert!(file.contains(&format!("for {dev}")), "{file}");
    resolved.flush().await;
    let found = sh(
        "dscacheutil",
        &["-q", "host", "-a", "name", "git.acme.internal"],
    );
    assert!(
        found.contains("10.40.1.10"),
        "macOS asked craked's server: {found}"
    );

    // Every other name too, while a tunnel is up: craked's server stands
    // in for the primary service's DNS servers, and asks those.
    let before = nameservers();
    craked::resolved::set_public(true);
    let upstream = craked::sys::system_dns();
    assert!(
        !upstream.is_empty() && !upstream.contains(&IpAddr::V4(craked::dns_queue::ADDR)),
        "the servers it stands in for: {upstream:?}"
    );
    let found = sh(
        "dscacheutil",
        &["-q", "host", "-a", "name", "www.globex.example"],
    );
    assert!(
        found.contains("10.40.1.10"),
        "macOS asked craked's server for a public name: {found}\n{}",
        sh("scutil", &["--dns"])
    );
    craked::resolved::set_public(false);
    assert!(craked::sys::system_dns().is_empty());
    assert_eq!(nameservers(), before, "the system's own servers back");

    // A full tunnel without an IPv6 address refuses IPv6 with reject
    // routes (sys::macos::cmd::reject_args, here for a test prefix): a
    // connection fails at once instead of going around the tunnel.
    sh(
        "route",
        &[
            "-n",
            "add",
            "-inet6",
            "-net",
            "2001:db8:dead::/48",
            "::1",
            "-reject",
        ],
    );
    let start = std::time::Instant::now();
    let refused = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect("[2001:db8:dead::1]:80"),
    )
    .await;
    sh(
        "route",
        &["-n", "delete", "-inet6", "-net", "2001:db8:dead::/48"],
    );
    assert!(
        matches!(refused, Ok(Err(_))) && start.elapsed() < std::time::Duration::from_secs(2),
        "refused at once: {refused:?} after {:?}",
        start.elapsed()
    );

    // Switched off: its files and routes go.
    resolved.revert(&dev).await;
    assert!(!std::path::Path::new("/etc/resolver/acme.internal").exists());
    routing.down("acme", 0, Some(&dev)).await.unwrap();
    assert_ne!(way("10.40.1.10"), dev);
    assert_ne!(way("10.40.2.7"), dev);
    assert_ne!(way("fd40:1::10"), dev);
}

/// Every DNS server macOS' resolvers name (`scutil --dns`).
fn nameservers() -> Vec<String> {
    let mut found: Vec<String> = sh("scutil", &["--dns"])
        .lines()
        .filter(|l| l.trim_start().starts_with("nameserver["))
        .map(|l| l.split_once(':').map_or(l, |(_, s)| s).trim().to_owned())
        .collect();
    found.sort();
    found.dedup();
    found
}

/// The source of the first SYN to `dst` that comes out of the utun device
/// (each packet led by its address family), within 8 seconds.
fn syn_to(fd: std::os::fd::RawFd, dst: Ipv4Addr) -> Option<Ipv4Addr> {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let mut buf = [0u8; 2048];
    while std::time::Instant::now() < end {
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one pollfd; then a read into `buf`, as long as it is.
        let n = unsafe {
            if libc::poll(&raw mut poll, 1, 500) <= 0 {
                continue;
            }
            libc::read(fd, buf.as_mut_ptr().cast(), buf.len())
        };
        let Ok(n) = usize::try_from(n) else { continue };
        if n < 24 {
            continue;
        }
        let ip = &buf[4..n];
        let ihl = usize::from(ip[0] & 0x0f) * 4;
        let to = Ipv4Addr::new(ip[16], ip[17], ip[18], ip[19]);
        // TCP, and its flags say SYN.
        if ip[0] >> 4 == 4
            && ip[9] == 6
            && to == dst
            && ip.get(ihl + 13).is_some_and(|f| f & 0x02 != 0)
        {
            return Some(Ipv4Addr::new(ip[12], ip[13], ip[14], ip[15]));
        }
    }
    None
}

/// A DNS answer to `query`: its question, one A record, 10.40.1.10.
fn answer(query: &[u8]) -> Vec<u8> {
    // The question ends after its name's labels, its type and class.
    let mut end = 12;
    while query[end] != 0 {
        end += 1 + usize::from(query[end]);
    }
    let mut a = query[..end + 5].to_vec();
    a[2] = 0x81; // a response, recursion desired
    a[3] = 0x80; // recursion available, no error
    a[6..8].copy_from_slice(&1u16.to_be_bytes()); // one answer
    a[8..12].fill(0); // nothing else (an EDNS record left behind)
    // The name by pointer to the question, A, IN, TTL 30, 4 bytes.
    a.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 30, 0, 4, 10, 40, 1, 10]);
    a
}

/// Probing's view of the machine: a new connection that gets no answer
/// shows up as one stuck in SYN_SENT (no root needed).
#[tokio::test]
async fn the_connection_table_sees_a_stuck_connection() {
    use craked::conntrack::{Events, Kind, TcpState};
    let mut events = Events::open().unwrap();
    // TEST-NET-1: nothing answers there.
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    let connecting = tokio::spawn(socket.connect("192.0.2.1:9".parse().unwrap()));
    let seen = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let batch = events.next().await.unwrap();
            if let Some(e) = batch
                .iter()
                .find(|e| e.flow.dst == Ipv4Addr::new(192, 0, 2, 1) && e.flow.dport == 9)
            {
                return (e.kind, e.state);
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the new connection in the table\n{}", layout()));
    assert_eq!(seen, (Kind::New, Some(TcpState::SynSent)));
    connecting.abort();
}

/// Where 192.0.2.1 and port 9 sit in the kernel's records, for when the
/// table can't be read: each record (offset, length, kind), and the
/// offsets of the address and port within the one that has them.
fn layout() -> String {
    let name = c"net.inet.tcp.pcblist_n";
    let mut len = 0;
    // SAFETY: a size query, then a buffer of that size.
    let buf = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        );
        len += 65536;
        let mut buf = vec![0u8; len];
        libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        );
        buf.truncate(len);
        buf
    };
    let mut out = format!("{} bytes\n", buf.len());
    let mut at = 24;
    while at + 24 < buf.len() {
        let rec_len = u32::from_ne_bytes(buf[at..at + 4].try_into().unwrap()) as usize;
        let kind = u32::from_ne_bytes(buf[at + 4..at + 8].try_into().unwrap());
        let rec = &buf[at..(at + rec_len).min(buf.len())];
        if let Some(i) = rec.windows(4).position(|w| w == [192, 0, 2, 1]) {
            let ports: Vec<usize> = rec
                .windows(2)
                .enumerate()
                .filter(|(_, w)| *w == [0, 9])
                .map(|(i, _)| i)
                .collect();
            out += &format!(
                "record at {at}: len {rec_len} kind {kind:#x}: 192.0.2.1 at +{i}, port 9 at {ports:?}\n"
            );
            out += &format!("  {:02x?}\n", &rec[..rec.len().min(120)]);
        }
        if rec_len < 8 {
            break;
        }
        at += rec_len.next_multiple_of(8);
    }
    out
}
