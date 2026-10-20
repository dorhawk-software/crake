//! Linux: the e2e containers (scripts/e2e-test.sh). craked with real
//! nftables, policy routing and the container's own systemd-resolved
//! (scripts/e2e/client.sh starts it, with the gateway's public DNS
//! stand-in, 203.0.113.53, as the network's DNS).

use std::path::Path;

use craked::config::Config;

use super::sh;

/// A public host that answers only directly (scripts/e2e/start.sh), and
/// a public name the public DNS stand-in gives it.
pub(crate) const PUBLIC: &str = "203.0.113.80:80";
pub(crate) const PUBLIC_NAME: (&str, u16) = ("one.one.one.one", 80);
/// What the report's DNS part names while a tunnel is up: the tunnel's
/// DNS, registered with resolved.
pub(crate) const REPORT_DNS: &str = "10.40.0.2";
/// What the gateway's public DNS stand-in was asked (dnsmasq's log).
const PUBLIC_DNS_LOG: &str = "/mnt/link/public-dns.log";

pub(crate) fn config(dir: &Path) -> Config {
    Config {
        use_resolved: true,
        tunnels: vec![],
        ..crate::e2e_tests::config(dir)
    }
}

/// The OpenVPN gateway ("host port"), its transport and its CA's file.
pub(crate) fn openvpn_gateway() -> (String, &'static str, String) {
    (
        "198.51.100.10 1194".into(),
        "udp",
        "/mnt/link/openvpn-ca.crt".into(),
    )
}

/// The device the system sends `ip` out of: a tunnel's when one of
/// Crake's sets holds it (its rule marks it for the tunnel's table),
/// else the main table's.
pub(crate) fn way(ip: &str) -> String {
    for tunnel in super::super::TUNNELS {
        for set in ["learned", "pinned"] {
            let found = std::process::Command::new("nft")
                .args([
                    "get",
                    "element",
                    "inet",
                    "crake",
                    &format!("{set}_{tunnel}"),
                    &format!("{{ {ip} }}"),
                ])
                .output()
                .is_ok_and(|o| o.status.success());
            if found {
                return format!("crake-{tunnel}");
            }
        }
    }
    let route = sh(&format!("ip -4 route get {ip}"));
    route
        .split_whitespace()
        .skip_while(|w| *w != "dev")
        .nth(1)
        .unwrap_or_default()
        .to_owned()
}

/// `name` asked over TCP of the tunnel's DNS, as an app asking it
/// directly would (craked holds the answer until its addresses are
/// routed, `dns_tcp.rs`).
pub(crate) fn tcp_lookup(name: &str) -> String {
    super::dns_over_tcp("10.40.0.2", name)
}

/// What the network's public DNS has been asked so far, one question a
/// line (None where the test can't see it).
pub(crate) fn public_questions() -> Option<String> {
    let log = std::fs::read_to_string(PUBLIC_DNS_LOG).unwrap_or_default();
    Some(
        log.lines()
            .filter(|l| l.contains("query["))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// What the system makes of the tunnel, for a failure's message.
pub(crate) fn diagnose(device: &str) -> String {
    sh(&format!(
        "ip -o addr show {device}; ip rule; ip route show table all | grep -v '^local'; \
         nft list ruleset; resolvectl status"
    ))
}

/// What a tunnel switched off left of itself.
pub(crate) fn left_behind(device: &str) -> String {
    let mut left = sh(&format!("ip -o link show {device} 2>/dev/null"));
    left += &sh(&format!("ip route show table all | grep -w {device}"));
    left
}

/// What craked left behind once it stopped.
pub(crate) fn left_after_stop() -> String {
    let mut left = String::new();
    let tables = sh("nft list tables");
    if tables.contains("crake") {
        left += &tables;
    }
    left += &sh("ip -o link | grep crake-");
    left
}
