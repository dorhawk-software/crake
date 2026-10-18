//! Where one openvpn's management interface listens, and how craked reaches
//! it: a Unix socket in craked's runtime folder (Linux, macOS). Windows'
//! openvpn has no Unix sockets, and 2.7 there ignores the management
//! password file (the password it asks for never matched; Linux's asks
//! for it), so craked listens instead, on a loopback port, openvpn
//! connects to it (`--management-client`), and craked takes the one
//! connection that comes from that openvpn process (Windows' TCP table
//! names each connection's process).

#[cfg(unix)]
use std::path::{Path, PathBuf};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};

/// How long openvpn gets to open its management interface.
const WAIT: std::time::Duration = std::time::Duration::from_secs(10);
const POLL: std::time::Duration = std::time::Duration::from_millis(50);

pub(super) type Reader = Box<dyn AsyncRead + Unpin + Send>;
pub(super) type Writer = Box<dyn AsyncWrite + Unpin + Send>;

pub(super) struct Listen {
    #[cfg(unix)]
    socket: PathBuf,
    #[cfg(windows)]
    listener: tokio::net::TcpListener,
    #[cfg(windows)]
    port: u16,
}

impl Listen {
    /// For tunnel `name`, in `dir` (craked's runtime folder).
    #[cfg(unix)]
    pub(super) fn new(dir: &Path, name: &str) -> anyhow::Result<Self> {
        let socket = dir.join(format!("openvpn-{name}.sock"));
        let _ = std::fs::remove_file(&socket);
        Ok(Self { socket })
    }

    /// A loopback port of craked's own, any free one.
    #[cfg(windows)]
    pub(super) fn new() -> anyhow::Result<Self> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok(Self {
            listener: tokio::net::TcpListener::from_std(listener)?,
            port,
        })
    }

    /// openvpn's `--management` for it.
    pub(super) fn args(&self, cmd: &mut Command) {
        #[cfg(unix)]
        cmd.arg("--management").arg(&self.socket).arg("unix");
        #[cfg(windows)]
        cmd.args(["--management", "127.0.0.1", &self.port.to_string()])
            .arg("--management-client");
    }

    /// The interface, once openvpn has opened it; None if openvpn ended
    /// first or took too long.
    pub(super) async fn connect(&self, child: &mut Child) -> Option<(Reader, Writer)> {
        let deadline = std::time::Instant::now() + WAIT;
        while std::time::Instant::now() < deadline {
            if let Some(halves) = self.try_connect(child).await {
                return Some(halves);
            }
            if child.try_wait().ok().flatten().is_some() {
                return None;
            }
            tokio::time::sleep(POLL).await;
        }
        None
    }

    #[cfg(unix)]
    async fn try_connect(&self, _: &Child) -> Option<(Reader, Writer)> {
        let stream = tokio::net::UnixStream::connect(&self.socket).await.ok()?;
        let (r, w) = stream.into_split();
        Some((Box::new(r), Box::new(w)))
    }

    /// A connection to craked's port from openvpn; anyone else's is closed.
    #[cfg(windows)]
    async fn try_connect(&self, child: &Child) -> Option<(Reader, Writer)> {
        let (stream, from) = tokio::time::timeout(POLL, self.listener.accept())
            .await
            .ok()?
            .ok()?;
        let owner = crate::sys::conntrack::loopback_owner(from.port(), self.port);
        if owner.is_none() || owner != child.id() {
            tracing::warn!("closed a connection to openvpn's management port from {owner:?}");
            return None;
        }
        let (r, w) = stream.into_split();
        Some((Box::new(r), Box::new(w)))
    }
}

#[cfg(unix)]
impl Drop for Listen {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}
