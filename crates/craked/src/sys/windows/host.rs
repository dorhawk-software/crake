//! Windows: the machine's interfaces (GetAdaptersAddresses), sockets bound
//! to a tunnel's interface (IP_UNICAST_IF), openconnect's process, the
//! console session.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use crake_os::windows::host as adapters;
use tokio::process::{Child, Command};
use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;

/// Every IPv4 address on the machine: interface (its alias, as `crake-acme`
/// for a tunnel), address, netmask.
pub fn interfaces() -> Vec<(String, Ipv4Addr, u32)> {
    let mut found = Vec::new();
    for a in adapters::adapters(windows_sys::Win32::Networking::WinSock::AF_INET) {
        for &(addr, prefix) in &a.unicast {
            if let IpAddr::V4(addr) = addr {
                let prefix = u32::from(prefix.min(crate::hosts::V4_BITS));
                let mask = u32::MAX.checked_shl(Ipv4Addr::BITS - prefix).unwrap_or(0);
                found.push((a.name.clone(), addr, mask));
            }
        }
    }
    found
}

/// Every IPv6 address on the machine: interface, address, prefix length.
pub fn interfaces6() -> Vec<(String, std::net::Ipv6Addr, u8)> {
    let mut found = Vec::new();
    for a in adapters::adapters(windows_sys::Win32::Networking::WinSock::AF_INET6) {
        for &(addr, prefix) in &a.unicast {
            if let IpAddr::V6(addr) = addr {
                found.push((a.name.clone(), addr, prefix.min(crate::hosts::V6_BITS)));
            }
        }
    }
    found
}

/// The DNS servers Windows was given (DHCP, or by hand) on the adapters
/// that are up, Crake's own tunnels left out: where public names go.
pub fn system_dns() -> Vec<std::net::IpAddr> {
    let mut found: Vec<IpAddr> = Vec::new();
    for a in adapters::adapters(windows_sys::Win32::Networking::WinSock::AF_INET) {
        if !a.up || a.name.starts_with("crake-") {
            continue;
        }
        for addr in a.dns_servers {
            if addr.is_ipv4() && !addr.is_loopback() && !found.contains(&addr) {
                found.push(addr);
            }
        }
    }
    found
}

/// The DNS suffixes of the adapters that are up, Crake's own left out:
/// each one's own (DHCP's, or a VPN client's on its adapter, as
/// AnyConnect sets) and its suffix search list.
pub fn adapter_domains() -> Vec<String> {
    adapters::adapters(windows_sys::Win32::Networking::WinSock::AF_UNSPEC)
        .into_iter()
        .filter(|a| a.up && !a.name.starts_with("crake-"))
        .flat_map(|a| a.suffixes)
        .collect()
}

/// Which tunnel interface each routing mark stands for. Probing names a
/// tunnel by its mark (Linux's fwmark); Windows binds to its interface.
/// Filled by `routing` as tunnels come up and go down.
static PATHS: Mutex<BTreeMap<u32, String>> = Mutex::new(BTreeMap::new());

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
pub fn mark(sock: &impl std::os::windows::io::AsRawSocket, mark: u32) -> std::io::Result<()> {
    let index = if mark == crate::plan::DIRECT_MARK {
        direct_index()?
    } else {
        let device = PATHS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&mark)
            .cloned()
            .ok_or_else(|| std::io::Error::other("no tunnel is up for this path"))?;
        super::iphelper::index(&device).map_err(std::io::Error::other)?
    };
    unicast_index(sock.as_raw_socket(), index)
}

/// The interface the default route takes (the best route to 0.0.0.0: a
/// learned host's is more specific, and leaves it alone); none while that's
/// a tunnel of Crake's (a full tunnel's halves): nothing goes direct then.
fn direct_index() -> std::io::Result<u32> {
    use super::iphelper;
    let (best, _) = iphelper::best_route(std::net::Ipv4Addr::UNSPECIFIED.into())
        .map_err(std::io::Error::other)?;
    let index = iphelper::index_of(best.luid).map_err(std::io::Error::other)?;
    let tunnels: Vec<String> = PATHS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .cloned()
        .collect();
    if tunnels
        .iter()
        .any(|d| iphelper::index(d).is_ok_and(|i| i == index))
    {
        return Err(std::io::Error::other("everything goes through a tunnel"));
    }
    Ok(index)
}

/// A socket that leaves through this interface.
pub fn bind_device(
    sock: &impl std::os::windows::io::AsRawSocket,
    device: &str,
) -> std::io::Result<()> {
    unicast_if(sock.as_raw_socket(), device)
}

/// IP_UNICAST_IF (the interface's index in network byte order) for an
/// IPv4 socket, IPV6_UNICAST_IF (in host byte order) for an IPv6 one. The
/// socket's family isn't asked: each is tried, and one taking it is enough.
fn unicast_if(raw: std::os::windows::io::RawSocket, device: &str) -> std::io::Result<()> {
    let index = super::iphelper::index(device).map_err(std::io::Error::other)?;
    unicast_index(raw, index)
}

/// The same, by the interface's index.
fn unicast_index(raw: std::os::windows::io::RawSocket, index: u32) -> std::io::Result<()> {
    adapters::unicast_index(raw, index)
}

/// openconnect gets a console of its own (in the service's session, so
/// nobody sees it): console events are how it's told to stop (`stop`).
pub fn own_group(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NEW_CONSOLE);
}

/// Stop openconnect: Ctrl+Break detaches and keeps its session (SIGHUP's
/// counterpart), Ctrl+C logs off. Killed if it doesn't end in 10 s.
/// openconnect gets this long to stop after the console event.
const STOP_WAIT: Duration = Duration::from_secs(10);
/// A console event arrives on a thread of its own, about this soon.
const EVENT_ARRIVES: Duration = Duration::from_millis(500);

pub async fn stop(child: &mut Child, keep_session: bool) {
    if let Some(pid) = child.id() {
        let event = if keep_session { "break" } else { "c" };
        if send_console_event(pid, event).await
            && tokio::time::timeout(STOP_WAIT, child.wait()).await.is_ok()
        {
            return;
        }
    }
    let _ = child.kill().await;
}

/// A copy of craked delivers the event (`deliver_console_event`): whoever
/// attaches to openconnect's console to send it receives it too, and in a
/// service that ends the process (it ended craked). The copy takes the hit.
async fn send_console_event(pid: u32, event: &str) -> bool {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    Command::new(exe)
        .args(["--console-event", &pid.to_string(), event])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .await
        .is_ok_and(|s| s.success())
}

/// `craked --console-event <pid> c|break`: attach to `pid`'s console and
/// send the event to everything on it, this process swallowing its own copy
/// until it exits.
pub fn deliver_console_event(pid: u32, event: &str) -> bool {
    let ctrl_c = match event {
        "c" => true,
        "break" => false,
        _ => return false,
    };
    // The event arrives on a thread of its own: be here for it.
    adapters::deliver_console_event(pid, ctrl_c, EVENT_ARRIVES)
}

/// `pid` runs in the session at the console (the signed-in user's).
pub fn in_console_session(pid: u32) -> bool {
    adapters::in_console_session(pid)
}

/// A folder for SYSTEM and administrators alone (craked's own, OpenVPN's
/// profiles and runs): its access not inherited from ProgramData, where
/// every user may read; what's in it already inherits that (`/reset`: the
/// grants are for folders and what they hold, so `/T` with them left files
/// with no access at all, SYSTEM's included, and craked couldn't start).
/// By SID: the groups' names are the system language's.
pub fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::fs::create_dir_all(dir)?;
    let status = std::process::Command::new("icacls")
        .arg(dir)
        .args([
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:(OI)(CI)F",
            "*S-1-5-32-544:(OI)(CI)F",
            "/q",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    for entry in std::fs::read_dir(dir)?.flatten() {
        let _ = std::process::Command::new("icacls")
            .arg(entry.path())
            .args(["/reset", "/t", "/c", "/q"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "icacls couldn\u{2019}t lock {}",
            dir.display()
        )))
    }
}

/// A new file for craked alone, replacing any older one: in a folder made
/// with `create_private_dir`.
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let _ = std::fs::remove_file(path);
    std::fs::File::create(path)
}

/// (bytes on physical interfaces, bytes on Crake's tunnels, bytes per
/// tunnel device), both directions, from Windows' interface table.
pub fn counters(
    devices: &std::collections::HashMap<String, String>,
) -> (u64, u64, std::collections::HashMap<String, u64>) {
    let (mut physical, mut tunnels, mut each) = (0, 0, std::collections::HashMap::new());
    for (name, bytes, hardware) in adapters::interface_bytes() {
        if name.starts_with("crake-") || devices.contains_key(&name) {
            tunnels += bytes;
            each.insert(name, bytes);
        } else if hardware {
            physical += bytes;
        }
    }
    (physical, tunnels, each)
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
