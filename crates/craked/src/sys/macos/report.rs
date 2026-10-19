//! macOS's part of a report: its resolvers (`scutil --dns`, Crake's
//! /etc/resolver files among them), its routes, and pf's NAT in Crake's
//! anchor with whether pf is on.

use crake_proto::report::Part;

use crate::sys::command_lines;

pub async fn parts() -> Vec<Part> {
    let dns = command_lines("scutil", &["--dns"]).await;
    // -W: IPv6 addresses whole, not cut to fit a column.
    let routes: Vec<String> = command_lines("netstat", &["-rnW"])
        .await
        .into_iter()
        .map(|l| written_out(&l))
        .collect();
    let mut pf = command_lines("pfctl", &["-a", super::routing::ANCHOR, "-s", "nat"]).await;
    pf.extend(
        command_lines("pfctl", &["-s", "info"])
            .await
            .into_iter()
            .filter(|l| l.starts_with("Status:")),
    );
    vec![
        Part::new("DNS, as macOS has it (scutil --dns)", dns),
        Part::new("Routes (netstat -rnW)", routes),
        Part::new(
            format!("Crake's rules (pf, anchor {})", super::routing::ANCHOR),
            pf.into_iter().filter(|l| !l.contains("ALTQ")),
        ),
    ]
}

/// The octets in an IPv4 address, and the bits in each.
const OCTETS: usize = 4;
const OCTET_BITS: usize = 8;

/// netstat writes a network short ("10.40/16", "192.168.1"): its first
/// word written out ("10.40.0.0/16", "192.168.1.0/24"), so it's known for
/// an address, hidden as one, and marked when it's Crake's.
fn written_out(line: &str) -> String {
    let Some(first) = line.split_whitespace().next() else {
        return line.to_owned();
    };
    let (net, bits) = match first.split_once('/') {
        Some((n, b)) => (n, Some(b)),
        None => (first, None),
    };
    let octets: Vec<&str> = net.split('.').collect();
    let short = (1..OCTETS).contains(&octets.len())
        && octets
            .iter()
            .all(|o| !o.is_empty() && o.chars().all(|c| c.is_ascii_digit()));
    if !short {
        return line.to_owned();
    }
    let mut full = octets.clone();
    full.resize(OCTETS, "0");
    let bits = bits.map_or_else(|| (octets.len() * OCTET_BITS).to_string(), str::to_owned);
    line.replacen(first, &format!("{}/{bits}", full.join(".")), 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_networks_are_written_out() {
        assert_eq!(
            written_out("10.40/16  utun4  UGSc  utun4"),
            "10.40.0.0/16  utun4  UGSc  utun4"
        );
        assert_eq!(
            written_out("192.168.1  link#6  UCS  en0"),
            "192.168.1.0/24  link#6  UCS  en0"
        );
        assert_eq!(
            written_out("127  127.0.0.1  UCS  lo0"),
            "127.0.0.0/8  127.0.0.1  UCS  lo0"
        );
        for kept in [
            "default  192.0.2.1  UGScg  en0",
            "10.40.1.10  utun4  UHS  utun4",
            "Internet:",
        ] {
            assert_eq!(written_out(kept), kept);
        }
    }
}
