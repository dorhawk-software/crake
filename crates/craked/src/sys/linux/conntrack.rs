//! Connection tracking over netlink: events for new and changing TCP
//! connections (observation only, nothing is held), and deleting one entry so
//! the next packet of a flow is routed afresh.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use anyhow::{Context, Result};
use tokio::io::unix::AsyncFd;

const NETLINK_NETFILTER: i32 = 12;
const NFNL_SUBSYS_CTNETLINK: u16 = 1;
const IPCTNL_MSG_CT_NEW: u16 = 0;
const IPCTNL_MSG_CT_DELETE: u16 = 2;
const NF_NETLINK_CONNTRACK_NEW: u32 = 1;
const NF_NETLINK_CONNTRACK_UPDATE: u32 = 2;
const NF_NETLINK_CONNTRACK_DESTROY: u32 = 4;
const NLM_F_CREATE: u16 = 0x400;
const NLA_F_NESTED: u16 = 0x8000;

const CTA_TUPLE_ORIG: u16 = 1;
const CTA_PROTOINFO: u16 = 4;
const CTA_TUPLE_IP: u16 = 1;
const CTA_TUPLE_PROTO: u16 = 2;
const CTA_IP_V4_SRC: u16 = 1;
const CTA_IP_V4_DST: u16 = 2;
const CTA_IP_V6_SRC: u16 = 3;
const CTA_IP_V6_DST: u16 = 4;
const CTA_PROTO_NUM: u16 = 1;
const CTA_PROTO_SRC_PORT: u16 = 2;
const CTA_PROTO_DST_PORT: u16 = 3;
const CTA_PROTOINFO_TCP: u16 = 1;
const CTA_PROTOINFO_TCP_STATE: u16 = 1;

/// nfnetlink's message types: the subsystem in the high byte, its own
/// message in the low byte.
const SUBSYS_SHIFT: u32 = 8;
const MESSAGE: u16 = 0xff;
/// `struct nfgenmsg` after the netlink header: family, version, resource id.
const NFGENMSG: usize = 4;
/// Bursts of new connections shouldn't overflow the queue (4 MiB); one read
/// takes up to 64 KiB of events.
const SOCKET_BUFFER: libc::c_int = 4 << 20;
const EVENT_BUFFER: usize = 1 << 16;
/// The kernel's TCP states (enum tcp_conntrack, nf_conntrack_tcp.h): a SYN
/// out, then everything after a reply, then a close.
const SYN_SENT: u8 = 1;
const SYN_RECV: u8 = 2;
const TIME_WAIT: u8 = 7;
const CLOSE: u8 = 8;
const SYN_SENT2: u8 = 9;

use super::netlink;

pub use crate::flows::{Event, Flow, Kind, TcpState};

impl TcpState {
    fn from_raw(s: u8) -> Self {
        match s {
            SYN_SENT | SYN_SENT2 => Self::SynSent,
            SYN_RECV..=TIME_WAIT => Self::Answered,
            CLOSE => Self::Close,
            _ => Self::Other,
        }
    }
}

/// Subscribed to conntrack events (needs CAP_NET_ADMIN).
pub struct Events {
    fd: AsyncFd<OwnedFd>,
    buf: Vec<u8>,
}

impl Events {
    pub fn open() -> Result<Self> {
        let groups =
            NF_NETLINK_CONNTRACK_NEW | NF_NETLINK_CONNTRACK_UPDATE | NF_NETLINK_CONNTRACK_DESTROY;
        let fd = netlink_socket(groups, true).context("conntrack events")?;
        let size = SOCKET_BUFFER;
        // SAFETY: plain setsockopt with an int.
        unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVBUFFORCE,
                (&size as *const libc::c_int).cast(),
                std::mem::size_of::<libc::c_int>() as u32,
            );
        }
        Ok(Self {
            fd: AsyncFd::new(fd)?,
            buf: vec![0; EVENT_BUFFER],
        })
    }

    /// The next batch of TCP/IPv4 events.
    pub async fn next(&mut self) -> io::Result<Vec<Event>> {
        loop {
            let mut guard = self.fd.readable().await?;
            let fd = self.fd.get_ref().as_raw_fd();
            // SAFETY: reading into our own buffer.
            let n = unsafe { libc::recv(fd, self.buf.as_mut_ptr().cast(), self.buf.len(), 0) };
            if n < 0 {
                let e = io::Error::last_os_error();
                match e.raw_os_error() {
                    Some(libc::EAGAIN) => {
                        guard.clear_ready();
                        continue;
                    }
                    // Overflow: some events were lost; carry on.
                    Some(libc::ENOBUFS) => continue,
                    _ => return Err(e),
                }
            }
            return Ok(parse_messages(&self.buf[..n as usize]));
        }
    }
}

fn netlink_socket(groups: u32, nonblocking: bool) -> io::Result<OwnedFd> {
    let flags =
        libc::SOCK_RAW | libc::SOCK_CLOEXEC | if nonblocking { libc::SOCK_NONBLOCK } else { 0 };
    // SAFETY: plain socket/bind calls; the fd is owned right away.
    unsafe {
        let fd = libc::socket(libc::AF_NETLINK, flags, NETLINK_NETFILTER);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let owned = OwnedFd::from_raw_fd(fd);
        let mut addr: libc::sockaddr_nl = std::mem::zeroed();
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_groups = groups;
        let r = libc::bind(
            fd,
            (&addr as *const libc::sockaddr_nl).cast(),
            std::mem::size_of::<libc::sockaddr_nl>() as u32,
        );
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(owned)
    }
}

/// Close this machine's TCP connections to `target` (iproute2's `ss -K`,
/// the kernel's socket destroy): the apps see them end and open new ones,
/// which are found and routed as on a first visit. After Forget, an open
/// keep-alive connection would otherwise sit there, its packets now going
/// direct to nothing.
pub async fn close_to(target: crate::hosts::Target) {
    let _ = tokio::process::Command::new("ss")
        .args(["-K", "-t", "dst", &target.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
}

/// Forget one flow, so its next packet starts a new entry (and is routed and
/// NATed afresh). Blocking; call from a blocking context.
pub fn delete(flow: Flow) -> io::Result<()> {
    let fd = netlink_socket(0, false)?;
    let mut ip = Vec::new();
    let family = match (flow.src, flow.dst) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => {
            netlink::put_attr(&mut ip, CTA_IP_V4_SRC, &src.octets());
            netlink::put_attr(&mut ip, CTA_IP_V4_DST, &dst.octets());
            libc::AF_INET
        }
        (IpAddr::V6(src), IpAddr::V6(dst)) => {
            netlink::put_attr(&mut ip, CTA_IP_V6_SRC, &src.octets());
            netlink::put_attr(&mut ip, CTA_IP_V6_DST, &dst.octets());
            libc::AF_INET6
        }
        _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
    };
    let mut proto = Vec::new();
    netlink::put_attr(&mut proto, CTA_PROTO_NUM, &[libc::IPPROTO_TCP as u8]);
    netlink::put_attr(&mut proto, CTA_PROTO_SRC_PORT, &flow.sport.to_be_bytes());
    netlink::put_attr(&mut proto, CTA_PROTO_DST_PORT, &flow.dport.to_be_bytes());
    let mut tuple = Vec::new();
    netlink::put_attr(&mut tuple, CTA_TUPLE_IP | NLA_F_NESTED, &ip);
    netlink::put_attr(&mut tuple, CTA_TUPLE_PROTO | NLA_F_NESTED, &proto);
    let mut body = vec![family as u8, 0, 0, 0]; // nfgenmsg
    netlink::put_attr(&mut body, CTA_TUPLE_ORIG | NLA_F_NESTED, &tuple);

    let msg = netlink::request(
        (NFNL_SUBSYS_CTNETLINK << SUBSYS_SHIFT) | IPCTNL_MSG_CT_DELETE,
        &body,
    );

    // SAFETY: send/recv on our own socket and buffers.
    unsafe {
        if libc::send(fd.as_raw_fd(), msg.as_ptr().cast(), msg.len(), 0) < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut ack = [0u8; netlink::ACK_BUFFER];
        let n = libc::recv(fd.as_raw_fd(), ack.as_mut_ptr().cast(), ack.len(), 0);
        let code = usize::try_from(n)
            .ok()
            .and_then(|n| netlink::ack_code(&ack[..n]));
        // ENOENT: already gone, which is what we wanted.
        if let Some(err) = code
            && err != 0
            && -err != libc::ENOENT
        {
            return Err(io::Error::from_raw_os_error(-err));
        }
    }
    Ok(())
}

fn child<'a>(list: &[(u16, &'a [u8])], kind: u16) -> Option<&'a [u8]> {
    list.iter().find(|(k, _)| *k == kind).map(|(_, v)| *v)
}

pub fn parse_messages(b: &[u8]) -> Vec<Event> {
    netlink::messages(b)
        .into_iter()
        .filter_map(|(kind, flags, body)| {
            let family = body.first().map(|f| i32::from(*f));
            if kind >> SUBSYS_SHIFT != NFNL_SUBSYS_CTNETLINK
                || !matches!(family, Some(libc::AF_INET | libc::AF_INET6))
            {
                return None;
            }
            let kind = match kind & MESSAGE {
                IPCTNL_MSG_CT_NEW if flags & NLM_F_CREATE != 0 => Kind::New,
                IPCTNL_MSG_CT_NEW => Kind::Update,
                IPCTNL_MSG_CT_DELETE => Kind::Destroy,
                _ => return None,
            };
            parse_event(kind, body.get(NFGENMSG..)?)
        })
        .collect()
}

fn parse_event(kind: Kind, body: &[u8]) -> Option<Event> {
    let top = netlink::attrs(body);
    let tuple = netlink::attrs(child(&top, CTA_TUPLE_ORIG)?);
    let ip = netlink::attrs(child(&tuple, CTA_TUPLE_IP)?);
    let proto = netlink::attrs(child(&tuple, CTA_TUPLE_PROTO)?);
    if child(&proto, CTA_PROTO_NUM)?.first() != Some(&(libc::IPPROTO_TCP as u8)) {
        return None;
    }
    // An IPv4 tuple carries the V4 attributes, an IPv6 one the V6.
    let addr = |v4: u16, v6: u16| -> Option<IpAddr> {
        if let Some(v) = child(&ip, v4) {
            let octets: [u8; size_of::<Ipv4Addr>()] =
                v.get(..size_of::<Ipv4Addr>())?.try_into().ok()?;
            return Some(Ipv4Addr::from(octets).into());
        }
        let v = child(&ip, v6)?;
        let octets: [u8; size_of::<Ipv6Addr>()] =
            v.get(..size_of::<Ipv6Addr>())?.try_into().ok()?;
        Some(Ipv6Addr::from(octets).into())
    };
    let port = |k| -> Option<u16> {
        let v = child(&proto, k)?;
        Some(u16::from_be_bytes([*v.first()?, *v.get(1)?]))
    };
    let state = child(&top, CTA_PROTOINFO)
        .and_then(|p| child(&netlink::attrs(p), CTA_PROTOINFO_TCP))
        .and_then(|t| child(&netlink::attrs(t), CTA_PROTOINFO_TCP_STATE))
        .and_then(|s| s.first().copied())
        .map(TcpState::from_raw);
    Some(Event {
        kind,
        flow: Flow {
            src: addr(CTA_IP_V4_SRC, CTA_IP_V6_SRC)?,
            dst: addr(CTA_IP_V4_DST, CTA_IP_V6_DST)?,
            sport: port(CTA_PROTO_SRC_PORT)?,
            dport: port(CTA_PROTO_DST_PORT)?,
        },
        state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an event message the way the kernel does.
    fn message(msg: u16, flags: u16, state: u8) -> Vec<u8> {
        let mut ip = Vec::new();
        netlink::put_attr(&mut ip, CTA_IP_V4_SRC, &[192, 168, 0, 242]);
        netlink::put_attr(&mut ip, CTA_IP_V4_DST, &[10, 58, 70, 119]);
        framed(libc::AF_INET, &ip, msg, flags, state)
    }

    /// The same for an IPv6 connection.
    fn message6(msg: u16, flags: u16, state: u8) -> Vec<u8> {
        let src: Ipv6Addr = "2001:db8:1::42".parse().unwrap();
        let dst: Ipv6Addr = "2001:db8:40::10".parse().unwrap();
        let mut ip = Vec::new();
        netlink::put_attr(&mut ip, CTA_IP_V6_SRC, &src.octets());
        netlink::put_attr(&mut ip, CTA_IP_V6_DST, &dst.octets());
        framed(libc::AF_INET6, &ip, msg, flags, state)
    }

    fn framed(family: i32, ip: &[u8], msg: u16, flags: u16, state: u8) -> Vec<u8> {
        let mut proto = Vec::new();
        netlink::put_attr(&mut proto, CTA_PROTO_NUM, &[6]);
        netlink::put_attr(&mut proto, CTA_PROTO_SRC_PORT, &40000u16.to_be_bytes());
        netlink::put_attr(&mut proto, CTA_PROTO_DST_PORT, &5432u16.to_be_bytes());
        let mut tuple = Vec::new();
        netlink::put_attr(&mut tuple, CTA_TUPLE_IP | NLA_F_NESTED, ip);
        netlink::put_attr(&mut tuple, CTA_TUPLE_PROTO | NLA_F_NESTED, &proto);
        let mut tcp = Vec::new();
        netlink::put_attr(&mut tcp, CTA_PROTOINFO_TCP_STATE, &[state]);
        let mut info = Vec::new();
        netlink::put_attr(&mut info, CTA_PROTOINFO_TCP | NLA_F_NESTED, &tcp);
        let mut body = vec![family as u8, 0, 0, 0];
        netlink::put_attr(&mut body, CTA_TUPLE_ORIG | NLA_F_NESTED, &tuple);
        netlink::put_attr(&mut body, 3, &2u32.to_be_bytes()); // CTA_STATUS, skipped
        netlink::put_attr(&mut body, CTA_PROTOINFO | NLA_F_NESTED, &info);
        let len = 16 + body.len();
        let mut m = Vec::new();
        m.extend_from_slice(&(len as u32).to_ne_bytes());
        m.extend_from_slice(&((NFNL_SUBSYS_CTNETLINK << 8) | msg).to_ne_bytes());
        m.extend_from_slice(&flags.to_ne_bytes());
        m.extend_from_slice(&[0; 8]);
        m.extend_from_slice(&body);
        m
    }

    #[test]
    fn parses_events() {
        let mut batch = message(IPCTNL_MSG_CT_NEW, NLM_F_CREATE, 1);
        batch.extend(message(IPCTNL_MSG_CT_NEW, 0, 8));
        batch.extend(message(IPCTNL_MSG_CT_DELETE, 0, 3));
        let ev = parse_messages(&batch);
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].kind, Kind::New);
        assert_eq!(ev[0].state, Some(TcpState::SynSent));
        assert_eq!(
            ev[0].flow,
            Flow {
                src: "192.168.0.242".parse().unwrap(),
                dst: "10.58.70.119".parse().unwrap(),
                sport: 40000,
                dport: 5432
            }
        );
        assert_eq!(
            (ev[1].kind, ev[1].state),
            (Kind::Update, Some(TcpState::Close))
        );
        assert_eq!(
            (ev[2].kind, ev[2].state),
            (Kind::Destroy, Some(TcpState::Answered))
        );
        assert!(parse_messages(b"short").is_empty());
    }

    #[test]
    fn parses_ipv6_events() {
        let ev = parse_messages(&message6(IPCTNL_MSG_CT_NEW, NLM_F_CREATE, 1));
        assert_eq!(
            (ev[0].kind, ev[0].state, ev[0].flow),
            (
                Kind::New,
                Some(TcpState::SynSent),
                Flow {
                    src: "2001:db8:1::42".parse().unwrap(),
                    dst: "2001:db8:40::10".parse().unwrap(),
                    sport: 40000,
                    dport: 5432
                }
            )
        );
    }
}
