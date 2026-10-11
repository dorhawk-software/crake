//! `software.dorhawk.Crake1` on the system bus. Reads are open to every local
//! user; switching a tunnel needs polkit's `software.dorhawk.crake.manage-tunnels`.

use std::collections::HashMap;
use std::sync::Arc;

use crake_proto::{BUS_NAME, BUS_PATH};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, fdo, interface};

use crate::manager::{Manager, Snapshot};

pub const POLKIT_ACTION: &str = "software.dorhawk.crake.manage-tunnels";

pub struct Service {
    mgr: Arc<Manager>,
    /// Off on the session bus (development), where there is no polkit to ask.
    polkit: bool,
}

#[interface(name = "software.dorhawk.Crake1")]
impl Service {
    /// One dictionary per tunnel: name, display_name, state, interface, gateway,
    /// protocol, address, dns, domains, split_include, split_exclude.
    async fn list_tunnels(&self) -> Vec<HashMap<String, OwnedValue>> {
        self.mgr.list().iter().map(to_dict).collect()
    }

    async fn get_tunnel(&self, name: &str) -> fdo::Result<HashMap<String, OwnedValue>> {
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
    async fn settings(&self) -> HashMap<String, String> {
        self.mgr.settings()
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
    async fn recent_log(&self, count: u32) -> Vec<String> {
        self.mgr
            .log()
            .recent(count.min(10_000) as usize)
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect()
    }

    /// Direct traffic per minute for the last hour: (minute since the epoch,
    /// bytes), oldest first. Counters only.
    async fn traffic_last_hour(&self) -> Vec<(u64, u64)> {
        self.mgr.traffic()
    }

    /// All traffic per minute for the last two days: (minute since
    /// the epoch the slot starts at, bytes), oldest first. Counters only.
    async fn traffic_days(&self) -> Vec<(u64, u64)> {
        self.mgr.traffic_days()
    }

    /// Traffic through one tunnel per minute, two days: (minute since the
    /// epoch, bytes), oldest first. Counters only.
    async fn tunnel_traffic(&self, tunnel: &str) -> Vec<(u64, u64)> {
        self.mgr.tunnel_traffic(tunnel)
    }

    /// Tunnel detail's host list: one dictionary per host, with `target`,
    /// `name`, `kind` (pinned | learned | direct), `learned_at` (seconds
    /// since the epoch), `how` (probe | dns | inside | split | you),
    /// `used_ago` and `expires_in` (seconds), each only when known.
    async fn host_details(&self, tunnel: &str) -> fdo::Result<Vec<HashMap<String, String>>> {
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
            .collect())
    }

    /// A new log entry (JSON), as it's written.
    #[zbus(signal)]
    async fn log_appended(emitter: &SignalEmitter<'_>, entry: &str) -> zbus::Result<()>;

    /// (target, DNS name or "", kind: pinned | learned | direct, seconds until
    /// a learned host expires; 0 otherwise)
    async fn list_hosts(&self, tunnel: &str) -> fdo::Result<Vec<(String, String, String, u64)>> {
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

impl Service {
    async fn authorize(&self, hdr: &Header<'_>, conn: &Connection) -> fdo::Result<()> {
        if !self.polkit {
            return Ok(());
        }
        let sender = hdr
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("no sender".into()))?;
        match check_polkit(conn, sender.as_str()).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(fdo::Error::AccessDenied(
                "not authorized to switch tunnels".into(),
            )),
            Err(e) => Err(fdo::Error::AccessDenied(format!("polkit: {e}"))),
        }
    }
}

fn failed(e: anyhow::Error) -> fdo::Error {
    fdo::Error::Failed(format!("{e:#}"))
}

async fn check_polkit(conn: &Connection, sender: &str) -> zbus::Result<bool> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .await?;
    let subject = (
        "system-bus-name",
        HashMap::from([("name", Value::from(sender))]),
    );
    let details: HashMap<&str, &str> = HashMap::new();
    // Flags 0: never prompt from the daemon; the action's defaults decide.
    let (authorized, _challenge, _details): (bool, bool, HashMap<String, String>) = proxy
        .call(
            "CheckAuthorization",
            &(subject, POLKIT_ACTION, details, 0u32, ""),
        )
        .await?;
    Ok(authorized)
}

fn to_dict(s: &Snapshot) -> HashMap<String, OwnedValue> {
    let info = s.info.as_ref();
    let strings = |v: Vec<String>| OwnedValue::try_from(Value::from(v)).unwrap();
    let text = |v: &str| OwnedValue::try_from(Value::from(v)).unwrap();
    let mut d = HashMap::new();
    d.insert("name".into(), text(&s.config.name));
    d.insert("display_name".into(), text(s.config.display()));
    d.insert("state".into(), text(s.state.as_str()));
    d.insert("interface".into(), text(&s.interface));
    d.insert("gateway".into(), text(&s.config.gateway));
    d.insert("protocol".into(), text(s.config.protocol.as_str()));
    d.insert("hosts".into(), OwnedValue::from(s.hosts as u32));
    d.insert("signed_in".into(), OwnedValue::from(s.signed_in));
    d.insert("user".into(), text(s.config.user.as_deref().unwrap_or("")));
    // What sign-in needs, so the user's session can run it.
    d.insert("auth_args".into(), strings(s.config.auth_args.clone()));
    d.insert("extra_args".into(), strings(s.config.extra_args.clone()));
    // Who decides (Managed profiles): the panel says it in plain words.
    let managed = s.config.managed.as_ref();
    d.insert("managed".into(), OwnedValue::from(managed.is_some()));
    d.insert(
        "policy".into(),
        text(
            &managed
                .map(|m| m.describe(s.config.display()))
                .unwrap_or_default(),
        ),
    );
    d.insert(
        "full_tunnel".into(),
        OwnedValue::from(s.config.learning() == crate::policy::Learning::FullTunnel),
    );
    // What a managed tunnel may learn: names and networks (Settings, Managed by IT).
    let learn = match s.config.learning() {
        crate::policy::Learning::Only { names, nets } => names
            .into_iter()
            .chain(nets.iter().map(|n| n.to_string()))
            .collect(),
        _ => Vec::new(),
    };
    d.insert("learn".into(), strings(learn));
    // Where it comes from: what the app may change.
    let source = match (&s.config.managed, &s.config.own) {
        (Some(_), _) => "managed",
        (_, Some(_)) => "app",
        _ => "config",
    };
    d.insert("source".into(), text(source));
    d.insert(
        "organisation".into(),
        text(managed.map(|m| m.organisation.as_str()).unwrap_or("")),
    );
    // An app tunnel's fields, for editing (keys as AddTunnel's).
    let own: HashMap<String, String> = s
        .config
        .own
        .as_ref()
        .map(|o| o.to_map().into_iter().collect())
        .unwrap_or_default();
    d.insert(
        "own".into(),
        OwnedValue::try_from(Value::from(own)).unwrap(),
    );
    // Names that go to its DNS: pushed by the gateway, plus the config's.
    let mut dns_domains: Vec<String> = info.map(|i| i.domains.clone()).unwrap_or_default();
    for d in &s.config.dns_domains {
        if !dns_domains.contains(d) {
            dns_domains.push(d.clone());
        }
    }
    d.insert("dns_domains".into(), strings(dns_domains));
    d.insert(
        "managed_file".into(),
        text(
            &managed
                .map(|m| m.file.display().to_string())
                .unwrap_or_default(),
        ),
    );
    d.insert("since".into(), OwnedValue::from(s.since.unwrap_or(0)));
    d.insert("clash".into(), text(s.clash.as_deref().unwrap_or("")));
    d.insert("dns_down".into(), OwnedValue::from(s.dns_down));
    d.insert(
        "transport".into(),
        text(s.transport.as_deref().unwrap_or("")),
    );
    d.insert(
        "address".into(),
        text(
            &info
                .and_then(|i| i.address)
                .map(|a| a.to_string())
                .unwrap_or_default(),
        ),
    );
    d.insert(
        "dns".into(),
        strings(
            info.map(|i| i.dns.iter().map(|a| a.to_string()).collect())
                .unwrap_or_default(),
        ),
    );
    d.insert(
        "domains".into(),
        strings(info.map(|i| i.domains.clone()).unwrap_or_default()),
    );
    d.insert(
        "split_include".into(),
        strings(info.map(|i| i.split_include.clone()).unwrap_or_default()),
    );
    d.insert(
        "split_exclude".into(),
        strings(info.map(|i| i.split_exclude.clone()).unwrap_or_default()),
    );
    d
}

/// Claim the bus name, publish the object and relay changes as signals.
pub async fn serve(mgr: Arc<Manager>, system_bus: bool) -> zbus::Result<Connection> {
    let service = Service {
        mgr: Arc::clone(&mgr),
        polkit: system_bus,
    };
    let builder = if system_bus {
        zbus::connection::Builder::system()?
    } else {
        zbus::connection::Builder::session()?
    };
    let conn = builder
        .name(BUS_NAME)?
        .serve_at(BUS_PATH, service)?
        .build()
        .await?;
    relay(conn.clone(), mgr);
    Ok(conn)
}

/// The same API without a bus: a private socket only `owner` can open
/// (development, where nothing is installed). Polkit isn't asked; the
/// socket's permissions are the guard.
#[cfg(unix)]
pub fn serve_p2p(mgr: Arc<Manager>, path: &std::path::Path, owner: u32) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::remove_file(path);
    let listener = tokio::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    std::os::unix::fs::chown(path, Some(owner), None)?;
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let mgr = Arc::clone(&mgr);
            tokio::spawn(async move {
                let service = Service {
                    mgr: Arc::clone(&mgr),
                    polkit: false,
                };
                let built = async {
                    zbus::connection::Builder::unix_stream(stream)
                        .server(zbus::Guid::generate())?
                        .p2p()
                        .serve_at(BUS_PATH, service)?
                        .build()
                        .await
                };
                match built.await {
                    Ok(conn) => relay(conn, mgr),
                    Err(e) => tracing::warn!("UI connection: {e}"),
                }
            });
        }
    });
    Ok(())
}

/// Windows: the app's named pipe (phase 3 of the Windows plan in CLAUDE.md).
#[cfg(windows)]
pub fn serve_p2p(_mgr: Arc<Manager>, _path: &std::path::Path, _owner: u32) -> anyhow::Result<()> {
    Err(crate::sys::not_yet("the app's pipe"))
}

/// Tunnel state changes and new log entries, as signals on `conn`.
fn relay(conn: Connection, mgr: Arc<Manager>) {
    let mut changes = mgr.subscribe();
    let mut entries = mgr.log().subscribe();
    tokio::spawn(async move {
        let Ok(iface) = conn.object_server().interface::<_, Service>(BUS_PATH).await else {
            return;
        };
        loop {
            use tokio::sync::broadcast::error::RecvError;
            let sent = tokio::select! {
                name = changes.recv() => match name {
                    // An empty name: the settings changed.
                    Ok(name) if name.is_empty() => Service::settings_changed(iface.signal_emitter()).await,
                    Ok(name) => match mgr.get(&name) {
                        Some(snap) => Service::tunnel_changed(iface.signal_emitter(), &name, snap.state.as_str()).await,
                        // Removed (Settings, Tunnels).
                        None => Service::tunnel_changed(iface.signal_emitter(), &name, "removed").await,
                    },
                    Err(RecvError::Lagged(_)) => Ok(()),
                    Err(RecvError::Closed) => return,
                },
                entry = entries.recv() => match entry {
                    Ok(entry) => match serde_json::to_string(&entry) {
                        Ok(json) => Service::log_appended(iface.signal_emitter(), &json).await,
                        Err(_) => Ok(()),
                    },
                    Err(RecvError::Lagged(_)) => Ok(()),
                    Err(RecvError::Closed) => return,
                },
            };
            // A peer that went away: stop relaying to it.
            if sent.is_err() {
                return;
            }
        }
    });
}
