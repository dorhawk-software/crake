//! Windows: the machine's interfaces (GetAdaptersAddresses), sockets bound
//! to a tunnel's interface (IP_UNICAST_IF), openconnect's process, the
//! console session. Not built yet: what has no Windows form yet says so.

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

/// A socket that leaves through this interface.
pub fn bind_device(_sock: &tokio::net::UdpSocket, _device: &str) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "binding to a tunnel isn't built for Windows yet",
    ))
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
        let event = if keep_session {
            CTRL_BREAK_EVENT
        } else {
            CTRL_C_EVENT
        };
        let sent = tokio::task::spawn_blocking(move || console_event(pid, event))
            .await
            .unwrap_or(false);
        if sent
            && tokio::time::timeout(Duration::from_secs(10), child.wait())
                .await
                .is_ok()
        {
            return;
        }
    }
    let _ = child.kill().await;
}

/// Send `event` to everything on `pid`'s console, craked swallowing its
/// own copy while attached. One at a time: a process has one console.
fn console_event(pid: u32, event: u32) -> bool {
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: plain console calls; `swallow` lives for the whole program.
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return false;
        }
        SetConsoleCtrlHandler(Some(swallow), 1);
        let sent = GenerateConsoleCtrlEvent(event, 0) != 0;
        FreeConsole();
        // The event arrives on another thread: give it time before craked
        // stops swallowing.
        std::thread::sleep(Duration::from_millis(500));
        SetConsoleCtrlHandler(Some(swallow), 0);
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
