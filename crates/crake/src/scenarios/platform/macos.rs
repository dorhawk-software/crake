//! macOS: CI's Mac, the gateways through Cloudflare
//! (scripts/ci-through-cloudflare.sh), craked in the test as root.

use std::path::Path;

use craked::config::Config;

use super::sh;

/// A public address that answers anywhere, and a public name.
pub(crate) const PUBLIC: &str = "1.1.1.1:443";
pub(crate) const PUBLIC_NAME: (&str, u16) = ("one.one.one.one", 443);
/// craked's DNS server stands in for the system's while a tunnel is up.
pub(crate) const REPORT_DNS: &str = "127.53.0.1";

pub(crate) fn config(dir: &Path) -> Config {
    let hook = std::env::var("CRAKE_E2E_HOOK").expect("CRAKE_E2E_HOOK: craked's crake-hook");
    Config {
        hook_path: hook.into(),
        hook_socket: dir.join("hook.sock"),
        log_path: dir.join("log"),
        state_path: dir.join("hosts.json"),
        probe_wait_secs: 1,
        race_first_syn: false,
        managed_dir: dir.join("managed.d"),
        // A tunnel's DNS asked every 5 s: an outage noticed in seconds.
        dns_check_secs: super::DNS_CHECK_SECS,
        ..Config::default()
    }
}

/// The OpenVPN gateway through Cloudflare ("host port"), over TCP.
pub(crate) fn openvpn_gateway() -> (String, &'static str, String) {
    (
        std::env::var("CRAKE_GATEWAY_OPENVPN").expect("CRAKE_GATEWAY_OPENVPN"),
        "tcp-client",
        std::env::var("CRAKE_GATEWAY_CA").expect("CRAKE_GATEWAY_CA"),
    )
}

/// The device the system sends `ip` out of.
pub(crate) fn way(ip: &str) -> String {
    let family = if ip.contains(':') { "-inet6 " } else { "" };
    sh(&format!("route -n get {family}{ip}"))
        .lines()
        .find_map(|l| l.trim().strip_prefix("interface: ").map(str::to_owned))
        .unwrap_or_default()
}

/// `name` asked over TCP of craked's DNS server, where macOS sends the
/// tunnel's domains (/etc/resolver).
pub(crate) fn tcp_lookup(name: &str) -> String {
    super::dns_over_tcp("127.53.0.1", name)
}

/// The system's DNS servers are the network's own: the test can't see
/// what they were asked.
pub(crate) fn public_questions() -> Option<String> {
    None
}

pub(crate) fn diagnose(device: &str) -> String {
    sh(&format!(
        "route -n get 10.40.1.10; ifconfig {device}; netstat -rn -f inet | head -40; \
         scutil --dns | head -60; cat /etc/resolver/* 2>/dev/null"
    ))
}

/// Crake's NAT rule left in pf (the Doctor saw one stay with the tunnel
/// off and craked gone, and pf on, 2026-10-18).
fn nat_left() -> String {
    let nat = sh("pfctl -a com.apple/250.Crake -s nat 2>/dev/null");
    if nat.contains("nat on") {
        nat
    } else {
        String::new()
    }
}

pub(crate) fn left_behind(device: &str) -> String {
    let mut left = nat_left();
    let dev = sh(&format!("ifconfig {device} 2>&1"));
    if dev.contains("flags=") {
        left += &dev;
    }
    left
}

pub(crate) fn left_after_stop() -> String {
    let mut left = nat_left();
    left += &sh("grep -l 'Written by Crake' /etc/resolver/* 2>/dev/null");
    // The system's own resolver (#1, every name), not the ones per domain.
    let dns = sh("scutil --dns");
    let primary = dns.split("resolver #2").next().unwrap_or_default();
    if primary.contains("127.53.0.1") {
        left += primary;
    }
    left
}

/// Where the test's own pf rules go (macOS' pf.conf takes every anchor
/// under com.apple; craked's own is 250.Crake).
const TEST_ANCHOR: &str = "com.apple/251.CrakeTest";

/// Packets to `ip` dropped on their way out, until it's dropped (a
/// tunnel's DNS that stopped answering). pf is on while a tunnel is up.
pub(crate) struct Silenced;

pub(crate) fn silence(ip: &str) -> Silenced {
    let out = sh(&format!(
        "echo 'block drop out quick proto {{ tcp udp }} from any to {ip}' | pfctl -a {TEST_ANCHOR} -f - 2>&1"
    ));
    let rules = sh(&format!("pfctl -a {TEST_ANCHOR} -s rules 2>/dev/null"));
    assert!(rules.contains(ip), "silencing {ip}: {out}");
    Silenced
}

impl Drop for Silenced {
    fn drop(&mut self) {
        sh(&format!("pfctl -a {TEST_ANCHOR} -F rules 2>/dev/null"));
    }
}

/// How the system answers `name` now, and craked's DNS server.
pub(crate) fn name_diagnosis(name: &str) -> String {
    sh(&format!(
        "dscacheutil -q host -a name {name}; dig +time=5 @127.53.0.1 {name} 2>&1"
    ))
}

/// The network has IPv6 of its own: a default route that isn't a utun's.
pub(crate) fn network_has_ipv6() -> bool {
    sh("route -n get -inet6 default 2>/dev/null")
        .lines()
        .find_map(|l| l.trim().strip_prefix("interface: ").map(str::to_owned))
        .is_some_and(|i| !i.starts_with("utun"))
}

/// The system asks for a name's IPv6 addresses only where the network
/// has IPv6 of its own (macOS' resolver).
pub(crate) const NAMES_NEED_NETWORK_IPV6: bool = true;

/// Split-horizon's names: one with a public address and a private one in
/// ACME's DNS, and one with only a public one (the internet's (the gateway is named under dorhawk.software here, scripts/ci-through-cloudflare.sh)).
pub(crate) const SPLIT: (&str, &str) = ("packages.dorhawk.software", "10.40.1.16");
pub(crate) const PUBLIC_UNDER_ORG: &str = "dorhawk.software";
/// Split-horizon with ACME's own public address: a name public DNS gives
/// one public address and ACME's DNS another (companies.sh), which
/// answers only through the tunnel.
pub(crate) const OTHER_PUBLIC: (&str, &str) = ("www.dorhawk.software", "192.0.2.40");
/// What another program does to the tunnel's DNS: a VPN client clearing
/// /etc/resolver takes Crake's file for ACME's domain.
pub(crate) fn rewrite_tunnel_dns(_dev: &str) -> String {
    sh("rm /etc/resolver/acme.internal")
}

/// Whether the file is Crake's again, and what's there.
pub(crate) fn tunnel_dns_is_crakes(dev: &str) -> (bool, String) {
    let file = sh("cat /etc/resolver/acme.internal 2>&1");
    (file.contains(&format!("Written by Crake for {dev}")), file)
}

/// The system's DNS cache emptied (mDNSResponder's).
pub(crate) fn flush_dns_cache() {
    sh("dscacheutil -flushcache; killall -HUP mDNSResponder");
}
