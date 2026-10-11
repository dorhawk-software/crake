//! What the app needs from the system it runs on, one module per system:
//! the tray, the keyring, one Crake per session, the monitors, private
//! files, and the development connection to craked. Everything else is
//! portable and reaches these through `crate::sys` (or the module that
//! re-exports them: `tray`, `keyring`, `place`).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
