//! openconnect's `--script` (and openvpn's `--up`/`--down`, strongSwan's updown). Forwards the call and its environment to `craked`
//! over a Unix socket (Windows: a named pipe) and waits for the answer, so openconnect doesn't carry on
//! before the address is set. Std only, no async runtime.
// Unsafe code lives in crake-os alone.
#![forbid(unsafe_code)]

use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;

use crake_proto::{
    DEFAULT_HOOK_SOCKET, ENV_SOCKET, ENV_TUNNEL, HookMessage, HookReply, OPENVPN_SCRIPT_TYPE,
    STRONGSWAN_VERB, collect_env, openvpn_reason, strongswan_reason,
};

fn main() -> ExitCode {
    let reason = match std::env::var(OPENVPN_SCRIPT_TYPE) {
        Ok(script) => {
            let context = std::env::var("script_context").unwrap_or_default();
            openvpn_reason(&script, &context).to_owned()
        }
        Err(_) => match std::env::var(STRONGSWAN_VERB) {
            Ok(verb) => strongswan_reason(&verb).to_owned(),
            Err(_) => std::env::var("reason").unwrap_or_default(),
        },
    };
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

/// How long the hook waits for craked's answer.
#[cfg(unix)]
const ANSWER_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

/// craked's socket, giving up on an answer after ANSWER_WAIT.
#[cfg(unix)]
fn open(socket: &str) -> std::io::Result<std::os::unix::net::UnixStream> {
    let stream = std::os::unix::net::UnixStream::connect(socket)?;
    let timeout = Some(ANSWER_WAIT);
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
