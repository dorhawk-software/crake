//! Netlink on the wire (linux/netlink.h), as craked speaks it to conntrack
//! and to rtnetlink: a header, a family's own header, then attributes, each
//! padded to four bytes. Offsets come from libc's own structs.

use std::mem::offset_of;

/// `struct nlmsghdr`: length, type, flags, sequence, port.
pub const HEADER: usize = size_of::<libc::nlmsghdr>();
const LENGTH: usize = offset_of!(libc::nlmsghdr, nlmsg_len);
const TYPE: usize = offset_of!(libc::nlmsghdr, nlmsg_type);
const FLAGS: usize = offset_of!(libc::nlmsghdr, nlmsg_flags);
/// `struct nlattr`: length and type before each attribute's payload.
const ATTR_HEADER: usize = size_of::<libc::nlattr>();
const ATTR_LENGTH: usize = offset_of!(libc::nlattr, nla_len);
const ATTR_TYPE: usize = offset_of!(libc::nlattr, nla_type);
const ALIGN: usize = libc::NLA_ALIGNTO as usize;
/// An acknowledgement (NLMSG_ERROR) carries `struct nlmsgerr` after its
/// header: the error code first (0: done).
const ERROR_CODE: usize = HEADER + offset_of!(libc::nlmsgerr, error);
/// Room for an acknowledgement (its header, the code, the request's header).
pub const ACK_BUFFER: usize = 512;

/// Netlink pads everything to four bytes.
pub fn align(len: usize) -> usize {
    len.next_multiple_of(ALIGN)
}

/// Append one attribute: length, type, payload, padding.
pub fn put_attr(buf: &mut Vec<u8>, kind: u16, payload: &[u8]) {
    let len = ATTR_HEADER + payload.len();
    buf.extend_from_slice(&u16::try_from(len).unwrap_or(u16::MAX).to_ne_bytes());
    buf.extend_from_slice(&kind.to_ne_bytes());
    buf.extend_from_slice(payload);
    buf.resize(buf.len() + align(len) - len, 0);
}

/// One attribute on its own.
pub fn attr(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut a = Vec::with_capacity(align(ATTR_HEADER + payload.len()));
    put_attr(&mut a, kind, payload);
    a
}

/// The attributes of one level: (type without its flags, payload).
pub fn attrs(mut b: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    while b.len() >= ATTR_HEADER {
        let len = usize::from(ne16(&b[ATTR_LENGTH..]));
        let kind = ne16(&b[ATTR_TYPE..]) & libc::NLA_TYPE_MASK as u16;
        if len < ATTR_HEADER || len > b.len() {
            break;
        }
        out.push((kind, &b[ATTR_HEADER..len]));
        b = &b[align(len).min(b.len())..];
    }
    out
}

/// A request of type `kind` the kernel acknowledges, carrying `body`.
pub fn request(kind: u16, body: &[u8]) -> Vec<u8> {
    let len = HEADER + body.len();
    let flags = (libc::NLM_F_REQUEST | libc::NLM_F_ACK) as u16;
    let mut msg = Vec::with_capacity(len);
    msg.extend_from_slice(&u32::try_from(len).unwrap_or(u32::MAX).to_ne_bytes());
    msg.extend_from_slice(&kind.to_ne_bytes());
    msg.extend_from_slice(&flags.to_ne_bytes());
    msg.extend_from_slice(&1u32.to_ne_bytes()); // sequence
    msg.extend_from_slice(&0u32.to_ne_bytes()); // port: the kernel's
    msg.extend_from_slice(body);
    msg
}

/// A dump request of type `kind` (every entry of a table), carrying `body`:
/// the kernel answers in parts, then NLMSG_DONE.
pub fn dump_request(kind: u16, body: &[u8]) -> Vec<u8> {
    let mut msg = request(kind, body);
    let flags = (libc::NLM_F_REQUEST | libc::NLM_F_DUMP) as u16;
    msg[FLAGS..FLAGS + size_of::<u16>()].copy_from_slice(&flags.to_ne_bytes());
    msg
}

/// The error code of an acknowledgement (0: done), if `msg` is one.
pub fn ack_code(msg: &[u8]) -> Option<i32> {
    if ne16(msg.get(TYPE..)?) != libc::NLMSG_ERROR as u16 {
        return None;
    }
    let code = msg.get(ERROR_CODE..ERROR_CODE + size_of::<i32>())?;
    Some(i32::from_ne_bytes(code.try_into().ok()?))
}

/// The messages in a buffer the kernel sent: (type, flags, what follows
/// the header).
pub fn messages(mut b: &[u8]) -> Vec<(u16, u16, &[u8])> {
    let mut out = Vec::new();
    while b.len() >= HEADER {
        let Some(len) = b
            .get(LENGTH..LENGTH + size_of::<u32>())
            .and_then(|l| l.try_into().ok())
            .map(u32::from_ne_bytes)
        else {
            break;
        };
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        if len < HEADER || len > b.len() {
            break;
        }
        out.push((ne16(&b[TYPE..]), ne16(&b[FLAGS..]), &b[HEADER..len]));
        b = &b[align(len).min(b.len())..];
    }
    out
}

fn ne16(b: &[u8]) -> u16 {
    u16::from_ne_bytes([b[0], b[1]])
}
