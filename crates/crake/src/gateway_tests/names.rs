//! The gateway tests' names: the internet by name, a name public DNS
//! doesn't know answered by the tunnel's, a name asked over TCP, and a
//! tunnel's names kept private while it's off.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs as _};
use std::time::Duration;

use super::{PUBLIC_NAME, WAIT, diagnose, diagnose_name, sh};
use crate::test_app::get;

/// A name public DNS doesn't know (.example) but ACME's DNS does
/// (scripts/e2e/ci-start.sh): craked asks the tunnels for it.
const RESCUED: &str = "portal.acme.example";
const RESCUED_AT: &str = "10.40.1.14";
/// How quickly a name kept private is refused: at once, not after a
/// public DNS server's timeout.
const AT_ONCE: Duration = Duration::from_secs(2);
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
pub(super) async fn names_work(what: &str, wait: Duration) {
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

/// A name public DNS doesn't know but the tunnel's does, and a name asked
/// over TCP (craked's DNS server takes both, on Windows and macOS).
pub(super) async fn names_hold(device: &str) {
    let (addr, page) = tokio::task::spawn_blocking(|| {
        let addr: Vec<SocketAddr> = (RESCUED, 80)
            .to_socket_addrs()
            .map(Iterator::collect)
            .unwrap_or_default();
        (addr, get(RESCUED_AT, Duration::from_secs(5)))
    })
    .await
    .unwrap();
    assert!(
        addr.iter().any(|a| a.ip().to_string() == RESCUED_AT),
        "{RESCUED}: {addr:?}\n{}",
        diagnose_name(device)
    );
    assert!(
        page.is_ok_and(|b| b.contains("crex")),
        "{RESCUED} at once through the tunnel\n{}",
        diagnose(device)
    );
    let tcp = tokio::task::spawn_blocking(|| tcp_lookup("git.acme.internal"))
        .await
        .unwrap();
    assert!(
        tcp.contains("10.40.1.10"),
        "git.acme.internal over TCP: {tcp}"
    );
}

/// `name`'s addresses, asked over TCP of the DNS the system asks for it.
#[cfg(windows)]
fn tcp_lookup(name: &str) -> String {
    sh(&format!(
        "(Resolve-DnsName {name} -Type A -TcpOnly -DnsOnly -ErrorAction Continue).IPAddress"
    ))
}
#[cfg(target_os = "macos")]
fn tcp_lookup(name: &str) -> String {
    sh(&format!("dig +tcp +short +time=5 @127.53.0.1 {name}"))
}

/// With its tunnel off, a name only it knows is refused at once (Keep
/// internal names private), never left to time out at a public server.
pub(super) async fn kept_private(name: &'static str) {
    let end = std::time::Instant::now() + WAIT;
    loop {
        let (failed, took) = tokio::task::spawn_blocking(move || {
            let start = std::time::Instant::now();
            let failed = (name, 80).to_socket_addrs().is_err();
            (failed, start.elapsed())
        })
        .await
        .unwrap();
        if failed && took < AT_ONCE {
            return;
        }
        assert!(
            std::time::Instant::now() < end,
            "{name} with its tunnel off: failed {failed}, after {took:?}"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
