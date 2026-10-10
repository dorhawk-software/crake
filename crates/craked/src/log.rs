use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tokio::sync::broadcast;

use anyhow::{Context, Result};
use crake_proto::{LogEntry, LogKind};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Append-only JSON lines. Each entry is one decision or system event.
/// Settings decide how long they're kept; with "Don't keep a log" nothing
/// goes to disk and only recent entries live in memory.
pub struct Log {
    file: Mutex<File>,
    path: PathBuf,
    appended: broadcast::Sender<LogEntry>,
    /// Days to keep; None: don't write the log at all.
    keep: Mutex<Option<u64>>,
    /// The latest entries, for the panel when nothing is kept on disk.
    memory: Mutex<std::collections::VecDeque<LogEntry>>,
}

/// Entries kept in memory.
const MEMORY: usize = 500;

impl Log {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        Ok(Self {
            file: Mutex::new(file),
            path: path.to_owned(),
            appended: broadcast::channel(256).0,
            keep: Mutex::new(Some(30)),
            memory: Mutex::new(Default::default()),
        })
    }

    /// Keep entries this many days (older ones go now); None: keep none on disk.
    pub fn keep(&self, days: Option<u64>) {
        *self.keep.lock().unwrap() = days;
        match days {
            Some(d) => self.prune(d),
            None => self.rewrite(Vec::new()),
        }
    }

    /// Drop entries older than `days` from the file.
    pub fn prune(&self, days: u64) {
        // Timestamps are UTC RFC 3339: their first 19 characters
        // ("2026-10-09T21:14:00") compare as text.
        let cutoff = (OffsetDateTime::now_utc() - time::Duration::days(days as i64))
            .format(&Rfc3339)
            .unwrap_or_default();
        let cutoff = cutoff.get(..19).unwrap_or_default().to_owned();
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        let total = text.lines().count();
        let kept: Vec<&str> = text
            .lines()
            .filter(|l| {
                serde_json::from_str::<LogEntry>(l)
                    .ok()
                    .and_then(|e| e.ts.get(..19).map(|t| t >= cutoff.as_str()))
                    .unwrap_or(true)
            })
            .collect();
        if kept.len() != total {
            self.rewrite(kept);
        }
    }

    /// Empty the log, on disk and in memory.
    pub fn clear(&self) {
        self.memory.lock().unwrap().clear();
        self.rewrite(Vec::new());
    }

    /// Replace the file with these lines (whole or not at all).
    fn rewrite(&self, lines: Vec<&str>) {
        let mut file = self.file.lock().unwrap();
        let tmp = self.path.with_extension("tmp");
        let mut text = lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &self.path).is_ok() {
            match OpenOptions::new().append(true).open(&self.path) {
                Ok(f) => *file = f,
                Err(e) => tracing::error!("reopening the log: {e}"),
            }
        }
    }

    /// Entries as they're written.
    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.appended.subscribe()
    }

    /// The last `count` entries, oldest first.
    pub fn recent(&self, count: usize) -> Vec<LogEntry> {
        if self.keep.lock().unwrap().is_none() {
            let memory = self.memory.lock().unwrap();
            let start = memory.len().saturating_sub(count);
            return memory.iter().skip(start).cloned().collect();
        }
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.len().saturating_sub(count);
        lines[start..]
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    pub fn system(&self, tunnel: Option<&str>, message: impl Into<String>, reason: Option<String>) {
        self.append(LogEntry {
            ts: now(),
            kind: LogKind::System,
            tunnel: tunnel.map(str::to_owned),
            message: message.into(),
            reason,
            hosts: Vec::new(),
            others: Vec::new(),
        });
    }

    pub fn append(&self, entry: LogEntry) {
        let mut line = match serde_json::to_string(&entry) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("encoding log entry: {e}");
                return;
            }
        };
        line.push('\n');
        {
            let mut memory = self.memory.lock().unwrap();
            if memory.len() == MEMORY {
                memory.pop_front();
            }
            memory.push_back(entry.clone());
        }
        if self.keep.lock().unwrap().is_some() {
            // One write call per entry keeps lines whole.
            let res = self.file.lock().unwrap().write_all(line.as_bytes());
            if let Err(e) = res {
                tracing::error!("writing log: {e}");
            }
        }
        let _ = self.appended.send(entry);
    }
}

pub fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(ts: &str, message: &str) -> LogEntry {
        LogEntry {
            ts: ts.into(),
            kind: LogKind::System,
            tunnel: None,
            message: message.into(),
            reason: None,
            hosts: Vec::new(),
            others: Vec::new(),
        }
    }

    #[test]
    fn keeps_what_settings_say() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::open(&dir.path().join("log")).unwrap();
        log.append(entry("2020-01-01T00:00:00Z", "old"));
        log.append(entry(&now(), "new"));
        log.keep(Some(30));
        let left: Vec<String> = log.recent(10).into_iter().map(|e| e.message).collect();
        assert_eq!(left, ["new"], "older than 30 days goes");
        log.append(entry(&now(), "newer"));
        assert_eq!(log.recent(10).len(), 2, "still appending after a prune");
        log.keep(None);
        assert_eq!(std::fs::read_to_string(dir.path().join("log")).unwrap(), "");
        log.append(entry(&now(), "only in memory"));
        assert_eq!(std::fs::read_to_string(dir.path().join("log")).unwrap(), "");
        assert_eq!(log.recent(1)[0].message, "only in memory");
        log.keep(Some(7));
        log.clear();
        assert!(log.recent(10).is_empty());
    }
}
