//! Linux: netlink sockets (conntrack, rtnetlink), a socket's mark, tokio
//! watching what owns a descriptor, and the one environment change.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use tokio::io::unix::AsyncFd;

/// A netlink socket, owned: closed when dropped.
pub struct Netlink(OwnedFd);

impl Netlink {
    /// A netlink socket of `protocol` (NETLINK_ROUTE, NETLINK_NETFILTER),
    /// bound to the multicast `groups` (0: none), non-blocking if asked.
    pub fn open(protocol: libc::c_int, groups: u32, nonblocking: bool) -> io::Result<Self> {
        let flags =
            libc::SOCK_RAW | libc::SOCK_CLOEXEC | if nonblocking { libc::SOCK_NONBLOCK } else { 0 };
        // SAFETY: socket(2) takes plain integers.
        let fd = unsafe { libc::socket(libc::AF_NETLINK, flags, protocol) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a fresh descriptor, open, that nothing else owns.
        let owned = Self(unsafe { OwnedFd::from_raw_fd(fd) });
        // SAFETY: an all-zero sockaddr_nl is a valid one (plain integers).
        let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        addr.nl_family = libc::sa_family_t::try_from(libc::AF_NETLINK).unwrap_or_default();
        addr.nl_groups = groups;
        // SAFETY: bind reads a sockaddr_nl of the length given, ours.
        let r = unsafe {
            libc::bind(
                owned.0.as_raw_fd(),
                std::ptr::from_ref(&addr).cast(),
                socklen::<libc::sockaddr_nl>(),
            )
        };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(owned)
    }

    /// Send a whole message.
    pub fn send(&self, msg: &[u8]) -> io::Result<usize> {
        // SAFETY: send reads `msg.len()` bytes from a slice that long.
        let n = unsafe { libc::send(self.0.as_raw_fd(), msg.as_ptr().cast(), msg.len(), 0) };
        usize::try_from(n).map_err(|_| io::Error::last_os_error())
    }

    /// Receive into `buf`: the bytes received.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        // SAFETY: recv writes at most `buf.len()` bytes into a slice that
        // long, borrowed mutably for the call.
        let n = unsafe { libc::recv(self.0.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
        usize::try_from(n).map_err(|_| io::Error::last_os_error())
    }

    /// A receive buffer of `size` bytes, past the system's limit
    /// (SO_RCVBUFFORCE, CAP_NET_ADMIN): conntrack's events come in bursts.
    pub fn force_receive_buffer(&self, size: libc::c_int) -> io::Result<()> {
        set_option(self, libc::SOL_SOCKET, libc::SO_RCVBUFFORCE, size)
    }
}

impl AsRawFd for Netlink {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

/// A socket's mark (SO_MARK): its packets take the route of the tunnel
/// whose mark it is.
pub fn set_mark(sock: &impl AsRawFd, mark: u32) -> io::Result<()> {
    set_option(sock, libc::SOL_SOCKET, libc::SO_MARK, mark)
}

/// setsockopt with a plain integer value.
fn set_option<T: Copy>(
    sock: &impl AsRawFd,
    level: libc::c_int,
    name: libc::c_int,
    value: T,
) -> io::Result<()> {
    // SAFETY: setsockopt reads `size_of::<T>()` bytes from `value`, which
    // lives for the call; T is a plain integer for every option used here.
    let r = unsafe {
        libc::setsockopt(
            sock.as_raw_fd(),
            level,
            name,
            std::ptr::from_ref(&value).cast(),
            socklen::<T>(),
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn socklen<T>() -> libc::socklen_t {
    libc::socklen_t::try_from(size_of::<T>()).unwrap_or(libc::socklen_t::MAX)
}

/// What owns its descriptor for as long as it lives: open, and the same
/// one, until dropped. Only such things may be watched (`watch`), which
/// is tokio's `AsyncFd::register` contract.
pub trait OwnsFd: AsRawFd {}

impl OwnsFd for Netlink {}
impl OwnsFd for OwnedFd {}
// nfq's Queue opens its netlink socket when made and closes it when
// dropped, and hands out that one descriptor all along.
impl OwnsFd for nfq_updated::Queue {}

/// tokio watching `inner` for readiness.
pub fn watch<T: OwnsFd>(inner: T) -> io::Result<AsyncFd<T>> {
    // SAFETY: `OwnsFd`: the descriptor is valid, open and the same for
    // as long as the AsyncFd owns `inner`.
    unsafe { AsyncFd::register(inner) }.map_err(io::Error::from)
}

/// A tun device named `name` (no packet information: plain IP packets),
/// open for reading and writing, non-blocking if asked: tests make their
/// own tunnels' devices with it.
pub fn open_tun(name: &str, nonblocking: bool) -> io::Result<std::fs::File> {
    /// struct ifreq: the name, then the flags, padded to its size.
    #[repr(C)]
    struct IfReq {
        name: [u8; libc::IFNAMSIZ],
        flags: libc::c_short,
        pad: [u8; IFREQ_PAD],
    }
    if name.len() >= libc::IFNAMSIZ {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let tun = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/net/tun")?;
    let mut req = IfReq {
        name: [0; libc::IFNAMSIZ],
        flags: libc::c_short::try_from(libc::IFF_TUN | libc::IFF_NO_PI).unwrap_or_default(),
        pad: [0; IFREQ_PAD],
    };
    req.name[..name.len()].copy_from_slice(name.as_bytes());
    // SAFETY: TUNSETIFF reads and writes a struct ifreq, which `req` is
    // (laid out as the kernel's, its full size), ours for the call.
    if unsafe { libc::ioctl(tun.as_raw_fd(), libc::TUNSETIFF, &raw mut req) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if nonblocking {
        // SAFETY: fcntl(2) on a descriptor we own, with plain integers.
        if unsafe { libc::fcntl(tun.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(tun)
}

/// The bytes after struct ifreq's name and flags (40 bytes in all).
const IFREQ_PAD: usize = 22;

/// Unset `name` from the environment, only while this process has one
/// thread (the first thing in `main`): with another thread, which might
/// read the environment at the same time, nothing is done. Whether it was.
pub fn unset_env_while_alone(name: &str) -> bool {
    if !alone() {
        return false;
    }
    // SAFETY: one thread, checked just above: nothing reads or writes
    // the environment at the same time.
    unsafe { std::env::remove_var(name) };
    true
}

/// This process has one thread (its tasks in /proc).
fn alone() -> bool {
    std::fs::read_dir("/proc/self/task").is_ok_and(|d| d.count() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_netlink_socket_answers() {
        // A route dump: every system has one, no privileges needed.
        let sock = Netlink::open(libc::NETLINK_ROUTE, 0, false).unwrap();
        let mut msg = vec![0u8; 16 + 12]; // nlmsghdr + rtgenmsg, padded
        let len = u32::try_from(msg.len()).unwrap();
        msg[..4].copy_from_slice(&len.to_ne_bytes());
        msg[4..6].copy_from_slice(&libc::RTM_GETLINK.to_ne_bytes());
        let flags = u16::try_from(libc::NLM_F_REQUEST | libc::NLM_F_DUMP).unwrap();
        msg[6..8].copy_from_slice(&flags.to_ne_bytes());
        assert_eq!(sock.send(&msg).unwrap(), msg.len());
        let mut buf = vec![0u8; 8192];
        assert!(sock.recv(&mut buf).unwrap() > 0);
    }

    #[test]
    fn a_test_has_more_than_one_thread() {
        // The test harness runs threads: nothing is unset.
        assert!(!unset_env_while_alone("CRAKE_NOTHING"));
    }
}
