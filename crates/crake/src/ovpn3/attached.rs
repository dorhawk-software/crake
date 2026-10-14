//! The handover: once a session is connected, what craked needs to route
//! through it.

use std::collections::HashMap;

use zbus::Connection;

use super::{NETCFG, SESSIONS, property};

/// Connected: what craked needs to route through it. (DNS and domains
/// too, where openvpn3 lets the user read them; craked otherwise takes
/// them from resolved.)
pub(super) async fn attached(
    conn: &Connection,
    path: &str,
) -> Result<HashMap<String, String>, String> {
    // openvpn3 doesn't always name the session's device ("" for a moment
    // after it's connected, or for good with an Azure gateway's profile):
    // craked then finds it from the process running the session.
    let device: String = property(conn, SESSIONS, path, SESSIONS, "device_name")
        .await
        .unwrap_or_default();
    let backend: Option<u32> = property(conn, SESSIONS, path, SESSIONS, "backend_pid")
        .await
        .ok();
    if device.is_empty() && backend.is_none() {
        return Err(
            "OpenVPN 3 connected, but didn\u{2019}t say which device the session uses.".into(),
        );
    }
    let mut fields = HashMap::new();
    fields.insert("device".to_owned(), device);
    if let Some(pid) = backend {
        fields.insert("backend_pid".to_owned(), pid.to_string());
    }
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
