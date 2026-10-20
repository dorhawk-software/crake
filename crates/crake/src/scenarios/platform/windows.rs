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
         Resolve-DnsName {name} -Server 127.53.0.1 -DnsOnly -ErrorAction Continue | Out-String"
    ))
}
