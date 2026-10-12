//! Windows: new TCP connections from the connection table
//! (GetExtendedTcpTable), looked at four times a second; what changed
//! becomes the events Linux gets from conntrack (`flows::changes`).

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use anyhow::Result;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP_STATE_CLOSED, MIB_TCP_STATE_DELETE_TCB, MIB_TCP_STATE_ESTAB,
    MIB_TCP_STATE_LISTEN, MIB_TCP_STATE_SYN_RCVD, MIB_TCP_STATE_SYN_SENT, MIB_TCPROW_LH,
    MIB_TCPROW_LH_0, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, SetTcpEntry,
    TCP_TABLE_OWNER_PID_CONNECTIONS,
};
use windows_sys::Win32::Networking::WinSock::AF_INET;

pub use crate::flows::{Event, Flow, Kind, TcpState};

const EVERY: Duration = Duration::from_millis(250);

pub struct Events {
    seen: HashMap<Flow, TcpState>,
    /// When to look next. Kept across calls: the caller's select! drops an
    /// unfinished `next` whenever its own timer fires (every 200 ms), and a
    /// fresh 250 ms sleep each time would never end.
    due: tokio::time::Instant,
}

impl Events {
    /// What's open now is the starting point: only what comes after counts.
    pub fn open() -> Result<Self> {
        Ok(Self {
            seen: table().map_err(anyhow::Error::from)?,
            due: tokio::time::Instant::now() + EVERY,
        })
    }

    /// Cancel-safe: everything after the wait runs without awaiting.
    pub async fn next(&mut self) -> io::Result<Vec<Event>> {
        loop {
            tokio::time::sleep_until(self.due).await;
            self.due = tokio::time::Instant::now() + EVERY;
            let now = table()?;
            let events = crate::flows::changes(&self.seen, &now);
            self.seen = now;
            if !events.is_empty() {
                return Ok(events);
            }
        }
    }
}

fn state(raw: u32) -> TcpState {
    match raw as i32 {
        s if s == MIB_TCP_STATE_SYN_SENT => TcpState::SynSent,
        s if s == MIB_TCP_STATE_CLOSED => TcpState::Close,
        s if s == MIB_TCP_STATE_LISTEN || s == MIB_TCP_STATE_SYN_RCVD => TcpState::Other,
        // Established and every state after it: the other side answered.
        s if s >= MIB_TCP_STATE_ESTAB => TcpState::Answered,
        _ => TcpState::Other,
    }
}

/// A port as the table holds it (network byte order, in the low 16 bits).
fn port(raw: u32) -> u16 {
    u16::from_be(raw as u16)
}

/// The machine's IPv4 TCP connections (not listeners) and their states.
fn table() -> io::Result<HashMap<Flow, TcpState>> {
    let mut size = 0;
    // SAFETY: a size query (null table), then a table of that size.
    unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &raw mut size,
            0,
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        );
    }
    // Room for connections opened meanwhile.
    const ROOM_ROWS: u32 = 64;
    size += ROOM_ROWS * size_of::<MIB_TCPROW_OWNER_PID>() as u32;
    let mut buf = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    // SAFETY: `buf` holds `size` bytes, 8-byte aligned.
    let err = unsafe {
        GetExtendedTcpTable(
            buf.as_mut_ptr().cast(),
            &raw mut size,
            0,
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        )
    };
    if err != 0 {
        return Err(io::Error::from_raw_os_error(err as i32));
    }
    // SAFETY: Windows filled `buf` with a MIB_TCPTABLE_OWNER_PID, whose
    // `dwNumEntries` rows follow its header.
    let rows: &[MIB_TCPROW_OWNER_PID] = unsafe {
        let table = &*buf.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
        std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
    };
    Ok(rows
        .iter()
        .map(|r| {
            let flow = Flow {
                src: Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes()).into(),
                dst: Ipv4Addr::from(r.dwRemoteAddr.to_ne_bytes()).into(),
                sport: port(r.dwLocalPort),
                dport: port(r.dwRemotePort),
            };
            (flow, state(r.dwState))
        })
        .collect())
}

/// Close this machine's TCP connections to `target`, so the apps open new
/// ones, found and routed as on a first visit (after Forget, an open
/// keep-alive connection would sit there, its packets now going direct).
pub async fn close_to(target: crate::hosts::Target) {
    let Ok(now) = table() else { return };
    for flow in now.into_keys().filter(|f| target.contains(f.dst)) {
        let _ = delete(flow);
    }
}

/// Reset a connection stuck without an answer, so the app's next attempt
/// takes the route Crake just added (Linux deletes its conntrack entry).
pub fn delete(flow: Flow) -> io::Result<()> {
    // The table read here is IPv4's only.
    let (IpAddr::V4(src), IpAddr::V4(dst)) = (flow.src, flow.dst) else {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    };
    let net = |port: u16| u32::from(port.to_be());
    let row = MIB_TCPROW_LH {
        Anonymous: MIB_TCPROW_LH_0 {
            dwState: MIB_TCP_STATE_DELETE_TCB as u32,
        },
        dwLocalAddr: u32::from_ne_bytes(src.octets()),
        dwLocalPort: net(flow.sport),
        dwRemoteAddr: u32::from_ne_bytes(dst.octets()),
        dwRemotePort: net(flow.dport),
    };
    // SAFETY: a filled row, only read.
    match unsafe { SetTcpEntry(&raw const row) } {
        0 => Ok(()),
        e => Err(io::Error::from_raw_os_error(e as i32)),
    }
}
