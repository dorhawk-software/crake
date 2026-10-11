//! One Crake per session: starting it again (from your apps, or when the
//! bird is hidden from the menu bar) opens the one that's running.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

fn socket() -> Option<PathBuf> {
    Some(super::private_dir().ok()?.join("ui.sock"))
}

/// If Crake already runs, ask it to open and return true (this one then
/// exits). Otherwise listen, calling `open` whenever another start asks.
pub fn claim(open: impl Fn() + Send + 'static) -> bool {
    let Some(path) = socket() else { return false };
    if let Ok(mut s) = UnixStream::connect(&path) {
        return s.write_all(b"open\n").is_ok();
    }
    // Nobody answered: a stale socket from a crash, or none.
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return false;
    };
    std::thread::Builder::new()
        .name("single-instance".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut line = String::new();
                if BufReader::new(stream).read_line(&mut line).is_ok() && line.trim() == "open" {
                    open();
                }
            }
        })
        .ok();
    false
}
