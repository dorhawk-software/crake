//! macOS: new TCP connections from the kernel's connection list (the
//! `net.inet.tcp.pcblist_n` sysctl that netstat reads), looked at four
//! times a second; what changed becomes the events Linux gets from
//! conntrack (`flows::changes`), as on Windows. The same lists give the
//! Traffic window each connection's bytes (`open_connections`): TCP's, and
//! UDP sockets connected to one address (`net.inet.udp.pcblist_n`).

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
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

/// The machine's TCP connections, IPv4 and IPv6 (not listeners), and
/// their states.
fn table() -> io::Result<HashMap<Flow, TcpState>> {
    Ok(parse(&list(c"net.inet.tcp.pcblist_n")?))
}

/// One of the kernel's lists, as the sysctl gives it.
fn list(name: &std::ffi::CStr) -> io::Result<Vec<u8>> {
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
        Ok(buf)
    }
}

/// Record kinds (xnu's XSO_*).
const XSO_STATS: u32 = 0x08;
const XSO_INPCB: u32 = 0x10;
const XSO_TCPCB: u32 = 0x20;
/// The list starts and ends with an `xinpgen` this long.
const XINPGEN: usize = 24;
/// A connection's record says IPv4 (`inp_vflag & INP_IPV4`) or IPv6.
const INP_IPV4: u8 = 0x1;
const INP_IPV6: u8 = 0x2;
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
/// After inp_vflag (and the TTL, the protocol and a pad byte): the foreign
/// and local addresses, each a 16-byte union, an in6_addr or an
/// in_addr_4in6 (IPv4 in its last four bytes).
const FADDR6_AFTER_VFLAG: usize = 4;
const LADDR6_AFTER_VFLAG: usize = 20;
const FADDR_AFTER_VFLAG: usize = 16;
const LADDR_AFTER_VFLAG: usize = 32;
/// xsockstat_n: after its length and kind, per traffic class
/// (SO_TC_STATS_MAX of them) a `data_stats`: packets and bytes received,
/// packets and bytes sent (u64 each).
const STATS_AT: usize = 8;
const TRAFFIC_CLASSES: usize = 4;
const DATA_STATS: usize = 32;
const RX_BYTES: usize = 8;
const TX_BYTES: usize = 24;
/// xtcpcb_n: t_state.
const TCP_STATE: usize = 36;
/// BSD's TCP states (netinet/tcp_fsm.h): closed, a SYN out, established
/// (and every state after it).
const TCPS_CLOSED: u32 = 0;
const TCPS_SYN_SENT: u32 = 2;
const TCPS_ESTABLISHED: u32 = 4;

/// TCP's list: each connection and its state.
pub fn parse(buf: &[u8]) -> HashMap<Flow, TcpState> {
    records(buf, false)
        .into_iter()
        .filter_map(|(flow, state, _)| Some((flow, state?)))
        .collect()
}

/// A list: an `xinpgen`, then per connection its records (`xinpcb_n`, the
/// socket's, its two buffers', its statistics, and for TCP `xtcpcb_n`),
/// each led by its length and kind and padded to 8 bytes, then another
/// `xinpgen`. Each connection with its TCP state (None for UDP) and its
/// bytes both ways.
fn records(buf: &[u8], udp: bool) -> Vec<(Flow, Option<TcpState>, u64)> {
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
            IpAddr::V4(Ipv4Addr::from(octets))
        })
    };
    let ip6_at = |at: usize| {
        buf.get(at..at + size_of::<Ipv6Addr>()).map(|b| {
            let mut octets = [0; size_of::<Ipv6Addr>()];
            octets.copy_from_slice(b);
            IpAddr::V6(Ipv6Addr::from(octets))
        })
    };
    let u64_at = |at: usize| {
        buf.get(at..at + size_of::<u64>())
            .map(|b| u64::from_ne_bytes(b.try_into().unwrap()))
    };
    let mut found = Vec::new();
    let mut current: Option<(Flow, u64)> = None;
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
                let addrs = flag.and_then(|v| match buf.get(at + v)? {
                    f if f & INP_IPV4 != 0 => Some((
                        ip_at(at + v + FADDR_AFTER_VFLAG)?,
                        ip_at(at + v + LADDR_AFTER_VFLAG)?,
                    )),
                    f if f & INP_IPV6 != 0 => Some((
                        ip6_at(at + v + FADDR6_AFTER_VFLAG)?,
                        ip6_at(at + v + LADDR6_AFTER_VFLAG)?,
                    )),
                    _ => None,
                });
                if let Some((dst, src)) = addrs
                    && let (Some(dport), Some(sport)) =
                        (u16_net(at + INP_FPORT), u16_net(at + INP_LPORT))
                    && !dst.is_unspecified()
                {
                    current = Some((
                        Flow {
                            src,
                            dst,
                            sport,
                            dport,
                            udp,
                        },
                        0,
                    ));
                }
            }
            // xsockstat_n: the bytes, summed over the traffic classes. A
            // UDP socket's records end here.
            XSO_STATS => {
                if let Some((flow, bytes)) = current.as_mut() {
                    *bytes = (0..TRAFFIC_CLASSES)
                        .map(|c| at + STATS_AT + c * DATA_STATS)
                        .map(|d| {
                            u64_at(d + RX_BYTES).unwrap_or(0) + u64_at(d + TX_BYTES).unwrap_or(0)
                        })
                        .sum();
                    if udp {
                        found.push((*flow, None, *bytes));
                        current = None;
                    }
                }
            }
            // xtcpcb_n: t_state at 36.
            XSO_TCPCB => {
                if let (Some((flow, bytes)), Some(state)) = (current.take(), u32_at(at + TCP_STATE))
                {
                    found.push((flow, Some(tcp_state(state)), bytes));
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

/// The open connections with their bytes both ways, for the Traffic
/// window: TCP's (closed ones left out), and UDP sockets connected to one
/// address (an unconnected one has no other end to show). macOS counts
/// every socket's bytes all along: nothing to switch on.
pub fn open_connections() -> Option<Vec<(Flow, u64)>> {
    let tcp = match list(c"net.inet.tcp.pcblist_n") {
        Ok(buf) => records(&buf, false),
        Err(e) => {
            tracing::warn!("listing connections: {e}");
            return None;
        }
    };
    let udp = list(c"net.inet.udp.pcblist_n")
        .map(|buf| records(&buf, true))
        .unwrap_or_default();
    Some(
        tcp.into_iter()
            .filter(|(_, state, _)| *state != Some(TcpState::Close))
            .chain(udp)
            .map(|(flow, _, bytes)| (flow, bytes))
            .collect(),
    )
}

/// Nothing to switch back off here: macOS counts all along.
pub fn connections_idle(_force: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// One connection's records as the kernel lays them out, its
    /// `xinpcb_n` packed to 4 bytes or not.
    fn connection(
        packed: bool,
        (src, sport): (&[u8], u16),
        (dst, dport): (&[u8], u16),
        state: u32,
    ) -> Vec<u8> {
        let (len, flag) = if packed { (104, 44) } else { (112, 48) };
        let mut inpcb = vec![0u8; len];
        inpcb[0..4].copy_from_slice(&(len as u32).to_ne_bytes());
        inpcb[4..8].copy_from_slice(&XSO_INPCB.to_ne_bytes());
        inpcb[16..18].copy_from_slice(&dport.to_be_bytes());
        inpcb[18..20].copy_from_slice(&sport.to_be_bytes());
        match (src.len(), dst.len()) {
            (4, 4) => {
                inpcb[flag] = INP_IPV4;
                inpcb[flag + 16..flag + 20].copy_from_slice(dst);
                inpcb[flag + 32..flag + 36].copy_from_slice(src);
            }
            _ => {
                inpcb[flag] = INP_IPV6;
                inpcb[flag + 4..flag + 20].copy_from_slice(dst);
                inpcb[flag + 20..flag + 36].copy_from_slice(src);
            }
        }
        let mut socket = vec![0u8; 92]; // padded to 96
        socket[0..4].copy_from_slice(&92u32.to_ne_bytes());
        socket[4..8].copy_from_slice(&1u32.to_ne_bytes());
        socket.resize(96, 0);
        let stats = stats_record(state.into());
        let mut tcpcb = vec![0u8; 40];
        tcpcb[0..4].copy_from_slice(&40u32.to_ne_bytes());
        tcpcb[4..8].copy_from_slice(&XSO_TCPCB.to_ne_bytes());
        tcpcb[36..40].copy_from_slice(&state.to_ne_bytes());
        [inpcb, socket, stats, tcpcb].concat()
    }

    /// An `xsockstat_n` whose traffic classes carry `bytes` in all: half
    /// received in class 0, half sent in class 2.
    fn stats_record(bytes: u64) -> Vec<u8> {
        let len = STATS_AT + TRAFFIC_CLASSES * DATA_STATS;
        let mut r = vec![0u8; len];
        r[0..4].copy_from_slice(&(len as u32).to_ne_bytes());
        r[4..8].copy_from_slice(&XSO_STATS.to_ne_bytes());
        let half = bytes / 2;
        let at = STATS_AT + RX_BYTES;
        r[at..at + 8].copy_from_slice(&half.to_ne_bytes());
        let at = STATS_AT + 2 * DATA_STATS + TX_BYTES;
        r[at..at + 8].copy_from_slice(&(bytes - half).to_ne_bytes());
        r
    }

    #[test]
    fn each_connection_brings_its_bytes() {
        let gen_ = || {
            let mut g = vec![0u8; XINPGEN];
            g[0..4].copy_from_slice(&(XINPGEN as u32).to_ne_bytes());
            g
        };
        // TCP: the state record follows the statistics (its state, here 4,
        // doubles as the bytes in this test's records).
        let tcp = [
            gen_(),
            connection(true, (&[192, 0, 2, 5], 50001), (&[198, 51, 100, 7], 443), 4),
            gen_(),
        ]
        .concat();
        let got = records(&tcp, false);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].1, Some(TcpState::Answered));
        assert_eq!(got[0].2, 4);
        // UDP: the statistics end a socket's records.
        let mut udp = connection(
            true,
            (&[192, 0, 2, 5], 50002),
            (&[198, 51, 100, 9], 443),
            1_200,
        );
        udp.truncate(udp.len() - 40); // no xtcpcb_n
        let got = records(&[gen_(), udp, gen_()].concat(), true);
        assert_eq!(got.len(), 1);
        assert!(got[0].0.udp);
        assert_eq!((got[0].1, got[0].2), (None, 1_200));
    }

    /// A real connection on this Mac (loopback, so nothing leaves it):
    /// listed, its bytes counted.
    #[test]
    fn a_real_connection_is_listed_with_its_bytes() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = std::net::TcpStream::connect(addr).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        client.write_all(&[7; 5000]).unwrap();
        let mut got = [0u8; 5000];
        server.read_exact(&mut got).unwrap();
        let all = open_connections().expect("macOS lists connections");
        let ours = all
            .iter()
            .find(|(f, _)| f.dport == addr.port() && f.sport == client.local_addr().unwrap().port())
            .unwrap_or_else(|| panic!("not listed: {all:?}"));
        assert!(ours.1 >= 5000, "its bytes: {}", ours.1);
    }

    fn v6(a: &str) -> [u8; 16] {
        a.parse::<Ipv6Addr>().unwrap().octets()
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
                connection(packed, (&[192, 0, 2, 5], 50000), (&[10, 40, 1, 10], 443), 2),
                connection(
                    packed,
                    (&[192, 0, 2, 5], 50001),
                    (&[198, 51, 100, 7], 80),
                    4,
                ),
                connection(
                    packed,
                    (&v6("2001:db8::5"), 50002),
                    (&v6("fd40:1::10"), 443),
                    2,
                ),
                generation(32),
            ]
            .concat();
            let found = parse(&buf);
            assert_eq!(found.len(), 3, "packed: {packed}");
            let stuck6 = Flow {
                src: "2001:db8::5".parse().unwrap(),
                dst: "fd40:1::10".parse().unwrap(),
                sport: 50002,
                dport: 443,
                udp: false,
            };
            assert_eq!(found.get(&stuck6), Some(&TcpState::SynSent));
            let stuck = Flow {
                src: Ipv4Addr::new(192, 0, 2, 5).into(),
                dst: Ipv4Addr::new(10, 40, 1, 10).into(),
                sport: 50000,
                dport: 443,
                udp: false,
            };
            assert_eq!(found.get(&stuck), Some(&TcpState::SynSent));
            assert!(found.values().any(|s| *s == TcpState::Answered));
        }
    }
}
