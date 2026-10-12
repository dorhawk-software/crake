//! The first-SYN race is Linux-only (NFQUEUE). Elsewhere craked relies on
//! connection probing without holding the SYN; this is a no-op so the
//! manager can start it the same way on every system.

use std::sync::Arc;

use anyhow::Result;

use crate::manager::Manager;

pub fn spawn(_mgr: Arc<Manager>) -> Result<()> {
    Ok(())
}
