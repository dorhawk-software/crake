//! Where craked serves its API: Linux's system bus (polkit decides), a
//! private development socket, macOS' app socket, Windows' loopback port;
//! each connection relays tunnel changes and new log entries as signals, to
//! the callers the guard let in.

use std::sync::Arc;

use crake_proto::{BUS_NAME, BUS_PATH};
use zbus::Connection;
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;

use super::guard::Allowed;
use super::{Guard, Service};
use crate::manager::Manager;

/// Claim the bus name, publish the object and relay changes as signals.
pub async fn serve(mgr: Arc<Manager>, system_bus: bool) -> zbus::Result<Connection> {
    if system_bus {
        serve_on(zbus::connection::Builder::system()?, Guard::Polkit, mgr).await
    } else {
        serve_on(zbus::connection::Builder::session()?, Guard::Open, mgr).await
    }
}

async fn serve_on(
    builder: zbus::connection::Builder<'_>,
    guard: Guard,
    mgr: Arc<Manager>,
) -> zbus::Result<Connection> {
    let allowed = Arc::new(Allowed::default());
    let service = Service {
        mgr: Arc::clone(&mgr),
        guard,
        allowed: Arc::clone(&allowed),
    };
    let conn = builder
        .name(BUS_NAME)?
        .serve_at(BUS_PATH, service)?
        .build()
        .await?;
    relay(conn.clone(), mgr, allowed, true);
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
    std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(crate::sys::OWNER_ONLY),
    )?;
    std::os::unix::fs::chown(path, Some(owner), None)?;
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let mgr = Arc::clone(&mgr);
            tokio::spawn(async move {
                let allowed = Arc::new(Allowed::default());
                let service = Service {
                    mgr: Arc::clone(&mgr),
                    guard: Guard::Open,
                    allowed: Arc::clone(&allowed),
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
                    Ok(conn) => relay(conn, mgr, allowed, false),
                    Err(e) => tracing::warn!("UI connection: {e}"),
                }
            });
        }
    });
    Ok(())
}

/// The macOS console socket: anyone may open it; the API checks who asks.
#[cfg(target_os = "macos")]
const ANYONE: u32 = 0o666;

/// macOS: the same API on a Unix socket anyone may open
/// (`crake_proto::APP_SOCKET`); changes need the console's user.
#[cfg(target_os = "macos")]
pub fn serve_console(mgr: Arc<Manager>, path: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::remove_file(path);
    let listener = tokio::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(ANYONE))?;
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let mgr = Arc::clone(&mgr);
            tokio::spawn(async move {
                let allowed = Arc::new(Allowed::default());
                let service = Service {
                    mgr: Arc::clone(&mgr),
                    guard: Guard::ConsoleUser,
                    allowed: Arc::clone(&allowed),
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
                    Ok(conn) => relay(conn, mgr, allowed, false),
                    Err(e) => tracing::warn!("app connection: {e}"),
                }
            });
        }
    });
    Ok(())
}

/// Windows: the same API on a loopback port (`crake_proto::APP_ADDRESS`).
/// zbus learns each caller's account from the connection (EXTERNAL with
/// the process's SID); changes need the console session.
#[cfg(windows)]
pub async fn serve_local(mgr: Arc<Manager>, address: &str) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let mgr = Arc::clone(&mgr);
            tokio::spawn(async move {
                let allowed = Arc::new(Allowed::default());
                let service = Service {
                    mgr: Arc::clone(&mgr),
                    guard: Guard::ConsoleSession,
                    allowed: Arc::clone(&allowed),
                };
                let built = async {
                    zbus::connection::Builder::tcp_stream(stream)
                        .server(zbus::Guid::generate())?
                        .p2p()
                        .serve_at(BUS_PATH, service)?
                        .build()
                        .await
                };
                match built.await {
                    Ok(conn) => relay(conn, mgr, allowed, false),
                    Err(e) => tracing::warn!("app connection: {e}"),
                }
            });
        }
    });
    Ok(())
}

/// Tunnel state changes and new log entries, as signals: on the system
/// bus to each caller the guard let in, addressed to it (broadcast, they
/// reached everyone on the computer: `guard`); on a private connection
/// only once its one peer was let in.
fn relay(conn: Connection, mgr: Arc<Manager>, allowed: Arc<Allowed>, bus: bool) {
    let mut changes = mgr.subscribe();
    let mut entries = mgr.log().subscribe();
    tokio::spawn(async move {
        loop {
            use tokio::sync::broadcast::error::RecvError;
            let signal = tokio::select! {
                name = changes.recv() => match name {
                    // An empty name: the settings changed.
                    Ok(name) if name.is_empty() => Signal::Settings,
                    Ok(name) => {
                        // Removed (Settings, Tunnels): "removed".
                        let state = mgr
                            .get(&name)
                            .map_or_else(|| "removed".to_owned(), |snap| snap.state.as_str().to_owned());
                        Signal::Tunnel(name, state)
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                },
                entry = entries.recv() => match entry {
                    Ok(entry) => match serde_json::to_string(&entry) {
                        Ok(json) => Signal::Log(json),
                        Err(_) => continue,
                    },
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                },
            };
            let to: Vec<Option<String>> = if bus {
                allowed.hearing().into_iter().map(Some).collect()
            } else if allowed.hears("") {
                vec![None]
            } else {
                Vec::new()
            };
            for dest in to {
                // A private connection's peer that went away: stop relaying to it.
                if send(&conn, dest.as_deref(), &signal).await.is_err() && !bus {
                    return;
                }
            }
        }
    });
}

/// What `relay` says.
enum Signal {
    Settings,
    Tunnel(String, String),
    Log(String),
}

/// `signal` on `conn`, to `dest` alone when there's one.
async fn send(conn: &Connection, dest: Option<&str>, signal: &Signal) -> zbus::Result<()> {
    let mut emitter = SignalEmitter::new(conn, BUS_PATH)?;
    if let Some(dest) = dest {
        emitter = emitter.set_destination(BusName::try_from(dest)?);
    }
    match signal {
        Signal::Settings => Service::settings_changed(&emitter).await,
        Signal::Tunnel(name, state) => Service::tunnel_changed(&emitter, name, state).await,
        Signal::Log(json) => Service::log_appended(&emitter, json).await,
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader};
    use std::time::Duration;

    use futures_util::StreamExt;

    use super::*;

    /// A bus of the test's own: a caller craked let in hears a tunnel
    /// change, one that only listens hears nothing (until 2026-10-21 every
    /// signal was broadcast to the whole system bus).
    #[tokio::test]
    async fn signals_reach_only_the_callers_let_in() {
        let mut daemon = std::process::Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("dbus-daemon (the dbus package) for a bus of the test's own");
        let out = daemon.stdout.take().unwrap();
        let _daemon = Ends(daemon);
        let mut address = String::new();
        BufReader::new(out).read_line(&mut address).unwrap();
        let address = address.trim().to_owned();
        let dir = tempfile::tempdir().unwrap();
        let mgr = Manager::new(crate::config::Config {
            log_path: dir.path().join("log"),
            state_path: dir.path().join("hosts.json"),
            managed_dir: dir.path().join("managed.d"),
            probing: false,
            use_resolved: false,
            apply_network: false,
            ..Default::default()
        })
        .unwrap();
        let bus = || async {
            zbus::connection::Builder::address(address.as_str())
                .unwrap()
                .build()
                .await
                .unwrap()
        };
        let on_bus = zbus::connection::Builder::address(address.as_str()).unwrap();
        let _craked = serve_on(on_bus, Guard::Open, Arc::clone(&mgr))
            .await
            .unwrap();
        let listen = |conn: &Connection| {
            let conn = conn.clone();
            async move {
                let rule = zbus::MatchRule::builder()
                    .msg_type(zbus::message::Type::Signal)
                    .interface(BUS_NAME)
                    .unwrap()
                    .build();
                zbus::MessageStream::for_match_rule(rule, &conn, None)
                    .await
                    .unwrap()
            }
        };
        let app = bus().await;
        let mut heard = listen(&app).await;
        app.call_method(Some(BUS_NAME), BUS_PATH, Some(BUS_NAME), "ListTunnels", &())
            .await
            .unwrap();
        let stranger = bus().await;
        let mut overheard = listen(&stranger).await;
        mgr.add_tunnel(&HashMap::from([
            ("display_name".to_owned(), "Globex".to_owned()),
            ("gateway".to_owned(), "vpn.globex.example".to_owned()),
            ("protocol".to_owned(), "anyconnect".to_owned()),
        ]))
        .unwrap();
        let signal = tokio::time::timeout(Duration::from_secs(5), heard.next())
            .await
            .expect("the app heard the change")
            .unwrap()
            .unwrap();
        // The tunnel's change or its log entry, whichever came first.
        let member = signal.header().member().map(|m| m.to_string());
        assert!(
            matches!(member.as_deref(), Some("TunnelChanged" | "LogAppended")),
            "{member:?}"
        );
        let none = tokio::time::timeout(Duration::from_secs(1), overheard.next()).await;
        assert!(none.is_err(), "a caller never let in heard {none:?}");
    }

    /// The bus ends with the test, passed or not.
    struct Ends(std::process::Child);

    impl Drop for Ends {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
