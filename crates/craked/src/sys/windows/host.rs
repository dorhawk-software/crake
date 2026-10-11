//! Windows: the machine's interfaces (GetAdaptersAddresses), sockets bound
//! to a tunnel's interface (IP_UNICAST_IF), openconnect's process, the
//! console session. Not built yet: what has no Windows form yet says so.

use std::net::Ipv4Addr;
use std::path::Path;
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

/// Every IPv4 address on the machine: interface, address, netmask.
pub fn interfaces() -> Vec<(String, Ipv4Addr, u32)> {
    Vec::new()
}

/// A socket that takes the tunnel's path.
pub fn mark(_sock: &tokio::net::TcpSocket, _mark: u32) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "a tunnel's path for probes isn't built for Windows yet",
    ))
}

/// A socket that leaves through this interface (IP_UNICAST_IF, the
/// interface's index in network byte order).
pub fn bind_device(sock: &tokio::net::UdpSocket, device: &str) -> std::io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{IP_UNICAST_IF, IPPROTO_IP, SOCKET, setsockopt};
    let index = super::iphelper::index(device).map_err(std::io::Error::other)?;
    let value = index.to_be();
    // SAFETY: a live socket of ours; `value` is the u32 the option takes.
    let err = unsafe {
        setsockopt(
            sock.as_raw_socket() as SOCKET,
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
