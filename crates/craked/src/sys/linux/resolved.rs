//! Registering a tunnel's DNS servers with systemd-resolved, per link, with the
//! tunnel's domains as routing-only domains: names under those domains are
//! asked of the tunnel's DNS (and of no one else), all other names are not.

use std::ffi::CString;
use std::future::Future;
use std::net::Ipv4Addr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use tokio::sync::OnceCell;
use zbus::Connection;

/// How long resolved gets to answer a call. It answers at once when it
/// works; one that doesn't answer counts as failing, so a tunnel still comes
/// up (openconnect's hook waits 15 s for craked) and says why.
const WAIT: Duration = Duration::from_secs(3);

/// `call`, given `wait` to finish.
async fn within<T>(
    what: &str,
    wait: Duration,
    call: impl Future<Output = zbus::Result<T>>,
) -> Result<T> {
    tokio::time::timeout(wait, call)
        .await
        .map_err(|_| anyhow!("{what}: systemd-resolved didn't answer"))?
        .context(what.to_owned())
}

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
            .get_or_try_init(|| within("connecting to the system bus", WAIT, Connection::system()))
            .await?;
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
        servers: &[std::net::IpAddr],
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
            .map(|a| match a {
                std::net::IpAddr::V4(a) => (libc::AF_INET, a.octets().to_vec()),
                std::net::IpAddr::V6(a) => (libc::AF_INET6, a.octets().to_vec()),
            })
            .collect();
        let mut domains: Vec<(&str, bool)> = domains.iter().map(|d| (d.as_str(), true)).collect();
        if everything {
            domains.push((".", true)); // "~.": every name
        }
        within(
            "SetLinkDNS",
            WAIT,
            m.call::<_, _, ()>("SetLinkDNS", &(index, servers)),
        )
        .await?;
        within(
            "SetLinkDomains",
            WAIT,
            m.call::<_, _, ()>("SetLinkDomains", &(index, domains)),
        )
        .await?;
        // Never the place for names outside the tunnel's domains, unless
        // everything goes through it.
        within(
            "SetLinkDefaultRoute",
            WAIT,
            m.call::<_, _, ()>("SetLinkDefaultRoute", &(index, everything)),
        )
        .await?;
        // The tunnel encrypts it already, and a company's DNS rarely speaks
        // TLS: with DNS over TLS on (strict), resolved would ask it over TLS
        // and every internal name would fail. Off for this link only; an
        // older resolved without the call keeps its default.
        let _ = within(
            "SetLinkDNSOverTLS",
            WAIT,
            m.call::<_, _, ()>("SetLinkDNSOverTLS", &(index, "no")),
        )
        .await;
        Ok(())
    }

    /// Whether the system sends its DNS encrypted (resolved's DNSOverTLS:
    /// "yes", or "opportunistic" where the server speaks it). Public
    /// answers are then out of Crake's sight.
    pub async fn encrypts_dns(&self) -> bool {
        let Ok(Some(m)) = self.manager().await else {
            return false;
        };
        let mode = within("DNSOverTLS", WAIT, m.get_property::<String>("DNSOverTLS")).await;
        mode.is_ok_and(|m| m == "yes" || m == "opportunistic")
    }

    /// What another program (openvpn3) registered for `device`: its DNS
    /// servers (IPv4) and its domains, without "." (send everything).
    pub async fn link(&self, device: &str) -> Result<(Vec<Ipv4Addr>, Vec<String>)> {
        let Some(m) = self.manager().await? else {
            return Ok((Vec::new(), Vec::new()));
        };
        let index = ifindex(device)?;
        let path: zbus::zvariant::OwnedObjectPath =
            within("GetLink", WAIT, m.call("GetLink", &(index,))).await?;
        let link = zbus::Proxy::new(
            m.connection(),
            "org.freedesktop.resolve1",
            path,
            "org.freedesktop.resolve1.Link",
        )
        .await?;
        let servers: Vec<(i32, Vec<u8>)> = within("DNS", WAIT, link.get_property("DNS")).await?;
        let domains: Vec<(String, bool)> =
            within("Domains", WAIT, link.get_property("Domains")).await?;
        Ok((
            servers
                .into_iter()
                .filter(|(family, _)| *family == libc::AF_INET)
                .filter_map(|(_, a)| {
                    <[u8; size_of::<Ipv4Addr>()]>::try_from(a)
                        .ok()
                        .map(Ipv4Addr::from)
                })
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
            let _ = within("FlushCaches", WAIT, m.call::<_, _, ()>("FlushCaches", &())).await;
        }
    }

    /// Undo `set_link`. Fine if the link is already gone (resolved forgets it then).
    pub async fn revert(&self, device: &str) {
        let Ok(Some(m)) = self.manager().await else {
            return;
        };
        if let Ok(index) = ifindex(device) {
            let _ = within(
                "RevertLink",
                WAIT,
                m.call::<_, _, ()>("RevertLink", &(index,)),
            )
            .await;
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

/// Who answers the system's DNS, for helpdesk copies (`Manager::diagnostics`).
pub fn view() -> Vec<String> {
    vec![
        "Public names: the system asks its own DNS (systemd-resolved); Crake watches the answers in the firewall."
            .to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_resolved_that_never_answers_is_a_failure() {
        let silent = std::future::pending::<zbus::Result<()>>();
        let e = within("SetLinkDNS", Duration::from_millis(20), silent)
            .await
            .unwrap_err();
        assert_eq!(e.to_string(), "SetLinkDNS: systemd-resolved didn't answer");
        let answered = within("SetLinkDNS", Duration::from_millis(20), async { Ok(7) });
        assert_eq!(answered.await.unwrap(), 7);
    }
}
