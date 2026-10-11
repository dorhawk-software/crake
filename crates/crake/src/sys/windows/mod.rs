//! Windows: the notification area (`sys::menu_tray`, shared with macOS),
//! Credential Manager, the monitors as Windows lays them out, craked as a
//! service.

pub mod keyring;
pub mod single;

mod host;
pub use host::*;
