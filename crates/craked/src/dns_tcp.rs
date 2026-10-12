//! A tunnel DNS server's answers over TCP, put together from their
//! segments. An answer in one segment is read as it comes; one spread over
//! several is read when its last segment arrives, which is the one held
//! (NFQUEUE) until its addresses are routed: the app can't have the answer
//! before then, so its first packet still goes through the tunnel. The
//! earlier segments pass at once, copied here.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use crate::packet;

/// DNS over TCP: each message after its two-byte length (RFC 1035, 4.2.2).
const LENGTH: usize = 2;
/// A conversation left unfinished this long is forgotten.
const STALE: Duration = Duration::from_secs(30);
/// At most this many conversations at once (more are a flood, not DNS).
const MOST: usize = 256;

/// One TCP conversation, server to client: (server, client, ports).
type Key = (IpAddr, IpAddr, u16, u16);

struct Partial {
    /// The sequence number the next segment should start at.
    next: u32,
    /// Bytes after the last whole message: the start of the next one.
    data: Vec<u8>,
    seen: Instant,
}

#[derive(Default)]
pub struct Reassembly {
    flows: HashMap<Key, Partial>,
}

impl Reassembly {
    /// A TCP segment from a DNS server (port 53), as the queue holds it:
    /// the whole messages it completes (none for an earlier segment, or
    /// one that's out of order).
    pub fn segment(&mut self, packet: &[u8]) -> Vec<Vec<u8>> {
        let Some((key, tcp)) = from_dns_server(packet) else {
            return Vec::new();
        };
        let (Some(seq), Some(data)) = (packet::tcp_seq(tcp), packet::tcp_data(tcp)) else {
            return Vec::new();
        };
        let ends = packet::tcp_ends(tcp);
        let now = Instant::now();
        self.flows.retain(|_, p| now.duration_since(p.seen) < STALE);
        if data.is_empty() {
            if ends {
                self.flows.remove(&key);
            }
            return Vec::new();
        }
        let next = seq.wrapping_add(u32::try_from(data.len()).unwrap_or(u32::MAX));
        let partial = match self.flows.get_mut(&key) {
            Some(p) if p.next == seq => p,
            // A segment again (retransmitted), or one past a gap: wait for
            // the expected one (TCP sends it again).
            Some(_) => return Vec::new(),
            None => {
                if self.flows.len() >= MOST {
                    return Vec::new();
                }
                self.flows.entry(key).or_insert(Partial {
                    next: seq,
                    data: Vec::new(),
                    seen: now,
                })
            }
        };
        partial.data.extend_from_slice(data);
        partial.next = next;
        partial.seen = now;
        let mut whole = Vec::new();
        while partial.data.len() >= LENGTH {
            let len = usize::from(u16::from_be_bytes([partial.data[0], partial.data[1]]));
            if partial.data.len() < LENGTH + len {
                break;
            }
            whole.push(partial.data[LENGTH..LENGTH + len].to_vec());
            partial.data.drain(..LENGTH + len);
        }
        // Nothing pending: the next message starts afresh.
        if ends || partial.data.is_empty() {
            self.flows.remove(&key);
        }
        whole
    }
}

/// The conversation a TCP segment from port 53 belongs to, and the segment.
fn from_dns_server(packet: &[u8]) -> Option<(Key, &[u8])> {
    let (src, dst, tcp): (IpAddr, IpAddr, &[u8]) = match packet::V4::parse(packet, packet::TCP) {
        Some(ip) => (ip.src().into(), ip.dst().into(), ip.payload_to_total()?),
        None => {
            let ip = packet::V6::parse(packet, packet::TCP)?;
            (ip.src().into(), ip.dst().into(), ip.payload()?)
        }
    };
    let sport = packet::source_port(tcp)?;
    if sport != crate::dns::PORT {
        return None;
    }
    Some(((src, dst, sport, packet::destination_port(tcp)?), tcp))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A segment from 10.40.0.2:53 to the app at `seq`, carrying `data`.
    fn segment(seq: u32, data: &[u8], flags: u8) -> Vec<u8> {
        let mut tcp = vec![0, 53, 0x9c, 0x40];
        tcp.extend_from_slice(&seq.to_be_bytes());
        tcp.extend_from_slice(&[0, 0, 0, 1, 0x50, flags, 0xff, 0xff, 0, 0, 0, 0]);
        tcp.extend_from_slice(data);
        let mut ip = vec![
            0x45, 0, 0, 0, 0, 0, 0, 0, 64, 6, 0, 0, 10, 40, 0, 2, 10, 40, 255, 7,
        ];
        let total = (ip.len() + tcp.len()) as u16;
        ip[2..4].copy_from_slice(&total.to_be_bytes());
        ip.extend_from_slice(&tcp);
        ip
    }

    fn framed(msg: &[u8]) -> Vec<u8> {
        let mut f = (msg.len() as u16).to_be_bytes().to_vec();
        f.extend_from_slice(msg);
        f
    }

    #[test]
    fn puts_an_answer_together_from_its_segments() {
        let mut r = Reassembly::default();
        let answer = framed(&[7u8; 300]);
        let (a, b) = answer.split_at(100);
        assert!(
            r.segment(&segment(1000, a, 0x18)).is_empty(),
            "not whole yet"
        );
        // The same segment again (retransmitted) changes nothing.
        assert!(r.segment(&segment(1000, a, 0x18)).is_empty());
        assert_eq!(r.segment(&segment(1100, b, 0x18)), vec![vec![7u8; 300]]);
        // Two messages in one segment, then a third split in two.
        let mut two = framed(b"one");
        two.extend(framed(b"two"));
        let third = framed(b"three");
        two.extend_from_slice(&third[..3]);
        let out = r.segment(&segment(2000, &two, 0x18));
        assert_eq!(out, vec![b"one".to_vec(), b"two".to_vec()]);
        let at = 2000 + two.len() as u32;
        assert_eq!(
            r.segment(&segment(at, &third[3..], 0x18)),
            vec![b"three".to_vec()]
        );
        // A gap: nothing until the missing segment comes.
        let gapped = framed(&[1u8; 50]);
        assert!(r.segment(&segment(5000, &gapped[..10], 0x18)).is_empty());
        assert!(r.segment(&segment(5020, &gapped[20..], 0x18)).is_empty());
        assert_eq!(
            r.segment(&segment(5010, &gapped[10..], 0x18)),
            vec![vec![1u8; 50]]
        );
        // A FIN ends the conversation.
        assert!(
            r.segment(&segment(9000, &framed(b"x")[..1], 0x11))
                .is_empty()
        );
        assert!(r.flows.is_empty());
    }
}
