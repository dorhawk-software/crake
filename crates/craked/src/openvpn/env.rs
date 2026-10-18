//! What openvpn hands its `--up` script, as craked's `TunnelInfo`: the
//! device, its addresses, the gateway, and what was pushed (routes, DNS
//! servers and domains), hints as always.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::{Context, Result};

use crate::info::{TunnelInfo, prefix_from_mask};

/// More than any server pushes: the rest is ignored.
const MAX_ITEMS: usize = 1024;

pub fn info(env: &BTreeMap<String, String>) -> Result<TunnelInfo> {
    let get = |k: &str| env.get(k).map(String::as_str).filter(|v| !v.is_empty());
    let device = get("dev").context("dev is missing")?.to_owned();
    let address = get("ifconfig_local")
        .map(|a| a.parse::<Ipv4Addr>().context("ifconfig_local"))
        .transpose()?;
    // A subnet's mask; point-to-point (net30, p2p) names the far end
    // instead, and the address stands alone.
    let prefix = match get("ifconfig_netmask") {
        Some(mask) => Some(
            mask.parse::<Ipv4Addr>()
                .ok()
                .and_then(prefix_from_mask)
                .context("ifconfig_netmask")?,
        ),
        None => address.map(|_| crate::hosts::V4_BITS),
    };
    let address6 = match get("ifconfig_ipv6_local") {
        Some(a) => Some((
            a.parse::<Ipv6Addr>().context("ifconfig_ipv6_local")?,
            get("ifconfig_ipv6_netbits")
                .and_then(|b| b.parse::<u8>().ok())
                .filter(|b| *b <= crate::hosts::V6_BITS)
                .unwrap_or(crate::hosts::V6_BITS),
        )),
        None => None,
    };
    let gateway = get("trusted_ip").and_then(|g| g.parse::<Ipv4Addr>().ok());
    let gateway6 = get("trusted_ip6").and_then(|g| g.parse::<Ipv6Addr>().ok());

    let mut servers: Vec<IpAddr> = Vec::new();
    let mut domains: Vec<String> = Vec::new();
    let mut add_domain = |d: &str| {
        let d = d.trim().trim_end_matches('.').to_ascii_lowercase();
        if crate::dns::valid_domain(&d) && !domains.contains(&d) {
            domains.push(d);
        }
    };
    // `dhcp-option`s, in the order they were pushed.
    for i in 1..=MAX_ITEMS {
        let Some(option) = get(&format!("foreign_option_{i}")) else {
            break;
        };
        let words: Vec<&str> = option.split_whitespace().collect();
        match words.as_slice() {
            ["dhcp-option", "DNS" | "DNS6", addr, ..] => {
                if let Ok(a) = addr.parse::<IpAddr>()
                    && !servers.contains(&a)
                {
                    servers.push(a);
                }
            }
            [
                "dhcp-option",
                "DOMAIN" | "DOMAIN-SEARCH" | "ADAPTER_DOMAIN_SUFFIX",
                rest @ ..,
            ] => {
                rest.iter().for_each(|d| add_domain(d));
            }
            _ => {}
        }
    }
    // OpenVPN 2.6's `dns` option: servers by priority (any number, lowest
    // first), each with its addresses and the domains it serves.
    let mut by_priority: BTreeMap<(i32, u32), &str> = BTreeMap::new();
    let mut served: BTreeMap<i32, &str> = BTreeMap::new();
    for (k, v) in env {
        let Some(rest) = k.strip_prefix("dns_server_") else {
            continue;
        };
        let Some((n, what)) = rest.split_once('_') else {
            continue;
        };
        let Ok(n) = n.parse::<i32>() else {
            continue;
        };
        if let Some(m) = what.strip_prefix("address_").and_then(|m| m.parse().ok()) {
            by_priority.insert((n, m), v);
        } else if what == "resolve_domains" {
            served.insert(n, v);
        }
    }
    for a in by_priority.values() {
        if let Ok(addr) = without_port(a).parse::<IpAddr>()
            && !servers.contains(&addr)
        {
            servers.push(addr);
        }
    }
    for d in served.values() {
        d.split_whitespace().for_each(&mut add_domain);
    }
    for i in 1..=MAX_ITEMS {
        let Some(d) = get(&format!("dns_search_domain_{i}")) else {
            break;
        };
        add_domain(d);
    }
    let dns = servers
        .iter()
        .filter_map(|a| match a {
            IpAddr::V4(a) => Some(*a),
            IpAddr::V6(_) => None,
        })
        .collect();
    let dns6 = servers
        .iter()
        .filter_map(|a| match a {
            IpAddr::V6(a) => Some(*a),
            IpAddr::V4(_) => None,
        })
        .collect();

    // Routes pushed (or in the profile): hints. A default route
    // (redirect-gateway) isn't one: what goes through is Crake's to decide.
    let mut split_include = Vec::new();
    for i in 1..=MAX_ITEMS {
        let (Some(net), Some(mask)) = (
            get(&format!("route_network_{i}")),
            get(&format!("route_netmask_{i}")),
        ) else {
            break;
        };
        if let (Ok(net), Some(len)) = (
            net.parse::<Ipv4Addr>(),
            mask.parse::<Ipv4Addr>().ok().and_then(prefix_from_mask),
        ) && len > 0
        {
            split_include.push(format!("{net}/{len}"));
        }
    }
    for i in 1..=MAX_ITEMS {
        let Some(net) = get(&format!("route_ipv6_network_{i}")) else {
            break;
        };
        if net.split('/').nth(1).is_some_and(|l| l != "0") {
            split_include.push(net.to_owned());
        }
    }
    Ok(TunnelInfo {
        device,
        gateway,
        gateway6,
        address,
        prefix,
        mtu: get("tun_mtu").and_then(|m| m.parse().ok()),
        dns,
        address6,
        dns6,
        servers,
        domains,
        split_include,
        split_exclude: Vec::new(),
    })
}

/// A DNS server's address, without the port it may carry ("10.0.0.2:53",
/// "[fd00::2]:53").
fn without_port(a: &str) -> &str {
    if let Some(rest) = a.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    match a.split_once(':') {
        Some((host, port)) if !port.contains(':') => host,
        _ => a,
    }
}

/// What a gateway pushed, as openvpn's own output says it ("PUSH:
/// Received control message: 'PUSH_REPLY,...'", openvpn redacting any
/// token): its DNS options. Windows' openvpn keeps `dhcp-option`s for its
/// adapter and hands the hook none (seen 2026-10-18 on CI's Windows: no
/// foreign_option or dns_* at all); Linux's and macOS' hand them over.
#[derive(Debug, Default, Clone)]
pub struct Pushed {
    options: Vec<String>,
    /// The last message said more follow ("push-continuation 2").
    more: bool,
}

/// How openvpn's output brings a pushed reply.
const PUSH_REPLY: &str = "PUSH: Received control message: 'PUSH_REPLY,";

impl Pushed {
    /// One line of openvpn's output.
    pub fn heard(&mut self, line: &str) {
        let Some(rest) = line.split_once(PUSH_REPLY).map(|(_, r)| r) else {
            return;
        };
        let reply = rest.trim_end().trim_end_matches('\'');
        if !self.more {
            self.options.clear();
        }
        self.more = false;
        for option in reply.split(',') {
            let option = option.trim();
            if option == "push-continuation 2" {
                self.more = true;
            } else if option.starts_with("dhcp-option ") || option.starts_with("dns ") {
                self.options.push(option.to_owned());
            }
        }
    }

    /// The hook's variables for what was pushed, where it has none of its
    /// own: `foreign_option_N` for `dhcp-option`s, `dns_server_*` and
    /// `dns_search_domain_N` for 2.6's `dns`.
    pub fn fill(&self, env: &mut BTreeMap<String, String>) {
        let has = |p: &str| env.keys().any(|k| k.starts_with(p));
        if has("foreign_option_") || has("dns_server_") || has("dns_search_domain_") {
            return;
        }
        let (mut foreign, mut search) = (0, 0);
        for option in &self.options {
            let words: Vec<&str> = option.split_whitespace().collect();
            match words.as_slice() {
                ["dhcp-option", ..] => {
                    foreign += 1;
                    env.insert(format!("foreign_option_{foreign}"), option.clone());
                }
                ["dns", "server", n, "address", addrs @ ..] => {
                    for (m, a) in addrs.iter().enumerate() {
                        env.insert(format!("dns_server_{n}_address_{}", m + 1), (*a).to_owned());
                    }
                }
                ["dns", "server", n, "resolve-domains", domains @ ..] => {
                    env.insert(format!("dns_server_{n}_resolve_domains"), domains.join(" "));
                }
                ["dns", "search-domains", domains @ ..] => {
                    for d in domains {
                        search += 1;
                        env.insert(format!("dns_search_domain_{search}"), (*d).to_owned());
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_windows_openvpn_keeps_to_itself_comes_from_its_output() {
        // As CI's Windows run printed it (2026-10-18), and a 2.6 `dns` push
        // in two messages.
        let mut p = Pushed::default();
        p.heard("2026-10-18 15:51:29 PUSH: Received control message: 'PUSH_REPLY,topology subnet,route-gateway 10.40.254.1,route 10.40.0.0 255.255.0.0,dhcp-option DNS 10.40.0.2,dhcp-option DOMAIN acme.internal,redirect-gateway def1,block-outside-dns,ifconfig 10.40.254.10 255.255.255.0,peer-id 0,cipher AES-256-GCM'");
        let mut env: BTreeMap<String, String> = [
            ("dev", "crake-initech"),
            ("ifconfig_local", "10.40.254.10"),
            ("ifconfig_netmask", "255.255.255.0"),
            ("route_network_1", "10.40.0.0"),
            ("route_netmask_1", "255.255.0.0"),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
        p.fill(&mut env);
        let i = info(&env).unwrap();
        assert_eq!(i.dns, [Ipv4Addr::new(10, 40, 0, 2)]);
        assert_eq!(i.domains, ["acme.internal"]);
        // The hook's own, where there are any, win.
        let mut own = env.clone();
        own.retain(|k, _| !k.starts_with("foreign_option_"));
        own.insert("foreign_option_1".into(), "dhcp-option DNS 10.9.9.9".into());
        p.fill(&mut own);
        assert_eq!(info(&own).unwrap().dns, [Ipv4Addr::new(10, 9, 9, 9)]);

        let mut two = Pushed::default();
        two.heard("PUSH: Received control message: 'PUSH_REPLY,dns server 0 address 10.40.0.2,push-continuation 2'");
        two.heard("PUSH: Received control message: 'PUSH_REPLY,dns server 0 resolve-domains acme.internal,dns search-domains corp.acme.internal,push-continuation 1'");
        let mut env: BTreeMap<String, String> = [("dev".to_owned(), "crake-initech".to_owned())]
            .into_iter()
            .collect();
        two.fill(&mut env);
        let i = info(&env).unwrap();
        assert_eq!(i.dns, [Ipv4Addr::new(10, 40, 0, 2)]);
        assert_eq!(i.domains, ["acme.internal", "corp.acme.internal"]);
        // A new reply replaces the old.
        two.heard("PUSH: Received control message: 'PUSH_REPLY,dhcp-option DNS 10.1.1.1'");
        let mut env: BTreeMap<String, String> = [("dev".to_owned(), "crake-initech".to_owned())]
            .into_iter()
            .collect();
        two.fill(&mut env);
        assert_eq!(info(&env).unwrap().dns, [Ipv4Addr::new(10, 1, 1, 1)]);
    }

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// What openvpn 2.7 gave the up script in the lab (2026-10-18): a
    /// server pushing a subnet, IPv6, routes, DNS and domains.
    #[test]
    fn what_a_server_pushed() {
        let e = env(&[
            ("dev", "crake-acme"),
            ("tun_mtu", "1500"),
            ("ifconfig_local", "10.8.0.10"),
            ("ifconfig_netmask", "255.255.255.0"),
            ("ifconfig_ipv6_local", "fd40:ff::10"),
            ("ifconfig_ipv6_netbits", "64"),
            ("trusted_ip", "192.0.2.10"),
            ("foreign_option_1", "dhcp-option DNS 10.40.0.2"),
            ("foreign_option_2", "dhcp-option DNS6 fd40::2"),
            ("foreign_option_3", "dhcp-option DOMAIN acme.internal"),
            (
                "foreign_option_4",
                "dhcp-option DOMAIN-SEARCH corp.acme.internal",
            ),
            ("route_network_1", "10.40.0.0"),
            ("route_netmask_1", "255.255.0.0"),
            ("route_network_2", "10.41.1.0"),
            ("route_netmask_2", "255.255.255.0"),
            ("route_ipv6_network_1", "fd40:1::/64"),
            ("route_redirect_gateway_ipv4", "1"),
            ("script_type", "up"),
            ("script_context", "init"),
        ]);
        let i = info(&e).unwrap();
        assert_eq!(i.device, "crake-acme");
        assert_eq!(i.address, Some("10.8.0.10".parse().unwrap()));
        assert_eq!(i.prefix, Some(24));
        assert_eq!(i.address6, Some(("fd40:ff::10".parse().unwrap(), 64)));
        assert_eq!(i.gateway, Some("192.0.2.10".parse().unwrap()));
        assert_eq!(i.mtu, Some(1500));
        assert_eq!(i.dns, vec!["10.40.0.2".parse::<Ipv4Addr>().unwrap()]);
        assert_eq!(i.dns6, vec!["fd40::2".parse::<Ipv6Addr>().unwrap()]);
        assert_eq!(i.servers.len(), 2);
        assert_eq!(i.domains, ["acme.internal", "corp.acme.internal"]);
        assert_eq!(
            i.split_include,
            ["10.40.0.0/16", "10.41.1.0/24", "fd40:1::/64"]
        );
    }

    #[test]
    fn openvpn_2_6_dns_option() {
        let e = env(&[
            ("dev", "crake-acme"),
            ("ifconfig_local", "10.8.0.6"),
            ("ifconfig_remote", "10.8.0.5"),
            ("dns_server_0_address_1", "10.40.0.2"),
            ("dns_server_0_address_2", "[fd40::2]:53"),
            (
                "dns_server_0_resolve_domains",
                "acme.internal globex.internal",
            ),
            ("dns_search_domain_1", "corp.acme.internal"),
        ]);
        let i = info(&e).unwrap();
        // Point-to-point: the address alone.
        assert_eq!(i.prefix, Some(32));
        assert_eq!(
            i.servers,
            [
                "10.40.0.2".parse::<IpAddr>().unwrap(),
                "fd40::2".parse().unwrap()
            ]
        );
        assert_eq!(
            i.domains,
            ["acme.internal", "globex.internal", "corp.acme.internal"]
        );
    }
}
