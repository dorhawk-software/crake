//! Connection probing: new outbound TCP connections that get no answer within
//! the configured wait, or get refused, are tried through the tunnels. If one
//! answers, the manager routes the host through it.
//!
//! Watching is passive (conntrack events); no packet is held.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::net::TcpSocket;

use crate::conntrack::{Events, Flow, Kind, TcpState};
use crate::hosts::Target;
use crate::manager::Manager;

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

/// The connection an ICMP "unreachable" is about, when it is about one of
/// this machine's TCP connections: network or host unreachable, or
/// prohibited (never "fragmentation needed", which path MTU discovery uses).
/// It carries the start of the packet that couldn't be delivered.
pub fn unreachable(packet: &[u8]) -> Option<Flow> {
    const CODES: [u8; 5] = [0, 1, 9, 10, 13];
    if packet.len() < 20 || packet[0] >> 4 != 4 || packet[9] != 1 {
        return None;
    }
    let icmp = packet.get(usize::from(packet[0] & 0x0f) * 4..)?;
    if *icmp.first()? != 3 || !CODES.contains(icmp.get(1)?) {
        return None;
    }
    let inner = icmp.get(8..)?;
    if inner.len() < 20 || inner[0] >> 4 != 4 || inner[9] != 6 {
        return None;
    }
    let tcp = inner.get(usize::from(inner[0] & 0x0f) * 4..)?;
    Some(Flow {
        src: Ipv4Addr::new(inner[12], inner[13], inner[14], inner[15]),
        dst: Ipv4Addr::new(inner[16], inner[17], inner[18], inner[19]),
        sport: u16::from_be_bytes([*tcp.first()?, *tcp.get(1)?]),
        dport: u16::from_be_bytes([*tcp.get(2)?, *tcp.get(3)?]),
    })
}

/// Watch for the daemon's lifetime.
pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    let mut events = Events::open()?;
    let wait = mgr.probe_wait();
    tokio::spawn(async move {
        let mut pending: HashMap<Flow, Instant> = HashMap::new();
        let mut tick = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                batch = events.next() => match batch {
                    Ok(batch) => {
                        for ev in batch {
                            match (ev.kind, ev.state) {
                                (Kind::New, Some(TcpState::SynSent)) => {
                                    if mgr.any_routed() {
                                        pending.insert(ev.flow, Instant::now());
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
                        .filter(|(_, at)| at.elapsed() >= wait)
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
pub async fn through_tunnel(addr: SocketAddrV4, mark: u32, timeout: Duration) -> Option<Duration> {
    let sock = TcpSocket::new_v4().ok()?;
    crate::sys::mark(&sock, mark).ok()?;
    let start = Instant::now();
    match tokio::time::timeout(timeout, sock.connect(addr.into())).await {
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

/// The machine's own IPv4 addresses and the networks they're on, excluding
/// Crake's tunnels.
pub struct Local {
    nets: Vec<(Ipv4Addr, u32)>,
}

impl Local {
    pub fn load() -> Self {
        let nets = crate::sys::interfaces()
            .into_iter()
            .filter(|(name, _, _)| !name.starts_with("crake-"))
            .map(|(_, a, m)| (a, m))
            .collect();
        Self { nets }
    }

    /// The networks the machine is directly attached to (not tunnels).
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
    pub fn is_own(&self, ip: Ipv4Addr) -> bool {
        self.nets.iter().any(|(a, _)| *a == ip)
    }

    /// On a network the machine is directly attached to (the LAN, a bridge).
    /// Those are never a tunnel's business.
    pub fn on_link(&self, ip: Ipv4Addr) -> bool {
        self.nets
            .iter()
            .any(|(a, m)| *m != u32::MAX && u32::from(*a) & m == u32::from(ip) & m)
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
                Ipv4Addr::new(192, 168, 0, 242),
                Ipv4Addr::new(10, 70, 0, 8),
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
        };
        assert!(l.is_own("192.168.0.242".parse().unwrap()));
        assert!(l.on_link("192.168.0.50".parse().unwrap()));
        assert!(
            !l.on_link("10.40.255.8".parse().unwrap()),
            "a /32 is no network"
        );
        assert!(!l.on_link("10.58.70.119".parse().unwrap()));
    }
}
