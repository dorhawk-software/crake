//! macOS: a tunnel's DNS and domains as /etc/resolver files, not built yet.

use std::net::Ipv4Addr;

use anyhow::Result;

pub enum Resolved {
    Real,
    Noop,
}

impl Resolved {
    pub fn new(enabled: bool) -> Self {
        if enabled { Self::Real } else { Self::Noop }
    }

    pub async fn set_link(
        &self,
        _device: &str,
        _servers: &[Ipv4Addr],
        _domains: &[String],
        _everything: bool,
    ) -> Result<()> {
        match self {
            Self::Real => Err(super::not_yet("a tunnel's DNS")),
            Self::Noop => Ok(()),
        }
    }

    pub async fn link(&self, _device: &str) -> Result<(Vec<Ipv4Addr>, Vec<String>)> {
        Ok((Vec::new(), Vec::new()))
    }

    pub async fn flush(&self) {}

    pub async fn revert(&self, _device: &str) {}
}
