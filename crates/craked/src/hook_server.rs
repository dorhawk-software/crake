//! The hook's channel: openconnect's hook (`crake-hook`) sends what the
//! gateway pushed, craked answers once it's applied.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;

use crate::manager::Manager;

#[cfg(unix)]
pub use unix::serve;

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::PermissionsExt;

    use anyhow::Context;
    use crake_proto::{HookMessage, HookReply};
    use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
    use tokio::net::{UnixListener, UnixStream};

    use super::*;

    /// Bind the hook socket (root-only) and serve it in the background.
    pub fn serve(mgr: Arc<Manager>, path: &Path) -> Result<tokio::task::JoinHandle<()>> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let _ = std::fs::remove_file(path);
        let listener =
            UnixListener::bind(path).with_context(|| format!("binding {}", path.display()))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let mgr = Arc::clone(&mgr);
                        tokio::spawn(async move {
                            if let Err(e) = handle(&mgr, stream).await {
                                tracing::warn!("hook connection: {e:#}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("hook socket: {e}");
                        return;
                    }
                }
            }
        }))
    }

    async fn handle(mgr: &Manager, stream: UnixStream) -> Result<()> {
        let (r, mut w) = stream.into_split();
        let mut line = String::new();
        // A hook message is small; refuse anything absurd.
        BufReader::new(r).take_line(&mut line, 256 * 1024).await?;
        let msg: HookMessage = serde_json::from_str(&line).context("decoding hook message")?;
        tracing::debug!(tunnel = %msg.tunnel, reason = %msg.reason, "hook");
        let reply = match mgr.handle_hook(msg).await {
            Ok(()) => HookReply {
                ok: true,
                error: None,
            },
            Err(e) => {
                tracing::warn!("hook rejected: {e:#}");
                HookReply {
                    ok: false,
                    error: Some(format!("{e:#}")),
                }
            }
        };
        let mut out = serde_json::to_vec(&reply)?;
        out.push(b'\n');
        w.write_all(&out).await?;
        Ok(())
    }

    trait TakeLine {
        async fn take_line(&mut self, buf: &mut String, max: u64) -> Result<()>;
    }

    impl<R: AsyncRead + Unpin> TakeLine for BufReader<R> {
        async fn take_line(&mut self, buf: &mut String, max: u64) -> Result<()> {
            use tokio::io::AsyncReadExt;
            BufReader::new(self.take(max)).read_line(buf).await?;
            Ok(())
        }
    }
}

/// Windows: a named pipe only the service's account can open (phase 3 of
/// the Windows plan in CLAUDE.md).
#[cfg(windows)]
pub fn serve(_mgr: Arc<Manager>, _path: &Path) -> Result<tokio::task::JoinHandle<()>> {
    Err(crate::sys::not_yet("the hook's pipe"))
}
