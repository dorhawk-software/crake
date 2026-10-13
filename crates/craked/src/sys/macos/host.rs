//! macOS: sockets that take a tunnel's path (IP_BOUND_IF, IPV6_BOUND_IF),
//! and the interfaces' byte counters (the rest is `sys::unix`'s, shared
//! with Linux).

use std::collections::{BTreeMap, HashMap};
use std::os::fd::AsRawFd;
use std::sync::Mutex;

/// Which tunnel interface each routing mark stands for. Probing names a
/// tunnel by its mark (Linux's fwmark); macOS binds to its interface.
static PATHS: Mutex<BTreeMap<u32, String>> = Mutex::new(BTreeMap::new());

/// Filled by `routing` as tunnels come up and go down.
pub(crate) fn set_path(mark: u32, device: Option<&str>) {
    let mut paths = PATHS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match device {
        Some(d) => paths.insert(mark, d.to_owned()),
        None => paths.remove(&mark),
    };
}

/// A socket that takes the tunnel's path: bound to its interface.
pub fn mark(sock: &impl AsRawFd, mark: u32) -> std::io::Result<()> {
    let device = PATHS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&mark)
        .cloned()
        .ok_or_else(|| std::io::Error::other("no tunnel is up for this path"))?;
    bind_device(sock, &device)
}

/// A socket that leaves through this interface: IP_BOUND_IF for an IPv4
/// socket, IPV6_BOUND_IF for an IPv6 one. The socket's family isn't asked:
/// each is tried, and one taking it is enough.
pub fn bind_device(sock: &impl AsRawFd, device: &str) -> std::io::Result<()> {
    let name = std::ffi::CString::new(device).map_err(std::io::Error::other)?;
    // SAFETY: a NUL-terminated name.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    if index == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let set = |level: libc::c_int, option: libc::c_int| {
        // SAFETY: plain setsockopt with a u32 on a socket we own.
        let r = unsafe {
            libc::setsockopt(
                sock.as_raw_fd(),
                level,
                option,
                (&raw const index).cast(),
                std::mem::size_of::<u32>() as u32,
            )
        };
        if r == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    };
    let v4 = set(libc::IPPROTO_IP, libc::IP_BOUND_IF);
    let v6 = set(libc::IPPROTO_IPV6, libc::IPV6_BOUND_IF);
    v4.or(v6)
}

/// (bytes on physical interfaces, bytes on Crake's tunnels, bytes per
/// tunnel device), both directions: each link's `if_data` (getifaddrs).
pub fn counters(devices: &HashMap<String, String>) -> (u64, u64, HashMap<String, u64>) {
    let (mut physical, mut tunnels, mut each) = (0, 0, HashMap::new());
    // SAFETY: getifaddrs/freeifaddrs pair; an AF_LINK entry's data is its
    // if_data, only read.
    unsafe {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&raw mut head) != 0 {
            return (0, 0, each);
        }
        let mut p = head;
        while !p.is_null() {
            let ifa = &*p;
            p = ifa.ifa_next;
            if ifa.ifa_addr.is_null()
                || i32::from((*ifa.ifa_addr).sa_family) != libc::AF_LINK
                || ifa.ifa_data.is_null()
            {
                continue;
            }
            let name = std::ffi::CStr::from_ptr(ifa.ifa_name)
                .to_string_lossy()
                .into_owned();
            let data = &*ifa.ifa_data.cast::<libc::if_data>();
            let bytes = u64::from(data.ifi_ibytes) + u64::from(data.ifi_obytes);
            if devices.contains_key(&name) {
                tunnels += bytes;
                each.insert(name, bytes);
            } else if name.starts_with("en") {
                // Ethernet and Wi-Fi: not loopback, bridges or other tunnels.
                physical += bytes;
            }
        }
        libc::freeifaddrs(head);
    }
    (physical, tunnels, each)
}

/// The system's own DNS servers, which craked's server asks for public
/// names: the ones it stands in for (`primary_dns`).
pub fn system_dns() -> Vec<std::net::IpAddr> {
    super::primary_dns::upstream()
}
