//! The scenarios, each run against every kind of gateway on every system
//! (`every_scenario!`), starting from a tunnel that's up (`Up::start`).

use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::platform::{self, PUBLIC, PUBLIC_NAME, answers, look_up, way};
use super::{Up, WAIT};
use crate::test_app::get;

/// ACME's hosts (scripts/e2e/start.sh, ci-start.sh).
const GIT: &str = "10.40.1.10";
const WIKI: (&str, &str) = ("wiki.acme.internal", "10.40.1.11");
/// A UDP service of ACME's (an echo), only through a tunnel.
const UDP_HOST: (&str, u16) = ("10.40.1.18", 7007);
/// One the network's router turns away when asked direct.
const TURNED_AWAY: &str = "10.40.1.17";
/// One nobody asks for: it stays direct until someone does.
const UNTOUCHED: &str = "10.40.1.12";
/// A name public DNS doesn't know (.example) but ACME's DNS does.
const RESCUED: (&str, &str) = ("portal.acme.example", "10.40.1.14");
/// How quickly a name kept private is refused: at once, not after a
/// public DNS server's timeout.
const AT_ONCE: Duration = Duration::from_secs(2);
/// How long the system gets to have its DNS back after craked's end.
const BACK: Duration = Duration::from_secs(45);

/// `ip`'s page, retried until it answers (the first try may wait for the
/// probe), or None after `WAIT`.
async fn page_within(ip: &'static str) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        let end = Instant::now() + WAIT;
        while Instant::now() < end {
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

/// `ip`'s page at once (5 s), on a fresh connection.
async fn page(ip: &'static str) -> std::io::Result<String> {
    tokio::task::spawn_blocking(move || get(ip, Duration::from_secs(5)))
        .await
        .unwrap()
}

/// The internet works by name within `wait`.
async fn names_work(what: &str, wait: Duration) {
    let end = Instant::now() + wait;
    while !tokio::task::spawn_blocking(platform::public_name_answers)
        .await
        .unwrap()
    {
        assert!(
            Instant::now() < end,
            "public names don't resolve {what}: {PUBLIC_NAME:?}"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Switched on, the gateway's whole network is a hint, not a route: a
/// host nobody asked for still goes the way it went.
pub(crate) async fn nothing_is_routed_up_front(up: &mut Up) {
    let way = way(UNTOUCHED);
    assert_ne!(
        way,
        up.device,
        "{UNTOUCHED} was routed before anyone asked\n{}",
        platform::diagnose(&up.device)
    );
    let address = up
        .app
        .model
        .tunnels
        .iter()
        .find(|t| t.name == up.name)
        .map(|t| t.address.clone())
        .unwrap_or_default();
    assert!(address.starts_with("10.40."), "its address: {address:?}");
    // What the gateway pushed is shown as hints (tunnel detail's "From the
    // gateway"): ACME's network; the IPsec gateway also hands out
    // 10.42.0.0/16, in a second pair of selectors charon reports apart.
    #[cfg(target_os = "linux")]
    let ipsec = up.gateway == super::Gateway::Ipsec;
    #[cfg(not(target_os = "linux"))]
    let ipsec = false;
    let hints: &[&str] = if ipsec {
        &["10.40.0.0/16", "10.42.0.0/16"]
    } else {
        &["10.40.0.0/16"]
    };
    let name = up.name;
    let shown = |m: &crate::model::Model| {
        m.tunnels
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.split_include.clone())
            .unwrap_or_default()
    };
    up.app
        .within(WAIT, "the gateway's networks as hints", |m| {
            let s = shown(m);
            hints.iter().all(|h| s.iter().any(|n| n == h))
        })
        .await;
}

/// A host that doesn't answer directly but does through the tunnel:
/// probed, learned, said in the log, and the app's own retry gets through.
pub(crate) async fn a_host_is_learned_by_probing(up: &mut Up) {
    let probed = page_within(GIT).await;
    if !probed.is_some_and(|b| b.contains("crex")) {
        // What craked said meanwhile, applied.
        up.app.settle(Duration::from_secs(1)).await;
        panic!(
            "{GIT} through {}\n{}\ncraked said:\n{}",
            up.label,
            platform::diagnose(&up.device),
            up.said()
        );
    }
    assert_eq!(way(GIT), up.device, "{GIT}'s way");
    let said = format!("Sent {GIT} through {}.", up.label);
    up.app
        .within(WAIT, "the probe's decision", |m| {
            m.log.iter().any(|e| e.message == said)
        })
        .await;
}

/// A UDP service only the tunnel reaches (10.40.1.18:7007, an echo):
/// its first datagrams get no answer directly, the app's own datagram is
/// tried through the tunnel, the host is learned, and the app's next
/// datagram is answered (Linux: craked follows UDP conversations in
/// conntrack; macOS and Windows learn TCP hosts only, and Cloudflare
/// carries no UDP to the CI gateways).
pub(crate) async fn a_udp_host_is_learned(up: &mut Up) {
    if !cfg!(target_os = "linux") {
        super::not_here("UDP hosts are learned on Linux only");
        return;
    }
    let echoed = tokio::task::spawn_blocking(|| {
        let s = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
        s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut buf = [0u8; 64];
        let end = Instant::now() + WAIT;
        while Instant::now() < end {
            // As an app retrying: the same socket, a datagram a second.
            s.send_to(b"crex?", (UDP_HOST.0, UDP_HOST.1)).unwrap();
            if let Ok((n, _)) = s.recv_from(&mut buf) {
                return Some(buf[..n].to_vec());
            }
        }
        None
    })
    .await
    .unwrap();
    assert_eq!(
        echoed.as_deref(),
        Some(&b"crex?"[..]),
        "{}:{} through {}\ncraked said:\n{}",
        UDP_HOST.0,
        UDP_HOST.1,
        up.label,
        up.said()
    );
    assert_eq!(way(UDP_HOST.0), up.device, "{}'s way", UDP_HOST.0);
    let said = format!("Sent {} through {}.", UDP_HOST.0, up.label);
    up.app
        .within(WAIT, "the UDP host's decision", |m| {
            m.log.iter().any(|e| e.message == said)
        })
        .await;
}

/// A host the network's router turns away ("host unreachable", as the
/// e2e gateway does for 10.40.1.17 from the wire): the probe starts at
/// once, not after the wait, and says why (Linux: craked reads routers'
/// ICMP in its queue; macOS and Windows probe on the timeout alone, and
/// their path to ACME's network isn't the test gateway's).
pub(crate) async fn a_routers_unreachable_starts_the_probe(up: &mut Up) {
    if !cfg!(target_os = "linux") {
        super::not_here("routers' ICMP is read on Linux only, and only its gateway sends one");
        return;
    }
    let probed = page_within(TURNED_AWAY).await;
    assert!(
        probed.is_some_and(|b| b.contains("crex")),
        "{TURNED_AWAY} through {}\ncraked said:\n{}",
        up.label,
        up.said()
    );
    assert_eq!(way(TURNED_AWAY), up.device, "{TURNED_AWAY}'s way");
    let said = format!("Sent {TURNED_AWAY} through {}.", up.label);
    up.app
        .within(
            WAIT,
            "the decision, saying the router turned it away",
            |m| {
                m.log.iter().any(|e| {
                    e.message == said
                        && e.reason
                            .as_deref()
                            .is_some_and(|r| r.starts_with("Unreachable directly."))
                })
            },
        )
        .await;
}

/// A name only the tunnel's DNS knows, asked as an app asks: its address
/// goes through the tunnel before the app's first packet.
pub(crate) async fn a_tunnel_name_is_routed_before_its_first_packet(up: &mut Up) {
    let (name, at) = WIKI;
    let found = look_up(name).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{name}: {found:?}\n{}",
        platform::diagnose(&up.device)
    );
    assert_eq!(way(at), up.device, "{at}'s way, before any connection");
    assert!(page(at).await.is_ok_and(|b| b.contains("crex")), "{name}");
}

/// The internet answers, and not through the tunnel (OpenVPN's gateway
/// pushes redirect-gateway; Crake leaves it).
pub(crate) async fn the_internet_stays_direct(up: &mut Up) {
    let public = tokio::task::spawn_blocking(|| {
        TcpStream::connect_timeout(&PUBLIC.parse().unwrap(), Duration::from_secs(10))
    })
    .await
    .unwrap();
    assert!(public.is_ok(), "{PUBLIC} answered: {public:?}");
    let host = PUBLIC.split(':').next().unwrap();
    let way = way(host);
    assert!(
        !way.is_empty() && way != up.device,
        "{host} goes {way:?}, not through {}",
        up.device
    );
}

/// Public names still resolve while a tunnel is up ("my internet
/// stopped" is almost always names: craked stands in for the system's
/// DNS on Windows and macOS).
pub(crate) async fn public_names_resolve(_up: &mut Up) {
    names_work("while a tunnel is up", WAIT).await;
}

/// A name public DNS doesn't know but the tunnel's does: craked asks the
/// tunnels, and its address is routed before the first packet.
pub(crate) async fn a_name_public_dns_doesnt_know_is_rescued(up: &mut Up) {
    let (name, at) = RESCUED;
    let found = look_up(name).await;
    if !found.iter().any(|a| a.to_string() == at) {
        up.app.settle(Duration::from_secs(1)).await;
        panic!(
            "{name}: {found:?}\n{}\n{}\ncraked said:\n{}",
            platform::name_diagnosis(name),
            platform::diagnose(&up.device),
            up.said()
        );
    }
    assert_eq!(way(at), up.device, "{at}'s way, before any connection");
    assert!(page(at).await.is_ok_and(|b| b.contains("crex")), "{name}");
}

/// A name asked over TCP: held like one over UDP until its address is
/// routed.
pub(crate) async fn a_name_asked_over_tcp_is_routed(up: &mut Up) {
    let answer = tokio::task::spawn_blocking(|| platform::tcp_lookup("git.acme.internal"))
        .await
        .unwrap();
    assert!(answer.contains(GIT), "git.acme.internal over TCP: {answer}");
    assert_eq!(way(GIT), up.device, "{GIT}'s way, before any connection");
}

/// Settings → Diagnostics → "Save a report…" while the tunnel is up: the
/// internet direct, the tunnel's names through it, the system's DNS, its
/// routes with Crake's marked, Crake's rules. Printed whole for the log.
pub(crate) async fn a_report_says_where_things_go(up: &mut Up) {
    let label = up.label;
    // A host by name through the tunnel first: the report tries the
    // tunnel's last-used one.
    let (name, at) = WIKI;
    assert!(!look_up(name).await.is_empty(), "{name}");
    assert!(page(at).await.is_ok(), "{name}");
    let report = crate::report::gather_from(up.crake.socket()).await;
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
    assert!(part("DNS").contains(platform::REPORT_DNS), "{text}");
    assert!(part("Routes").contains("\u{2190} Crake: "), "{text}");
    assert!(!part("Crake's rules").is_empty(), "{text}");
    // "Hide names and addresses when sharing": the hosts go, the
    // computer's plumbing (the DNS Crake set up) stays.
    let hidden = crate::report::text(&report, true, &jiff::Zoned::now());
    assert!(
        !hidden.contains(name) && !hidden.contains(at),
        "{name} or {at} in a hidden report:\n{hidden}"
    );
    assert!(
        hidden.contains(platform::REPORT_DNS),
        "the DNS Crake set up, hidden:\n{hidden}"
    );
}

/// Switched off: the tunnel's network is gone again, and so is what Crake
/// put in place for it.
pub(crate) async fn switching_off_leaves_nothing(up: &mut Up) {
    let _ = page_within(GIT).await;
    up.off().await;
    assert!(!answers(GIT).await, "{GIT} answered with the tunnel off");
    let left = platform::left_behind(&up.device);
    assert!(left.trim().is_empty(), "left behind:\n{left}");
}

/// Switched off, the internet still works by name.
pub(crate) async fn names_resolve_after_switching_off(up: &mut Up) {
    up.off().await;
    names_work("after switching off", WAIT).await;
}

/// With its tunnel off, a name only it knows is refused at once (Keep
/// internal names private), never sent to public DNS.
pub(crate) async fn names_are_kept_private_while_off(up: &mut Up) {
    up.off().await;
    let before = platform::public_questions();
    let name = WIKI.0;
    let end = Instant::now() + WAIT;
    loop {
        let (found, took) = tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let found = std::net::ToSocketAddrs::to_socket_addrs(&(name, 80)).is_ok();
            (found, start.elapsed())
        })
        .await
        .unwrap();
        if !found && took < AT_ONCE {
            break;
        }
        assert!(
            Instant::now() < end,
            "{name} with its tunnel off: found {found}, after {took:?}"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if let (Some(before), Some(after)) = (before, platform::public_questions()) {
        let since = after.strip_prefix(before.as_str()).unwrap_or(&after);
        assert!(
            !since.contains(name),
            "{name} was asked of public DNS with its tunnel off:\n{since}"
        );
    }
}

/// An upgrade with the tunnel up: it comes back signed in, asking
/// nothing (IPsec: its password, which the app keeps), and what it had
/// learned goes through it again (Windows: this
/// run's installer over the running service; Linux and macOS: craked
/// handing over as the packages' upgrade scripts make it).
pub(crate) async fn an_upgrade_keeps_the_tunnel_signed_in(up: &mut Up) {
    assert!(
        page_within(GIT).await.is_some(),
        "{GIT} through {}",
        up.label
    );
    let Some(app) = up.crake.upgrade().await else {
        super::not_here("no installer to upgrade with (CRAKE_INSTALLER)");
        return;
    };
    up.app = app;
    let name = up.name;
    // IPsec's password is the app's: the new craked asks it, and the app
    // answers from the keyring (these tests keep none: answered here).
    if !up.gateway.keeps_sign_in() {
        let ask = up.app.asked("password").await;
        up.app.send(crate::client::Command::Answer(
            name.into(),
            ask["id"].parse().unwrap(),
            "hunter2".into(),
        ));
    }
    let said = "Crake was updated. Your tunnels are reconnecting with your sign-ins.";
    up.app
        .within(WAIT, "back, signed in", |m| {
            m.tunnels
                .iter()
                .any(|t| t.name == name && t.state == "connected")
                && m.log.iter().any(|e| e.message == said)
        })
        .await;
    up.device = up.crake.device(name);
    let end = Instant::now() + WAIT;
    while way(GIT) != up.device {
        assert!(
            Instant::now() < end,
            "{GIT} goes {:?} after the upgrade, not {}",
            way(GIT),
            up.device
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(answers(GIT).await, "{GIT} after the upgrade");
}

/// craked stopping with the tunnel up (the service stopped, a reboot):
/// the system is as it was.
pub(crate) async fn stopping_craked_leaves_nothing(up: &mut Up) {
    let _ = page_within(GIT).await;
    up.crake.end().await;
    let end = Instant::now() + BACK;
    loop {
        let left = platform::left_after_stop();
        if left.trim().is_empty() {
            break;
        }
        assert!(Instant::now() < end, "left behind:\n{left}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    assert!(!answers(GIT).await, "{GIT} answered with craked gone");
    names_work("after craked stopped", BACK).await;
}
