//! What Linux and macOS do alike: the machine's interfaces, interface
//! names and indexes, signals.

use std::ffi::{CStr, CString};
use std::net::{Ipv4Addr, Ipv6Addr};

/// Every IPv4 address on the machine: interface, address, netmask (as a
/// host-order u32).
pub fn interfaces() -> Vec<(String, Ipv4Addr, u32)> {
    let mut found = Vec::new();
    each_address(|name, addr, mask| {
        if i32::from(addr.sa_family) == libc::AF_INET {
            // SAFETY: `each_address` hands over addresses of their family's
            // own length: an AF_INET address is a sockaddr_in.
            let (a, m) = unsafe {
                (
                    &*std::ptr::from_ref(addr).cast::<libc::sockaddr_in>(),
                    &*std::ptr::from_ref(mask).cast::<libc::sockaddr_in>(),
                )
            };
            found.push((
                name.to_owned(),
                Ipv4Addr::from(u32::from_be(a.sin_addr.s_addr)),
                u32::from_be(m.sin_addr.s_addr),
            ));
        }
    });
    found
}

/// Every IPv6 address on the machine: interface, address, prefix length.
pub fn interfaces6() -> Vec<(String, Ipv6Addr, u8)> {
    let mut found = Vec::new();
    each_address(|name, addr, mask| {
        if i32::from(addr.sa_family) == libc::AF_INET6 {
            // SAFETY: as above: an AF_INET6 address is a sockaddr_in6.
            let (a, m) = unsafe {
                (
                    &*std::ptr::from_ref(addr).cast::<libc::sockaddr_in6>(),
                    &*std::ptr::from_ref(mask).cast::<libc::sockaddr_in6>(),
                )
            };
            let prefix = u128::from_be_bytes(m.sin6_addr.s6_addr).leading_ones();
            found.push((
                name.to_owned(),
                Ipv6Addr::from(a.sin6_addr.s6_addr),
                u8::try_from(prefix).unwrap_or(u8::MAX),
            ));
        }
    });
    found
}

/// Each interface address with a netmask, from getifaddrs: its name, the
/// address and the mask, valid for the call.
fn each_address(mut f: impl FnMut(&str, &libc::sockaddr, &libc::sockaddr)) {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills `head` with a list we own until
    // freeifaddrs; each node, its name and its addresses stay valid until
    // then, and are only read here. An address is as long as its family
    // says (the kernel's own sockaddr for it).
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return;
        }
        let mut p = head;
        while !p.is_null() {
            let ifa = &*p;
            if !ifa.ifa_addr.is_null() && !ifa.ifa_netmask.is_null() && !ifa.ifa_name.is_null() {
                let name = CStr::from_ptr(ifa.ifa_name).to_string_lossy();
                f(&name, &*ifa.ifa_addr, &*ifa.ifa_netmask);
            }
            p = ifa.ifa_next;
        }
        libc::freeifaddrs(head);
    }
}

/// An interface's index by its name, or None.
pub fn index_of(name: &str) -> Option<u32> {
    let name = CString::new(name).ok()?;
    // SAFETY: a valid NUL-terminated string, only read.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    (index != 0).then_some(index)
}

/// An interface's name by its index, or None.
pub fn name_of(index: u32) -> Option<String> {
    let mut buf = [0 as libc::c_char; libc::IF_NAMESIZE];
    // SAFETY: a buffer of IF_NAMESIZE, as if_indextoname writes at most.
    let p = unsafe { libc::if_indextoname(index, buf.as_mut_ptr()) };
    if p.is_null() {
        return None;
    }
    // SAFETY: if_indextoname wrote a NUL-terminated name into buf.
    let name = unsafe { CStr::from_ptr(buf.as_ptr()) };
    Some(name.to_string_lossy().into_owned())
}

/// Send `signal` to the process `pid` (openconnect's: SIGHUP keeps its
/// session, SIGTERM logs off).
pub fn signal(pid: u32, signal: libc::c_int) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: kill(2) takes plain integers and touches no memory of ours.
    if unsafe { libc::kill(pid, signal) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_is_there_both_ways() {
        let lo = interfaces()
            .into_iter()
            .find(|(_, a, _)| a.is_loopback())
            .expect("an IPv4 loopback address");
        let index = index_of(&lo.0).expect("its index");
        assert_eq!(name_of(index).as_deref(), Some(lo.0.as_str()));
        assert_eq!(index_of("no-such-interface"), None);
        assert_eq!(name_of(u32::MAX), None);
    }

    #[test]
    fn signalling_nobody_says_so() {
        assert!(signal(u32::MAX, libc::SIGTERM).is_err());
    }
}
