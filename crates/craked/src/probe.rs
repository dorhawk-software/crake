//! Connection probing: new outbound TCP connections that get no answer within
//! the configured wait, or get refused, are tried through the tunnels. If one
//! answers, the manager routes the host through it.
//!
//! Watching is passive (conntrack events); no packet is held.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::os::fd::AsRawFd;
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
    // SAFETY: plain setsockopt with a u32 on a socket we own.
    let r = unsafe {
        libc::setsockopt(
            sock.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_MARK,
            (&mark as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    if r != 0 {
        return None;
    }
    let start = Instant::now();
    match tokio::time::timeout(timeout, sock.connect(addr.into())).await {
        Ok(Ok(_stream)) => Some(start.elapsed()),
        _ => None,
    }
}

/// A device's IPv4 address and prefix (an OpenVPN 3 tunnel's, which
/// openvpn3 sets, not craked).
pub fn ipv4_of(device: &str) -> Option<(Ipv4Addr, u8)> {
    let mut found = None;
    // SAFETY: getifaddrs/freeifaddrs pair; we only read the list.
    unsafe {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut head) == 0 {
            let mut p = head;
            while !p.is_null() {
                let ifa = &*p;
                let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy();
                if found.is_none()
                    && name == device
                    && !ifa.ifa_addr.is_null()
                    && i32::from((*ifa.ifa_addr).sa_family) == libc::AF_INET
                    && !ifa.ifa_netmask.is_null()
                {
                    let a = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    let m = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
                    found = Some((
                        Ipv4Addr::from(u32::from_be(a.sin_addr.s_addr)),
                        u32::from_be(m.sin_addr.s_addr).leading_ones() as u8,
                    ));
                }
                p = ifa.ifa_next;
            }
            libc::freeifaddrs(head);
        }
    }
    found
}

/// The machine's own IPv4 addresses and the networks they're on, excluding
/// Crake's tunnels.
pub struct Local {
    nets: Vec<(Ipv4Addr, u32)>,
}

impl Local {
    pub fn load() -> Self {
        let mut nets = Vec::new();
        // SAFETY: getifaddrs/freeifaddrs pair; we only read the list.
        unsafe {
            let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
            if libc::getifaddrs(&mut head) == 0 {
                let mut p = head;
                while !p.is_null() {
                    let ifa = &*p;
                    let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy();
                    if !ifa.ifa_addr.is_null()
                        && i32::from((*ifa.ifa_addr).sa_family) == libc::AF_INET
                        && !ifa.ifa_netmask.is_null()
                        && !name.starts_with("crake-")
                    {
                        let a = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                        let m = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
                        nets.push((
                            Ipv4Addr::from(u32::from_be(a.sin_addr.s_addr)),
                            u32::from_be(m.sin_addr.s_addr),
                        ));
                    }
                    p = ifa.ifa_next;
                }
                libc::freeifaddrs(head);
            }
        }
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
