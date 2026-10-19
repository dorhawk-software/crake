//! The machine's adapters and interfaces (GetAdaptersAddresses,
//! GetIfTable2), a socket bound to an interface (IP_UNICAST_IF), console
//! events, the console session: as plain values and safe calls.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::windows::io::RawSocket;

use windows_sys::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    GetIfTable2, IP_ADAPTER_ADDRESSES_LH, MIB_IF_TABLE2,
};
use windows_sys::Win32::Networking::WinSock::{
    ADDRESS_FAMILY, AF_INET, AF_INET6, IP_UNICAST_IF, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF,
    SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, SOCKET, setsockopt,
};
use windows_sys::Win32::System::Console::{
    AttachConsole, CTRL_BREAK_EVENT, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent,
    SetConsoleCtrlHandler,
};
use windows_sys::Win32::System::RemoteDesktop::{
    ProcessIdToSessionId, WTSGetActiveConsoleSessionId,
};

/// An adapter as GetAdaptersAddresses tells it.
#[derive(Clone, Debug, Default)]
pub struct Adapter {
    /// Its alias ("Wi-Fi", "crake-acme").
    pub name: String,
    /// Operationally up.
    pub up: bool,
    /// Its own addresses, with their on-link prefix lengths.
    pub unicast: Vec<(IpAddr, u8)>,
    /// The DNS servers it was given.
    pub dns_servers: Vec<IpAddr>,
    /// Its DNS suffix, then its suffix search list (empty ones left out).
    pub suffixes: Vec<String>,
}

/// IfOperStatusUp.
const OPER_UP: i32 = 1;
const ERROR_BUFFER_OVERFLOW: u32 = 111;
/// A first guess at the list's size; the call says if it needs more.
const FIRST_SIZE: u32 = 16 * 1024;

/// Every adapter with addresses of `family` (AF_INET, AF_INET6, AF_UNSPEC).
pub fn adapters(family: ADDRESS_FAMILY) -> Vec<Adapter> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let mut size: u32 = FIRST_SIZE;
    let mut buf: Vec<u64>;
    loop {
        buf = vec![
            0;
            usize::try_from(size)
                .unwrap_or(0)
                .div_ceil(size_of::<u64>())
        ];
        // SAFETY: `buf` holds `size` bytes, 8-byte aligned, as the call is told.
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
            ERROR_BUFFER_OVERFLOW => {}
            _ => return Vec::new(),
        }
    }
    let mut found = Vec::new();
    // SAFETY: a list Windows wrote into `buf`, which outlives this walk,
    // followed by its own links; its strings are NUL-terminated and its
    // socket addresses are as long as their family says.
    unsafe {
        let mut adapter = buf.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !adapter.is_null() {
            let a = &*adapter;
            let mut one = Adapter {
                name: wide(a.FriendlyName),
                up: a.OperStatus == OPER_UP,
                ..Adapter::default()
            };
            let mut unicast = a.FirstUnicastAddress;
            while !unicast.is_null() {
                let u = &*unicast;
                if let Some(addr) = address(u.Address.lpSockaddr) {
                    one.unicast.push((addr, u.OnLinkPrefixLength));
                }
                unicast = u.Next;
            }
            let mut server = a.FirstDnsServerAddress;
            while !server.is_null() {
                let s = &*server;
                if let Some(addr) = address(s.Address.lpSockaddr) {
                    one.dns_servers.push(addr);
                }
                server = s.Next;
            }
            one.suffixes.push(wide(a.DnsSuffix));
            let mut suffix = a.FirstDnsSuffix;
            while !suffix.is_null() {
                let s = &*suffix;
                one.suffixes.push(wide(s.String.as_ptr()));
                suffix = s.Next;
            }
            one.suffixes.retain(|s| !s.is_empty());
            found.push(one);
            adapter = a.Next;
        }
    }
    found
}

/// A socket address's address, IPv4 or IPv6.
///
/// # Safety
/// `sa` is null or points to a socket address Windows filled in, as long
/// as its family says.
unsafe fn address(sa: *const SOCKADDR) -> Option<IpAddr> {
    // SAFETY: as the caller promises; the family says which it is.
    unsafe {
        if sa.is_null() {
            return None;
        }
        match (*sa).sa_family {
            AF_INET => {
                let sin = &*sa.cast::<SOCKADDR_IN>();
                Some(Ipv4Addr::from(sin.sin_addr.S_un.S_addr.to_ne_bytes()).into())
            }
            AF_INET6 => {
                let sin6 = &*sa.cast::<SOCKADDR_IN6>();
                Some(Ipv6Addr::from(sin6.sin6_addr.u.Byte).into())
            }
            _ => None,
        }
    }
}

/// A NUL-terminated UTF-16 string Windows owns, copied.
///
/// # Safety
/// `p` is null or points to a NUL-terminated UTF-16 string.
pub(crate) unsafe fn wide(p: *const u16) -> String {
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

/// Each interface's bytes in and out so far: its alias, the bytes, and
/// whether it's hardware (not loopback, virtual switches or filters).
pub fn interface_bytes() -> Vec<(String, u64, bool)> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    let mut found = Vec::new();
    // SAFETY: Windows allocates the table, with NumEntries rows; they're
    // read, then the table is freed.
    unsafe {
        if GetIfTable2(&raw mut table) != 0 {
            return found;
        }
        let rows = std::slice::from_raw_parts(
            (*table).Table.as_ptr(),
            usize::try_from((*table).NumEntries).unwrap_or(0),
        );
        for row in rows {
            // Bit 0 of the flags: a hardware interface.
            let hardware = row.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            found.push((
                wide(row.Alias.as_ptr()),
                row.InOctets + row.OutOctets,
                hardware,
            ));
        }
        FreeMibTable(table.cast());
    }
    found
}

/// A socket that leaves through the interface `index`: IP_UNICAST_IF (in
/// network byte order) for an IPv4 socket, IPV6_UNICAST_IF (host order)
/// for an IPv6 one. The family isn't asked: each is tried, one is enough.
pub fn unicast_index(sock: RawSocket, index: u32) -> io::Result<()> {
    let set = |level: i32, option: i32, value: u32| {
        // SAFETY: setsockopt reads the u32 `value`, which lives for the
        // call; a socket that isn't valid is an error, not a fault.
        let err = unsafe {
            setsockopt(
                sock as SOCKET,
                level,
                option,
                (&raw const value).cast(),
                i32::try_from(size_of::<u32>()).unwrap_or(i32::MAX),
            )
        };
        if err == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    };
    let v4 = set(IPPROTO_IP, IP_UNICAST_IF, index.to_be());
    let v6 = set(IPPROTO_IPV6, IPV6_UNICAST_IF, index);
    v4.or(v6)
}

/// Attach to `pid`'s console and send Ctrl+C (`ctrl_c`) or Ctrl+Break to
/// everything on it, this process swallowing its own copy; waits `settle`
/// for the event, which arrives on a thread of its own. Whether it was sent.
pub fn deliver_console_event(pid: u32, ctrl_c: bool, settle: std::time::Duration) -> bool {
    let event = if ctrl_c {
        CTRL_C_EVENT
    } else {
        CTRL_BREAK_EVENT
    };
    // SAFETY: plain console calls with integers; `swallow` is a function
    // that lives for the whole program.
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return false;
        }
        SetConsoleCtrlHandler(Some(swallow), 1);
        let sent = GenerateConsoleCtrlEvent(event, 0) != 0;
        std::thread::sleep(settle);
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
