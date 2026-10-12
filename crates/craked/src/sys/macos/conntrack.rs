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

/// macOS can't close another process's connections from outside: after
/// Forget, an open connection ends when the app gives up on it.
pub async fn close_to(_target: crate::hosts::Target) {}

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
        len += ROOM;
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
/// The list's size as asked, plus room for connections opened meanwhile.
const ROOM: usize = 64 * 1024;
/// Each record starts with its length and kind (u32 each), and is padded to
/// eight bytes.
const RECORD_KIND: usize = 4;
const RECORD_MIN: usize = 8;
const RECORD_ALIGN: usize = 8;
/// xinpcb_n: the foreign and local ports.
const INP_FPORT: usize = 16;
const INP_LPORT: usize = 18;
/// Where inp_vflag is: the struct packed to four bytes (104 long; macOS 26)
/// or not (112).
const PACKED_LEN: usize = 104;
const PACKED_VFLAG: usize = 44;
const ALIGNED_LEN: usize = 112;
const ALIGNED_VFLAG: usize = 48;
/// After inp_vflag: the foreign and local addresses, each an in_addr_4in6.
const FADDR_AFTER_VFLAG: usize = 16;
const LADDR_AFTER_VFLAG: usize = 32;
/// xtcpcb_n: t_state.
const TCP_STATE: usize = 36;
/// BSD's TCP states (netinet/tcp_fsm.h): closed, a SYN out, established
/// (and every state after it).
const TCPS_CLOSED: u32 = 0;
const TCPS_SYN_SENT: u32 = 2;
const TCPS_ESTABLISHED: u32 = 4;

/// The list: an `xinpgen`, then per connection its records (`xinpcb_n`,
/// the socket's, `xtcpcb_n`), each led by its length and kind and padded
/// to 8 bytes, then another `xinpgen`.
pub fn parse(buf: &[u8]) -> HashMap<Flow, TcpState> {
    let u32_at = |at: usize| {
        buf.get(at..at + size_of::<u32>())
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
    };
    let u16_net = |at: usize| {
        buf.get(at..at + 2)
            .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
    };
    let ip_at = |at: usize| {
        buf.get(at..at + size_of::<Ipv4Addr>()).map(|b| {
            let mut octets = [0; size_of::<Ipv4Addr>()];
            octets.copy_from_slice(b);
            Ipv4Addr::from(octets)
        })
    };
    let mut found = HashMap::new();
    let mut current: Option<Flow> = None;
    let mut at = XINPGEN;
    while at + XINPGEN < buf.len() {
        let (Some(len), Some(kind)) = (u32_at(at), u32_at(at + RECORD_KIND)) else {
            break;
        };
        let len = len as usize;
        if len < RECORD_MIN {
            break;
        }
        match kind {
            // xinpcb_n: ports at 16 and 18; then inp_vflag, and the
            // foreign and local addresses (in_addr_4in6, IPv4 in their last
            // four bytes) 16 and 32 bytes after it. Where the flag is
            // depends on whether the kernel packs the struct to 4 bytes (104
            // long, the flag at 44: macOS 26 does) or not (112, at 48).
            XSO_INPCB => {
                current = None;
                let flag = match len {
                    PACKED_LEN => Some(PACKED_VFLAG),
                    ALIGNED_LEN => Some(ALIGNED_VFLAG),
                    _ => None,
                };
                if let Some(v) = flag
                    && buf.get(at + v).is_some_and(|f| f & INP_IPV4 != 0)
                    && let (Some(dport), Some(sport), Some(dst), Some(src)) = (
                        u16_net(at + INP_FPORT),
                        u16_net(at + INP_LPORT),
                        ip_at(at + v + FADDR_AFTER_VFLAG),
                        ip_at(at + v + LADDR_AFTER_VFLAG),
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
                if let (Some(flow), Some(state)) = (current.take(), u32_at(at + TCP_STATE)) {
                    found.insert(flow, tcp_state(state));
                }
            }
            _ => {}
        }
        at += len.next_multiple_of(RECORD_ALIGN);
    }
    found
}

/// BSD's TCP states (netinet/tcp_fsm.h).
fn tcp_state(raw: u32) -> TcpState {
    match raw {
        TCPS_CLOSED => TcpState::Close,
        TCPS_SYN_SENT => TcpState::SynSent,
        // Established and every state after it: the other side answered.
        s if s >= TCPS_ESTABLISHED => TcpState::Answered,
        // Listening, a SYN received.
        _ => TcpState::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One connection's records as the kernel lays them out, its
    /// `xinpcb_n` packed to 4 bytes or not.
    fn connection(
        packed: bool,
        (src, sport): ([u8; 4], u16),
        (dst, dport): ([u8; 4], u16),
        state: u32,
    ) -> Vec<u8> {
        let (len, flag) = if packed { (104, 44) } else { (112, 48) };
        let mut inpcb = vec![0u8; len];
        inpcb[0..4].copy_from_slice(&(len as u32).to_ne_bytes());
        inpcb[4..8].copy_from_slice(&XSO_INPCB.to_ne_bytes());
        inpcb[16..18].copy_from_slice(&dport.to_be_bytes());
        inpcb[18..20].copy_from_slice(&sport.to_be_bytes());
        inpcb[flag] = INP_IPV4;
        inpcb[flag + 16..flag + 20].copy_from_slice(&dst);
        inpcb[flag + 32..flag + 36].copy_from_slice(&src);
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
        for packed in [false, true] {
            let buf = [
                generation(32),
                connection(packed, ([192, 0, 2, 5], 50000), ([10, 40, 1, 10], 443), 2),
                connection(packed, ([192, 0, 2, 5], 50001), ([198, 51, 100, 7], 80), 4),
                generation(32),
            ]
            .concat();
            let found = parse(&buf);
            assert_eq!(found.len(), 2, "packed: {packed}");
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
}
