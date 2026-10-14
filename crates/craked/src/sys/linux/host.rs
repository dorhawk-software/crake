//! Linux: sockets that take a tunnel's path, and the interfaces' byte
//! counters (the rest is `sys::unix`'s, shared with macOS).

use std::os::fd::AsRawFd;

/// The tunnel's mark on a socket: its packets take the tunnel's route.
pub fn mark(sock: &impl AsRawFd, mark: u32) -> std::io::Result<()> {
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

/// (bytes on physical interfaces, bytes on Crake's tunnels, bytes per
/// tunnel device), both directions.
pub fn counters(
    devices: &std::collections::HashMap<String, String>,
) -> (u64, u64, std::collections::HashMap<String, u64>) {
    let (mut physical, mut tunnels, mut each) = (0, 0, std::collections::HashMap::new());
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else {
        return (0, 0, each);
    };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let read = |f: &str| {
            std::fs::read_to_string(e.path().join("statistics").join(f))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0)
        };
        let bytes = read("rx_bytes") + read("tx_bytes");
        if name.starts_with("crake-") || devices.contains_key(&name) {
            tunnels += bytes;
            each.insert(name, bytes);
        } else if e.path().join("device").exists() {
            // A real device: not loopback, bridges, Docker or other tunnels.
            physical += bytes;
        }
    }
    (physical, tunnels, each)
}

/// The tun device a process holds open (Linux shows it in the fd's info as
/// "iff:\t<name>"): how an OpenVPN 3 session's device is found when
/// openvpn3 doesn't name it. Only when it holds exactly one.
pub fn tun_of_process(pid: u32) -> Option<String> {
    let dir = std::fs::read_dir(format!("/proc/{pid}/fdinfo")).ok()?;
    let mut found: Vec<String> = dir
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|info| iff(&info).map(str::to_owned))
        .collect();
    found.dedup();
    (found.len() == 1).then(|| found.remove(0))
}

/// The interface named in a tun fd's info.
fn iff(fdinfo: &str) -> Option<&str> {
    fdinfo
        .lines()
        .find_map(|l| l.strip_prefix("iff:"))
        .map(str::trim)
        .filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_tun_fds_info_names_its_interface() {
        // As the kernel wrote it for an openvpn3 session's backend.
        let info = "pos:\t0\nflags:\t02\nmnt_id:\t26\nino:\t1061\niff:\ttun1\n";
        assert_eq!(super::iff(info), Some("tun1"));
        assert_eq!(super::iff("pos:\t0\nflags:\t02\n"), None);
    }
}
