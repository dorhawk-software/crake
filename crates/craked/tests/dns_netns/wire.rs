//! Packets the DNS test builds and reads: DNS questions and answers over
//! UDP and TCP, IPv4 and IPv6, and the TCP handshake's SYN-ACK.

use std::net::{IpAddr, Ipv4Addr};

/// A SYN in this packet, IPv4 or IPv6: (destination, port).
pub fn syn_to(p: &[u8]) -> Option<(IpAddr, u16)> {
    let (dst, t): (IpAddr, &[u8]) = match *p.first()? >> 4 {
        4 if p.len() >= 40 && p[9] == 6 => {
            let ihl = usize::from(p[0] & 0x0f) * 4;
            (
                Ipv4Addr::new(p[16], p[17], p[18], p[19]).into(),
                p.get(ihl..)?,
            )
        }
        6 if p.len() >= 60 && p[6] == 6 => {
            let dst: [u8; 16] = p[24..40].try_into().ok()?;
            (dst.into(), &p[40..])
        }
        _ => return None,
    };
    (t.len() >= 20 && t[13] & 0x12 == 0x02).then(|| (dst, u16::from_be_bytes([t[2], t[3]])))
}

/// The SYN-ACK answering `syn` (IPv4 or IPv6).
pub fn tcp_synack(syn: &[u8]) -> Vec<u8> {
    let sum16 = |d: &[u8]| -> u32 {
        d.chunks(2)
            .map(|c| u32::from(u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)])))
            .sum()
    };
    let fold = |mut s: u32| -> u16 {
        while s >> 16 != 0 {
            s = (s & 0xffff) + (s >> 16);
        }
        !(s as u16)
    };
    let v6 = syn[0] >> 4 == 6;
    let ihl = if v6 {
        40
    } else {
        usize::from(syn[0] & 0x0f) * 4
    };
    let t = &syn[ihl..];
    let seq = u32::from_be_bytes([t[4], t[5], t[6], t[7]]);
    let mut tcp = vec![0u8; 20];
    tcp[0..2].copy_from_slice(&t[2..4]);
    tcp[2..4].copy_from_slice(&t[0..2]);
    tcp[4..8].copy_from_slice(&777_000u32.to_be_bytes());
    tcp[8..12].copy_from_slice(&seq.wrapping_add(1).to_be_bytes());
    tcp[12] = 5 << 4;
    tcp[13] = 0x12;
    tcp[14..16].copy_from_slice(&65535u16.to_be_bytes());
    let (src, dst) = if v6 {
        (&syn[24..40], &syn[8..24])
    } else {
        (&syn[16..20], &syn[12..16])
    };
    let mut pseudo = Vec::new();
    pseudo.extend_from_slice(src);
    pseudo.extend_from_slice(dst);
    if v6 {
        pseudo.extend_from_slice(&[0, 0, 0, 20, 0, 0, 0, 6]);
    } else {
        pseudo.extend_from_slice(&[0, 6, 0, 20]);
    }
    let c = fold(sum16(&pseudo) + sum16(&tcp));
    tcp[16..18].copy_from_slice(&c.to_be_bytes());
    if v6 {
        let mut ip = vec![0x60, 0, 0, 0, 0, 20, 6, 64];
        ip.extend_from_slice(src);
        ip.extend_from_slice(dst);
        ip.extend_from_slice(&tcp);
        return ip;
    }
    let mut ip = vec![0x45, 0, 0, 40, 0, 0, 0, 0, 64, 6, 0, 0];
    ip.extend_from_slice(src);
    ip.extend_from_slice(dst);
    let c = fold(sum16(&ip));
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    ip.extend_from_slice(&tcp);
    ip
}

/// The answer packet to an IPv6 query packet: rcode plus records.
pub fn reply6(q: &[u8], rcode: u8, addrs: &[IpAddr]) -> Vec<u8> {
    let dns = answer_message(&q[48..], rcode, addrs);
    let mut udp = Vec::new();
    udp.extend_from_slice(&q[42..44]);
    udp.extend_from_slice(&q[40..42]);
    udp.extend_from_slice(&((8 + dns.len()) as u16).to_be_bytes());
    udp.extend_from_slice(&[0, 0]);
    udp.extend_from_slice(&dns);
    // UDP over IPv6 must carry its checksum (pseudo-header included).
    let mut pseudo = Vec::new();
    pseudo.extend_from_slice(&q[24..40]);
    pseudo.extend_from_slice(&q[8..24]);
    pseudo.extend_from_slice(&(udp.len() as u32).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0, 17]);
    pseudo.extend_from_slice(&udp);
    if pseudo.len() % 2 == 1 {
        pseudo.push(0);
    }
    let c = checksum(&pseudo);
    udp[6..8].copy_from_slice(&c.to_be_bytes());
    let mut ip = vec![0x60, 0, 0, 0];
    ip.extend_from_slice(&(udp.len() as u16).to_be_bytes());
    ip.extend_from_slice(&[17, 64]);
    ip.extend_from_slice(&q[24..40]);
    ip.extend_from_slice(&q[8..24]);
    ip.extend_from_slice(&udp);
    ip
}

pub fn checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = header
        .chunks(2)
        .map(|c| u32::from(u16::from_be_bytes([c[0], c[1]])))
        .sum();
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// Turn the query packet into an answer packet with one A record.
pub fn answer(query: &[u8], addr: Ipv4Addr) -> Vec<u8> {
    reply(query, 0, &[addr.into()])
}

pub fn nxdomain(query: &[u8]) -> Vec<u8> {
    reply(query, 3, &[])
}

/// Turn the query packet into an answer packet: rcode plus A (or AAAA)
/// records.
pub fn reply(query: &[u8], rcode: u8, addrs: &[IpAddr]) -> Vec<u8> {
    let ihl = usize::from(query[0] & 0x0f) * 4;
    let udp = &query[ihl..];
    let dns = answer_message(&udp[8..], rcode, addrs);
    let mut udp_out = Vec::new();
    udp_out.extend_from_slice(&udp[2..4]); // sport = their dport (53)
    udp_out.extend_from_slice(&udp[0..2]); // dport = their sport
    udp_out.extend_from_slice(&((8 + dns.len()) as u16).to_be_bytes());
    udp_out.extend_from_slice(&[0, 0]); // no UDP checksum
    udp_out.extend_from_slice(&dns);

    let mut ip = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0];
    ip.extend_from_slice(&query[16..20]); // src = the DNS server
    ip.extend_from_slice(&query[12..16]); // dst = us
    let total = (20 + udp_out.len()) as u16;
    ip[2..4].copy_from_slice(&total.to_be_bytes());
    let c = checksum(&ip);
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    ip.extend_from_slice(&udp_out);
    ip
}

/// The answer to a DNS query message: rcode plus A records (AAAA for an
/// IPv6 address).
pub fn answer_message(query: &[u8], rcode: u8, addrs: &[IpAddr]) -> Vec<u8> {
    let mut dns = query.to_vec();
    dns[2] = 0x81; // response, recursion desired
    dns[3] = 0x80 | rcode; // recursion available
    dns[7] = addrs.len() as u8;
    dns[10] = 0; // drop EDNS from additional count
    dns[11] = 0;
    // Cut the additional section: question ends after the name plus 4 bytes.
    let mut i = 12;
    while dns[i] != 0 {
        i += usize::from(dns[i]) + 1;
    }
    dns.truncate(i + 5);
    for addr in addrs {
        let (kind, data) = match addr {
            IpAddr::V4(a) => (1, a.octets().to_vec()),
            IpAddr::V6(a) => (28, a.octets().to_vec()),
        };
        dns.extend_from_slice(&[0xc0, 0x0c, 0, kind, 0, 1, 0, 0, 0, 60, 0]);
        dns.push(data.len() as u8);
        dns.extend_from_slice(&data);
    }
    dns
}

/// A reply segment to the client's `seg` (PSH|ACK) from the server at `seq`,
/// acknowledging what `seg` carried.
pub fn tcp_reply(seg: &[u8], seq: u32, data: &[u8]) -> Vec<u8> {
    let ihl = usize::from(seg[0] & 0x0f) * 4;
    let t = &seg[ihl..];
    let carried = seg.len() - ihl - usize::from(t[12] >> 4) * 4;
    let their_seq = u32::from_be_bytes([t[4], t[5], t[6], t[7]]);
    let mut tcp = vec![0u8; 20];
    tcp[0..2].copy_from_slice(&t[2..4]);
    tcp[2..4].copy_from_slice(&t[0..2]);
    tcp[4..8].copy_from_slice(&seq.to_be_bytes());
    tcp[8..12].copy_from_slice(&their_seq.wrapping_add(carried as u32).to_be_bytes());
    tcp[12] = 5 << 4;
    tcp[13] = 0x18;
    tcp[14..16].copy_from_slice(&65535u16.to_be_bytes());
    tcp.extend_from_slice(data);
    let (src, dst) = (&seg[16..20], &seg[12..16]);
    let mut pseudo = Vec::new();
    pseudo.extend_from_slice(src);
    pseudo.extend_from_slice(dst);
    pseudo.extend_from_slice(&[0, 6]);
    pseudo.extend_from_slice(&(tcp.len() as u16).to_be_bytes());
    let sum = |d: &[u8]| -> u32 {
        d.chunks(2)
            .map(|c| u32::from(u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)])))
            .sum()
    };
    let mut s = sum(&pseudo) + sum(&tcp);
    while s >> 16 != 0 {
        s = (s & 0xffff) + (s >> 16);
    }
    tcp[16..18].copy_from_slice(&(!(s as u16)).to_be_bytes());
    let mut ip = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 6, 0, 0];
    ip.extend_from_slice(src);
    ip.extend_from_slice(dst);
    ip[2..4].copy_from_slice(&((20 + tcp.len()) as u16).to_be_bytes());
    let c = checksum(&ip);
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    ip.extend_from_slice(&tcp);
    ip
}

pub fn query(name: &str) -> Vec<u8> {
    query_for(name, 1)
}

/// A query for `name`'s records of type `kind` (1: A, 28: AAAA).
pub fn query_for(name: &str, kind: u8) -> Vec<u8> {
    let mut m = vec![0xab, 0xcd, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        m.push(label.len() as u8);
        m.extend_from_slice(label.as_bytes());
    }
    m.extend_from_slice(&[0, 0, kind, 0, 1]);
    m
}
