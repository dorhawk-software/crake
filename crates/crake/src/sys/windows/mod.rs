//! Windows: the notification area, Credential Manager, the monitors as
//! Windows lays them out. Not yet: the connection to craked (phase 3 of
//! the Windows plan in CLAUDE.md, with craked as a service).

pub mod keyring;
pub mod single;
pub mod tray;

mod host;
pub use host::*;
