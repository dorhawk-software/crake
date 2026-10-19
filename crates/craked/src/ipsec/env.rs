//! What strongSwan's updown hands the hook (`PLUTO_*`), in the shape the
//! rest of craked knows from openconnect: the address the gateway gave
//! (`PLUTO_MY_SOURCEIP`, mode-cfg), its DNS servers (`PLUTO_DNS4_n`,
//! `PLUTO_DNS6_n`, with craked's charon set to hand them to updown), the
//! gateway (`PLUTO_PEER`), and the network the gateway narrowed the
//! tunnel to (`PLUTO_PEER_CLIENT`), a hint as split networks are. The
//! device is the tunnel's own XFRM interface: updown doesn't name one.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::{Context, Result};

use crate::info::TunnelInfo;

/// More DNS servers than any gateway hands over: the rest is ignored.
const MAX_DNS: usize = 16;

pub fn info(env: &BTreeMap<String, String>, device: &str) -> Result<TunnelInfo> {
    let get = |k: &str| env.get(k).map(String::as_str).filter(|v| !v.is_empty());
    // The address, or addresses (strongSwan 5.9.4 and later number them by
    // family; the plain one is the first).
    let mut sources: Vec<IpAddr> = Vec::new();
    for k in [
        "PLUTO_MY_SOURCEIP",
        "PLUTO_MY_SOURCEIP4_1",
        "PLUTO_MY_SOURCEIP6_1",
    ] {
        if let Some(v) = get(k) {
            let a = v.parse::<IpAddr>().with_context(|| k.to_owned())?;
            if !sources.contains(&a) {
                sources.push(a);
            }
        }
    }
    let address = sources.iter().find_map(|a| match a {
        IpAddr::V4(a) => Some(*a),
        IpAddr::V6(_) => None,
    });
    let address6 = sources.iter().find_map(|a| match a {
        IpAddr::V6(a) => Some((*a, crate::hosts::V6_BITS)),
        IpAddr::V4(_) => None,
    });
    let mut servers: Vec<IpAddr> = Vec::new();
    for family in ["PLUTO_DNS4_", "PLUTO_DNS6_"] {
        for n in 1..=MAX_DNS {
            let Some(v) = get(&format!("{family}{n}")) else {
                break;
            };
            let a = v
                .parse::<IpAddr>()
                .with_context(|| format!("{family}{n}"))?;
            if !servers.contains(&a) {
                servers.push(a);
            }
        }
    }
    let dns: Vec<Ipv4Addr> = servers
        .iter()
        .filter_map(|a| match a {
            IpAddr::V4(a) => Some(*a),
            IpAddr::V6(_) => None,
        })
        .collect();
    let dns6: Vec<Ipv6Addr> = servers
        .iter()
        .filter_map(|a| match a {
            IpAddr::V6(a) => Some(*a),
            IpAddr::V4(_) => None,
        })
        .collect();
    let (gateway, gateway6) = match get("PLUTO_PEER").and_then(|g| g.parse::<IpAddr>().ok()) {
        Some(IpAddr::V4(a)) => (Some(a), None),
        Some(IpAddr::V6(a)) => (None, Some(a)),
        None => (None, None),
    };
    // Everything (0.0.0.0/0) says nothing about where the company is.
    let split_include = get("PLUTO_PEER_CLIENT")
        .filter(|c| !matches!(*c, "0.0.0.0/0" | "::/0"))
        .map(str::to_owned)
        .into_iter()
        .collect();
    Ok(TunnelInfo {
        device: device.to_owned(),
        gateway,
        gateway6,
        address,
        prefix: address.map(|_| crate::hosts::V4_BITS),
        mtu: Some(super::MTU),
        dns,
        address6,
        dns6,
        servers,
        domains: Vec::new(),
        split_include,
        split_exclude: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn what_a_gateway_hands_over() {
        let info = info(
            &env(&[
                ("PLUTO_VERB", "up-client"),
                ("PLUTO_MY_SOURCEIP", "10.40.9.10"),
                ("PLUTO_MY_SOURCEIP4_1", "10.40.9.10"),
                ("PLUTO_PEER", "198.51.100.10"),
                ("PLUTO_PEER_CLIENT", "10.40.0.0/16"),
                ("PLUTO_DNS4_1", "10.40.0.2"),
                ("PLUTO_DNS4_2", "10.40.0.3"),
                ("PLUTO_DNS6_1", "fd40::2"),
            ]),
            "crake-acme",
        )
        .unwrap();
        assert_eq!(info.device, "crake-acme");
        assert_eq!(info.address, Some(Ipv4Addr::new(10, 40, 9, 10)));
        assert_eq!(info.prefix, Some(crate::hosts::V4_BITS));
        assert_eq!(info.gateway, Some(Ipv4Addr::new(198, 51, 100, 10)));
        assert_eq!(info.dns.len(), 2);
        assert_eq!(info.dns6, ["fd40::2".parse::<Ipv6Addr>().unwrap()]);
        assert_eq!(info.servers.len(), 3, "in the gateway's order");
        assert_eq!(info.split_include, ["10.40.0.0/16"]);
        assert_eq!(info.mtu, Some(super::super::MTU));
    }

    #[test]
    fn a_full_selector_is_no_hint_and_ipv6_comes_alone() {
        let info = info(
            &env(&[
                ("PLUTO_MY_SOURCEIP6_1", "fd40:9::10"),
                ("PLUTO_PEER", "2001:db8::1"),
                ("PLUTO_PEER_CLIENT", "0.0.0.0/0"),
            ]),
            "crake-acme",
        )
        .unwrap();
        assert!(info.split_include.is_empty());
        assert_eq!(info.address, None);
        assert_eq!(
            info.address6,
            Some(("fd40:9::10".parse().unwrap(), crate::hosts::V6_BITS))
        );
        assert_eq!(info.gateway6, Some("2001:db8::1".parse().unwrap()));
        assert!(info.servers.is_empty());
    }

    #[test]
    fn nonsense_is_refused() {
        assert!(info(&env(&[("PLUTO_MY_SOURCEIP", "nope")]), "crake-acme").is_err());
        assert!(info(&env(&[("PLUTO_DNS4_1", "nope")]), "crake-acme").is_err());
    }
}
