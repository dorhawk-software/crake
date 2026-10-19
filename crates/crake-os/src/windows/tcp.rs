//! The machine's TCP connections (GetExtendedTcpTable), a connection reset
//! (SetTcpEntry) and its extended statistics (its bytes), as plain rows
//! and safe calls.

use std::io;

use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetPerTcp6ConnectionEStats, GetPerTcpConnectionEStats, MIB_TCP6ROW,
    MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_LH, MIB_TCPROW_OWNER_PID,
    MIB_TCPTABLE_OWNER_PID, SetPerTcp6ConnectionEStats, SetPerTcpConnectionEStats, SetTcpEntry,
    TCP_ESTATS_DATA_ROD_v0, TCP_ESTATS_DATA_RW_v0, TCP_TABLE_CLASS,
    TCP_TABLE_OWNER_PID_CONNECTIONS, TCP_TABLE_OWNER_PID_LISTENER, TcpConnectionEstatsData,
};
use windows_sys::Win32::Networking::WinSock::{ADDRESS_FAMILY, AF_INET, AF_INET6};

/// Room for connections opened between the size query and the read.
const ROOM_ROWS: u32 = 64;

/// IPv4's connections, with the process owning each.
pub fn table_v4() -> io::Result<Vec<MIB_TCPROW_OWNER_PID>> {
    rows_v4(TCP_TABLE_OWNER_PID_CONNECTIONS)
}

/// IPv4's listening sockets, with the process owning each.
pub fn listeners_v4() -> io::Result<Vec<MIB_TCPROW_OWNER_PID>> {
    rows_v4(TCP_TABLE_OWNER_PID_LISTENER)
}

fn rows_v4(class: TCP_TABLE_CLASS) -> io::Result<Vec<MIB_TCPROW_OWNER_PID>> {
    let buf = raw_table(AF_INET, class, size_of::<MIB_TCPROW_OWNER_PID>())?;
    // SAFETY: Windows wrote a MIB_TCPTABLE_OWNER_PID into `buf` (8-byte
    // aligned, as big as it said), whose `dwNumEntries` rows follow its
    // header; they're copied out while `buf` lives.
    Ok(unsafe {
        let table = &*buf.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
        std::slice::from_raw_parts(
            table.table.as_ptr(),
            usize::try_from(table.dwNumEntries).unwrap_or(0),
        )
        .to_vec()
    })
}

/// IPv6's connections (an error where the machine has no IPv6).
pub fn table_v6() -> io::Result<Vec<MIB_TCP6ROW_OWNER_PID>> {
    let buf = raw_table(
        AF_INET6,
        TCP_TABLE_OWNER_PID_CONNECTIONS,
        size_of::<MIB_TCP6ROW_OWNER_PID>(),
    )?;
    // SAFETY: as in `table_v4`, a MIB_TCP6TABLE_OWNER_PID.
    Ok(unsafe {
        let table = &*buf.as_ptr().cast::<MIB_TCP6TABLE_OWNER_PID>();
        std::slice::from_raw_parts(
            table.table.as_ptr(),
            usize::try_from(table.dwNumEntries).unwrap_or(0),
        )
        .to_vec()
    })
}

/// One family's table as GetExtendedTcpTable writes it (rows `row` bytes
/// each), in an 8-byte aligned buffer.
fn raw_table(family: ADDRESS_FAMILY, class: TCP_TABLE_CLASS, row: usize) -> io::Result<Vec<u64>> {
    let mut size = 0;
    // SAFETY: a size query: a null table, `size` ours to write.
    unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &raw mut size,
            0,
            u32::from(family),
            class,
            0,
        );
    }
    size += ROOM_ROWS * u32::try_from(row).unwrap_or(u32::MAX);
    let mut buf = vec![
        0u64;
        usize::try_from(size)
            .unwrap_or(0)
            .div_ceil(size_of::<u64>())
    ];
    // SAFETY: `buf` holds `size` bytes, 8-byte aligned, as the call is told.
    let err = unsafe {
        GetExtendedTcpTable(
            buf.as_mut_ptr().cast(),
            &raw mut size,
            0,
            u32::from(family),
            class,
            0,
        )
    };
    if err != 0 {
        return Err(io::Error::from_raw_os_error(
            i32::try_from(err).unwrap_or(i32::MAX),
        ));
    }
    Ok(buf)
}

/// Set an IPv4 connection's state (MIB_TCP_STATE_DELETE_TCB resets it).
pub fn set_entry(row: &MIB_TCPROW_LH) -> io::Result<()> {
    // SAFETY: a filled row, only read.
    match unsafe { SetTcpEntry(row) } {
        0 => Ok(()),
        e => Err(io::Error::from_raw_os_error(
            i32::try_from(e).unwrap_or(i32::MAX),
        )),
    }
}

/// A connection as the statistics calls name it.
pub enum Row {
    V4(MIB_TCPROW_LH),
    V6(MIB_TCP6ROW),
}

/// Switch a connection's data statistics on or off; true when Windows
/// took it (it needs administrator's rights).
pub fn statistics(row: &Row, on: bool) -> bool {
    let rw = TCP_ESTATS_DATA_RW_v0 {
        EnableCollection: on,
    };
    let (rw_ptr, rw_len) = (
        (&raw const rw).cast::<u8>(),
        u32::try_from(size_of::<TCP_ESTATS_DATA_RW_v0>()).unwrap_or(0),
    );
    // SAFETY: a filled row and a filled settings struct of the size given,
    // both only read.
    let err = match row {
        Row::V4(r) => unsafe {
            SetPerTcpConnectionEStats(r, TcpConnectionEstatsData, rw_ptr, 0, rw_len, 0)
        },
        Row::V6(r) => unsafe {
            SetPerTcp6ConnectionEStats(r, TcpConnectionEstatsData, rw_ptr, 0, rw_len, 0)
        },
    };
    err == 0
}

/// A connection's bytes both ways since its statistics came on.
pub fn data_bytes(row: &Row) -> Option<u64> {
    let mut rod = TCP_ESTATS_DATA_ROD_v0::default();
    let (rod_ptr, rod_len) = (
        (&raw mut rod).cast::<u8>(),
        u32::try_from(size_of::<TCP_ESTATS_DATA_ROD_v0>()).unwrap_or(0),
    );
    let null = std::ptr::null_mut();
    // SAFETY: a filled row (read) and our own struct of the size given
    // (written); the other two outputs aren't asked for (null, 0).
    let err = match row {
        Row::V4(r) => unsafe {
            GetPerTcpConnectionEStats(
                r,
                TcpConnectionEstatsData,
                null,
                0,
                0,
                null,
                0,
                0,
                rod_ptr,
                0,
                rod_len,
            )
        },
        Row::V6(r) => unsafe {
            GetPerTcp6ConnectionEStats(
                r,
                TcpConnectionEstatsData,
                null,
                0,
                0,
                null,
                0,
                0,
                rod_ptr,
                0,
                rod_len,
            )
        },
    };
    (err == 0).then(|| rod.DataBytesIn + rod.DataBytesOut)
}
