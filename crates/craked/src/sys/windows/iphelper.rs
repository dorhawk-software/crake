//! The IP Helper API, as craked uses it: a tunnel's interface by name (its
//! alias, `crake-<name>`, as openconnect gives Wintun's adapter), its
//! address and MTU, and routes on it.

use std::net::Ipv4Addr;

use anyhow::{Result, bail};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    ConvertInterfaceAliasToLuid, ConvertInterfaceLuidToIndex, CreateIpForwardEntry2,
    CreateUnicastIpAddressEntry, DeleteIpForwardEntry2, GetBestRoute2, GetIpInterfaceEntry,
    InitializeIpForwardEntry, InitializeIpInterfaceEntry, InitializeUnicastIpAddressEntry,
    MIB_IPFORWARD_ROW2, MIB_IPINTERFACE_ROW, MIB_UNICASTIPADDRESS_ROW, SetIpInterfaceEntry,
};
use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, IN_ADDR, IN_ADDR_0, SOCKADDR_IN, SOCKADDR_INET,
};

const ERROR_NOT_FOUND: u32 = 1168;
const ERROR_OBJECT_ALREADY_EXISTS: u32 = 5010;

/// An interface, by its alias.
pub fn luid(alias: &str) -> Result<NET_LUID_LH> {
    let wide: Vec<u16> = alias.encode_utf16().chain([0]).collect();
    // SAFETY: a plain C union, all zero is valid; `wide` is NUL-terminated.
    let mut luid: NET_LUID_LH = unsafe { std::mem::zeroed() };
    let err = unsafe { ConvertInterfaceAliasToLuid(wide.as_ptr(), &raw mut luid) };
    if err != 0 {
        bail!("no interface {alias} ({})", os_error(err));
    }
    Ok(luid)
}

/// An interface's index, by its alias.
pub fn index(alias: &str) -> Result<u32> {
    let luid = luid(alias)?;
    let mut index = 0;
    // SAFETY: a LUID from Windows, an out pointer for the call.
    let err = unsafe { ConvertInterfaceLuidToIndex(&raw const luid, &raw mut index) };
    if err != 0 {
        bail!("no index for {alias} ({})", os_error(err));
    }
    Ok(index)
}

fn sockaddr(addr: Ipv4Addr) -> SOCKADDR_INET {
    SOCKADDR_INET {
        Ipv4: SOCKADDR_IN {
            sin_family: AF_INET,
            sin_port: 0,
            sin_addr: IN_ADDR {
                S_un: IN_ADDR_0 {
                    S_addr: u32::from_ne_bytes(addr.octets()),
                },
            },
            sin_zero: [0; 8],
        },
    }
}

/// `addr/prefix` on the interface.
pub fn set_address(luid: NET_LUID_LH, addr: Ipv4Addr, prefix: u8) -> Result<()> {
    // SAFETY: the row is initialized by Windows, then filled; the calls
    // only read it.
    let err = unsafe {
        let mut row: MIB_UNICASTIPADDRESS_ROW = std::mem::zeroed();
        InitializeUnicastIpAddressEntry(&raw mut row);
        row.InterfaceLuid = luid;
        row.Address = sockaddr(addr);
        row.OnLinkPrefixLength = prefix;
        CreateUnicastIpAddressEntry(&raw const row)
    };
    match err {
        0 | ERROR_OBJECT_ALREADY_EXISTS => Ok(()),
        e => bail!("setting {addr}/{prefix}: {}", os_error(e)),
    }
}

/// The interface's IPv4 MTU.
pub fn set_mtu(luid: NET_LUID_LH, mtu: u32) -> Result<()> {
    // SAFETY: as `set_address`; the row is read back first.
    let err = unsafe {
        let mut row: MIB_IPINTERFACE_ROW = std::mem::zeroed();
        InitializeIpInterfaceEntry(&raw mut row);
        row.Family = AF_INET;
        row.InterfaceLuid = luid;
        let got = GetIpInterfaceEntry(&raw mut row);
        if got != 0 {
            got
        } else {
            row.NlMtu = mtu;
            // Windows refuses an IPv4 row with a site prefix length.
            row.SitePrefixLength = 0;
            SetIpInterfaceEntry(&raw mut row)
        }
    };
    match err {
        0 => Ok(()),
        e => bail!("setting the MTU to {mtu}: {}", os_error(e)),
    }
}

/// A route: `dest/prefix` out of the interface, to `via` (unspecified: on
/// the link, as a tunnel carries it).
#[derive(Clone, Copy)]
pub struct Route {
    pub luid: NET_LUID_LH,
    pub dest: Ipv4Addr,
    pub prefix: u8,
    pub via: Ipv4Addr,
}

fn route(luid: NET_LUID_LH, dest: Ipv4Addr, prefix: u8, metric: u32) -> MIB_IPFORWARD_ROW2 {
    row(
        Route {
            luid,
            dest,
            prefix,
            via: Ipv4Addr::UNSPECIFIED,
        },
        metric,
    )
}

fn row(r: Route, metric: u32) -> MIB_IPFORWARD_ROW2 {
    // SAFETY: initialized by Windows, then filled.
    unsafe {
        let mut row: MIB_IPFORWARD_ROW2 = std::mem::zeroed();
        InitializeIpForwardEntry(&raw mut row);
        row.InterfaceLuid = r.luid;
        row.DestinationPrefix.Prefix = sockaddr(r.dest);
        row.DestinationPrefix.PrefixLength = r.prefix;
        row.NextHop = sockaddr(r.via);
        row.Metric = metric;
        row
    }
}

/// The way Windows sends to `dest` now: its route's interface, next hop
/// and prefix length (with `dest` as the destination).
pub fn best_route(dest: Ipv4Addr) -> Result<(Route, u8)> {
    let to = sockaddr(dest);
    // SAFETY: zeroed out values Windows fills; `to` is only read.
    let (err, best) = unsafe {
        let mut best: MIB_IPFORWARD_ROW2 = std::mem::zeroed();
        let mut source: SOCKADDR_INET = std::mem::zeroed();
        let err = GetBestRoute2(
            std::ptr::null(),
            0,
            std::ptr::null(),
            &raw const to,
            0,
            &raw mut best,
            &raw mut source,
        );
        (err, best)
    };
    if err != 0 {
        bail!("no route to {dest} ({})", os_error(err));
    }
    // SAFETY: an IPv4 route: its next hop is a SOCKADDR_IN.
    let via = unsafe { best.NextHop.Ipv4.sin_addr.S_un.S_addr };
    Ok((
        Route {
            luid: best.InterfaceLuid,
            dest,
            prefix: 32,
            via: Ipv4Addr::from(via.to_ne_bytes()),
        },
        best.DestinationPrefix.PrefixLength,
    ))
}

/// Add a route (already there: fine).
pub fn add(r: Route) -> Result<()> {
    let row = row(r, 0);
    // SAFETY: a filled row, only read.
    match unsafe { CreateIpForwardEntry2(&raw const row) } {
        0 | ERROR_OBJECT_ALREADY_EXISTS => Ok(()),
        e => bail!("routing {}/{}: {}", r.dest, r.prefix, os_error(e)),
    }
}

/// Remove a route (gone already: fine).
pub fn delete(r: Route) -> Result<()> {
    let row = row(r, 0);
    // SAFETY: a filled row, only read.
    match unsafe { DeleteIpForwardEntry2(&raw const row) } {
        0 | ERROR_NOT_FOUND => Ok(()),
        e => bail!(
            "removing the route to {}/{}: {}",
            r.dest,
            r.prefix,
            os_error(e)
        ),
    }
}

/// `dest/prefix` through the interface (already there: fine).
pub fn add_route(luid: NET_LUID_LH, dest: Ipv4Addr, prefix: u8) -> Result<()> {
    add_route_metric(luid, dest, prefix, 0)
}

/// The same with a route metric (higher: chosen only when nothing better).
pub fn add_route_metric(luid: NET_LUID_LH, dest: Ipv4Addr, prefix: u8, metric: u32) -> Result<()> {
    let row = route(luid, dest, prefix, metric);
    // SAFETY: a filled row, only read.
    match unsafe { CreateIpForwardEntry2(&raw const row) } {
        0 | ERROR_OBJECT_ALREADY_EXISTS => Ok(()),
        e => bail!("routing {dest}/{prefix}: {}", os_error(e)),
    }
}

/// Remove `dest/prefix` from the interface (gone already: fine).
pub fn delete_route(luid: NET_LUID_LH, dest: Ipv4Addr, prefix: u8) -> Result<()> {
    let row = route(luid, dest, prefix, 0);
    // SAFETY: a filled row, only read.
    match unsafe { DeleteIpForwardEntry2(&raw const row) } {
        0 | ERROR_NOT_FOUND => Ok(()),
        e => bail!("removing the route to {dest}/{prefix}: {}", os_error(e)),
    }
}

fn os_error(code: u32) -> std::io::Error {
    std::io::Error::from_raw_os_error(code as i32)
}
