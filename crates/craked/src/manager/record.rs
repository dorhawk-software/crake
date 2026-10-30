//! What the manager writes to the log about a tunnel's decisions.

use super::*;

impl Manager {
    pub(super) fn record(&self, kind: LogKind, tunnel: &str, message: &str, reason: String) {
        self.record_hosts(kind, tunnel, message, reason, Vec::new());
    }

    /// A decision about specific addresses (so it can be undone).
    pub(super) fn record_hosts(
        &self,
        kind: LogKind,
        tunnel: &str,
        message: &str,
        reason: String,
        hosts: Vec<String>,
    ) {
        self.log.append(LogEntry {
            ts: crate::log::now(),
            kind,
            tunnel: Some(tunnel.to_owned()),
            message: message.to_owned(),
            reason: Some(reason),
            hosts,
            others: Vec::new(),
            short: None,
        });
    }

    /// A decision about a short name: asked, chosen or forgotten (the app
    /// offers the answer, or the way back, from `short`).
    pub(super) fn record_short(
        &self,
        kind: LogKind,
        tunnel: &str,
        message: &str,
        reason: String,
        short: crake_proto::ShortName,
    ) {
        self.log.append(LogEntry {
            ts: crate::log::now(),
            kind,
            tunnel: Some(tunnel.to_owned()),
            message: message.to_owned(),
            reason: Some(reason),
            hosts: Vec::new(),
            others: Vec::new(),
            short: Some(short),
        });
    }
}
