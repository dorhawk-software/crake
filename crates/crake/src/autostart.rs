//! "Start when you log in" (Settings, General), each system its own way
//! (`sys::autostart`: Linux's autostart entry, macOS' LaunchAgent,
//! Windows' Run key), for this very binary. Written only when the user
//! switches it on.

/// The flag Crake is started with at login: it then switches on the
/// tunnels that were on when the user last used it.
pub const FLAG: &str = "--autostart";

pub fn enabled() -> bool {
    crate::sys::autostart::enabled()
}

pub fn set(on: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    crate::sys::autostart::set(on, &exe, FLAG);
}
