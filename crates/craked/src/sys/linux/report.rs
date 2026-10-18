//! Linux's part of a report: systemd-resolved's view of DNS, the routes
//! of every table with the policy rules that pick them, and Crake's own
//! nftables table.

use crake_proto::report::Part;

use crate::sys::command_lines;

pub async fn parts() -> Vec<Part> {
    let dns = command_lines("resolvectl", &["status", "--no-pager"]).await;
    let mut routes = Vec::new();
    for family in ["-4", "-6"] {
        // The local table is the machine's own addresses again, at length.
        let all = command_lines("ip", &[family, "route", "show", "table", "all"]).await;
        routes.extend(all.into_iter().filter(|l| !l.contains("table local")));
    }
    for family in ["-4", "-6"] {
        routes.extend(command_lines("ip", &[family, "rule"]).await);
    }
    let rules = command_lines("nft", &["list", "table", "inet", "crake"]).await;
    vec![
        Part::new("DNS, as systemd-resolved has it (resolvectl status)", dns),
        Part::new("Routes (ip route, every table; ip rule)", routes),
        Part::new("Crake's rules (nft list table inet crake)", rules),
    ]
}
