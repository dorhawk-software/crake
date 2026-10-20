//! Windows: CI's Windows runner, the installed service, the gateways
//! through Cloudflare (scripts/ci-through-cloudflare.sh).

use super::sh;

/// A public address that answers anywhere, and a public name.
pub(crate) const PUBLIC: &str = "1.1.1.1:443";
pub(crate) const PUBLIC_NAME: (&str, u16) = ("one.one.one.one", 443);
/// craked's DNS server, where NRPT sends names while a tunnel is up.
pub(crate) const REPORT_DNS: &str = "127.53.0.1";

/// The OpenVPN gateway through Cloudflare ("host port"), over TCP.
pub(crate) fn openvpn_gateway() -> (String, &'static str, String) {
    (
        std::env::var("CRAKE_GATEWAY_OPENVPN").expect("CRAKE_GATEWAY_OPENVPN"),
        "tcp-client",
        std::env::var("CRAKE_GATEWAY_CA").expect("CRAKE_GATEWAY_CA"),
    )
}

/// The interface the system sends `ip` out of.
pub(crate) fn way(ip: &str) -> String {
    sh(&format!(
        "(Find-NetRoute -RemoteIPAddress {ip} | Select-Object -First 1).InterfaceAlias"
    ))
    .trim()
    .to_owned()
}

/// `name` asked over TCP, of whatever DNS Windows asks for it (NRPT:
/// craked's server).
pub(crate) fn tcp_lookup(name: &str) -> String {
    sh(&format!(
        "(Resolve-DnsName {name} -Type A -TcpOnly -DnsOnly -ErrorAction Continue).IPAddress"
    ))
}

/// The system's DNS servers are the network's own: the test can't see
/// what they were asked.
pub(crate) fn public_questions() -> Option<String> {
    None
}

pub(crate) fn diagnose(device: &str) -> String {
    sh(&format!(
        "Find-NetRoute -RemoteIPAddress 10.40.1.10 | Format-List | Out-String; \
         Get-NetIPAddress -InterfaceAlias {device} -ErrorAction SilentlyContinue | Format-Table | Out-String; \
         Get-NetRoute -InterfaceAlias {device} -ErrorAction SilentlyContinue | Format-Table | Out-String; \
         'NRPT:'; Get-DnsClientNrptRule | Format-Table Namespace,NameServers -AutoSize | Out-String; \
         'decisions:'; Get-Content C:\\ProgramData\\Crake\\log -Tail 15 | Out-String; \
         'craked:'; Get-Content C:\\ProgramData\\Crake\\craked.log -Tail 40 | Out-String"
    ))
}

/// The tunnel's address still on its adapter.
pub(crate) fn left_behind(device: &str) -> String {
    sh(&format!(
        "Get-NetIPAddress -InterfaceAlias {device} -AddressFamily IPv4 -ErrorAction SilentlyContinue | \
         Where-Object IPAddress -like '10.40.*' | Format-Table | Out-String"
    ))
    .trim()
    .to_owned()
}

/// The test's firewall rule's name.
const TEST_RULE: &str = "CrakeTestSilenced";

/// Packets to `ip` dropped on their way out by a firewall rule, until
/// it's dropped (a tunnel's DNS that stopped answering).
pub(crate) struct Silenced;

pub(crate) fn silence(ip: &str) -> Silenced {
    let out = sh(&format!(
        "Remove-NetFirewallRule -Name {TEST_RULE} -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -Name {TEST_RULE} -DisplayName 'Crake test: {ip} silent' \
         -Direction Outbound -RemoteAddress {ip} -Action Block | Out-Null; \
         Get-NetFirewallProfile | Format-Table Name,Enabled | Out-String"
    ));
    println!("silenced {ip}; the firewall's profiles:{out}");
    Silenced
}

impl Drop for Silenced {
    fn drop(&mut self) {
        sh(&format!(
            "Remove-NetFirewallRule -Name {TEST_RULE} -ErrorAction SilentlyContinue"
        ));
    }
}

/// Whether ACME's DNS answers `name` with `at` when asked directly, as a
/// check that a block on it took.
pub(crate) fn acme_dns_answers(name: &str, at: &str) -> bool {
    sh(&format!(
        "(Resolve-DnsName {name} -Server {} -Type A -DnsOnly -QuickTimeout -ErrorAction SilentlyContinue).IPAddress",
        super::ACME_DNS
    ))
    .contains(at)
}

/// How the system answers `name` now, and craked's DNS server.
pub(crate) fn name_diagnosis(name: &str) -> String {
    sh(&format!(
        "Resolve-DnsName {name} -ErrorAction Continue | Out-String; \
         foreach ($t in 'A', 'AAAA') {{ \
           \"$t of craked's DNS server:\"; \
           Resolve-DnsName {name} -Type $t -Server 127.53.0.1 -DnsOnly -ErrorAction Continue | Out-String }}; \
         Get-DnsClientCache -Entry {name} -ErrorAction SilentlyContinue | Out-String"
    ))
}

/// The network has IPv6 of its own: a default route that isn't Crake's.
pub(crate) fn network_has_ipv6() -> bool {
    !sh("Get-NetRoute -DestinationPrefix ::/0 -ErrorAction SilentlyContinue | \
         Where-Object InterfaceAlias -notlike 'crake-*' | Select-Object -First 1 | Format-List | Out-String")
    .trim()
    .is_empty()
}

/// What the service left behind once it stopped: its adapters, its NRPT
/// rules, routes into ACME's network, its firewall rules.
pub(crate) fn left_after_stop() -> String {
    sh("Get-NetAdapter -Name 'crake-*' -ErrorAction SilentlyContinue | Format-Table Name,Status | Out-String; \
        Get-DnsClientNrptRule | Where-Object { $_.NameServers -contains '127.53.0.1' } | \
        Format-Table Namespace,NameServers | Out-String; \
        Get-NetRoute -AddressFamily IPv4 -ErrorAction SilentlyContinue | \
        Where-Object DestinationPrefix -like '10.40.*' | Format-Table DestinationPrefix,InterfaceAlias | Out-String; \
        Get-NetFirewallRule -Group Crake -ErrorAction SilentlyContinue | Format-Table DisplayName | Out-String")
    .trim()
    .to_owned()
}

/// The system asks for a name's IPv6 addresses only where the network
/// has IPv6 of its own (macOS' resolver).
pub(crate) const NAMES_NEED_NETWORK_IPV6: bool = false;

/// Split-horizon's names: one with a public address and a private one in
/// ACME's DNS, and one with only a public one (the internet's (the gateway is named under dorhawk.software here, scripts/ci-through-cloudflare.sh)).
pub(crate) const SPLIT: (&str, &str) = ("packages.dorhawk.software", "10.40.1.16");
pub(crate) const PUBLIC_UNDER_ORG: &str = "dorhawk.software";
/// The tunnel's NRPT rule, where craked writes it.
fn nrpt_rule(dev: &str) -> String {
    format!(
        r"HKLM:\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig\Crake {dev}"
    )
}

/// What another program does to the tunnel's DNS: a VPN client cleaning
/// NRPT up takes Crake's rule for the tunnel.
pub(crate) fn rewrite_tunnel_dns(dev: &str) -> String {
    sh(&format!(
        "Remove-Item -Path '{}' -Recurse -ErrorAction Stop",
        nrpt_rule(dev)
    ))
}

/// Whether the rule is Crake's again, and what it names.
pub(crate) fn tunnel_dns_is_crakes(dev: &str) -> (bool, String) {
    let names = sh(&format!(
        "(Get-ItemProperty -Path '{}' -Name Name -ErrorAction SilentlyContinue).Name",
        nrpt_rule(dev)
    ));
    (names.lines().any(|l| l.trim() == "acme.internal"), names)
}

/// The system's DNS cache emptied (the DNS client's).
pub(crate) fn flush_dns_cache() {
    sh("Clear-DnsClientCache");
}
