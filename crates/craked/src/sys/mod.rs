//! What craked needs from the operating system, one module per system:
//! routing, a tunnel's network setup, the system's DNS, the connection
//! table, DNS answers held until their addresses are routed, and a few
//! helpers (the machine's interfaces, sockets that take a tunnel's path,
//! openconnect's process, private files). Everything else in craked is
//! portable and reaches these by the same names on every system (lib.rs
//! re-exports the modules: `crate::routing`, `crate::net`, ...).

mod output;
pub use output::command_lines;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

// Windows and macOS: craked's own DNS server, and whose a name is.
#[cfg(any(windows, target_os = "macos"))]
mod dns_links;
#[cfg(any(windows, target_os = "macos"))]
mod dns_server;

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
