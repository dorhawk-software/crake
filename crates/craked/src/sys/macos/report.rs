//! macOS's part of a report: its resolvers (`scutil --dns`, Crake's
//! /etc/resolver files among them), its routes, and pf's NAT in Crake's
//! anchor with whether pf is on.

use crake_proto::report::Part;

use crate::sys::command_lines;

pub async fn parts() -> Vec<Part> {
    let dns = command_lines("scutil", &["--dns"]).await;
    let routes = command_lines("netstat", &["-rn"]).await;
    let mut pf = command_lines("pfctl", &["-a", super::routing::ANCHOR, "-s", "nat"]).await;
    pf.extend(
        command_lines("pfctl", &["-s", "info"])
            .await
            .into_iter()
            .filter(|l| l.starts_with("Status:")),
    );
    vec![
        Part::new("DNS, as macOS has it (scutil --dns)", dns),
        Part::new("Routes (netstat -rn)", routes),
        Part::new(
            format!("Crake's rules (pf, anchor {})", super::routing::ANCHOR),
            pf.into_iter().filter(|l| !l.contains("ALTQ")),
        ),
    ]
}
