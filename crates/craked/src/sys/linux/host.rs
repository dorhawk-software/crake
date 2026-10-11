//! Linux: the machine's interfaces, sockets that take a tunnel's path,
//! openconnect's process and private files.

use std::net::Ipv4Addr;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::Duration;

use tokio::process::{Child, Command};

/// Every IPv4 address on the machine: interface, address, netmask.
pub fn interfaces() -> Vec<(String, Ipv4Addr, u32)> {
    let mut found = Vec::new();
    // SAFETY: getifaddrs/freeifaddrs pair; we only read the list.
    unsafe {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut head) == 0 {
            let mut p = head;
            while !p.is_null() {
                let ifa = &*p;
                if !ifa.ifa_addr.is_null()
                    && i32::from((*ifa.ifa_addr).sa_family) == libc::AF_INET
                    && !ifa.ifa_netmask.is_null()
                {
                    let name = std::ffi::CStr::from_ptr(ifa.ifa_name)
                        .to_string_lossy()
                        .into_owned();
                    let a = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    let m = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
                    found.push((
                        name,
                        Ipv4Addr::from(u32::from_be(a.sin_addr.s_addr)),
                        u32::from_be(m.sin_addr.s_addr),
                    ));
                }
                p = ifa.ifa_next;
            }
            libc::freeifaddrs(head);
        }
    }
    found
}

/// The tunnel's mark on a socket: its packets take the tunnel's route.
pub fn mark(sock: &tokio::net::TcpSocket, mark: u32) -> std::io::Result<()> {
    // SAFETY: plain setsockopt with a u32 on a socket we own.
    let r = unsafe {
        libc::setsockopt(
            sock.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_MARK,
            (&mark as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// A socket bound to a device: it leaves through that interface.
pub fn bind_device(sock: &tokio::net::UdpSocket, device: &str) -> std::io::Result<()> {
    sock.bind_device(Some(device.as_bytes()))
}

/// openconnect in its own process group: Ctrl+C on a terminal reaches
/// craked only, which then stops openconnect in order.
pub fn own_group(cmd: &mut Command) {
    cmd.process_group(0);
}

/// Stop openconnect: SIGHUP keeps its session (to be resumed with the same
/// cookie), SIGTERM logs off; either way it calls the hook with
/// `disconnect`. Killed after a grace period.
pub async fn stop(child: &mut Child, keep_session: bool) {
    if let Some(pid) = child.id() {
        let signal = if keep_session {
            libc::SIGHUP
        } else {
            libc::SIGTERM
        };
        // SAFETY: plain signal to a child we own.
        unsafe { libc::kill(pid as i32, signal) };
    }
    if tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}

/// A new file for root alone (0600), replacing any older one.
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}
