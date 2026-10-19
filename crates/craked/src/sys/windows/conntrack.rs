//! Windows: new TCP connections from the connection tables, IPv4's and
//! IPv6's (GetExtendedTcpTable), looked at four times a second; what
//! changed becomes the events Linux gets from conntrack (`flows::changes`).
//! For the Traffic window, each connection's bytes from Windows' extended
//! TCP statistics, switched on per connection while the window looks
//! (`open_connections`).

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use anyhow::Result;
use crake_os::windows::tcp::{self, Row};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    MIB_TCP_STATE_CLOSED, MIB_TCP_STATE_DELETE_TCB, MIB_TCP_STATE_ESTAB, MIB_TCP_STATE_LISTEN,
    MIB_TCP_STATE_SYN_RCVD, MIB_TCP_STATE_SYN_SENT, MIB_TCP6ROW, MIB_TCPROW_LH, MIB_TCPROW_LH_0,
};
use windows_sys::Win32::Networking::WinSock::{IN6_ADDR, IN6_ADDR_0};

pub use crate::flows::{Event, Flow, Kind, TcpState};

const EVERY: Duration = Duration::from_millis(250);

pub struct Events {
    seen: HashMap<Flow, TcpState>,
    /// When to look next. Kept across calls: the caller's select! drops an
    /// unfinished `next` whenever its own timer fires (every 200 ms), and a
    /// fresh 250 ms sleep each time would never end.
    due: tokio::time::Instant,
}

impl Events {
    /// What's open now is the starting point: only what comes after counts.
    pub fn open() -> Result<Self> {
        Ok(Self {
            seen: table().map_err(anyhow::Error::from)?,
            due: tokio::time::Instant::now() + EVERY,
        })
    }

    /// Cancel-safe: everything after the wait runs without awaiting.
    pub async fn next(&mut self) -> io::Result<Vec<Event>> {
        loop {
            tokio::time::sleep_until(self.due).await;
            self.due = tokio::time::Instant::now() + EVERY;
            let now = table()?;
            let events = crate::flows::changes(&self.seen, &now);
            self.seen = now;
            if !events.is_empty() {
                return Ok(events);
            }
        }
    }
}

fn state(raw: u32) -> TcpState {
    match raw as i32 {
        s if s == MIB_TCP_STATE_SYN_SENT => TcpState::SynSent,
        s if s == MIB_TCP_STATE_CLOSED => TcpState::Close,
        s if s == MIB_TCP_STATE_LISTEN || s == MIB_TCP_STATE_SYN_RCVD => TcpState::Other,
        // Established and every state after it: the other side answered.
        s if s >= MIB_TCP_STATE_ESTAB => TcpState::Answered,
        _ => TcpState::Other,
    }
}

/// A port as the table holds it (network byte order, in the low 16 bits).
fn port(raw: u32) -> u16 {
    u16::from_be(raw as u16)
}

/// The machine's TCP connections (not listeners), IPv4 and IPv6, and their
/// states.
fn table() -> io::Result<HashMap<Flow, TcpState>> {
    let mut found = HashMap::new();
    for r in tcp::table_v4()? {
        let flow = Flow {
            src: Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes()).into(),
            dst: Ipv4Addr::from(r.dwRemoteAddr.to_ne_bytes()).into(),
            sport: port(r.dwLocalPort),
            dport: port(r.dwRemotePort),
            udp: false,
        };
        found.insert(flow, state(r.dwState));
    }
    // A machine without IPv6 has no IPv6 table: IPv4's is enough.
    let Ok(rows) = tcp::table_v6() else {
        return Ok(found);
    };
    for r in rows {
        let flow = Flow {
            src: Ipv6Addr::from(r.ucLocalAddr).into(),
            dst: Ipv6Addr::from(r.ucRemoteAddr).into(),
            sport: port(r.dwLocalPort),
            dport: port(r.dwRemotePort),
            udp: false,
        };
        found.insert(flow, state(r.dwState));
    }
    Ok(found)
}

/// The process whose loopback connection comes from `from` to `to` (two
/// ports on 127.0.0.1): who it is that connected to a port of craked's.
pub fn loopback_owner(from: u16, to: u16) -> Option<u32> {
    tcp::table_v4()
        .ok()?
        .iter()
        .find(|r| {
            Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes()).is_loopback()
                && port(r.dwLocalPort) == from
                && port(r.dwRemotePort) == to
        })
        .map(|r| r.dwOwningPid)
}

/// Close this machine's TCP connections to `target`, so the apps open new
/// ones, found and routed as on a first visit (after Forget, an open
/// keep-alive connection would sit there, its packets now going direct).
pub async fn close_to(target: crate::hosts::Target) {
    let Ok(now) = table() else { return };
    for flow in now.into_keys().filter(|f| target.contains(f.dst)) {
        let _ = delete(flow);
    }
}

/// Reset a connection stuck without an answer, so the app's next attempt
/// takes the route Crake just added (Linux deletes its conntrack entry).
/// IPv4 only: Windows can't reset another program's IPv6 connection, which
/// runs its course; the app's next one takes the route.
pub fn delete(flow: Flow) -> io::Result<()> {
    let (IpAddr::V4(src), IpAddr::V4(dst)) = (flow.src, flow.dst) else {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    };
    let net = |port: u16| u32::from(port.to_be());
    let row = MIB_TCPROW_LH {
        Anonymous: MIB_TCPROW_LH_0 {
            dwState: MIB_TCP_STATE_DELETE_TCB as u32,
        },
        dwLocalAddr: u32::from_ne_bytes(src.octets()),
        dwLocalPort: net(flow.sport),
        dwRemoteAddr: u32::from_ne_bytes(dst.octets()),
        dwRemotePort: net(flow.dport),
    };
    tcp::set_entry(&row)
}

/// Statistics are switched back off this long after the window's last look.
const COUNTING_IDLE: std::time::Duration = std::time::Duration::from_secs(60);

/// The connections whose statistics craked switched on, and when the
/// window last looked; None while it isn't looking.
static COUNTING: std::sync::Mutex<Option<(std::time::Instant, std::collections::HashSet<Flow>)>> =
    std::sync::Mutex::new(None);

/// The open TCP connections with their bytes both ways, for the Traffic
/// window. Windows counts a connection's bytes once its extended
/// statistics are on, so each is switched on when first seen here (it
/// counts from then: an older one starts at 0). UDP isn't listed: Windows'
/// UDP table has no other end (CLAUDE.md, Not now).
pub fn open_connections() -> Option<Vec<(Flow, u64)>> {
    let now = match table() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("listing connections: {e}");
            return None;
        }
    };
    let mut counting = COUNTING.lock().unwrap();
    let (looked, on) =
        counting.get_or_insert_with(|| (std::time::Instant::now(), Default::default()));
    *looked = std::time::Instant::now();
    on.retain(|f| now.contains_key(f));
    let mut out = Vec::new();
    for (flow, state) in now {
        if state == TcpState::Close {
            continue;
        }
        if !on.contains(&flow) && statistics(flow, true) {
            on.insert(flow);
        }
        out.push((flow, bytes(flow).unwrap_or(0)));
    }
    Some(out)
}

/// The statistics craked switched on, off again once the window stopped
/// looking (or now, `force`: craked stopping).
pub fn connections_idle(force: bool) {
    let mut counting = COUNTING.lock().unwrap();
    if counting
        .as_ref()
        .is_some_and(|(looked, _)| force || looked.elapsed() > COUNTING_IDLE)
        && let Some((_, on)) = counting.take()
    {
        for flow in on {
            statistics(flow, false);
        }
    }
}

fn row(flow: Flow) -> Option<Row> {
    let net = |port: u16| u32::from(port.to_be());
    match (flow.src, flow.dst) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => Some(Row::V4(MIB_TCPROW_LH {
            Anonymous: MIB_TCPROW_LH_0 {
                dwState: MIB_TCP_STATE_ESTAB as u32,
            },
            dwLocalAddr: u32::from_ne_bytes(src.octets()),
            dwLocalPort: net(flow.sport),
            dwRemoteAddr: u32::from_ne_bytes(dst.octets()),
            dwRemotePort: net(flow.dport),
        })),
        (IpAddr::V6(src), IpAddr::V6(dst)) => Some(Row::V6(MIB_TCP6ROW {
            State: MIB_TCP_STATE_ESTAB,
            LocalAddr: IN6_ADDR {
                u: IN6_ADDR_0 { Byte: src.octets() },
            },
            dwLocalScopeId: 0,
            dwLocalPort: net(flow.sport),
            RemoteAddr: IN6_ADDR {
                u: IN6_ADDR_0 { Byte: dst.octets() },
            },
            dwRemoteScopeId: 0,
            dwRemotePort: net(flow.dport),
        })),
        _ => None,
    }
}

/// Switch a connection's data statistics on or off; true when Windows
/// took it (it needs the service's rights).
fn statistics(flow: Flow, on: bool) -> bool {
    row(flow).is_some_and(|r| tcp::statistics(&r, on))
}

/// A connection's bytes both ways since its statistics came on.
fn bytes(flow: Flow) -> Option<u64> {
    tcp::data_bytes(&row(flow)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real connection on this PC (loopback: nothing leaves it), listed
    /// with its bytes once its statistics are on (CI's runner has the
    /// rights craked's service has).
    #[test]
    fn a_real_connection_is_listed_with_its_bytes() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = std::net::TcpStream::connect(addr).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        let sport = client.local_addr().unwrap().port();
        let ours = |all: &[(Flow, u64)]| {
            all.iter()
                .find(|(f, _)| f.dport == addr.port() && f.sport == sport)
                .map(|(_, b)| *b)
        };
        // The first look switches its statistics on; what's sent after counts.
        let first = open_connections().expect("Windows lists connections");
        assert!(ours(&first).is_some(), "listed: {first:?}");
        client.write_all(&[7; 5000]).unwrap();
        let mut got = [0u8; 5000];
        server.read_exact(&mut got).unwrap();
        let then = open_connections().unwrap();
        let counted = ours(&then).unwrap();
        connections_idle(true);
        assert!(counted >= 5000, "its bytes: {counted}");
    }
}
