//! Where craked serves its API: Linux's system bus (polkit decides), a
//! private development socket, macOS' app socket, Windows' loopback port;
//! each connection relays tunnel changes and new log entries as signals.

use std::sync::Arc;

use crake_proto::{BUS_NAME, BUS_PATH};
use zbus::Connection;

use super::{Guard, Service};
use crate::manager::Manager;

/// Claim the bus name, publish the object and relay changes as signals.
pub async fn serve(mgr: Arc<Manager>, system_bus: bool) -> zbus::Result<Connection> {
    let service = Service {
        mgr: Arc::clone(&mgr),
        guard: if system_bus {
            Guard::Polkit
        } else {
            Guard::Open
        },
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
                let service = Service {
                    mgr: Arc::clone(&mgr),
                    guard: Guard::Open,
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
                let service = Service {
                    mgr: Arc::clone(&mgr),
                    guard: Guard::ConsoleUser,
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
                let service = Service {
                    mgr: Arc::clone(&mgr),
                    guard: Guard::ConsoleSession,
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
                    Ok(conn) => relay(conn, mgr),
                    Err(e) => tracing::warn!("app connection: {e}"),
                }
            });
        }
    });
    Ok(())
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
