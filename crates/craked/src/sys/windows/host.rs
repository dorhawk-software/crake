//! Windows: the machine's interfaces (GetAdaptersAddresses), sockets bound
//! to a tunnel's interface (IP_UNICAST_IF), openconnect's process, the
//! console session.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use tokio::process::{Child, Command};
use windows_sys::Win32::System::Console::{
    AttachConsole, CTRL_BREAK_EVENT, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent,
    SetConsoleCtrlHandler,
};
use windows_sys::Win32::System::RemoteDesktop::{
    ProcessIdToSessionId, WTSGetActiveConsoleSessionId,
};
use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;

/// Every IPv4 address on the machine: interface (its alias, as `crake-acme`
/// for a tunnel), address, netmask.
pub fn interfaces() -> Vec<(String, Ipv4Addr, u32)> {
    let mut found = Vec::new();
    each_adapter(
        windows_sys::Win32::Networking::WinSock::AF_INET,
        |a, name| {
            // SAFETY: Windows' list of the adapter's addresses, walked by its
            // own links.
            unsafe {
                let mut unicast = a.FirstUnicastAddress;
                while !unicast.is_null() {
                    let u = &*unicast;
                    if let Some(addr) = ipv4(u.Address.lpSockaddr) {
                        let prefix = u32::from(u.OnLinkPrefixLength.min(crate::hosts::V4_BITS));
                        let mask = u32::MAX.checked_shl(Ipv4Addr::BITS - prefix).unwrap_or(0);
                        found.push((name.to_owned(), addr, mask));
                    }
                    unicast = u.Next;
                }
            }
        },
    );
    found
}

/// Every IPv6 address on the machine: interface, address, prefix length.
pub fn interfaces6() -> Vec<(String, std::net::Ipv6Addr, u8)> {
    use windows_sys::Win32::Networking::WinSock::{AF_INET6, SOCKADDR_IN6};
    let mut found = Vec::new();
    each_adapter(AF_INET6, |a, name| {
        // SAFETY: as in `interfaces`; AF_INET6 means a SOCKADDR_IN6.
        unsafe {
            let mut unicast = a.FirstUnicastAddress;
            while !unicast.is_null() {
                let u = &*unicast;
                let sa = u.Address.lpSockaddr;
                if !sa.is_null() && (*sa).sa_family == AF_INET6 {
                    let sin6 = &*sa.cast::<SOCKADDR_IN6>();
                    let addr = std::net::Ipv6Addr::from(sin6.sin6_addr.u.Byte);
                    let prefix = u.OnLinkPrefixLength.min(crate::hosts::V6_BITS);
                    found.push((name.to_owned(), addr, prefix));
                }
                unicast = u.Next;
            }
        }
    });
    found
}

/// The DNS servers Windows was given (DHCP, or by hand) on the adapters
/// that are up, Crake's own tunnels left out: where public names go.
pub fn system_dns() -> Vec<Ipv4Addr> {
    let mut found = Vec::new();
    each_adapter(
        windows_sys::Win32::Networking::WinSock::AF_INET,
        |a, name| {
            // 1: IfOperStatusUp.
            if a.OperStatus != 1 || name.starts_with("crake-") {
                return;
            }
            // SAFETY: as in `interfaces`.
            unsafe {
                let mut server = a.FirstDnsServerAddress;
                while !server.is_null() {
                    let s = &*server;
                    if let Some(addr) = ipv4(s.Address.lpSockaddr)
                        && !addr.is_loopback()
                        && !found.contains(&addr)
                    {
                        found.push(addr);
                    }
                    server = s.Next;
                }
            }
        },
    );
    found
}

/// Each adapter with addresses of `family`, with its alias.
fn each_adapter(
    family: windows_sys::Win32::Networking::WinSock::ADDRESS_FAMILY,
    mut f: impl FnMut(&windows_sys::Win32::NetworkManagement::IpHelper::IP_ADAPTER_ADDRESSES_LH, &str),
) {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
        IP_ADAPTER_ADDRESSES_LH,
    };
    const ERROR_BUFFER_OVERFLOW: u32 = 111;
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    // A first guess at the size; the call says if it needs more.
    const FIRST_SIZE: u32 = 16 * 1024;
    let mut size: u32 = FIRST_SIZE;
    let mut buf: Vec<u64>;
    loop {
        buf = vec![0; (size as usize).div_ceil(size_of::<u64>())];
        // SAFETY: `buf` holds `size` bytes, 8-byte aligned.
        let err = unsafe {
            GetAdaptersAddresses(
                u32::from(family),
                flags,
                std::ptr::null(),
                buf.as_mut_ptr().cast(),
                &raw mut size,
            )
        };
        match err {
            0 => break,
            ERROR_BUFFER_OVERFLOW => continue,
            _ => return,
        }
    }
    // SAFETY: a list Windows wrote into `buf`, walked by its own links.
    unsafe {
        let mut adapter = buf.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !adapter.is_null() {
            let a = &*adapter;
            f(a, &wide(a.FriendlyName));
            adapter = a.Next;
        }
    }
}

/// An IPv4 socket address's address.
///
/// # Safety
/// `sa` is null or points to a socket address Windows filled in.
unsafe fn ipv4(sa: *const windows_sys::Win32::Networking::WinSock::SOCKADDR) -> Option<Ipv4Addr> {
    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    // SAFETY: as the caller promises; AF_INET means a SOCKADDR_IN.
    unsafe {
        if sa.is_null() || (*sa).sa_family != AF_INET {
            return None;
        }
        let sin = &*sa.cast::<SOCKADDR_IN>();
        Some(Ipv4Addr::from(sin.sin_addr.S_un.S_addr.to_ne_bytes()))
    }
}

/// A NUL-terminated UTF-16 string Windows owns, copied.
///
/// # Safety
/// `p` is null or points to a NUL-terminated UTF-16 string.
unsafe fn wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: up to the NUL, as the caller promises.
    unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
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

/// A socket that takes the tunnel's path: bound to its interface.
pub fn mark(sock: &tokio::net::TcpSocket, mark: u32) -> std::io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    let device = PATHS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&mark)
        .cloned()
        .ok_or_else(|| std::io::Error::other("no tunnel is up for this path"))?;
    unicast_if(sock.as_raw_socket(), &device)
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
    use windows_sys::Win32::Networking::WinSock::{
        IP_UNICAST_IF, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF, SOCKET, setsockopt,
    };
    let index = super::iphelper::index(device).map_err(std::io::Error::other)?;
    let set = |level: i32, option: i32, value: u32| {
        // SAFETY: a live socket of ours; `value` is the u32 the option takes.
        let err = unsafe {
            setsockopt(
                raw as SOCKET,
                level,
                option,
                (&raw const value).cast(),
                size_of::<u32>() as i32,
            )
        };
        if err == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    };
    let v4 = set(IPPROTO_IP, IP_UNICAST_IF, index.to_be());
    let v6 = set(IPPROTO_IPV6, IPV6_UNICAST_IF, index);
    v4.or(v6)
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
    let event = match event {
        "c" => CTRL_C_EVENT,
        "break" => CTRL_BREAK_EVENT,
        _ => return false,
    };
    // SAFETY: plain console calls; `swallow` lives for the whole program.
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return false;
        }
        SetConsoleCtrlHandler(Some(swallow), 1);
        let sent = GenerateConsoleCtrlEvent(event, 0) != 0;
        // The event arrives on a thread of its own: be here for it.
        std::thread::sleep(EVENT_ARRIVES);
        sent
    }
}

unsafe extern "system" fn swallow(_event: u32) -> windows_sys::core::BOOL {
    1
}

/// `pid` runs in the session at the console (the signed-in user's).
pub fn in_console_session(pid: u32) -> bool {
    let mut session = 0;
    // SAFETY: `session` is a valid out pointer for the call.
    let found = unsafe { ProcessIdToSessionId(pid, &raw mut session) } != 0;
    // SAFETY: no arguments.
    found && session == unsafe { WTSGetActiveConsoleSessionId() }
}

/// A new file for craked alone, replacing any older one (the service's
/// folder is only open to SYSTEM and administrators).
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let _ = std::fs::remove_file(path);
    std::fs::File::create(path)
}

/// (bytes on physical interfaces, bytes on Crake's tunnels, bytes per
/// tunnel device), both directions, from Windows' interface table.
pub fn counters(
    devices: &std::collections::HashMap<String, String>,
) -> (u64, u64, std::collections::HashMap<String, u64>) {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIfTable2, MIB_IF_TABLE2,
    };
    let (mut physical, mut tunnels, mut each) = (0, 0, std::collections::HashMap::new());
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    // SAFETY: Windows allocates the table; it's read, then freed.
    unsafe {
        if GetIfTable2(&raw mut table) != 0 {
            return (0, 0, each);
        }
        let rows =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        for row in rows {
            let name = wide(row.Alias.as_ptr());
            let bytes = row.InOctets + row.OutOctets;
            // Bit 0 of the flags: a hardware interface (not loopback,
            // virtual switches or other filters stacked on one).
            let hardware = row.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if name.starts_with("crake-") || devices.contains_key(&name) {
                tunnels += bytes;
                each.insert(name, bytes);
            } else if hardware {
                physical += bytes;
            }
        }
        FreeMibTable(table.cast());
    }
    (physical, tunnels, each)
}
