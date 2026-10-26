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

/// A socket that takes the tunnel's path: bound to its interface. The
/// direct mark (`plan::DIRECT_MARK`, the re-check of public hosts learned
/// by mistake) binds to the default route's: a learned host's route goes
/// through its tunnel, and craked's own sockets follow routes too.
pub fn mark(sock: &impl AsRawFd, mark: u32) -> std::io::Result<()> {
    let tunnels = PATHS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let device = if mark == crate::plan::DIRECT_MARK {
        direct_device(&tunnels)?
    } else {
        tunnels
            .get(&mark)
            .cloned()
            .ok_or_else(|| std::io::Error::other("no tunnel is up for this path"))?
    };
    bind_device(sock, &device)
}

/// The IPv4 default gateway, and what `arp -n` says about it
/// (`networks::current`).
pub fn default_gateway() -> Option<(std::net::Ipv4Addr, Option<String>)> {
    let out = std::process::Command::new("route")
        .args(["-n", "get", "default"])
        .output()
        .ok()?;
    let (_, gateway) = super::cmd::parse_route_get(&String::from_utf8_lossy(&out.stdout));
    let gateway: std::net::Ipv4Addr = gateway?.parse().ok()?;
    let arp = std::process::Command::new("arp")
        .args(["-n", &gateway.to_string()])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    Some((gateway, arp))
}

/// The default route's interface (`route -n get default`: Crake's own
/// default routes are scoped to their utuns, a full tunnel's are halves);
/// none while that's a tunnel of Crake's: nothing goes direct then.
fn direct_device(tunnels: &BTreeMap<u32, String>) -> std::io::Result<String> {
    let out = std::process::Command::new("route")
        .args(["-n", "get", "default"])
        .output()?;
    let device = super::cmd::route_interface(&String::from_utf8_lossy(&out.stdout))
        .ok_or_else(|| std::io::Error::other("no default route"))?;
    if tunnels.values().any(|d| *d == device) {
        return Err(std::io::Error::other("everything goes through a tunnel"));
    }
    Ok(device)
}

/// A socket that leaves through this interface: IP_BOUND_IF for an IPv4
/// socket, IPV6_BOUND_IF for an IPv6 one. The socket's family isn't asked:
/// each is tried, and one taking it is enough.
pub fn bind_device(sock: &impl AsRawFd, device: &str) -> std::io::Result<()> {
    let index = crake_os::unix::index_of(device)
        .ok_or_else(|| std::io::Error::other(format!("no interface {device}")))?;
    let v4 = crake_os::unix::set_option(sock, libc::IPPROTO_IP, libc::IP_BOUND_IF, index);
    let v6 = crake_os::unix::set_option(sock, libc::IPPROTO_IPV6, libc::IPV6_BOUND_IF, index);
    v4.or(v6)
}

/// (bytes on physical interfaces, bytes on Crake's tunnels, bytes per
/// tunnel device), both directions: each link's `if_data` (getifaddrs).
pub fn counters(devices: &HashMap<String, String>) -> (u64, u64, HashMap<String, u64>) {
    let (mut physical, mut tunnels, mut each) = (0, 0, HashMap::new());
    for (name, bytes) in crake_os::macos::link_bytes() {
        if devices.contains_key(&name) {
            tunnels += bytes;
            each.insert(name, bytes);
        } else if name.starts_with("en") {
            // Ethernet and Wi-Fi: not loopback, bridges or other tunnels.
            physical += bytes;
        }
    }
    (physical, tunnels, each)
}

/// The system's own DNS servers, which craked's server asks for public
/// names: the ones it stands in for (`primary_dns`).
pub fn system_dns() -> Vec<std::net::IpAddr> {
    super::primary_dns::upstream()
}

/// The tun device a process holds open: OpenVPN 3 sessions are handed to
/// craked on Linux only (`ADOPTS_OPENVPN3`).
pub fn tun_of_process(_pid: u32) -> Option<String> {
    None
}

/// Whether an OpenVPN 3 session's process still runs: Linux only too.
pub fn process_alive(_pid: u32) -> bool {
    false
}
