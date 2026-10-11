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
