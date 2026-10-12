//! What the gateway pushed, parsed from the hook's environment.
//! Split-include networks and DNS domains are hints only; nothing here is routed.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use anyhow::{Context, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TunnelInfo {
    pub device: String,
    /// The gateway's IPv4 address, if it has one.
    pub gateway: Option<Ipv4Addr>,
    pub address: Option<Ipv4Addr>,
    pub prefix: Option<u8>,
    pub mtu: Option<u32>,
    pub dns: Vec<Ipv4Addr>,
    pub domains: Vec<String>,
    /// `a.b.c.d/len`
    pub split_include: Vec<String>,
    pub split_exclude: Vec<String>,
}

impl TunnelInfo {
    pub fn from_env(env: &BTreeMap<String, String>) -> Result<Self> {
        let get = |k: &str| env.get(k).map(String::as_str).filter(|v| !v.is_empty());

        let device = get("TUNDEV").context("TUNDEV is missing")?.to_owned();
        let address = get("INTERNAL_IP4_ADDRESS")
            .map(|a| a.parse::<Ipv4Addr>().context("INTERNAL_IP4_ADDRESS"))
            .transpose()?;
        let prefix = match (get("INTERNAL_IP4_NETMASKLEN"), get("INTERNAL_IP4_NETMASK")) {
            (Some(len), _) => Some(
                len.parse::<u8>()
                    .ok()
                    .filter(|l| *l <= crate::hosts::V4_BITS)
                    .context("INTERNAL_IP4_NETMASKLEN")?,
            ),
            (None, Some(mask)) => Some(
                mask.parse::<Ipv4Addr>()
                    .ok()
                    .and_then(prefix_from_mask)
                    .context("INTERNAL_IP4_NETMASK")?,
            ),
            // openconnect omits the mask for a point-to-point address.
            (None, None) => address.map(|_| crate::hosts::V4_BITS),
        };
        let mtu = get("INTERNAL_IP4_MTU").and_then(|m| m.parse().ok());
        let dns = get("INTERNAL_IP4_DNS")
            .unwrap_or("")
            .split_whitespace()
            .map(|d| d.parse::<Ipv4Addr>().context("INTERNAL_IP4_DNS"))
            .collect::<Result<Vec<_>>>()?;

        let mut domains: Vec<String> = Vec::new();
        for d in get("CISCO_DEF_DOMAIN")
            .unwrap_or("")
            .split_whitespace()
            .chain(get("CISCO_SPLIT_DNS").unwrap_or("").split(','))
        {
            let d = d.trim();
            if !d.is_empty() && !domains.iter().any(|x| x == d) {
                domains.push(d.to_owned());
            }
        }

        let gateway = get("VPNGATEWAY").and_then(|g| g.parse().ok());
        Ok(Self {
            device,
            gateway,
            address,
            prefix,
            mtu,
            dns,
            domains,
            split_include: split_list(env, "CISCO_SPLIT_INC"),
            split_exclude: split_list(env, "CISCO_SPLIT_EXC"),
        })
    }
}

/// More split networks than any gateway pushes: the rest is ignored.
const MAX_SPLITS: usize = 1024;

fn split_list(env: &BTreeMap<String, String>, base: &str) -> Vec<String> {
    let n: usize = env.get(base).and_then(|n| n.parse().ok()).unwrap_or(0);
    (0..n.min(MAX_SPLITS))
        .filter_map(|i| {
            let addr = env.get(&format!("{base}_{i}_ADDR"))?;
            let len = env.get(&format!("{base}_{i}_MASKLEN"))?;
            addr.parse::<Ipv4Addr>().ok()?;
            len.parse::<u8>()
                .ok()
                .filter(|l| *l <= crate::hosts::V4_BITS)?;
            Some(format!("{addr}/{len}"))
        })
        .collect()
}

/// `255.255.255.0` -> 24. Rejects non-contiguous masks.
pub fn prefix_from_mask(mask: Ipv4Addr) -> Option<u8> {
    let bits = u32::from(mask);
    let ones = bits.leading_ones();
    (bits.checked_shl(ones).unwrap_or(0) == 0).then_some(ones as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks() {
        assert_eq!(prefix_from_mask("255.255.255.0".parse().unwrap()), Some(24));
        assert_eq!(
            prefix_from_mask("255.255.255.255".parse().unwrap()),
            Some(32)
        );
        assert_eq!(prefix_from_mask("0.0.0.0".parse().unwrap()), Some(0));
        assert_eq!(prefix_from_mask("255.0.255.0".parse().unwrap()), None);
    }

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn parses_a_typical_connect() {
        let info = TunnelInfo::from_env(&env(&[
            ("TUNDEV", "crake-acme"),
            ("VPNGATEWAY", "192.0.2.10"),
            ("INTERNAL_IP4_ADDRESS", "10.40.1.7"),
            ("INTERNAL_IP4_NETMASK", "255.255.255.0"),
            ("INTERNAL_IP4_MTU", "1300"),
            ("INTERNAL_IP4_DNS", "10.40.0.2 10.40.0.3"),
            ("CISCO_DEF_DOMAIN", "acme.internal"),
            ("CISCO_SPLIT_DNS", "acme.internal,corp.acme"),
            ("CISCO_SPLIT_INC", "2"),
            ("CISCO_SPLIT_INC_0_ADDR", "10.40.0.0"),
            ("CISCO_SPLIT_INC_0_MASK", "255.255.0.0"),
            ("CISCO_SPLIT_INC_0_MASKLEN", "16"),
            ("CISCO_SPLIT_INC_1_ADDR", "172.16.0.0"),
            ("CISCO_SPLIT_INC_1_MASKLEN", "12"),
        ]))
        .unwrap();
        assert_eq!(info.gateway, Some("192.0.2.10".parse().unwrap()));
        assert_eq!(info.address, Some("10.40.1.7".parse().unwrap()));
        assert_eq!(info.prefix, Some(24));
        assert_eq!(info.mtu, Some(1300));
        assert_eq!(info.dns.len(), 2);
        assert_eq!(info.domains, ["acme.internal", "corp.acme"]);
        assert_eq!(info.split_include, ["10.40.0.0/16", "172.16.0.0/12"]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(TunnelInfo::from_env(&env(&[])).is_err());
        assert!(
            TunnelInfo::from_env(&env(&[("TUNDEV", "x"), ("INTERNAL_IP4_ADDRESS", "nope")]))
                .is_err()
        );
        assert!(
            TunnelInfo::from_env(&env(&[
                ("TUNDEV", "x"),
                ("INTERNAL_IP4_DNS", "1.2.3.4; reboot")
            ]))
            .is_err()
        );
    }
}
