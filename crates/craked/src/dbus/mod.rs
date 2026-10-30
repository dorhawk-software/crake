//! `software.dorhawk.Crake1` on the system bus. Reading and changing alike
//! need polkit's `software.dorhawk.crake.manage-tunnels` (`guard`): the log
//! and the hosts say where someone connects.

use std::collections::HashMap;
use std::sync::Arc;

/// The most log entries one call returns.
const MAX_RECENT: u32 = 10_000;

use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, OwnedVariant, fdo, interface};

use crate::manager::Manager;

// Where the API is served: the system bus, private and app sockets, the
// loopback port.
mod serve;
pub use serve::*;
// Who may use it, reading included.
mod guard;
// A tunnel as a dictionary.
mod dict;
use dict::to_dict;

pub const POLKIT_ACTION: &str = "software.dorhawk.crake.manage-tunnels";

pub struct Service {
    mgr: Arc<Manager>,
    guard: Guard,
    /// Callers the guard let in: they hear the signals.
    allowed: Arc<guard::Allowed>,
}

/// Who may use the API, reading included (`guard`).
#[derive(Clone, Copy)]
enum Guard {
    /// Whoever can open the socket (the session bus, or a development
    /// socket only its owner can open).
    Open,
    /// Linux's system bus: polkit's `manage-tunnels`.
    Polkit,
    /// Windows' loopback port: a process in the console session (the
    /// counterpart of polkit's active session).
    #[cfg(windows)]
    ConsoleSession,
    /// macOS' app socket: the user at the console (or root).
    #[cfg(target_os = "macos")]
    ConsoleUser,
}

#[interface(name = "software.dorhawk.Crake1")]
impl Service {
    /// One dictionary per tunnel: name, display_name, state, interface, gateway,
    /// protocol, address, dns, domains, split_include, split_exclude.
    async fn list_tunnels(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<Vec<HashMap<String, OwnedVariant>>> {
        self.authorize(&hdr, conn).await?;
        Ok(self.mgr.list().iter().map(to_dict).collect())
    }

    async fn get_tunnel(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        name: &str,
    ) -> fdo::Result<HashMap<String, OwnedVariant>> {
        self.authorize(&hdr, conn).await?;
        self.mgr
            .get(name)
            .map(|s| to_dict(&s))
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("no tunnel named {name}")))
    }

    async fn connect(
        &self,
        name: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr
            .connect(name)
            .map_err(|e| fdo::Error::Failed(format!("{e:#}")))
    }

    /// Connect with a sign-in done in the user's session (`crake` runs
    /// `openconnect --authenticate` as the user). Keys: cookie, connect_url,
    /// fingerprint, resolve.
    async fn connect_with_session(
        &self,
        tunnel: &str,
        session: HashMap<String, String>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        let map = session.into_iter().collect();
        let session =
            crake_proto::Session::from_map(&map).map_err(|e| fdo::Error::InvalidArgs(e.into()))?;
        self.mgr.connect_with(tunnel, Some(session)).map_err(failed)
    }

    async fn disconnect(
        &self,
        name: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr
            .disconnect(name)
            .map_err(|e| fdo::Error::Failed(format!("{e:#}")))
    }

    /// Send one host through the tunnel; it expires after a while without traffic.
    async fn add_host(
        &self,
        tunnel: &str,
        host: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        self.mgr.add_host(tunnel, host).await.map_err(failed)
    }

    /// Keep a host or network (`a.b.c.d/len`) on the tunnel until unpinned.
    async fn pin(
        &self,
        tunnel: &str,
        target: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        self.mgr.pin(tunnel, target).await.map_err(failed)
    }

    async fn unpin(
        &self,
        tunnel: &str,
        target: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        self.mgr.unpin(tunnel, target).await.map_err(failed)
    }

    async fn forget(
        &self,
        tunnel: &str,
        target: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        self.mgr.forget(tunnel, target).await.map_err(failed)
    }

    /// A short name ("expert") is the tunnel's `full` name from now on
    /// (the panel's "Use ACME's"); Forget takes it back.
    async fn use_short_name(
        &self,
        tunnel: &str,
        short: &str,
        full: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        self.mgr
            .use_short_name(tunnel, short, full)
            .await
            .map_err(failed)
    }

    /// Forget everything learned for the tunnel, and what was sent direct
    /// with Undo (tunnel detail's "Forget all…"); pinned hosts stay.
    async fn forget_learned(
        &self,
        tunnel: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        self.mgr.forget_learned(tunnel).await.map_err(failed)
    }

    /// A tunnel that stopped on its own, seen: Off, its reason kept for
    /// tunnel detail (the panel's Dismiss).
    async fn dismiss_trouble(
        &self,
        tunnel: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.dismiss_trouble(tunnel).map_err(failed)
    }

    /// What an OpenVPN tunnel's sign-in asks the user now (empty when
    /// nothing): id, kind, label, url, retry. Guarded: a sign-in page is
    /// the user's.
    async fn asking(
        &self,
        tunnel: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<HashMap<String, String>> {
        self.authorize(&hdr, conn).await?;
        Ok(self
            .mgr
            .asking(tunnel)
            .map(|m| m.into_iter().collect())
            .unwrap_or_default())
    }

    /// The user's answer to what an OpenVPN tunnel's sign-in asked.
    async fn answer(
        &self,
        tunnel: &str,
        id: u32,
        text: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.answer(tunnel, id, text.to_owned()).map_err(failed)
    }

    /// The app found an OpenVPN 3 session's process still running after its
    /// session ended (the profile's Crake tunnel, or ""): logged, with how
    /// to stop it.
    async fn left_running(
        &self,
        tunnel: &str,
        profile: &str,
        pid: u32,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.left_running(tunnel, profile, pid);
        Ok(())
    }

    /// Send learned hosts direct (one decision), and keep them direct until redone.
    async fn undo(
        &self,
        tunnel: &str,
        hosts: Vec<String>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
        self.mgr.undo(tunnel, &hosts).await.map_err(failed)
    }

    async fn redo(
        &self,
        tunnel: &str,
        hosts: Vec<String>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
        self.mgr.redo(tunnel, &hosts).await.map_err(failed)
    }

    /// New tunnel (the app's New tunnel window): its fields in text form
    /// (display_name, gateway, protocol, group, sign_in, user, certificate,
    /// mode, list, forget_after_days, weak_ciphers, client_version).
    /// Returns the tunnel's name.
    async fn add_tunnel(
        &self,
        fields: HashMap<String, String>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        self.mgr.add_tunnel(&fields).map_err(failed)
    }

    /// Change a tunnel added in the app.
    async fn update_tunnel(
        &self,
        name: &str,
        fields: HashMap<String, String>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.update_tunnel(name, &fields).await.map_err(failed)
    }

    /// Remove a tunnel added in the app (switched off first).
    async fn remove_tunnel(
        &self,
        name: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.remove_tunnel(name).await.map_err(failed)
    }

    /// OpenVPN 3: the app's session for this tunnel is connected; its
    /// device, DNS servers and domains (comma-separated), the gateway's
    /// address and the routes it pushed.
    async fn attach(
        &self,
        tunnel: &str,
        fields: HashMap<String, String>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.attach(tunnel, &fields).await.map_err(failed)
    }

    /// OpenVPN 3: the session ended; `failed` says why if it dropped.
    async fn detach(
        &self,
        tunnel: &str,
        failed: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        let failed = (!failed.is_empty()).then(|| failed.to_owned());
        self.mgr.detach(tunnel, failed).await.map_err(self::failed)
    }

    /// Move hosts another tunnel answered for too (they stay direct for
    /// `from` until redone there).
    async fn move_hosts(
        &self,
        from: &str,
        to: &str,
        hosts: Vec<String>,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
        self.mgr.move_hosts(from, to, &hosts).await.map_err(failed)
    }

    /// The Settings window's values, each in its text form (lists one item
    /// per line, "never" for no limit).
    async fn settings(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<HashMap<String, String>> {
        self.authorize(&hdr, conn).await?;
        Ok(self.mgr.settings())
    }

    async fn set_setting(
        &self,
        key: &str,
        value: &str,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.set_setting(key, value).await.map_err(failed)
    }

    /// Empty the log. Learned hosts stay.
    async fn clear_log(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(&hdr, conn).await?;
        self.mgr.clear_log();
        Ok(())
    }

    /// A setting changed (from any client).
    #[zbus(signal)]
    async fn settings_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    /// The last `count` log entries, oldest first, one JSON object each.
    async fn recent_log(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        count: u32,
    ) -> fdo::Result<Vec<String>> {
        self.authorize(&hdr, conn).await?;
        Ok(self
            .mgr
            .log()
            .recent(count.min(MAX_RECENT) as usize)
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect())
    }

    /// craked's view of the computer, line by line (each tunnel's device,
    /// address and DNS; who answers the system's DNS): for the app's
    /// helpdesk copies.
    async fn diagnostics(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<Vec<String>> {
        self.authorize(&hdr, conn).await?;
        Ok(self.mgr.diagnostics())
    }

    /// Direct traffic per minute for the last hour: (minute since the epoch,
    /// bytes), oldest first. Counters only.
    async fn traffic_last_hour(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<Vec<(u64, u64)>> {
        self.authorize(&hdr, conn).await?;
        Ok(self.mgr.traffic())
    }

    /// All traffic per minute for the last two days: (minute since
    /// the epoch the slot starts at, bytes), oldest first. Counters only.
    async fn traffic_days(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<Vec<(u64, u64)>> {
        self.authorize(&hdr, conn).await?;
        Ok(self.mgr.traffic_days())
    }

    /// Traffic through one tunnel per minute, two days: (minute since the
    /// epoch, bytes), oldest first. Counters only.
    async fn tunnel_traffic(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        tunnel: &str,
    ) -> fdo::Result<Vec<(u64, u64)>> {
        self.authorize(&hdr, conn).await?;
        Ok(self.mgr.tunnel_traffic(tunnel))
    }

    /// Tunnel detail's host list: one dictionary per host, with `target`,
    /// `name`, `kind` (pinned | learned | direct), `learned_at` (seconds
    /// since the epoch), `how` (probe | dns | inside | split | you),
    /// `used_ago` and `expires_in` (seconds), each only when known. Then
    /// its short names: `target` the short name, `name` its full name,
    /// `kind` short, `learned_at` when you chose it.
    async fn host_details(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        tunnel: &str,
    ) -> fdo::Result<Vec<HashMap<String, String>>> {
        self.authorize(&hdr, conn).await?;
        let rows = self.mgr.hosts(tunnel).await.map_err(failed)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let mut d = HashMap::new();
                let kind = if r.pinned {
                    "pinned"
                } else if r.direct {
                    "direct"
                } else {
                    "learned"
                };
                d.insert("target".to_owned(), r.target.to_string());
                d.insert("kind".to_owned(), kind.to_owned());
                if let Some(n) = r.name {
                    d.insert("name".to_owned(), n);
                }
                if let Some((at, how)) = r.learned {
                    d.insert("learned_at".to_owned(), at.to_string());
                    d.insert("how".to_owned(), how);
                }
                if let Some(a) = r.used_ago {
                    d.insert("used_ago".to_owned(), a.to_string());
                }
                if let Some(e) = r.expires_in {
                    d.insert("expires_in".to_owned(), e.to_string());
                }
                d
            })
            .chain(
                self.mgr
                    .short_names(tunnel)
                    .into_iter()
                    .map(|(short, full, at)| {
                        HashMap::from([
                            ("target".to_owned(), short),
                            ("name".to_owned(), full),
                            ("kind".to_owned(), "short".to_owned()),
                            ("learned_at".to_owned(), at.to_string()),
                        ])
                    }),
            )
            .collect())
    }

    /// What goes where right now (crake_proto::live::Live as JSON): the
    /// app's Traffic window asks once a second while it's open. The open
    /// connections say where this computer connects, so only those who may
    /// switch tunnels get it.
    async fn live(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        serde_json::to_string(&self.mgr.live().await).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// The computer's side of a report (crake_proto::report::Computer, as
    /// JSON): its DNS, routes and Crake's rules, the internet and each
    /// tunnel's names tried now, craked's own log. It says where the
    /// computer connects, so only those who may switch tunnels get it.
    async fn report(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(&hdr, conn).await?;
        serde_json::to_string(&self.mgr.report().await)
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// A new log entry (JSON), as it's written.
    #[zbus(signal)]
    async fn log_appended(emitter: &SignalEmitter<'_>, entry: &str) -> zbus::Result<()>;

    /// (target, DNS name or "", kind: pinned | learned | direct, seconds until
    /// a learned host expires; 0 otherwise)
    async fn list_hosts(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        tunnel: &str,
    ) -> fdo::Result<Vec<(String, String, String, u64)>> {
        self.authorize(&hdr, conn).await?;
        let rows = self.mgr.hosts(tunnel).await.map_err(failed)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let kind = if r.pinned {
                    "pinned"
                } else if r.direct {
                    "direct"
                } else {
                    "learned"
                };
                let name = r.name.unwrap_or_default();
                (
                    r.target.to_string(),
                    name,
                    kind.to_owned(),
                    r.expires_in.unwrap_or(0),
                )
            })
            .collect())
    }

    #[zbus(property)]
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    #[zbus(signal)]
    async fn tunnel_changed(
        emitter: &SignalEmitter<'_>,
        name: &str,
        state: &str,
    ) -> zbus::Result<()>;
}

fn failed(e: anyhow::Error) -> fdo::Error {
    fdo::Error::Failed(format!("{e:#}"))
}
