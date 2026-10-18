//! Windows' part of a report: the Name Resolution Policy Table (Crake's
//! rules and any other VPN client's) with the adapters' DNS servers, the
//! routes, and Crake's firewall rules. One PowerShell for the first and
//! the last (its start is the slow part).

use crake_proto::report::Part;

use crate::sys::command_lines;

/// Between PowerShell's answers.
const BETWEEN: &str = "=====crake-report-part=====";

pub async fn parts() -> Vec<Part> {
    let script = format!(
        "Get-DnsClientNrptPolicy | Format-Table -AutoSize Namespace,NameServers | Out-String -Width 200; \
         Get-DnsClientServerAddress | Where-Object {{ $_.ServerAddresses }} | \
         Format-Table -AutoSize InterfaceAlias,AddressFamily,ServerAddresses | Out-String -Width 200; \
         '{BETWEEN}'; \
         Get-NetFirewallRule -Group {group} -ErrorAction SilentlyContinue | \
         Format-Table -AutoSize DisplayName,Enabled,Direction,Action | Out-String -Width 200",
        group = super::firewall::GROUP,
    );
    let said = command_lines(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )
    .await;
    let at = said.iter().position(|l| l.trim() == BETWEEN);
    let (dns, rules) = match at {
        Some(i) => (said[..i].to_vec(), said[i + 1..].to_vec()),
        None => (said, Vec::new()),
    };
    let rules = if rules.iter().all(|l| l.trim().is_empty()) {
        vec!["(none)".to_owned()]
    } else {
        rules
    };
    let routes = command_lines("route", &["print"]).await;
    vec![
        Part::new(
            "DNS, as Windows has it (the NRPT, then the adapters' DNS servers)",
            dns,
        ),
        Part::new("Routes (route print)", routes),
        Part::new("Crake's rules (Windows Firewall, group Crake)", rules),
    ]
}
