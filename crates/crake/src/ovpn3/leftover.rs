//! OpenVPN 3 processes that outlive their sessions. openvpn3 lost track of
//! a session's process when it didn't answer Connect in time (connect.rs):
//! the session went, the process kept reconnecting for days and rewrote
//! the other tunnels' DNS each time. Crake can't stop it (it's openvpn3's,
//! run as openvpn3's own user), so it says so, with the command.
//!
//! Only a process Crake saw belong to one of the user's sessions counts:
//! other users' sessions aren't visible to the app, and a process left
//! from before the app started isn't recognised (not now: telling it from
//! another user's would need root).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use zbus::Connection;

use super::{SESSIONS, property, sessions};

/// How long a process may outlive its session before it's said: openvpn3's
/// own take a few seconds to exit.
const OUTLIVED: Duration = Duration::from_secs(60);

/// A session's process: its command's file name.
const SESSION_PROCESS: &str = "openvpn3-service-client";

/// /proc/<pid>/stat: the field after the command's name, and the one with
/// when the process started (ticks since boot), counted from 1.
const FIRST_AFTER_NAME: usize = 3;
const STARTED_FIELD: usize = 22;

/// The processes of the user's sessions: (pid, profile).
pub fn backends() -> Vec<(u32, String)> {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Vec::new();
    };
    rt.block_on(async {
        let Ok(conn) = Connection::system().await else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for path in sessions(&conn).await {
            let pid = property::<u32>(&conn, SESSIONS, &path, SESSIONS, "backend_pid").await;
            let name = property::<String>(&conn, SESSIONS, &path, SESSIONS, "config_name").await;
            if let (Ok(pid), Ok(name)) = (pid, name)
                && pid > 0
            {
                found.push((pid, name));
            }
        }
        found
    })
}

/// When process `pid` started, if it's a session's process; None once it
/// has ended (or its number belongs to another program now).
pub fn started(pid: u32) -> Option<u64> {
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let command = cmdline.split(|b| *b == 0).next()?;
    if !command.ends_with(SESSION_PROCESS.as_bytes()) {
        return None;
    }
    stat_started(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// The start time in a /proc stat line (the command's name, in
/// parentheses, may hold spaces and parentheses itself).
fn stat_started(stat: &str) -> Option<u64> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace()
        .nth(STARTED_FIELD - FIRST_AFTER_NAME)?
        .parse()
        .ok()
}

/// The sessions' processes Crake has seen, and which outlived theirs.
#[derive(Default)]
pub struct Leftovers {
    seen: HashMap<u32, Seen>,
}

struct Seen {
    profile: String,
    started: u64,
    alone_since: Option<Instant>,
    said: bool,
}

impl Leftovers {
    /// The processes said to have outlived their sessions that still run:
    /// (pid, when it started, its session's profile).
    pub fn running(&self) -> Vec<(u32, u64, String)> {
        let mut all: Vec<_> = self
            .seen
            .iter()
            .filter(|(_, s)| s.said)
            .map(|(pid, s)| (*pid, s.started, s.profile.clone()))
            .collect();
        all.sort_unstable();
        all
    }

    /// One look: the sessions' processes now (`backends`), and when a
    /// process started (`started`). Returns each process to say, once:
    /// (pid, its session's profile).
    pub fn look(
        &mut self,
        now: Instant,
        backends: &[(u32, String)],
        started: impl Fn(u32) -> Option<u64>,
    ) -> Vec<(u32, String)> {
        for (pid, profile) in backends {
            let Some(at) = started(*pid) else { continue };
            let seen = self.seen.entry(*pid).or_insert_with(|| Seen {
                profile: profile.clone(),
                started: at,
                alone_since: None,
                said: false,
            });
            if seen.started != at {
                seen.started = at;
                seen.said = false;
            }
            seen.profile.clone_from(profile);
            seen.alone_since = None;
        }
        let mut say = Vec::new();
        self.seen.retain(|pid, seen| {
            if backends.iter().any(|(p, _)| p == pid) {
                return true;
            }
            if started(*pid) != Some(seen.started) {
                return false;
            }
            let since = *seen.alone_since.get_or_insert(now);
            if !seen.said && now.duration_since(since) >= OUTLIVED {
                seen.said = true;
                say.push((*pid, seen.profile.clone()));
            }
            true
        });
        say
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn processes(list: &[(u32, u64)]) -> impl Fn(u32) -> Option<u64> + '_ {
        |pid| list.iter().find(|(p, _)| *p == pid).map(|(_, at)| *at)
    }

    #[test]
    fn a_process_that_outlives_its_session_is_said_once() {
        let t0 = Instant::now();
        let mut l = Leftovers::default();
        let running = [(4242, 100)];
        let globex = vec![(4242, "globex".to_owned())];
        assert!(l.look(t0, &globex, processes(&running)).is_empty());
        // The session is gone, its process isn't: a minute's grace.
        assert!(l.look(t0, &[], processes(&running)).is_empty());
        let later = t0 + Duration::from_secs(59);
        assert!(l.look(later, &[], processes(&running)).is_empty());
        let minute = t0 + OUTLIVED;
        assert_eq!(
            l.look(minute, &[], processes(&running)),
            vec![(4242, "globex".to_owned())]
        );
        assert!(
            l.look(minute + OUTLIVED, &[], processes(&running))
                .is_empty()
        );
        assert_eq!(l.running(), vec![(4242, 100, "globex".to_owned())]);
        // Ended: no longer running.
        l.look(minute + OUTLIVED, &[], processes(&[]));
        assert!(l.running().is_empty());
    }

    #[test]
    fn processes_that_end_or_were_never_a_sessions_arent_said() {
        let t0 = Instant::now();
        let mut l = Leftovers::default();
        let globex = vec![(4242, "globex".to_owned())];
        l.look(t0, &globex, processes(&[(4242, 100), (5151, 7)]));
        // It ended with its session; 5151 never was one of the user's.
        l.look(t0, &[], processes(&[(5151, 7)]));
        assert!(
            l.look(t0 + OUTLIVED, &[], processes(&[(5151, 7)]))
                .is_empty()
        );
        // Its number taken by a new process: not the one seen.
        l.look(t0, &globex, processes(&[(4242, 100)]));
        l.look(t0, &[], processes(&[(4242, 900)]));
        assert!(
            l.look(t0 + OUTLIVED, &[], processes(&[(4242, 900)]))
                .is_empty()
        );
    }

    #[test]
    fn a_session_back_on_its_process_is_not_left_over() {
        let t0 = Instant::now();
        let mut l = Leftovers::default();
        let globex = vec![(4242, "globex".to_owned())];
        let running = [(4242, 100)];
        l.look(t0, &globex, processes(&running));
        l.look(t0, &[], processes(&running));
        l.look(t0 + Duration::from_secs(30), &globex, processes(&running));
        assert!(l.look(t0 + OUTLIVED, &[], processes(&running)).is_empty());
    }

    #[test]
    fn the_start_time_is_read_past_a_name_with_spaces() {
        let stat = "4242 (openvpn3 (client)) S 1 4242 4242 0 -1 4194560 1 0 0 0 \
                    3 2 0 0 20 0 3 0 123456 0 0";
        assert_eq!(stat_started(stat), Some(123_456));
        assert_eq!(stat_started("garbage"), None);
    }
}
