//! macOS: tunnel DNS answers come through craked's own DNS forwarder
//! (/etc/resolver files point the tunnels' domains at it), not built yet.

use std::sync::Arc;

use anyhow::Result;

use crate::manager::Manager;

pub fn spawn(_mgr: Arc<Manager>) -> Result<()> {
    Err(super::not_yet(
        "routing DNS answers before the first packet",
    ))
}
