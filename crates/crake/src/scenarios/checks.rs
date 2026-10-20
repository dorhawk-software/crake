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
/// One nobody asks for: it stays direct until someone does.
const UNTOUCHED: &str = "10.40.1.12";
/// A name public DNS doesn't know (.example) but ACME's DNS does.
const RESCUED: (&str, &str) = ("portal.acme.example", "10.40.1.14");
/// How quickly a name kept private is refused: at once, not after a
/// public DNS server's timeout.
const AT_ONCE: Duration = Duration::from_secs(2);
/// How long the system gets to have its DNS back after craked's end.
#[cfg(unix)]
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
pub(super) async fn nothing_is_routed_up_front(up: &mut Up) {
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
}

/// A host that doesn't answer directly but does through the tunnel:
/// probed, learned, said in the log, and the app's own retry gets through.
pub(super) async fn a_host_is_learned_by_probing(up: &mut Up) {
    let probed = page_within(GIT).await;
    assert!(
        probed.is_some_and(|b| b.contains("crex")),
        "{GIT} through {}\n{}",
        up.label,
        platform::diagnose(&up.device)
    );
    assert_eq!(way(GIT), up.device, "{GIT}'s way");
    let said = format!("Sent {GIT} through {}.", up.label);
    up.app
        .within(WAIT, "the probe's decision", |m| {
            m.log.iter().any(|e| e.message == said)
        })
        .await;
}

/// A name only the tunnel's DNS knows, asked as an app asks: its address
/// goes through the tunnel before the app's first packet.
pub(super) async fn a_tunnel_name_is_routed_before_its_first_packet(up: &mut Up) {
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
pub(super) async fn the_internet_stays_direct(up: &mut Up) {
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
pub(super) async fn public_names_resolve(_up: &mut Up) {
    names_work("while a tunnel is up", WAIT).await;
}

/// A name public DNS doesn't know but the tunnel's does: craked asks the
/// tunnels, and its address is routed before the first packet.
pub(super) async fn a_name_public_dns_doesnt_know_is_rescued(up: &mut Up) {
    let (name, at) = RESCUED;
    let found = look_up(name).await;
    assert!(
        found.iter().any(|a| a.to_string() == at),
        "{name}: {found:?}\n{}",
        platform::diagnose(&up.device)
    );
    assert_eq!(way(at), up.device, "{at}'s way, before any connection");
    assert!(page(at).await.is_ok_and(|b| b.contains("crex")), "{name}");
}

/// A name asked over TCP: held like one over UDP until its address is
/// routed.
pub(super) async fn a_name_asked_over_tcp_is_routed(up: &mut Up) {
    let answer = tokio::task::spawn_blocking(|| platform::tcp_lookup("git.acme.internal"))
        .await
        .unwrap();
    assert!(answer.contains(GIT), "git.acme.internal over TCP: {answer}");
    assert_eq!(way(GIT), up.device, "{GIT}'s way, before any connection");
}

/// Settings → Diagnostics → "Save a report…" while the tunnel is up: the
/// internet direct, the tunnel's names through it, the system's DNS, its
/// routes with Crake's marked, Crake's rules. Printed whole for the log.
pub(super) async fn a_report_says_where_things_go(up: &mut Up) {
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
}

/// Switched off: the tunnel's network is gone again, and so is what Crake
/// put in place for it.
pub(super) async fn switching_off_leaves_nothing(up: &mut Up) {
    let _ = page_within(GIT).await;
    up.off().await;
    assert!(!answers(GIT).await, "{GIT} answered with the tunnel off");
    let left = platform::left_behind(&up.device);
    assert!(left.trim().is_empty(), "left behind:\n{left}");
}

/// Switched off, the internet still works by name.
pub(super) async fn names_resolve_after_switching_off(up: &mut Up) {
    up.off().await;
    names_work("after switching off", WAIT).await;
}

/// With its tunnel off, a name only it knows is refused at once (Keep
/// internal names private), never sent to public DNS.
pub(super) async fn names_are_kept_private_while_off(up: &mut Up) {
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

/// craked stopping with the tunnel up (the service stopped, a reboot):
/// the system is as it was.
#[cfg(unix)]
pub(super) async fn stopping_craked_leaves_nothing(up: &mut Up) {
    let _ = page_within(GIT).await;
    up.crake.stop().await;
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

#[cfg(windows)]
pub(super) async fn stopping_craked_leaves_nothing(_up: &mut Up) {
    super::not_here("the service stays for the next test; scripts/windows-vm-test.sh stops it");
}
