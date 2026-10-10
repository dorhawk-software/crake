//! Address and link setup for a tunnel interface. Installs no routes except
//! a /32 to each of the tunnel's DNS servers.

use std::net::Ipv4Addr;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum Net {
    /// Shell out to iproute2.
    Ip,
    /// Do nothing (development and tests).
    Noop,
}

impl Net {
    pub fn new(apply: bool) -> Self {
        if apply { Self::Ip } else { Self::Noop }
    }

    /// Give `dev` its address, bring it up. Idempotent.
    pub async fn assign(
        &self,
        dev: &str,
        addr: Ipv4Addr,
        prefix: u8,
        mtu: Option<u32>,
    ) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        ip(&["addr", "replace", &format!("{addr}/{prefix}"), "dev", dev]).await?;
        let mut up = vec!["link".to_string(), "set".into(), "dev".into(), dev.into()];
        if let Some(mtu) = mtu {
            up.extend(["mtu".into(), mtu.to_string()]);
        }
        up.push("up".into());
        let up: Vec<&str> = up.iter().map(String::as_str).collect();
        ip(&up).await
    }

    /// A dummy link with one address (see `hush`). Idempotent.
    pub async fn dummy(&self, dev: &str, addr: Ipv4Addr) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        let _ = ip(&["link", "add", dev, "type", "dummy"]).await; // fine if it exists
        ip(&["addr", "replace", &format!("{addr}/32"), "dev", dev]).await?;
        ip(&["link", "set", "dev", dev, "up"]).await
    }

    /// Remove a link; fine if it's gone.
    pub async fn remove(&self, dev: &str) {
        if let Self::Ip = self {
            let _ = ip(&["link", "del", dev]).await;
        }
    }

    /// The only route Crake installs up front.
    pub async fn route_dns(&self, dev: &str, dns: Ipv4Addr) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        ip(&["route", "replace", &format!("{dns}/32"), "dev", dev]).await
    }
}

async fn ip(args: &[&str]) -> Result<()> {
    let out = Command::new("ip")
        .args(args)
        .output()
        .await
        .context("running ip")?;
    if !out.status.success() {
        bail!(
            "ip {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// `255.255.255.0` -> 24. Rejects non-contiguous masks.
pub fn prefix_from_mask(mask: Ipv4Addr) -> Option<u8> {
    let bits = u32::from(mask);
    let ones = bits.leading_ones();
    (bits.checked_shl(ones).unwrap_or(0) == 0).then_some(ones as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks() {
        assert_eq!(prefix_from_mask("255.255.255.0".parse().unwrap()), Some(24));
        assert_eq!(
            prefix_from_mask("255.255.255.255".parse().unwrap()),
            Some(32)
        );
        assert_eq!(prefix_from_mask("0.0.0.0".parse().unwrap()), Some(0));
        assert_eq!(prefix_from_mask("255.0.255.0".parse().unwrap()), None);
    }
}
