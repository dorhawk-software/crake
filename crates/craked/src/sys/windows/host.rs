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
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
        GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    const ERROR_BUFFER_OVERFLOW: u32 = 111;
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size: u32 = 16 * 1024;
    let mut buf: Vec<u64>;
    loop {
        buf = vec![0; (size as usize).div_ceil(8)];
        // SAFETY: `buf` holds `size` bytes, 8-byte aligned.
        let err = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_INET),
                flags,
                std::ptr::null(),
                buf.as_mut_ptr().cast(),
                &raw mut size,
            )
        };
        match err {
            0 => break,
            ERROR_BUFFER_OVERFLOW => continue,
            _ => return Vec::new(),
        }
    }
    let mut found = Vec::new();
    // SAFETY: a list Windows wrote into `buf`, walked by its own links.
    unsafe {
        let mut adapter = buf.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !adapter.is_null() {
            let a = &*adapter;
            let name = wide(a.FriendlyName);
            let mut unicast = a.FirstUnicastAddress;
            while !unicast.is_null() {
                let u = &*unicast;
                let sa = u.Address.lpSockaddr;
                if !sa.is_null() && (*sa).sa_family == AF_INET {
                    let sin = &*sa.cast::<SOCKADDR_IN>();
                    let addr = Ipv4Addr::from(sin.sin_addr.S_un.S_addr.to_ne_bytes());
                    let prefix = u32::from(u.OnLinkPrefixLength.min(32));
                    let mask = u32::MAX.checked_shl(32 - prefix).unwrap_or(0);
                    found.push((name.clone(), addr, mask));
                }
                unicast = u.Next;
            }
            adapter = a.Next;
        }
    }
    found
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
pub fn bind_device(sock: &tokio::net::UdpSocket, device: &str) -> std::io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    unicast_if(sock.as_raw_socket(), device)
}

/// IP_UNICAST_IF: the interface's index, in network byte order.
fn unicast_if(raw: std::os::windows::io::RawSocket, device: &str) -> std::io::Result<()> {
    use windows_sys::Win32::Networking::WinSock::{IP_UNICAST_IF, IPPROTO_IP, SOCKET, setsockopt};
    let index = super::iphelper::index(device).map_err(std::io::Error::other)?;
    let value = index.to_be();
    // SAFETY: a live socket of ours; `value` is the u32 the option takes.
    let err = unsafe {
        setsockopt(
            raw as SOCKET,
            IPPROTO_IP,
            IP_UNICAST_IF,
            (&raw const value).cast(),
            size_of::<u32>() as i32,
        )
    };
    if err == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// openconnect gets a console of its own (in the service's session, so
/// nobody sees it): console events are how it's told to stop (`stop`).
pub fn own_group(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NEW_CONSOLE);
}

/// Stop openconnect: Ctrl+Break detaches and keeps its session (SIGHUP's
/// counterpart), Ctrl+C logs off. Killed if it doesn't end in 10 s.
pub async fn stop(child: &mut Child, keep_session: bool) {
    if let Some(pid) = child.id() {
        let event = if keep_session { "break" } else { "c" };
        if send_console_event(pid, event).await
            && tokio::time::timeout(Duration::from_secs(10), child.wait())
                .await
                .is_ok()
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
        std::thread::sleep(Duration::from_millis(500));
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
