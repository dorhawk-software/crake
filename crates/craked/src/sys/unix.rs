//! What Linux and macOS do alike: the machine's interfaces (getifaddrs),
//! openconnect's process (its own group, stopped with signals) and private
//! files.

use std::net::Ipv4Addr;
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
    if tokio::time::timeout(STOP_WAIT, child.wait()).await.is_err() {
        let _ = child.kill().await;
    }
}

/// Read and written by its owner alone.
pub const OWNER_ONLY: u32 = 0o600;
/// A stopped child gets this long before it's killed.
const STOP_WAIT: Duration = Duration::from_secs(10);

/// A new file for root alone (0600), replacing any older one.
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(OWNER_ONLY)
        .open(path)
}
