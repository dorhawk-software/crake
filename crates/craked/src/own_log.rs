//! craked's own log, the last of it kept in memory for reports (D-Bus
//! `Report`): every line tracing writes goes where it always went (the
//! journal, a file, the console) and here too, so a report has it on every
//! system and under `--dev` alike, with nothing read back from disk.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crake_proto::report::{OWN_LOG_LINES, private};

/// How far back a report's log goes.
const LAST: Duration = Duration::from_secs(60 * 60);

static LINES: Mutex<VecDeque<(Instant, String)>> = Mutex::new(VecDeque::new());

/// tracing's writer for one event: its line, kept when the event is done.
#[derive(Default)]
pub struct Line(Vec<u8>);

impl Write for Line {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for Line {
    fn drop(&mut self) {
        let text = String::from_utf8_lossy(&self.0);
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            keep(line);
        }
    }
}

fn keep(line: &str) {
    let mut lines = LINES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if lines.len() >= OWN_LOG_LINES {
        lines.pop_front();
    }
    lines.push_back((Instant::now(), line.to_owned()));
}

/// The last hour's lines (at most OWN_LOG_LINES), oldest first, without
/// sign-in material.
pub fn recent() -> Vec<String> {
    let lines = LINES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    lines
        .iter()
        .filter(|(at, _)| at.elapsed() < LAST)
        .filter_map(|(_, l)| private(l))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_last_lines_without_sign_ins() {
        for i in 0..OWN_LOG_LINES + 3 {
            let mut line = Line::default();
            writeln!(line, "INFO craked: line {i}").unwrap();
        }
        let mut secret = Line::default();
        writeln!(secret, "DEBUG openvpn: >PASSWORD:Auth-Token:abc").unwrap();
        drop(secret);
        let got = recent();
        assert!(got.len() <= OWN_LOG_LINES);
        assert_eq!(
            got.last().unwrap(),
            &format!("INFO craked: line {}", OWN_LOG_LINES + 2)
        );
        assert!(!got.iter().any(|l| l.contains("Auth-Token")));
    }
}
