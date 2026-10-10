//! Registering a tunnel's DNS servers with systemd-resolved, per link, with the
//! tunnel's domains as routing-only domains: names under those domains are
//! asked of the tunnel's DNS (and of no one else), all other names are not.

use std::ffi::CString;
use std::net::Ipv4Addr;

use anyhow::{Context, Result, bail};
use tokio::sync::OnceCell;
use zbus::Connection;

pub enum Resolved {
    Real(OnceCell<Connection>),
    Noop,
}

impl Resolved {
    pub fn new(enabled: bool) -> Self {
        if enabled {
            Self::Real(OnceCell::new())
        } else {
            Self::Noop
        }
    }

    async fn manager(&self) -> Result<Option<zbus::Proxy<'static>>> {
        let Self::Real(cell) = self else {
            return Ok(None);
        };
        let conn = cell
            .get_or_try_init(|| async { Connection::system().await })
            .await
            .context("connecting to the system bus")?;
        let proxy = zbus::Proxy::new(
            conn,
            "org.freedesktop.resolve1",
            "/org/freedesktop/resolve1",
            "org.freedesktop.resolve1.Manager",
        )
        .await?;
        Ok(Some(proxy))
    }

    pub async fn set_link(
        &self,
        device: &str,
        servers: &[Ipv4Addr],
        domains: &[String],
        // A full tunnel: every name goes to its DNS.
        everything: bool,
    ) -> Result<()> {
        let Some(m) = self.manager().await? else {
            return Ok(());
        };
        let index = ifindex(device)?;
        let servers: Vec<(i32, Vec<u8>)> = servers
            .iter()
            .map(|a| (libc::AF_INET, a.octets().to_vec()))
            .collect();
        let mut domains: Vec<(&str, bool)> = domains.iter().map(|d| (d.as_str(), true)).collect();
        if everything {
            domains.push((".", true)); // "~.": every name
        }
        m.call::<_, _, ()>("SetLinkDNS", &(index, servers))
            .await
            .context("SetLinkDNS")?;
        m.call::<_, _, ()>("SetLinkDomains", &(index, domains))
            .await
            .context("SetLinkDomains")?;
        // Never the place for names outside the tunnel's domains, unless
        // everything goes through it.
        m.call::<_, _, ()>("SetLinkDefaultRoute", &(index, everything))
            .await
            .context("SetLinkDefaultRoute")?;
        Ok(())
    }

    /// What another program (openvpn3) registered for `device`: its DNS
    /// servers (IPv4) and its domains, without "." (send everything).
    pub async fn link(&self, device: &str) -> Result<(Vec<Ipv4Addr>, Vec<String>)> {
        let Some(m) = self.manager().await? else {
            return Ok((Vec::new(), Vec::new()));
        };
        let index = ifindex(device)?;
        let path: zbus::zvariant::OwnedObjectPath =
            m.call("GetLink", &(index,)).await.context("GetLink")?;
        let link = zbus::Proxy::new(
            m.connection(),
            "org.freedesktop.resolve1",
            path,
            "org.freedesktop.resolve1.Link",
        )
        .await?;
        let servers: Vec<(i32, Vec<u8>)> = link.get_property("DNS").await.context("DNS")?;
        let domains: Vec<(String, bool)> = link.get_property("Domains").await.context("Domains")?;
        Ok((
            servers
                .into_iter()
                .filter(|(family, _)| *family == libc::AF_INET)
                .filter_map(|(_, a)| <[u8; 4]>::try_from(a).ok().map(Ipv4Addr::from))
                .collect(),
            domains
                .into_iter()
                .map(|(d, _)| d)
                .filter(|d| d != "." && !d.is_empty())
                .collect(),
        ))
    }

    /// Drop resolved's cache, so answers from before a tunnel came up (or
    /// went down) aren't reused.
    pub async fn flush(&self) {
        if let Ok(Some(m)) = self.manager().await {
            let _ = m.call::<_, _, ()>("FlushCaches", &()).await;
        }
    }

    /// Undo `set_link`. Fine if the link is already gone (resolved forgets it then).
    pub async fn revert(&self, device: &str) {
        let Ok(Some(m)) = self.manager().await else {
            return;
        };
        if let Ok(index) = ifindex(device) {
            let _ = m.call::<_, _, ()>("RevertLink", &(index,)).await;
        }
    }
}

fn ifindex(device: &str) -> Result<i32> {
    let c = CString::new(device)?;
    // SAFETY: valid NUL-terminated string.
    let i = unsafe { libc::if_nametoindex(c.as_ptr()) };
    if i == 0 {
        bail!("no interface {device}");
    }
    Ok(i as i32)
}
