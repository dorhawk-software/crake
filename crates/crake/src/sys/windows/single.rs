//! One Crake per session: starting it again (from the Start menu, or when
//! the bird is hidden from the notification area) opens the one that's
//! running. The first one owns a named pipe; later ones write to it.

use std::io::Write;

use tokio::io::AsyncBufReadExt;
use tokio::net::windows::named_pipe::ServerOptions;

fn pipe() -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    format!(r"\\.\pipe\crake-ui-{user}")
}

/// Another Crake owns the pipe (asking it nothing).
pub fn taken() -> bool {
    std::fs::OpenOptions::new().write(true).open(pipe()).is_ok()
}

/// If Crake already runs, ask it to open and return true (this one then
/// exits). Otherwise listen, calling `open` whenever another start asks.
pub fn claim(open: impl Fn() + Send + 'static) -> bool {
    let name = pipe();
    if let Ok(mut running) = std::fs::OpenOptions::new().write(true).open(&name) {
        return running.write_all(b"open\n").is_ok();
    }
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
    else {
        return false;
    };
    let first = {
        let _in = rt.enter();
        ServerOptions::new().first_pipe_instance(true).create(&name)
    };
    // Another Crake owns the pipe but was busy: it runs; leave it be.
    let Ok(mut server) = first else { return true };
    std::thread::Builder::new()
        .name("single-instance".into())
        .spawn(move || {
            rt.block_on(async move {
                loop {
                    if server.connect().await.is_err() {
                        return;
                    }
                    let Ok(next) = ServerOptions::new().create(&name) else {
                        return;
                    };
                    let mut line = String::new();
                    let mut reader = tokio::io::BufReader::new(&mut server);
                    if reader.read_line(&mut line).await.is_ok() && line.trim() == "open" {
                        open();
                    }
                    server = next;
                }
            });
        })
        .ok();
    false
}
