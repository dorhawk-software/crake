//! The header fields craked reads and writes, by name: in the packets it
//! holds in NFQUEUE (DNS answers, first SYNs) and the ICMP notices it reads.
//! IPv4 (RFC 791), IPv6 (RFC 8200), TCP (RFC 9293), UDP (RFC 768), ICMP
//! (RFC 792), ICMPv6 (RFC 4443).

use std::net::{Ipv4Addr, Ipv6Addr};
use std::ops::Range;

/// The IP version, in the top half of a packet's first byte.
const IPV4: u8 = 4;
const VERSION_SHIFT: u32 = 4;
/// The header's length, in 32-bit words, in the bottom half of that byte.
const HEADER_WORDS: u8 = 0x0f;
const WORD: usize = 4;
/// An IPv4 header without options.
pub const IPV4_MIN: usize = 20;
const TOTAL_LENGTH: Range<usize> = 2..4;
const PROTOCOL: usize = 9;
const CHECKSUM: Range<usize> = 10..12;
const SOURCE: Range<usize> = 12..16;
const DESTINATION: Range<usize> = 16..20;

/// IPv6: a fixed header, the next header's protocol in it. Extension
/// headers aren't followed (a first SYN or an ICMPv6 notice has none).
const IPV6: u8 = 6;
pub const IPV6_HEADER: usize = 40;
const NEXT_HEADER: usize = 6;
const SOURCE6: Range<usize> = 8..24;
const DESTINATION6: Range<usize> = 24..40;

/// IP protocol numbers.
pub const ICMP: u8 = 1;
pub const ICMPV6: u8 = 58;
pub const TCP: u8 = 6;
pub const UDP: u8 = 17;

/// TCP and UDP both begin with the two ports.
const SOURCE_PORT: Range<usize> = 0..2;
const DESTINATION_PORT: Range<usize> = 2..4;
/// UDP's header: the ports, the length (header included), the checksum.
pub const UDP_HEADER: usize = 8;
const UDP_LENGTH: Range<usize> = 4..6;
const UDP_CHECKSUM: Range<usize> = 6..8;
/// A TCP header without options; its own length (in words) is at
/// DATA_OFFSET, top half; SYN and ACK are among its flags.
pub const TCP_MIN: usize = 20;
const DATA_OFFSET: usize = 12;
const FLAGS: usize = 13;
const SYN: u8 = 0x02;
const ACK: u8 = 0x10;

/// ICMP "destination unreachable": type, code, four unused bytes, then the
/// start of the packet that couldn't be delivered.
const UNREACHABLE: u8 = 3;
/// ICMPv6's "destination unreachable", laid out the same way.
const UNREACHABLE6: u8 = 1;
const ICMP_TYPE: usize = 0;
const ICMP_CODE: usize = 1;
const ICMP_HEADER: usize = 8;

/// An IPv4 packet: where its header ends, and what it carries.
#[derive(Debug, Clone, Copy)]
pub struct V4<'a> {
    bytes: &'a [u8],
    header: usize,
}

impl<'a> V4<'a> {
    /// `packet`, if it is IPv4 and carries `protocol`.
    pub fn parse(packet: &'a [u8], protocol: u8) -> Option<Self> {
        if packet.len() < IPV4_MIN
            || packet[0] >> VERSION_SHIFT != IPV4
            || packet[PROTOCOL] != protocol
        {
            return None;
        }
        Some(Self {
            bytes: packet,
            header: usize::from(packet[0] & HEADER_WORDS) * WORD,
        })
    }

    pub fn header_len(self) -> usize {
        self.header
    }

    pub fn src(self) -> Ipv4Addr {
        addr(&self.bytes[SOURCE])
    }

    pub fn dst(self) -> Ipv4Addr {
        addr(&self.bytes[DESTINATION])
    }

    /// What it carries: to the end of the buffer.
    pub fn payload(self) -> Option<&'a [u8]> {
        self.bytes.get(self.header..)
    }

    /// What it carries: up to its own total length (a buffer may be longer).
    pub fn payload_to_total(self) -> Option<&'a [u8]> {
        let total = usize::from(be16(&self.bytes[TOTAL_LENGTH]));
        self.bytes.get(self.header..total.min(self.bytes.len()))
    }
}

/// An IPv6 packet whose next header is what it carries.
#[derive(Debug, Clone, Copy)]
pub struct V6<'a> {
    bytes: &'a [u8],
}

impl<'a> V6<'a> {
    /// `packet`, if it is IPv6 and carries `protocol` right after its header.
    pub fn parse(packet: &'a [u8], protocol: u8) -> Option<Self> {
        if packet.len() < IPV6_HEADER
            || packet[0] >> VERSION_SHIFT != IPV6
            || packet[NEXT_HEADER] != protocol
        {
            return None;
        }
        Some(Self { bytes: packet })
    }

    pub fn src(self) -> Ipv6Addr {
        addr6(&self.bytes[SOURCE6])
    }

    pub fn dst(self) -> Ipv6Addr {
        addr6(&self.bytes[DESTINATION6])
    }

    /// What it carries: to the end of the buffer.
    pub fn payload(self) -> Option<&'a [u8]> {
        self.bytes.get(IPV6_HEADER..)
    }
}

/// A TCP or UDP segment's ports.
pub fn source_port(segment: &[u8]) -> Option<u16> {
    segment.get(SOURCE_PORT).map(be16)
}

pub fn destination_port(segment: &[u8]) -> Option<u16> {
    segment.get(DESTINATION_PORT).map(be16)
}

/// What a TCP segment carries, after its header.
pub fn tcp_data(segment: &[u8]) -> Option<&[u8]> {
    let words = *segment.get(DATA_OFFSET)? >> VERSION_SHIFT;
    segment.get(usize::from(words) * WORD..)
}

/// A connection's first SYN: SYN set, ACK clear (not a SYN-ACK).
pub fn first_syn(segment: &[u8]) -> bool {
    segment.len() >= TCP_MIN && segment[FLAGS] & (SYN | ACK) == SYN
}

/// An ICMP "destination unreachable" with one of `codes`: the start of the
/// packet that couldn't be delivered.
pub fn unreachable_about<'a>(icmp: &'a [u8], codes: &[u8]) -> Option<&'a [u8]> {
    unreachable_of(icmp, UNREACHABLE, codes)
}

/// The same for ICMPv6.
pub fn unreachable6_about<'a>(icmp: &'a [u8], codes: &[u8]) -> Option<&'a [u8]> {
    unreachable_of(icmp, UNREACHABLE6, codes)
}

fn unreachable_of<'a>(icmp: &'a [u8], kind: u8, codes: &[u8]) -> Option<&'a [u8]> {
    if *icmp.get(ICMP_TYPE)? != kind || !codes.contains(icmp.get(ICMP_CODE)?) {
        return None;
    }
    icmp.get(ICMP_HEADER..)
}

/// The same IPv4 UDP packet carrying `data` instead: lengths and the header
/// checksum fixed, the (optional) UDP checksum cleared.
pub fn with_udp_data(packet: V4<'_>, data: &[u8]) -> Option<Vec<u8>> {
    let ip = packet.header;
    let udp_len = u16::try_from(UDP_HEADER + data.len()).ok()?;
    let total = u16::try_from(ip + UDP_HEADER + data.len()).ok()?;
    let mut out = Vec::with_capacity(usize::from(total));
    out.extend_from_slice(packet.bytes.get(..ip + UDP_HEADER)?);
    out.extend_from_slice(data);
    out[TOTAL_LENGTH].copy_from_slice(&total.to_be_bytes());
    out[CHECKSUM].fill(0);
    let sum = checksum(&out[..ip]);
    out[CHECKSUM].copy_from_slice(&sum.to_be_bytes());
    let udp = &mut out[ip..];
    udp[UDP_LENGTH].copy_from_slice(&udp_len.to_be_bytes());
    udp[UDP_CHECKSUM].fill(0);
    Some(out)
}

/// The Internet checksum (RFC 1071) of `bytes`: their 16-bit one's
/// complement sum, complemented.
pub fn checksum(bytes: &[u8]) -> u16 {
    let mut sum: u32 = bytes
        .chunks(size_of::<u16>())
        .map(|c| u32::from(u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)])))
        .sum();
    while sum > u32::from(u16::MAX) {
        sum = (sum & u32::from(u16::MAX)) + (sum >> u16::BITS);
    }
    !(sum as u16)
}

fn be16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

fn addr(b: &[u8]) -> Ipv4Addr {
    let mut octets = [0; size_of::<Ipv4Addr>()];
    octets.copy_from_slice(b);
    Ipv4Addr::from(octets)
}

fn addr6(b: &[u8]) -> Ipv6Addr {
    let mut octets = [0; size_of::<Ipv6Addr>()];
    octets.copy_from_slice(b);
    Ipv6Addr::from(octets)
}
