//! macOS: new TCP connections from the kernel's connection list (the
//! `net.inet.tcp.pcblist_n` sysctl that netstat reads), looked at four
//! times a second; what changed becomes the events Linux gets from
//! conntrack (`flows::changes`), as on Windows.

use std::collections::HashMap;
use std::io;
use std::net::Ipv4Addr;
use std::time::Duration;

use anyhow::Result;

pub use crate::flows::{Event, Flow, Kind, TcpState};

const EVERY: Duration = Duration::from_millis(250);

pub struct Events {
    seen: HashMap<Flow, TcpState>,
    /// When to look next. Kept across calls: the caller's select! drops an
    /// unfinished `next` whenever its own timer fires.
    due: tokio::time::Instant,
}

impl Events {
    /// What's open now is the starting point: only what comes after counts.
    pub fn open() -> Result<Self> {
        Ok(Self {
            seen: table()?,
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

/// Nothing to reset from outside on macOS: the app's next SYN takes the
/// route Crake just added.
pub fn delete(_flow: Flow) -> io::Result<()> {
    Ok(())
}

/// The machine's IPv4 TCP connections (not listeners) and their states.
fn table() -> io::Result<HashMap<Flow, TcpState>> {
    let name = c"net.inet.tcp.pcblist_n";
    let mut len = 0;
    // SAFETY: a size query (no buffer), then a buffer of that size (plus
    // room for connections opened meanwhile).
    unsafe {
        if libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return Err(io::Error::last_os_error());
        }
        len += 64 * 1024;
        let mut buf = vec![0u8; len];
        if libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return Err(io::Error::last_os_error());
        }
        buf.truncate(len);
        Ok(parse(&buf))
    }
}

/// Record kinds (xnu's XSO_*).
const XSO_INPCB: u32 = 0x10;
const XSO_TCPCB: u32 = 0x20;
/// The list starts and ends with an `xinpgen` this long.
const XINPGEN: usize = 24;
/// A connection's record says IPv4 (`inp_vflag & INP_IPV4`).
const INP_IPV4: u8 = 0x1;

/// The list: an `xinpgen`, then per connection its records (`xinpcb_n`,
/// the socket's, `xtcpcb_n`), each led by its length and kind and padded
/// to 8 bytes, then another `xinpgen`.
pub fn parse(buf: &[u8]) -> HashMap<Flow, TcpState> {
    let u32_at = |at: usize| {
        buf.get(at..at + 4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
    };
    let u16_net = |at: usize| {
        buf.get(at..at + 2)
            .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
    };
    let ip_at = |at: usize| {
        buf.get(at..at + 4)
            .map(|b| Ipv4Addr::new(b[0], b[1], b[2], b[3]))
    };
    let mut found = HashMap::new();
    let mut current: Option<Flow> = None;
    let mut at = XINPGEN;
    while at + XINPGEN < buf.len() {
        let (Some(len), Some(kind)) = (u32_at(at), u32_at(at + 4)) else {
            break;
        };
        let len = len as usize;
        if len < 8 {
            break;
        }
        match kind {
            // xinpcb_n: ports at 16 and 18, inp_vflag at 48, the foreign
            // and local addresses (in_addr_4in6) at 52 and 68, IPv4 in
            // their last four bytes.
            XSO_INPCB => {
                current = None;
                if buf.get(at + 48).is_some_and(|v| v & INP_IPV4 != 0)
                    && let (Some(dport), Some(sport), Some(dst), Some(src)) = (
                        u16_net(at + 16),
                        u16_net(at + 18),
                        ip_at(at + 64),
                        ip_at(at + 80),
                    )
                    && !dst.is_unspecified()
                {
                    current = Some(Flow {
                        src,
                        dst,
                        sport,
                        dport,
                    });
                }
            }
            // xtcpcb_n: t_state at 36.
            XSO_TCPCB => {
                if let (Some(flow), Some(state)) = (current.take(), u32_at(at + 36)) {
                    found.insert(flow, tcp_state(state));
                }
            }
            _ => {}
        }
        at += len.next_multiple_of(8);
    }
    found
}

/// BSD's TCP states (netinet/tcp_fsm.h).
fn tcp_state(raw: u32) -> TcpState {
    match raw {
        0 => TcpState::Close,
        2 => TcpState::SynSent,
        // Established and every state after it: the other side answered.
        s if s >= 4 => TcpState::Answered,
        // Listening, a SYN received.
        _ => TcpState::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One connection's records as the kernel lays them out.
    fn connection(src: [u8; 4], sport: u16, dst: [u8; 4], dport: u16, state: u32) -> Vec<u8> {
        let mut inpcb = vec![0u8; 104];
        inpcb[0..4].copy_from_slice(&104u32.to_ne_bytes());
        inpcb[4..8].copy_from_slice(&XSO_INPCB.to_ne_bytes());
        inpcb[16..18].copy_from_slice(&dport.to_be_bytes());
        inpcb[18..20].copy_from_slice(&sport.to_be_bytes());
        inpcb[48] = INP_IPV4;
        inpcb[64..68].copy_from_slice(&dst);
        inpcb[80..84].copy_from_slice(&src);
        let mut socket = vec![0u8; 92]; // padded to 96
        socket[0..4].copy_from_slice(&92u32.to_ne_bytes());
        socket[4..8].copy_from_slice(&1u32.to_ne_bytes());
        socket.resize(96, 0);
        let mut tcpcb = vec![0u8; 40];
        tcpcb[0..4].copy_from_slice(&40u32.to_ne_bytes());
        tcpcb[4..8].copy_from_slice(&XSO_TCPCB.to_ne_bytes());
        tcpcb[36..40].copy_from_slice(&state.to_ne_bytes());
        [inpcb, socket, tcpcb].concat()
    }

    #[test]
    fn reads_the_kernels_list() {
        let generation = |count: u32| {
            let mut g = vec![0u8; XINPGEN];
            g[0..4].copy_from_slice(&(XINPGEN as u32).to_ne_bytes());
            // The count where a record has its kind: a list of 16 or 32
            // mustn't read as one.
            g[4..8].copy_from_slice(&count.to_ne_bytes());
            g
        };
        let buf = [
            generation(32),
            connection([192, 0, 2, 5], 50000, [10, 40, 1, 10], 443, 2),
            connection([192, 0, 2, 5], 50001, [198, 51, 100, 7], 80, 4),
            generation(32),
        ]
        .concat();
        let found = parse(&buf);
        assert_eq!(found.len(), 2);
        let stuck = Flow {
            src: Ipv4Addr::new(192, 0, 2, 5),
            dst: Ipv4Addr::new(10, 40, 1, 10),
            sport: 50000,
            dport: 443,
        };
        assert_eq!(found.get(&stuck), Some(&TcpState::SynSent));
        assert!(found.values().any(|s| *s == TcpState::Answered));
    }
}
