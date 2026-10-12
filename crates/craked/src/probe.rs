//! Connection probing: new outbound TCP connections that get no answer within
//! the configured wait, or get refused, are tried through the tunnels. If one
//! answers, the manager routes the host through it.
//!
//! Watching is passive (conntrack events); no packet is held.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::net::TcpSocket;

use crate::conntrack::{Events, Flow, Kind, TcpState};
use crate::hosts::Target;
use crate::manager::Manager;
use crate::packet;

/// What happened to the direct attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direct {
    NoAnswer,
    Refused,
    /// A router said it can't reach the host (ICMP), noticed at once
    /// (`unreachable`), without waiting.
    Unreachable,
}

impl Direct {
    /// How the direct attempt went, for the log's reason.
    pub fn said(self) -> &'static str {
        match self {
            Self::NoAnswer => "It didn't answer directly.",
            Self::Refused => "Refused directly.",
            Self::Unreachable => "Unreachable directly.",
        }
    }
}

/// The flow of a first SYN held in NFQUEUE (an IPv4 TCP packet with SYN set
/// and ACK clear): (source, destination, ports).
pub fn syn_flow(packet: &[u8]) -> Option<Flow> {
    let ip = packet::V4::parse(packet, packet::TCP)?;
    let tcp = ip.payload()?;
    if !packet::first_syn(tcp) {
        return None;
    }
    Some(Flow {
        src: ip.src().into(),
        dst: ip.dst().into(),
        sport: packet::source_port(tcp)?,
        dport: packet::destination_port(tcp)?,
    })
}

/// The direct attempt's outcome in the first-SYN race (`race_held`).
pub enum DirectTry {
    /// Direct works: the host is reachable without a tunnel.
    Answered(Duration),
    /// Direct doesn't work (yet): why, for the log if a tunnel takes it.
    Failed(Direct),
}

/// Try `addr` by the default route (no tunnel mark), up to `timeout`. A RST
/// is "refused", an ICMP unreachable is "unreachable", a timeout is "no
/// answer" (the host is silently dropped: only a tunnel might reach it).
pub async fn direct(addr: SocketAddr, timeout: Duration) -> DirectTry {
    use std::io::ErrorKind;
    let start = Instant::now();
    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_)) => DirectTry::Answered(start.elapsed()),
        Ok(Err(e)) => DirectTry::Failed(match e.kind() {
            ErrorKind::ConnectionRefused => Direct::Refused,
            ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => Direct::Unreachable,
            _ => Direct::NoAnswer,
        }),
        Err(_) => DirectTry::Failed(Direct::NoAnswer),
    }
}

/// The connection an ICMP "unreachable" is about, when it is about one of
/// this machine's TCP connections: network or host unreachable, or
/// prohibited (never "fragmentation needed", which path MTU discovery uses).
/// It carries the start of the packet that couldn't be delivered.
pub fn unreachable(packet: &[u8]) -> Option<Flow> {
    // Network or host unreachable (0, 1); network, host or anything
    // administratively prohibited (9, 10, 13).
    const CODES: [u8; 5] = [0, 1, 9, 10, 13];
    let icmp = packet::V4::parse(packet, packet::ICMP)?.payload()?;
    let inner = packet::V4::parse(packet::unreachable_about(icmp, &CODES)?, packet::TCP)?;
    let tcp = inner.payload()?;
    Some(Flow {
        src: inner.src().into(),
        dst: inner.dst().into(),
        sport: packet::source_port(tcp)?,
        dport: packet::destination_port(tcp)?,
    })
}

/// How long an address a tunnel's DNS gave (for a name, private) gets to
/// answer directly before it's tried through the tunnel: a round trip,
/// not the probe wait. An app that kept the address from an earlier
/// lookup (its own DNS cache) reaches it without asking DNS, so Crake can't
/// route it before the first packet; this gets it routed before the
/// system's first SYN retry (1 s on Linux).
pub const KNOWN_INSIDE_WAIT: Duration = Duration::from_millis(300);

/// How often the connections still waiting for an answer are looked at.
const PENDING_CHECK: Duration = Duration::from_millis(100);

/// Watch for the daemon's lifetime.
pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    let mut events = Events::open()?;
    tokio::spawn(async move {
        // Each stuck connection: since when, and how long it may take.
        let mut pending: HashMap<Flow, (Instant, Duration)> = HashMap::new();
        let mut tick = tokio::time::interval(PENDING_CHECK);
        loop {
            tokio::select! {
                batch = events.next() => match batch {
                    Ok(batch) => {
                        for ev in batch {
                            match (ev.kind, ev.state) {
                                (Kind::New, Some(TcpState::SynSent)) => {
                                    if mgr.any_routed() {
                                        // Read each time: Settings can change it.
                                        let wait = if mgr.known_inside(ev.flow.dst) {
                                            KNOWN_INSIDE_WAIT
                                        } else {
                                            mgr.probe_wait()
                                        };
                                        pending.insert(ev.flow, (Instant::now(), wait));
                                    }
                                }
                                // A reset before it was ever answered: refused. The
                                // kernel usually drops such an entry at once (a
                                // destroy without state); an unanswered SYN can't
                                // time out this soon, so a destroy here means refused.
                                (Kind::Update, Some(TcpState::Close)) | (Kind::Destroy, _) => {
                                    if pending.remove(&ev.flow).is_some() {
                                        let mgr = Arc::clone(&mgr);
                                        tokio::spawn(async move { mgr.probe(ev.flow, Direct::Refused).await });
                                    }
                                }
                                (Kind::Update, Some(TcpState::Answered)) => {
                                    pending.remove(&ev.flow);
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("connection probing stopped: {e}");
                        return;
                    }
                },
                _ = tick.tick() => {
                    let due: Vec<Flow> = pending
                        .iter()
                        .filter(|(_, (at, wait))| at.elapsed() >= *wait)
                        .map(|(f, _)| *f)
                        .collect();
                    for flow in due {
                        pending.remove(&flow);
                        let mgr = Arc::clone(&mgr);
                        tokio::spawn(async move { mgr.probe(flow, Direct::NoAnswer).await });
                    }
                }
            }
        }
    });
    Ok(())
}

/// Try `addr` with the tunnel's mark, the same path its traffic would take.
/// Returns how long the handshake took.
pub async fn through_tunnel(
    addr: impl Into<SocketAddr>,
    mark: u32,
    timeout: Duration,
) -> Option<Duration> {
    let addr = addr.into();
    let sock = if addr.is_ipv4() {
        TcpSocket::new_v4()
    } else {
        TcpSocket::new_v6()
    }
    .ok()?;
    crate::sys::mark(&sock, mark).ok()?;
    let start = Instant::now();
    match tokio::time::timeout(timeout, sock.connect(addr)).await {
        Ok(Ok(_stream)) => Some(start.elapsed()),
        _ => None,
    }
}

/// A device's IPv4 address and prefix (an OpenVPN 3 tunnel's, which
/// openvpn3 sets, not craked).
pub fn ipv4_of(device: &str) -> Option<(Ipv4Addr, u8)> {
    crate::sys::interfaces()
        .into_iter()
        .find(|(name, _, _)| name == device)
        .map(|(_, a, m)| (a, m.leading_ones() as u8))
}

/// The machine's own addresses and the networks they're on, excluding
/// Crake's tunnels.
pub struct Local {
    nets: Vec<(Ipv4Addr, u32)>,
    /// IPv6: address and prefix length.
    nets6: Vec<(Ipv6Addr, u8)>,
}

impl Local {
    pub fn load() -> Self {
        let nets = crate::sys::interfaces()
            .into_iter()
            .filter(|(name, _, _)| !crate::sys::is_tunnel_device(name))
            .map(|(_, a, m)| (a, m))
            .collect();
        let nets6 = crate::sys::interfaces6()
            .into_iter()
            .filter(|(name, _, _)| !crate::sys::is_tunnel_device(name))
            .map(|(_, a, p)| (a, p))
            .collect();
        Self { nets, nets6 }
    }

    /// The IPv4 networks the machine is directly attached to (not tunnels).
    pub fn networks(&self) -> Vec<Target> {
        self.nets
            .iter()
            .filter_map(|(a, m)| {
                let prefix = m.leading_ones();
                format!("{}/{prefix}", Ipv4Addr::from(u32::from(*a) & m))
                    .parse()
                    .ok()
            })
            .collect()
    }

    /// One of the machine's own (non-tunnel) addresses.
    pub fn is_own(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(ip) => self.nets.iter().any(|(a, _)| *a == ip),
            IpAddr::V6(ip) => self.nets6.iter().any(|(a, _)| *a == ip),
        }
    }

    /// On a network the machine is directly attached to (the LAN, a bridge).
    /// Those are never a tunnel's business.
    pub fn on_link(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(ip) => self
                .nets
                .iter()
                .any(|(a, m)| *m != u32::MAX && u32::from(*a) & m == u32::from(ip) & m),
            // Link-local (fe80::/10) is on every link; the rest by prefix.
            IpAddr::V6(ip) => {
                ip.is_unicast_link_local()
                    || self.nets6.iter().any(|(a, p)| {
                        let p = u32::from(*p);
                        let m = u128::MAX.checked_shl(Ipv6Addr::BITS - p).unwrap_or(0);
                        p < Ipv6Addr::BITS && u128::from(*a) & m == u128::from(ip) & m
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An ICMP "destination unreachable" (`code`) about a SYN from
    /// 192.168.0.242:40000 to 10.70.0.8:443, as a router sends it.
    fn icmp(code: u8, inner_proto: u8) -> Vec<u8> {
        let mut p = vec![
            0x45, 0, 0, 56, 0, 0, 0, 0, 64, 1, 0, 0, 192, 168, 0, 1, 192, 168, 0, 242,
        ];
        p.extend_from_slice(&[3, code, 0, 0, 0, 0, 0, 0]);
        p.extend_from_slice(&[
            0x45,
            0,
            0,
            60,
            0,
            0,
            0x40,
            0,
            63,
            inner_proto,
            0,
            0,
            192,
            168,
            0,
            242,
            10,
            70,
            0,
            8,
        ]);
        p.extend_from_slice(&[0x9c, 0x40, 0x01, 0xbb, 0, 0, 0, 1]);
        p
    }

    #[test]
    fn reads_the_connection_an_unreachable_is_about() {
        let flow = unreachable(&icmp(1, 6)).unwrap();
        assert_eq!(
            (flow.src, flow.dst, flow.sport, flow.dport),
            (
                Ipv4Addr::new(192, 168, 0, 242).into(),
                Ipv4Addr::new(10, 70, 0, 8).into(),
                40000,
                443
            )
        );
        assert!(unreachable(&icmp(13, 6)).is_some(), "prohibited");
        assert!(
            unreachable(&icmp(4, 6)).is_none(),
            "fragmentation needed: path MTU"
        );
        assert!(
            unreachable(&icmp(3, 6)).is_none(),
            "port unreachable: not TCP's"
        );
        assert!(unreachable(&icmp(1, 17)).is_none(), "about UDP");
        assert!(unreachable(b"short").is_none());
    }

    #[test]
    fn local_networks() {
        let l = Local {
            nets: vec![
                ("192.168.0.242".parse().unwrap(), 0xffff_ff00),
                ("10.40.255.7".parse().unwrap(), u32::MAX),
            ],
            nets6: vec![
                ("2001:db8:1::42".parse().unwrap(), 64),
                ("2001:db8:9::7".parse().unwrap(), 128),
            ],
        };
        assert!(l.is_own("192.168.0.242".parse().unwrap()));
        assert!(l.on_link("192.168.0.50".parse().unwrap()));
        assert!(
            !l.on_link("10.40.255.8".parse().unwrap()),
            "a /32 is no network"
        );
        assert!(!l.on_link("10.58.70.119".parse().unwrap()));
        assert!(l.is_own("2001:db8:1::42".parse().unwrap()));
        assert!(l.on_link("2001:db8:1::99".parse().unwrap()));
        assert!(l.on_link("fe80::1".parse().unwrap()), "link-local");
        assert!(
            !l.on_link("2001:db8:9::8".parse().unwrap()),
            "a /128 is no network"
        );
        assert!(!l.on_link("2001:db8:2::1".parse().unwrap()));
    }
}
