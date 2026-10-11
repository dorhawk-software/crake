//! macOS: per-host routes on a tunnel's interface (the routing socket), not
//! built yet.

use std::net::Ipv4Addr;

use anyhow::Result;

use crate::hosts::Target;
pub use crate::plan::{ID_BASE, Ids, MARK_BASE, MAX_TUNNELS, Plan, ids};

pub enum Routing {
    Real,
    Noop,
}

impl Routing {
    pub fn new(apply: bool) -> Self {
        if apply { Self::Real } else { Self::Noop }
    }

    /// Changes the system (not development's no-op).
    pub fn is_real(&self) -> bool {
        matches!(self, Self::Real)
    }

    fn not_yet(&self, what: &str) -> Result<()> {
        match self {
            Self::Real => Err(super::not_yet(what)),
            Self::Noop => Ok(()),
        }
    }

    pub async fn reset_all(&self) -> Result<()> {
        self.not_yet("clearing Crake's routes")
    }

    pub async fn up(&self, _plan: &Plan<'_>) -> Result<()> {
        self.not_yet("a tunnel's routing")
    }

    pub async fn public_up(&self, _probing: bool) -> Result<()> {
        self.not_yet("watching public DNS")
    }

    pub async fn public_down(&self) -> Result<()> {
        self.not_yet("watching public DNS")
    }

    pub async fn down(&self, _name: &str, _index: usize, _device: Option<&str>) -> Result<()> {
        self.not_yet("removing a tunnel's routing")
    }

    pub async fn add_learned(&self, _name: &str, _addr: Ipv4Addr, _timeout: u64) -> Result<()> {
        self.not_yet("a host's route")
    }

    pub async fn remove_learned(&self, _name: &str, _addr: Ipv4Addr) -> Result<()> {
        self.not_yet("a host's route")
    }

    pub async fn add_pinned(&self, _name: &str, _t: Target) -> Result<()> {
        self.not_yet("a pinned route")
    }

    pub async fn remove_pinned(&self, _name: &str, _t: Target) -> Result<()> {
        self.not_yet("a pinned route")
    }

    pub async fn read_learned(&self, _name: &str) -> Result<Option<Vec<(Ipv4Addr, u64)>>> {
        Ok(None)
    }
}
