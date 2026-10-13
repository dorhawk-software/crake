//! Which decisions still stand, for the charts' marks. An undo takes back
//! the decision it's about; a redo brings that same decision back rather
//! than being a new one. So any run of undos and redos ends where it
//! began: the same tick, the same count.

use std::collections::HashMap;

use crake_proto::{LogEntry, LogKind};

/// A decision, and whether it still stands (false: undone since).
pub type Decision<'a> = (&'a LogEntry, bool);

/// The log's decisions (routed, left alone), oldest first, each with
/// whether it still stands (false: undone since). Redos aren't among them:
/// they bring back the decision they redo.
pub fn decisions(log: &[LogEntry]) -> Vec<Decision<'_>> {
    let mut order: Vec<&LogEntry> = log.iter().collect();
    order.sort_by_key(|e| e.ts.parse::<jiff::Timestamp>().ok());
    let mut out: Vec<Decision> = Vec::new();
    // Each tunnel's host: the routed decision about it (its place in `out`).
    let mut about: HashMap<(Option<&str>, &str), usize> = HashMap::new();
    for e in order {
        let tunnel = e.tunnel.as_deref();
        let decision_of = |h: &String| about.get(&(tunnel, h.as_str())).copied();
        match e.kind {
            LogKind::Routed => {
                // Every host one undone decision is about: that decision again.
                let first = e.hosts.first().and_then(decision_of);
                let redo = first
                    .filter(|&i| !out[i].1 && e.hosts.iter().all(|h| decision_of(h) == Some(i)));
                if let Some(i) = redo {
                    out[i].1 = true;
                    continue;
                }
                let i = out.len();
                out.push((e, true));
                for h in &e.hosts {
                    about.insert((tunnel, h.as_str()), i);
                }
            }
            LogKind::Undone => {
                for h in &e.hosts {
                    if let Some(i) = decision_of(h) {
                        out[i].1 = false;
                    }
                }
            }
            LogKind::LeftAlone => out.push((e, true)),
            LogKind::System | LogKind::Asked => {}
        }
    }
    out
}
