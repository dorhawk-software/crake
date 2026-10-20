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
/// The gateways' public name (the public DNS stand-in gives it
/// 198.51.100.10): ACME's organisation domain is acme.example, as a real
/// company's gateway names it (split-horizon asks its DNS about names
/// under it).
const GATEWAY_NAME: &str = "vpn.acme.example";
/// ACME's AnyConnect gateway behind a front that admits only clients
/// calling themselves AnyConnect (scripts/e2e/picky.py).
pub(crate) fn anyconnect_only_gateway() -> Option<String> {
    Some(format!("https://{GATEWAY_NAME}:8444"))
}

/// ACME's AnyConnect gateway behind Cisco's browser sign-in, played by
/// scripts/e2e/cisco_sso.py; the test's browser is scripts/e2e/client.sh's.
pub(crate) fn anyconnect_browser_gateway() -> Option<String> {
    Some(format!("https://{GATEWAY_NAME}:8446"))
}

/// ACME's AnyConnect gateway with groups to choose from (its own ocserv).
pub(crate) fn anyconnect_groups_gateway() -> Option<String> {
    Some(format!("https://{GATEWAY_NAME}:8445"))
}
/// IPsec's gateway.
pub(crate) const IPSEC_GATEWAY: &str = GATEWAY_NAME;

pub(crate) fn anyconnect_gateway() -> String {
    format!("https://{GATEWAY_NAME}")
}

pub(crate) fn config(dir: &Path) -> Config {
    Config {
        use_resolved: true,
        tunnels: vec![],
        // A tunnel's DNS asked every 5 s: an outage noticed in seconds.
        dns_check_secs: super::DNS_CHECK_SECS,
        ..crate::e2e_tests::config(dir)
    }
}

/// The OpenVPN gateway ("host port"), its transport and its CA's file.
pub(crate) fn openvpn_gateway() -> (String, &'static str, String) {
    (
        format!("{GATEWAY_NAME} 1194"),
        "udp",
        "/mnt/link/openvpn-ca.crt".into(),
    )
}

/// The device the system sends `ip` out of: a tunnel's when one of
/// Crake's sets holds it (its rule marks it for the tunnel's table),
/// else the main table's.
pub(crate) fn way(ip: &str) -> String {
    let v6 = ip.contains(':');
    for tunnel in super::super::TUNNELS {
        let sets = if v6 {
            ["learned6", "pinned6"]
        } else {
            ["learned", "pinned"]
        };
        for set in sets {
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
    let route = sh(&format!("ip -{} route get {ip}", if v6 { 6 } else { 4 }));
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

/// Questions to `ip` dropped on their way out, until it's dropped (a
/// tunnel's DNS that stopped answering).
pub(crate) struct Silenced;

pub(crate) fn silence(ip: &str) -> Silenced {
    let out = sh(&format!(
        "nft -f - <<'NFT'\ntable ip crake_test {{\n  chain out {{\n    \
         type filter hook output priority 0;\n    ip daddr {ip} drop\n  }}\n}}\nNFT"
    ));
    assert!(out.trim().is_empty(), "silencing {ip}: {out}");
    Silenced
}

impl Drop for Silenced {
    fn drop(&mut self) {
        sh("nft delete table ip crake_test");
    }
}

/// How the system answers `name` now, and craked's memory answerer.
pub(crate) fn name_diagnosis(name: &str) -> String {
    sh(&format!(
        "resolvectl query {name} 2>&1; dig +time=5 @169.254.53.54 {name} 2>&1; \
         ss -ulpn 2>&1 | grep ':53 '; ip addr show crake-keep"
    ))
}

/// The network has IPv6 of its own: a default route that isn't Crake's.
pub(crate) fn network_has_ipv6() -> bool {
    sh("ip -6 route show default")
        .lines()
        .any(|l| !l.contains("crake-") && !l.contains("table"))
}

/// The system asks for a name's IPv6 addresses only where the network
/// has IPv6 of its own (macOS' resolver).
pub(crate) const NAMES_NEED_NETWORK_IPV6: bool = false;

/// Split-horizon's names: one with a public address and a private one in
/// ACME's DNS, and one with only a public one (the public DNS stand-in's).
pub(crate) const SPLIT: (&str, &str) = ("shop.acme.example", "10.40.1.16");
pub(crate) const PUBLIC_UNDER_ORG: &str = "www.acme.example";
/// What another program does to the tunnel's DNS: openvpn3's reconnect
/// makes its link every name's DNS ("~." and the default route).
pub(crate) fn rewrite_tunnel_dns(dev: &str) -> String {
    sh(&format!(
        "resolvectl domain {dev} '~.' acme.internal && resolvectl default-route {dev} yes"
    ))
}

/// Whether the tunnel's link is Crake's again (routing-only, its domain),
/// and what resolved says.
pub(crate) fn tunnel_dns_is_crakes(dev: &str) -> (bool, String) {
    let link = sh(&format!("resolvectl status {dev}"));
    // "Default Route: no", or (older resolvectl) "-DefaultRoute".
    let back = !link.contains("Default Route: yes")
        && !link.contains("+DefaultRoute")
        && link.contains("~acme.internal")
        && !link.contains("~.");
    (back, link)
}

/// The system's DNS cache emptied (resolved's).
pub(crate) fn flush_dns_cache() {
    sh("resolvectl flush-caches");
}
