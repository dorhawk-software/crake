use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use crake_proto::{HookMessage, HookReply};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::manager::Manager;

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

impl<R: tokio::io::AsyncRead + Unpin> TakeLine for BufReader<R> {
    async fn take_line(&mut self, buf: &mut String, max: u64) -> Result<()> {
        use tokio::io::AsyncReadExt;
        BufReader::new(self.take(max)).read_line(buf).await?;
        Ok(())
    }
}
