//! The computer's side of a report (D-Bus `Report`; the log window's
//! "Save a report…", `crake report`): the system's DNS, routes and Crake's
//! rules (`sys::report`), with Crake's own lines marked; whether the
//! internet answers and each tunnel's names do, tried now; craked's own
//! log; and the computer's plumbing, which hiding keeps.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use crake_proto::TunnelState;
use crake_proto::report::{Computer, Part};

use super::Manager;

/// A public name every network can look up: Cloudflare's resolver.
const PUBLIC_NAME: &str = "one.one.one.one";
/// The ports a check connects to, the first that answers.
const PORTS: [u16; 2] = [443, 80];
/// How long a lookup, or a connection, may take.
const CHECK_WAIT: Duration = Duration::from_secs(5);
/// Bits in an IPv4 and an IPv6 address.
const V4_BITS: u32 = 32;
const V6_BITS: u32 = 128;
/// craked's own DNS server (Windows, macOS) and its dummy links' (Linux).
const OWN_DNS: [&str; 3] = ["127.53.0.1", "169.254.53.53", "169.254.53.54"];

impl Manager {
    pub async fn report(&self) -> Computer {
        let (labels, mut keep) = self.plumbing();
        if !self.routing.is_real() {
            // A test's craked changes nothing and has nothing to show.
            return Computer {
                own_log: crate::own_log::recent(),
                keep,
                ..Computer::default()
            };
        }
        let checks = self.checks().await;
        let mut parts = crate::sys::report::parts().await;
        for part in &mut parts {
            for line in &mut part.lines {
                if let Some(label) = first_address(line).and_then(|a| labels.get(&a)) {
                    *line = format!("{line}    ← Crake: {label}");
                }
            }
        }
        keep.extend(routers(&parts));
        Computer {
            checks,
            parts,
            own_log: crate::own_log::recent(),
            keep,
        }
    }

    /// What Crake put on the system, by address ("ACME's DNS", "learned
    /// through ACME"), and the computer's plumbing.
    fn plumbing(&self) -> (HashMap<IpAddr, String>, Vec<String>) {
        let mut labels = HashMap::new();
        let mut keep: Vec<String> = OWN_DNS.iter().map(|s| (*s).to_owned()).collect();
        for (_, addr, prefix) in crate::sys::interfaces() {
            keep.push(addr.to_string());
            keep.push(network(
                IpAddr::V4(addr),
                u8::try_from(prefix).unwrap_or(u8::MAX),
            ));
        }
        for (_, addr, prefix) in crate::sys::interfaces6() {
            keep.push(addr.to_string());
            keep.push(network(IpAddr::V6(addr), prefix));
        }
        let tunnels: Vec<(String, String, Option<crate::info::TunnelInfo>)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.state == TunnelState::Connected)
            .map(|(n, e)| (n.clone(), e.config.display().to_owned(), e.info.clone()))
            .collect();
        let store = self.hosts.lock().unwrap();
        for (name, display, info) in tunnels {
            if let Some(info) = info {
                keep.extend(info.address.map(|a| a.to_string()));
                keep.extend(info.address6.map(|(a, _)| a.to_string()));
                for server in &info.servers {
                    keep.push(server.to_string());
                    labels.insert(*server, format!("{display}'s DNS"));
                }
            }
            if let Some(hosts) = store.tunnels.get(&name) {
                for addr in hosts.learned.keys() {
                    labels.insert(*addr, format!("learned through {display}"));
                }
                for t in &hosts.pinned {
                    labels.insert(t.addr(), format!("pinned to {display}"));
                }
            }
        }
        (labels, keep)
    }

    /// The internet, and each tunnel's names: looked up as any app would,
    /// and connected to, with the way the connection took.
    async fn checks(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "The internet: {}",
            self.check(PUBLIC_NAME, None, true).await
        )];
        let mut up: Vec<(usize, String, String)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.state == TunnelState::Connected)
            .map(|(n, e)| (e.index, n.clone(), e.config.display().to_owned()))
            .collect();
        up.sort();
        for (_, name, display) in up {
            let line = match self.named_host(&name) {
                Some((host, addr)) => self.check(&host, Some(addr), false).await,
                None => format!(
                    "Crake hasn't sent a host with a name through {display} yet, so none was tried."
                ),
            };
            lines.push(format!("{display}'s names: {line}"));
        }
        lines
    }

    /// The host `tunnel` used last of those it learned by name: its name
    /// and the address it was learned under.
    fn named_host(&self, tunnel: &str) -> Option<(String, IpAddr)> {
        let store = self.hosts.lock().unwrap();
        let hosts = store.tunnels.get(tunnel)?;
        hosts
            .learned
            .iter()
            .filter_map(|(addr, expiry)| hosts.names.get(addr).map(|n| (*expiry, n.clone(), *addr)))
            .max()
            .map(|(_, n, a)| (n, a))
    }

    /// "one.one.one.one is 1.1.1.1 (DNS answered in 9 ms); a connection
    /// to it on 443 took 18 ms, direct." A name that doesn't resolve is
    /// tried at `known` (the address Crake learned it under): DNS broken
    /// and the host fine say different things. `quiet`: the address is
    /// kept from probing a while, so a report never teaches a tunnel a
    /// public host.
    async fn check(&self, name: &str, known: Option<IpAddr>, quiet: bool) -> String {
        let asked = Instant::now();
        let found =
            tokio::time::timeout(CHECK_WAIT, tokio::net::lookup_host((name, PORTS[0]))).await;
        let (resolved, said) = match found {
            Ok(Ok(addrs)) => {
                let addrs: Vec<IpAddr> = addrs.map(|a| a.ip()).collect();
                let first = addrs
                    .iter()
                    .find(|a| a.is_ipv4())
                    .or(addrs.first())
                    .copied();
                let took = ms(asked.elapsed());
                match first {
                    Some(a) => (
                        Some(a),
                        format!("{name} is {a} (DNS answered in {took} ms);"),
                    ),
                    None => (None, format!("{name} has no address;")),
                }
            }
            Ok(Err(e)) => (None, format!("{name} didn't resolve ({e});")),
            Err(_) => (
                None,
                format!("{name} didn't resolve within {} s;", CHECK_WAIT.as_secs()),
            ),
        };
        let Some(addr) = resolved.or(known) else {
            return format!("{}.", said.trim_end_matches(';'));
        };
        let at = if resolved.is_some() {
            String::new()
        } else {
            format!(" at {addr}, the address Crake learned it under,")
        };
        if quiet {
            self.probed.lock().unwrap().insert(addr, Instant::now());
        }
        let way = {
            let local = crate::probe::Local::load();
            let (full, gateways) = self.full_and_gateways();
            match self.way(addr, full.as_deref(), &gateways, &local) {
                Some(t) => format!("through {}", self.label(&t)),
                None => "direct".to_owned(),
            }
        };
        let mut tried = Vec::new();
        for port in PORTS {
            let start = Instant::now();
            match tokio::time::timeout(CHECK_WAIT, tokio::net::TcpStream::connect((addr, port)))
                .await
            {
                Ok(Ok(_)) => {
                    return format!(
                        "{said}{at} a connection to it on {port} took {} ms, {way}.",
                        ms(start.elapsed())
                    );
                }
                Ok(Err(e)) => tried.push(format!("on {port}: {e}")),
                Err(_) => tried.push(format!(
                    "on {port}: no answer within {} s",
                    CHECK_WAIT.as_secs()
                )),
            }
        }
        format!(
            "{said}{at} no connection to it, {way} ({}).",
            tried.join("; ")
        )
    }
}

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

/// "192.168.1.0/24" for 192.168.1.20 and 24.
fn network(addr: IpAddr, prefix: u8) -> String {
    let net = match addr {
        IpAddr::V4(a) => {
            let bits = u32::from(a);
            let mask = u32::MAX
                .checked_shl(V4_BITS - u32::from(prefix).min(V4_BITS))
                .unwrap_or(0);
            IpAddr::V4((bits & mask).into())
        }
        IpAddr::V6(a) => {
            let bits = u128::from(a);
            let mask = u128::MAX
                .checked_shl(V6_BITS - u32::from(prefix).min(V6_BITS))
                .unwrap_or(0);
            IpAddr::V6((bits & mask).into())
        }
    };
    format!("{net}/{prefix}")
}

/// A line's first word, when it's an address (a route's destination):
/// "10.40.0.2", "10.40.0.2/32", "fd40::2".
fn first_address(line: &str) -> Option<IpAddr> {
    let word = line.split_whitespace().next()?;
    let word = word
        .strip_suffix("/32")
        .or_else(|| word.strip_suffix("/128"))
        .unwrap_or(word);
    word.parse().ok()
}

/// The routers in the routes: "default via 192.0.2.1 …" (Linux),
/// "default 192.0.2.1 UGScg en0" (macOS), "0.0.0.0 0.0.0.0 192.0.2.1 …"
/// (Windows).
fn routers(parts: &[Part]) -> Vec<String> {
    let mut found = Vec::new();
    for line in parts.iter().flat_map(|p| &p.lines) {
        let words: Vec<&str> = line.split_whitespace().collect();
        let after = match words.as_slice() {
            ["default", rest @ ..] => rest,
            ["0.0.0.0", "0.0.0.0", rest @ ..] => rest,
            _ => continue,
        };
        let router = after.iter().find_map(|w| {
            let w = w.split('%').next().unwrap_or(w);
            w.parse::<IpAddr>().ok()
        });
        found.extend(router.map(|r| r.to_string()));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_routers_are_read_from_each_systems_routes() {
        let parts = [Part::new(
            "Routes",
            [
                "default via 192.0.2.1 dev wlp2s0 proto dhcp metric 600",
                "default via fe80::1 dev wlp2s0 proto ra metric 600",
                "default            198.51.100.1        UGScg            en0",
                "default            fe80::1%en0         UGcg             en0",
                "          0.0.0.0          0.0.0.0     203.0.113.1    203.0.113.20     35",
                "10.40.0.2 dev crake-acme scope link",
            ]
            .map(String::from),
        )];
        assert_eq!(
            routers(&parts),
            [
                "192.0.2.1",
                "fe80::1",
                "198.51.100.1",
                "fe80::1",
                "203.0.113.1"
            ]
        );
    }

    #[test]
    fn a_routes_destination_is_its_first_word() {
        assert_eq!(
            first_address("10.40.0.2 dev crake-acme scope link"),
            Some("10.40.0.2".parse().unwrap())
        );
        assert_eq!(
            first_address("10.40.1.10/32       utun4"),
            Some("10.40.1.10".parse().unwrap())
        );
        assert_eq!(first_address("default via 192.0.2.1"), None);
        assert_eq!(
            network("192.168.1.20".parse().unwrap(), 24),
            "192.168.1.0/24"
        );
        assert_eq!(network("fd40:ff::5".parse().unwrap(), 64), "fd40:ff::/64");
    }
}
