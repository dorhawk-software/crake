//! Windows: the IP Helper API (a tunnel's interface, address, MTU and
//! routes), and the rest of the Win32 calls craked and the app make.

pub mod credentials;
pub mod host;
pub mod iphelper;
pub mod registry;
pub mod services;
pub mod tcp;
#[cfg(feature = "app")]
pub mod ui;
