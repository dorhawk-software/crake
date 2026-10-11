//! The hook's channel: openconnect's hook (`crake-hook`) sends what the
//! gateway pushed, craked answers once it's applied. Linux: a Unix socket
//! only root can open; Windows: a named pipe, writable only by SYSTEM and
//! administrators (the pipe's default security; openconnect, and so its
//! hook, runs as craked's SYSTEM).

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use crake_proto::{HookMessage, HookReply};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::manager::Manager;

/// Bind the hook socket (root-only) and serve it in the background.
#[cfg(unix)]
pub fn serve(mgr: Arc<Manager>, path: &Path) -> Result<tokio::task::JoinHandle<()>> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let _ = std::fs::remove_file(path);
    let listener = tokio::net::UnixListener::bind(path)
        .with_context(|| format!("binding {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => answer(Arc::clone(&mgr), stream),
                Err(e) => {
                    tracing::error!("hook socket: {e}");
                    return;
                }
            }
        }
    }))
}

/// Create the hook's pipe (refused if another process already has one of
/// that name) and serve it in the background, one instance per call.
#[cfg(windows)]
pub fn serve(mgr: Arc<Manager>, path: &Path) -> Result<tokio::task::JoinHandle<()>> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let name = path.as_os_str().to_owned();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .with_context(|| format!("creating {}", path.display()))?;
    Ok(tokio::spawn(async move {
        loop {
            if let Err(e) = server.connect().await {
                tracing::error!("hook pipe: {e}");
                return;
            }
            // The next caller gets a new instance while this one answers.
            let next = match ServerOptions::new().create(&name) {
                Ok(next) => next,
                Err(e) => {
                    tracing::error!("hook pipe: {e}");
                    return;
                }
            };
            answer(Arc::clone(&mgr), std::mem::replace(&mut server, next));
        }
    }))
}

/// Answer one hook call in the background.
fn answer(mgr: Arc<Manager>, stream: impl AsyncRead + AsyncWrite + Send + 'static) {
    tokio::spawn(async move {
        if let Err(e) = handle(&mgr, stream).await {
            tracing::warn!("hook connection: {e:#}");
        }
    });
}

async fn handle(mgr: &Manager, stream: impl AsyncRead + AsyncWrite) -> Result<()> {
    let (r, mut w) = tokio::io::split(stream);
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
