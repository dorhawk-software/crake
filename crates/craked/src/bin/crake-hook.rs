//! openconnect's `--script`. Forwards the call and its environment to `craked`
//! over a Unix socket (Windows: a named pipe) and waits for the answer, so openconnect doesn't carry on
//! before the address is set. Std only, no async runtime.

use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;

use crake_proto::{
    DEFAULT_HOOK_SOCKET, ENV_SOCKET, ENV_TUNNEL, HookMessage, HookReply, collect_env,
};

fn main() -> ExitCode {
    let reason = std::env::var("reason").unwrap_or_default();
    // Steps that must succeed for the tunnel to be usable. For the rest, never
    // make openconnect stumble over Crake's bookkeeping.
    let critical = matches!(reason.as_str(), "pre-init" | "connect" | "reconnect");
    match run(&reason) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("crake-hook: {reason}: {e}");
            if critical {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
    }
}

fn run(reason: &str) -> Result<(), String> {
    let tunnel = std::env::var(ENV_TUNNEL).map_err(|_| format!("{ENV_TUNNEL} is not set"))?;
    let socket = std::env::var(ENV_SOCKET).unwrap_or_else(|_| DEFAULT_HOOK_SOCKET.to_owned());
    let msg = HookMessage {
        tunnel,
        reason: reason.to_owned(),
        env: collect_env(std::env::vars()),
    };

    let mut stream = open(&socket).map_err(|e| format!("connecting to {socket}: {e}"))?;
    let mut line = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
    line.push(b'\n');
    stream.write_all(&line).map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(&stream)
        .read_line(&mut reply)
        .map_err(|e| format!("waiting for craked: {e}"))?;
    let reply: HookReply = serde_json::from_str(&reply).map_err(|e| format!("bad reply: {e}"))?;
    if reply.ok {
        Ok(())
    } else {
        Err(reply.error.unwrap_or_else(|| "refused".into()))
    }
}

/// craked's socket, giving up on an answer after 15 s.
#[cfg(unix)]
fn open(socket: &str) -> std::io::Result<std::os::unix::net::UnixStream> {
    let stream = std::os::unix::net::UnixStream::connect(socket)?;
    let timeout = Some(std::time::Duration::from_secs(15));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    Ok(stream)
}

/// craked's named pipe, which opens like a file.
#[cfg(windows)]
fn open(socket: &str) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(socket)
}
