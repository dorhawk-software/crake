//! Windows, as an administrator (CI's Windows runner): a public host Crake
//! learned by mistake is a route through its tunnel, which craked's own
//! sockets follow too; the re-check's direct mark binds to the default
//! route's interface instead (`recheck_public`). Here a public address is
//! routed into the loopback interface, where nothing answers for it: a
//! plain connection gets nowhere, a direct one gets through. Skipped unless
//! CRAKE_WINDOWS_ADMIN=1.
#![cfg(windows)]

use std::process::Command;
use std::time::Duration;

fn ps(cmd: &str) -> String {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", cmd])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

#[tokio::test]
async fn the_direct_mark_passes_a_learned_route() {
    if std::env::var("CRAKE_WINDOWS_ADMIN").as_deref() != Ok("1") {
        eprintln!("skipped: needs an administrator (CRAKE_WINDOWS_ADMIN=1)");
        return;
    }
    let loopback = "(Get-NetIPInterface -AddressFamily IPv4 | Where-Object InterfaceAlias -like 'Loopback*' | Select-Object -First 1).InterfaceIndex";
    let added = ps(&format!(
        "New-NetRoute -DestinationPrefix 1.1.1.1/32 -InterfaceIndex ({loopback}) -NextHop 0.0.0.0 -PolicyStore ActiveStore -RouteMetric 0"
    ));
    let addr = std::net::SocketAddr::from(([1, 1, 1, 1], 443));
    let plain = tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(addr))
        .await
        .is_ok_and(|c| c.is_ok());
    let direct =
        craked::probe::through_tunnel(addr, craked::plan::DIRECT_MARK, Duration::from_secs(10))
            .await;
    ps("Remove-NetRoute -DestinationPrefix 1.1.1.1/32 -Confirm:$false");
    assert!(
        !plain,
        "a plain connection went around the loopback route: {added}"
    );
    assert!(direct.is_some(), "the direct mark didn't get through");
}
