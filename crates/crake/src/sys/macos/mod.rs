//! macOS: the menu bar (`sys::menu_tray`, shared with Windows), the
//! Keychain, the private folder, craked's socket.

pub mod keyring;
pub mod single;

mod host;
pub use host::*;
