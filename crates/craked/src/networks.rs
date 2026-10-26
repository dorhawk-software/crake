//! Networks Crake has learned something about: for now, the ones where UDP
//! doesn't get through, so tunnels there start without DTLS (CLAUDE.md, 7b).
//! A network is its default gateway: address plus hardware address, so the
//! same 192.168.1.1 at home and at a café are different networks.

use std::collections::BTreeSet;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The network the machine is on now ("192.168.1.1 aa:bb:cc:dd:ee:ff"), from
/// the main table's IPv4 default route and the neighbour table.
#[cfg(target_os = "linux")]
pub fn current() -> Option<String> {
    let route = std::fs::read_to_string("/proc/net/route").ok()?;
    let arp = std::fs::read_to_string("/proc/net/arp").ok()?;
    of(&route, &arp)
}

/// The same on a Mac or a Windows PC: the default gateway as the system
/// routes, and its hardware address from `arp` (`sys::default_gateway`).
/// Until 2026-10-26 there was none (/proc is Linux's): a network where UDP
/// doesn't get through was never remembered there, and each start waited
/// for DTLS to fail again.
#[cfg(not(target_os = "linux"))]
pub fn current() -> Option<String> {
    let (gateway, arp) = crate::sys::default_gateway()?;
    Some(named(gateway, arp.as_deref().and_then(hardware_address)))
}

/// A network's name: its gateway, and that gateway's hardware address
/// when known.
fn named(gateway: Ipv4Addr, hw: Option<String>) -> String {
    match hw {
        Some(hw) => format!("{gateway} {hw}"),
        None => gateway.to_string(),
    }
}

/// The hardware address in a neighbour table's words, as Linux writes it
/// ("0a:1b:…"): macOS's `arp` leaves out leading zeros ("a:1b:…"),
/// Windows' joins with dashes ("0a-1b-…"). None for an incomplete entry.
fn hardware_address(text: &str) -> Option<String> {
    const OCTETS: usize = 6;
    text.split_whitespace().find_map(|word| {
        let parts: Vec<&str> = word.split([':', '-']).collect();
        if parts.len() != OCTETS {
            return None;
        }
        let bytes: Vec<u8> = parts
            .iter()
            .map(|p| u8::from_str_radix(p, HEX).ok())
            .collect::<Option<_>>()?;
        if bytes.iter().all(|b| *b == 0) {
            return None;
        }
        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Some(hex.join(":"))
    })
}

/// Columns of /proc/net/route (hex, little-endian) and /proc/net/arp.
#[cfg(any(target_os = "linux", test))]
const ROUTE_DESTINATION: usize = 1;
#[cfg(any(target_os = "linux", test))]
const ROUTE_GATEWAY: usize = 2;
#[cfg(any(target_os = "linux", test))]
const ROUTE_METRIC: usize = 6;
#[cfg(any(target_os = "linux", test))]
const ROUTE_MASK: usize = 7;
#[cfg(any(target_os = "linux", test))]
const ARP_HW_ADDRESS: usize = 3;
const HEX: u32 = 16;

#[cfg(any(target_os = "linux", test))]
fn of(route: &str, arp: &str) -> Option<String> {
    // Iface Destination Gateway Flags RefCnt Use Metric Mask ...; hex, little-endian.
    let (gateway, _) = route
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let (dest, gw, metric, mask) = (
                f.get(ROUTE_DESTINATION)?,
                f.get(ROUTE_GATEWAY)?,
                f.get(ROUTE_METRIC)?,
                f.get(ROUTE_MASK)?,
            );
            if *dest != "00000000" || *mask != "00000000" || *gw == "00000000" {
                return None;
            }
            let gw = Ipv4Addr::from(u32::from_str_radix(gw, HEX).ok()?.swap_bytes());
            Some((gw, metric.parse::<u32>().ok()?))
        })
        .min_by_key(|(_, metric)| *metric)?;
    // IP address HW type Flags HW address Mask Device
    let mac = arp.lines().skip(1).find_map(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        let hw = *f.get(ARP_HW_ADDRESS)?;
        (f.first()?.parse::<Ipv4Addr>().ok()? == gateway)
            .then(|| hardware_address(hw))
            .flatten()
    });
    Some(named(gateway, mac))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    udp_blocked: BTreeSet<String>,
}

/// Remembered next to the host store, in `networks.json`.
pub struct Networks {
    path: Option<PathBuf>,
    saved: Saved,
}

impl Networks {
    pub fn load(path: &Path) -> Self {
        let saved = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            path: Some(path.to_owned()),
            saved,
        }
    }

    pub fn udp_blocked(&self, network: &str) -> bool {
        self.saved.udp_blocked.contains(network)
    }

    /// Returns whether it is news.
    pub fn block_udp(&mut self, network: &str) -> bool {
        let new = self.saved.udp_blocked.insert(network.to_owned());
        if new {
            self.save();
        }
        new
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Ok(json) = serde_json::to_string_pretty(&self.saved) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(tmp, path);
            }
        }
    }
}

/// openconnect's words (LC_ALL=C) for UDP not getting through: DTLS or ESP
/// never came up, or died, and it carries on over HTTPS.
pub fn udp_failed(line: &str) -> bool {
    const SIGNS: [&str; 6] = [
        "DTLS handshake timed out",
        "DTLS handshake failed",
        "Failed to connect DTLS tunnel; using HTTPS instead",
        "Failed to connect ESP tunnel; using HTTPS instead",
        "DTLS Dead Peer Detection detected dead peer",
        "ESP detected dead peer",
    ];
    SIGNS.iter().any(|s| line.contains(s))
}

/// openconnect's words for its UDP channel coming up.
pub fn udp_worked(line: &str) -> bool {
    line.starts_with("Established DTLS connection")
        || line.starts_with("DTLS session established")
        || line.starts_with("ESP session established")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_network_is_its_default_gateway() {
        let route = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
            wlp0s20f3\t00000000\t0101A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
            docker0\t000011AC\t00000000\t0001\t0\t0\t0\t0000FFFF\t0\t0\t0\n\
            enx0\t00000000\t0100000A\t0003\t0\t0\t700\t00000000\t0\t0\t0\n";
        let arp = "IP address       HW type     Flags       HW address            Mask     Device\n\
            192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:ff     *        wlp0s20f3\n";
        assert_eq!(
            of(route, arp).as_deref(),
            Some("192.168.1.1 aa:bb:cc:dd:ee:ff"),
            "the lowest metric wins"
        );
        assert_eq!(of(route, "").as_deref(), Some("192.168.1.1"));
        assert_eq!(of("Iface\tDestination\n", arp), None);
    }

    #[test]
    fn a_gateways_hardware_address_reads_the_same_everywhere() {
        let mac = "? (192.168.1.1) at a4:2b:b0:c:d2:e3 on en0 ifscope [ethernet]";
        let windows = "Interface: 192.168.1.5 --- 0x7\n  Internet Address      Physical Address      Type\n  192.168.1.1           a4-2b-b0-0c-d2-e3     dynamic\n";
        let want = Some("a4:2b:b0:0c:d2:e3".to_owned());
        assert_eq!(hardware_address(mac), want);
        assert_eq!(hardware_address(windows), want);
        assert_eq!(
            hardware_address("? (192.168.1.1) at (incomplete) on en0"),
            None
        );
        assert_eq!(hardware_address("192.168.1.1 00:00:00:00:00:00"), None);
        assert_eq!(
            named("192.168.1.1".parse().unwrap(), want),
            "192.168.1.1 a4:2b:b0:0c:d2:e3"
        );
    }

    #[test]
    fn knows_when_udp_does_not_get_through() {
        assert!(udp_failed("DTLS handshake timed out"));
        assert!(udp_failed(
            "Failed to connect DTLS tunnel; using HTTPS instead (state 3)."
        ));
        assert!(!udp_failed("Established DTLS connection (using GnuTLS)."));
        assert!(udp_worked(
            "Established DTLS connection (using GnuTLS). Ciphersuite x."
        ));
    }

    #[test]
    fn remembers_blocked_networks() {
        let dir = std::env::temp_dir().join(format!("crake-nets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("networks.json");
        let mut n = Networks::load(&file);
        assert!(!n.udp_blocked("10.0.0.1 aa"));
        assert!(n.block_udp("10.0.0.1 aa"));
        assert!(!n.block_udp("10.0.0.1 aa"), "not news twice");
        assert!(Networks::load(&file).udp_blocked("10.0.0.1 aa"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
