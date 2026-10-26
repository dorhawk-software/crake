//! What the app needs from the system it runs on, one module per system:
//! the tray, the keyring, one Crake per session, the monitors, private
//! files, and the development connection to craked. Everything else is
//! portable and reaches these through `crate::sys` (or the module that
//! re-exports them: `tray`, `keyring`, `place`).

// Linux and macOS alike: private files, the development socket.
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

// The tray through tray-icon: Windows' notification area, macOS' menu bar.
#[cfg(any(windows, target_os = "macos"))]
#[path = "menu_tray.rs"]
pub mod tray;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

/// A command's output goes to the console it was run from: Windows' own
/// (`windows::attach_console`); elsewhere it always does.
#[cfg(not(windows))]
pub fn attach_console() {}

// Crake's service as the system runs it (`crate::service`): on a Mac the
// app registers it (SMAppService, `macos::service`); elsewhere the package
// runs it, and there's nothing to ask.
#[cfg(not(target_os = "macos"))]
pub mod service {
    /// Whether the system lets craked run: not this system's way.
    pub fn on(_reachable: Option<bool>) -> Option<bool> {
        None
    }

    pub fn open_settings() {}
}
