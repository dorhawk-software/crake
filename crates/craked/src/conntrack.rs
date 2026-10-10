//! Connection tracking over netlink: events for new and changing TCP
//! connections (observation only, nothing is held), and deleting one entry so
//! the next packet of a flow is routed afresh.

use std::io;
use std::net::Ipv4Addr;
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
const NLM_F_REQUEST: u16 = 1;
const NLM_F_ACK: u16 = 4;
const NLM_F_CREATE: u16 = 0x400;
const NLA_F_NESTED: u16 = 0x8000;
const NLA_TYPE_MASK: u16 = !(0x8000 | 0x4000);

const CTA_TUPLE_ORIG: u16 = 1;
const CTA_PROTOINFO: u16 = 4;
const CTA_TUPLE_IP: u16 = 1;
const CTA_TUPLE_PROTO: u16 = 2;
const CTA_IP_V4_SRC: u16 = 1;
const CTA_IP_V4_DST: u16 = 2;
const CTA_PROTO_NUM: u16 = 1;
const CTA_PROTO_SRC_PORT: u16 = 2;
const CTA_PROTO_DST_PORT: u16 = 3;
const CTA_PROTOINFO_TCP: u16 = 1;
const CTA_PROTOINFO_TCP_STATE: u16 = 1;

/// The parts of a TCP state we care about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    SynSent,
    /// Any state after a reply: the other side answered.
    Answered,
    /// Reset before it was ever established: refused.
    Close,
    Other,
}

impl TcpState {
    fn from_raw(s: u8) -> Self {
        match s {
            1 | 9 => Self::SynSent,
            2..=7 => Self::Answered,
            8 => Self::Close,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Flow {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub sport: u16,
    pub dport: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    New,
    Update,
    Destroy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub kind: Kind,
    pub flow: Flow,
    pub state: Option<TcpState>,
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
        // Bursts of new connections shouldn't overflow the queue.
        let size: libc::c_int = 4 << 20;
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
            buf: vec![0; 1 << 16],
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

/// Forget one flow, so its next packet starts a new entry (and is routed and
/// NATed afresh). Blocking; call from a blocking context.
pub fn delete(flow: Flow) -> io::Result<()> {
    let fd = netlink_socket(0, false)?;
    let mut ip = Vec::new();
    put_attr(&mut ip, CTA_IP_V4_SRC, &flow.src.octets());
    put_attr(&mut ip, CTA_IP_V4_DST, &flow.dst.octets());
    let mut proto = Vec::new();
    put_attr(&mut proto, CTA_PROTO_NUM, &[libc::IPPROTO_TCP as u8]);
    put_attr(&mut proto, CTA_PROTO_SRC_PORT, &flow.sport.to_be_bytes());
    put_attr(&mut proto, CTA_PROTO_DST_PORT, &flow.dport.to_be_bytes());
    let mut tuple = Vec::new();
    put_attr(&mut tuple, CTA_TUPLE_IP | NLA_F_NESTED, &ip);
    put_attr(&mut tuple, CTA_TUPLE_PROTO | NLA_F_NESTED, &proto);
    let mut body = vec![libc::AF_INET as u8, 0, 0, 0]; // nfgenmsg
    put_attr(&mut body, CTA_TUPLE_ORIG | NLA_F_NESTED, &tuple);

    let len = 16 + body.len();
    let mut msg = Vec::with_capacity(len);
    msg.extend_from_slice(&(len as u32).to_ne_bytes());
    msg.extend_from_slice(&((NFNL_SUBSYS_CTNETLINK << 8) | IPCTNL_MSG_CT_DELETE).to_ne_bytes());
    msg.extend_from_slice(&(NLM_F_REQUEST | NLM_F_ACK).to_ne_bytes());
    msg.extend_from_slice(&1u32.to_ne_bytes()); // seq
    msg.extend_from_slice(&0u32.to_ne_bytes()); // pid
    msg.extend_from_slice(&body);

    // SAFETY: send/recv on our own socket and buffers.
    unsafe {
        if libc::send(fd.as_raw_fd(), msg.as_ptr().cast(), msg.len(), 0) < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut ack = [0u8; 256];
        let n = libc::recv(fd.as_raw_fd(), ack.as_mut_ptr().cast(), ack.len(), 0);
        if n >= 20 {
            let err = i32::from_ne_bytes([ack[16], ack[17], ack[18], ack[19]]);
            // ENOENT: already gone, which is what we wanted.
            if err != 0 && -err != libc::ENOENT {
                return Err(io::Error::from_raw_os_error(-err));
            }
        }
    }
    Ok(())
}

fn put_attr(buf: &mut Vec<u8>, kind: u16, payload: &[u8]) {
    let len = 4 + payload.len();
    buf.extend_from_slice(&(len as u16).to_ne_bytes());
    buf.extend_from_slice(&kind.to_ne_bytes());
    buf.extend_from_slice(payload);
    buf.resize(buf.len() + (4 - len % 4) % 4, 0);
}

/// Attributes of one nested level: (type, payload).
fn attrs(mut b: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    while b.len() >= 4 {
        let len = usize::from(u16::from_ne_bytes([b[0], b[1]]));
        let kind = u16::from_ne_bytes([b[2], b[3]]) & NLA_TYPE_MASK;
        if len < 4 || len > b.len() {
            break;
        }
        out.push((kind, &b[4..len]));
        let next = (len + 3) & !3;
        b = &b[next.min(b.len())..];
    }
    out
}

fn child<'a>(list: &[(u16, &'a [u8])], kind: u16) -> Option<&'a [u8]> {
    list.iter().find(|(k, _)| *k == kind).map(|(_, v)| *v)
}

pub fn parse_messages(mut b: &[u8]) -> Vec<Event> {
    let mut out = Vec::new();
    while b.len() >= 16 {
        let len = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as usize;
        if len < 16 || len > b.len() {
            break;
        }
        let kind = u16::from_ne_bytes([b[4], b[5]]);
        let flags = u16::from_ne_bytes([b[6], b[7]]);
        if kind >> 8 == NFNL_SUBSYS_CTNETLINK && len >= 20 && b[16] == libc::AF_INET as u8 {
            let kind = match kind & 0xff {
                IPCTNL_MSG_CT_NEW if flags & NLM_F_CREATE != 0 => Some(Kind::New),
                IPCTNL_MSG_CT_NEW => Some(Kind::Update),
                IPCTNL_MSG_CT_DELETE => Some(Kind::Destroy),
                _ => None,
            };
            if let Some(kind) = kind
                && let Some(ev) = parse_event(kind, &b[20..len])
            {
                out.push(ev);
            }
        }
        b = &b[((len + 3) & !3).min(b.len())..];
    }
    out
}

fn parse_event(kind: Kind, body: &[u8]) -> Option<Event> {
    let top = attrs(body);
    let tuple = attrs(child(&top, CTA_TUPLE_ORIG)?);
    let ip = attrs(child(&tuple, CTA_TUPLE_IP)?);
    let proto = attrs(child(&tuple, CTA_TUPLE_PROTO)?);
    if child(&proto, CTA_PROTO_NUM)?.first() != Some(&(libc::IPPROTO_TCP as u8)) {
        return None;
    }
    let v4 = |k| -> Option<Ipv4Addr> {
        let v = child(&ip, k)?;
        Some(Ipv4Addr::new(
            *v.first()?,
            *v.get(1)?,
            *v.get(2)?,
            *v.get(3)?,
        ))
    };
    let port = |k| -> Option<u16> {
        let v = child(&proto, k)?;
        Some(u16::from_be_bytes([*v.first()?, *v.get(1)?]))
    };
    let state = child(&top, CTA_PROTOINFO)
        .and_then(|p| child(&attrs(p), CTA_PROTOINFO_TCP))
        .and_then(|t| child(&attrs(t), CTA_PROTOINFO_TCP_STATE))
        .and_then(|s| s.first().copied())
        .map(TcpState::from_raw);
    Some(Event {
        kind,
        flow: Flow {
            src: v4(CTA_IP_V4_SRC)?,
            dst: v4(CTA_IP_V4_DST)?,
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
        put_attr(&mut ip, CTA_IP_V4_SRC, &[192, 168, 0, 242]);
        put_attr(&mut ip, CTA_IP_V4_DST, &[10, 58, 70, 119]);
        let mut proto = Vec::new();
        put_attr(&mut proto, CTA_PROTO_NUM, &[6]);
        put_attr(&mut proto, CTA_PROTO_SRC_PORT, &40000u16.to_be_bytes());
        put_attr(&mut proto, CTA_PROTO_DST_PORT, &5432u16.to_be_bytes());
        let mut tuple = Vec::new();
        put_attr(&mut tuple, CTA_TUPLE_IP | NLA_F_NESTED, &ip);
        put_attr(&mut tuple, CTA_TUPLE_PROTO | NLA_F_NESTED, &proto);
        let mut tcp = Vec::new();
        put_attr(&mut tcp, CTA_PROTOINFO_TCP_STATE, &[state]);
        let mut info = Vec::new();
        put_attr(&mut info, CTA_PROTOINFO_TCP | NLA_F_NESTED, &tcp);
        let mut body = vec![libc::AF_INET as u8, 0, 0, 0];
        put_attr(&mut body, CTA_TUPLE_ORIG | NLA_F_NESTED, &tuple);
        put_attr(&mut body, 3, &2u32.to_be_bytes()); // CTA_STATUS, skipped
        put_attr(&mut body, CTA_PROTOINFO | NLA_F_NESTED, &info);
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
}
