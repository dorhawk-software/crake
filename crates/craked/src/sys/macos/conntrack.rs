//! macOS: new TCP connections from the TCP connection table, not built
//! yet.

use std::io;

use anyhow::Result;

pub use crate::flows::{Event, Flow, Kind, TcpState};

pub struct Events;

impl Events {
    pub fn open() -> Result<Self> {
        Err(super::not_yet("watching new connections"))
    }

    pub async fn next(&mut self) -> io::Result<Vec<Event>> {
        std::future::pending().await
    }
}

/// Nothing to forget: macOS keeps no connection-tracking entry to reset.
pub fn delete(_flow: Flow) -> io::Result<()> {
    Ok(())
}
