//! macOS: the menu bar (`sys::menu_tray`, shared with Windows), the
//! Keychain, the private folder, craked's socket, and craked and "Start at
//! login" registered by the app (SMAppService).

pub mod autostart;
pub mod keyring;
pub mod service;
pub mod single;

mod host;
pub use host::*;
mod trust;
pub use trust::trusted_cafile;
