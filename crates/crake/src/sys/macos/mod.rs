//! macOS: the menu bar (`sys::menu_tray`, shared with Windows), the
//! Keychain, the private folder. Not yet: the connection to craked (the
//! macOS plan in CLAUDE.md, with craked as a launchd daemon).

pub mod keyring;
pub mod single;

mod host;
pub use host::*;
