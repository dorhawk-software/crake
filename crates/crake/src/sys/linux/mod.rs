//! Linux: a StatusNotifierItem tray, the GNOME keyring, X11's monitors.

pub mod autostart;
pub mod gnome_tray;
pub mod keyring;
pub mod single;
pub mod tray;

mod host;
pub use host::*;
