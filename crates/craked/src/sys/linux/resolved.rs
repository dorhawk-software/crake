//! Registering a tunnel's DNS servers with systemd-resolved, per link, with the
//! tunnel's domains as routing-only domains: names under those domains are
//! asked of the tunnel's DNS (and of no one else), all other names are not.

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
        let path: zbus::OwnedObjectPath =
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

    /// Whether `device`'s settings in resolved are still the ones
    /// `set_link` made (another program, openvpn3, may have rewritten
    /// them). Can't tell: yes.
    pub async fn link_as_set(&self, device: &str, domains: &[String], everything: bool) -> bool {
        let Ok(Some(m)) = self.manager().await else {
            return true;
        };
        let Ok(index) = ifindex(device) else {
            return true;
        };
        let Ok(path) = within(
            "GetLink",
            WAIT,
            m.call::<_, _, zbus::OwnedObjectPath>("GetLink", &(index,)),
        )
        .await
        else {
            return true;
        };
        let Ok(link) = zbus::Proxy::new(
            m.connection(),
            "org.freedesktop.resolve1",
            path,
            "org.freedesktop.resolve1.Link",
        )
        .await
        else {
            return true;
        };
        let found = within(
            "Domains",
            WAIT,
            link.get_property::<Vec<(String, bool)>>("Domains"),
        )
        .await;
        let default_route = within(
            "DefaultRoute",
            WAIT,
            link.get_property::<bool>("DefaultRoute"),
        )
        .await;
        match (found, default_route) {
            (Ok(found), Ok(default_route)) => as_set(&found, default_route, domains, everything),
            _ => true,
        }
    }

    /// The domains other programs' links serve in resolved (search or
    /// routing domains; the global ones too), not Crake's (`ours`, and its
    /// own crake-* links).
    pub async fn served_elsewhere(&self, ours: &[String]) -> Vec<String> {
        let Ok(Some(m)) = self.manager().await else {
            return Vec::new();
        };
        let Ok(all) = within(
            "Domains",
            WAIT,
            m.get_property::<Vec<(i32, String, bool)>>("Domains"),
        )
        .await
        else {
            return Vec::new();
        };
        all.into_iter()
            .filter(|(index, _, _)| {
                let name = ifname(*index);
                !name.starts_with("crake-") && !ours.contains(&name)
            })
            .map(|(_, domain, _)| domain)
            .filter(|d| !d.is_empty() && d != ".")
            .collect()
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

/// An interface's name ("" for 0, the global settings, or one that's gone).
fn ifname(index: i32) -> String {
    u32::try_from(index)
        .ok()
        .and_then(crake_os::unix::name_of)
        .unwrap_or_default()
}

fn ifindex(device: &str) -> Result<i32> {
    let Some(i) = crake_os::unix::index_of(device) else {
        bail!("no interface {device}");
    };
    Ok(i32::try_from(i)?)
}

/// resolved's domains for a link (name, routing only) and its default
/// route are what `set_link` made for `domains`.
fn as_set(
    found: &[(String, bool)],
    default_route: bool,
    domains: &[String],
    everything: bool,
) -> bool {
    let plain = |d: &str| {
        let d = d.trim_end_matches('.').to_ascii_lowercase();
        if d.is_empty() { ".".to_owned() } else { d }
    };
    let mut want: Vec<(String, bool)> = domains.iter().map(|d| (plain(d), true)).collect();
    if everything {
        want.push((".".to_owned(), true));
    }
    let mut found: Vec<(String, bool)> = found.iter().map(|(d, r)| (plain(d), *r)).collect();
    want.sort();
    want.dedup();
    found.sort();
    found.dedup();
    found == want && default_route == everything
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

    /// Seen on the user's machine: openvpn3, when another of its sessions
    /// reconnected, set an attached tunnel's link back to its own (its
    /// domains as search domains, "~." and the default route), undoing
    /// Crake's routing-only setup.
    #[test]
    fn a_link_another_program_rewrote_isnt_as_set() {
        let domains = [
            "gitlab.initech.internal".to_owned(),
            "initech.vpn".to_owned(),
        ];
        let ours = [
            ("gitlab.initech.internal".to_owned(), true),
            ("initech.vpn".to_owned(), true),
        ];
        assert!(as_set(&ours, false, &domains, false));
        // In another order, or with a trailing dot or capitals: the same.
        let same = [
            ("INITECH.VPN.".to_owned(), true),
            ("gitlab.initech.internal".to_owned(), true),
        ];
        assert!(as_set(&same, false, &domains, false));
        let openvpn3s = [
            ("gitlab.initech.internal".to_owned(), false),
            ("initech.vpn".to_owned(), false),
            (".".to_owned(), true),
        ];
        assert!(!as_set(&openvpn3s, true, &domains, false));
        assert!(
            !as_set(&ours, true, &domains, false),
            "the default route alone"
        );
        assert!(!as_set(&[], false, &domains, false), "emptied");
        // A full tunnel: every name ("~.") and the default route are ours.
        let full = [(".".to_owned(), true)];
        assert!(as_set(&full, true, &[], true));
        assert!(!as_set(&full, false, &[], true));
    }

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
