//! The handover: once a session is connected, what craked needs to route
//! through it.

use std::collections::HashMap;
use std::time::Duration;

use zbus::Connection;

use super::{NETCFG, SESSIONS, property};

/// openvpn3 may name the session's device a moment after it says it's
/// connected: asked every DEVICE_POLL, up to 5 s.
const DEVICE_POLLS: u32 = 25;
const DEVICE_POLL: Duration = Duration::from_millis(200);

/// Connected: what craked needs to route through it. (DNS and domains
/// too, where openvpn3 lets the user read them; craked otherwise takes
/// them from resolved.)
pub(super) async fn attached(
    conn: &Connection,
    path: &str,
) -> Result<HashMap<String, String>, String> {
    let device = named_device(|| async {
        property::<String>(conn, SESSIONS, path, SESSIONS, "device_name")
            .await
            .ok()
    })
    .await
    .ok_or("OpenVPN 3 connected, but didn\u{2019}t say which device the session uses.")?;
    let mut fields = HashMap::new();
    fields.insert("device".to_owned(), device);
    if let Ok((_, address, _)) =
        property::<(String, String, u32)>(conn, SESSIONS, path, SESSIONS, "connected_to").await
    {
        fields.insert("gateway".to_owned(), address);
    }
    if let Ok(device_path) = property::<String>(conn, SESSIONS, path, SESSIONS, "device_path").await
    {
        if let Ok(dns) =
            property::<Vec<String>>(conn, NETCFG, &device_path, NETCFG, "dns_name_servers").await
        {
            fields.insert("dns".to_owned(), dns.join(","));
        }
        if let Ok(domains) =
            property::<Vec<String>>(conn, NETCFG, &device_path, NETCFG, "dns_search_domains").await
        {
            fields.insert("domains".to_owned(), domains.join(","));
        }
    }
    Ok(fields)
}

/// The session's device, once openvpn3 has named it ("" until then).
async fn named_device<F, Fut>(mut read: F) -> Option<String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    for _ in 0..DEVICE_POLLS {
        if let Some(name) = read().await.filter(|name| !name.is_empty()) {
            return Some(name);
        }
        tokio::time::sleep(DEVICE_POLL).await;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seen on a real session: connected, and device_name still "" for a
    /// moment. craked was handed "" and refused it; the session ended.
    #[tokio::test]
    async fn the_device_is_waited_for_until_openvpn3_names_it() {
        let mut asked = 0;
        let named = named_device(|| {
            asked += 1;
            let name = if asked < 3 { "" } else { "tun0" };
            async move { Some(name.to_owned()) }
        })
        .await;
        assert_eq!(named.as_deref(), Some("tun0"));
    }

    #[tokio::test]
    async fn a_device_never_named_is_said() {
        let named = named_device(|| async { Some(String::new()) }).await;
        assert_eq!(named, None);
    }
}
