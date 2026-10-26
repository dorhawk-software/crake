//! Who may use craked's API: since 2026-10-21 reading it too, not only
//! changing things. Before, anyone on the computer could read the log (the
//! hosts reached, and when), the hosts learned and the tunnels, and heard
//! every decision as it was made, broadcast on the system bus. Now each
//! call is the same check as switching a tunnel (polkit's active session
//! on Linux, the console's user on macOS, the console session on Windows),
//! and signals go only to callers it let in (`relay`).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use zbus::message::Header;
use zbus::{Connection, Variant, fdo};

use super::{Guard, POLKIT_ACTION, Service};

/// A yes holds this long for the same caller (a bus name is never used
/// twice), so the app's frequent reads don't ask polkit each time; then
/// it's asked again, however often the caller called meanwhile.
const TRUSTED_FOR: Duration = Duration::from_secs(60);
/// A caller hears signals this long after its last call let in (the app's
/// traffic ticks keep it fresh).
const HEARS_FOR: Duration = Duration::from_secs(600);

/// The callers let in: their bus names, or "" for the one peer of a
/// private connection, with when they were last checked and last let in.
#[derive(Default)]
pub struct Allowed(Mutex<HashMap<String, Let>>);

#[derive(Clone, Copy)]
struct Let {
    checked: Instant,
    seen: Instant,
}

impl Allowed {
    /// Whether `who`'s last yes is recent enough to skip asking again.
    fn trusted(&self, who: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .get(who)
            .is_some_and(|l| l.checked.elapsed() < TRUSTED_FOR)
    }

    /// `who` was let in now, `checked` just now or earlier.
    fn allow(&self, who: &str, checked: bool) {
        let now = Instant::now();
        let mut seen = self.0.lock().unwrap();
        seen.retain(|_, l| l.seen.elapsed() < HEARS_FOR);
        let at = match (checked, seen.get(who)) {
            (false, Some(l)) => l.checked,
            _ => now,
        };
        seen.insert(
            who.to_owned(),
            Let {
                checked: at,
                seen: now,
            },
        );
    }

    /// Whether `who` hears signals.
    pub fn hears(&self, who: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .get(who)
            .is_some_and(|l| l.seen.elapsed() < HEARS_FOR)
    }

    /// Who hears signals now.
    pub fn hearing(&self) -> Vec<String> {
        let mut seen = self.0.lock().unwrap();
        seen.retain(|_, l| l.seen.elapsed() < HEARS_FOR);
        seen.keys().cloned().collect()
    }
}

impl Service {
    /// The caller may use the API (`check`), asked again once a minute.
    pub(super) async fn authorize(&self, hdr: &Header<'_>, conn: &Connection) -> fdo::Result<()> {
        let who = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let ask = !self.allowed.trusted(&who);
        if ask {
            self.check(hdr, conn).await?;
        }
        self.allowed.allow(&who, ask);
        Ok(())
    }
}

impl Service {
    pub(super) async fn check(&self, hdr: &Header<'_>, conn: &Connection) -> fdo::Result<()> {
        match self.guard {
            Guard::Open => return Ok(()),
            Guard::Polkit => {}
            #[cfg(windows)]
            Guard::ConsoleSession => return in_console_session(conn).await,
            #[cfg(target_os = "macos")]
            Guard::ConsoleUser => return console_user(conn).await,
        }
        let sender = hdr
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("no sender".into()))?;
        match check_polkit(conn, sender.as_str()).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(fdo::Error::AccessDenied(
                "not authorized to use Crake's service".into(),
            )),
            Err(e) => Err(fdo::Error::AccessDenied(format!("polkit: {e}"))),
        }
    }
}

/// The caller runs in the console session (Windows).
#[cfg(windows)]
async fn in_console_session(conn: &Connection) -> fdo::Result<()> {
    let pid = conn.peer_creds().await.ok().and_then(|c| c.process_id());
    if pid.is_some_and(crate::sys::in_console_session) {
        Ok(())
    } else {
        Err(fdo::Error::AccessDenied(
            "only the signed-in user can use Crake's service".into(),
        ))
    }
}

/// The caller is the user at the console, or root (macOS).
#[cfg(target_os = "macos")]
async fn console_user(conn: &Connection) -> fdo::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let caller = conn.peer_creds().await.ok().and_then(|c| c.unix_user_id());
    let console = std::fs::metadata("/dev/console").ok().map(|m| m.uid());
    match caller {
        Some(0) => Ok(()),
        Some(uid) if Some(uid) == console => Ok(()),
        _ => Err(fdo::Error::AccessDenied(
            "only the user at the console can use Crake's service".into(),
        )),
    }
}

async fn check_polkit(conn: &Connection, sender: &str) -> zbus::Result<bool> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .await?;
    let subject = (
        "system-bus-name",
        HashMap::from([("name", Variant::from(sender))]),
    );
    let details: HashMap<&str, &str> = HashMap::new();
    // Flags 0: never prompt from the daemon; the action's defaults decide.
    let (authorized, _challenge, _details): (bool, bool, HashMap<String, String>) = proxy
        .call(
            "CheckAuthorization",
            &(subject, POLKIT_ACTION, details, 0u32, ""),
        )
        .await?;
    Ok(authorized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_yes_is_asked_again_however_often_one_calls() {
        let allowed = Allowed::default();
        assert!(!allowed.trusted(":1.5") && !allowed.hears(":1.5"));
        allowed.allow(":1.5", true);
        assert!(allowed.trusted(":1.5") && allowed.hears(":1.5"));
        // Checked long ago, called since: the old check stays.
        let long_ago = Instant::now().checked_sub(TRUSTED_FOR * 2).unwrap();
        allowed.0.lock().unwrap().get_mut(":1.5").unwrap().checked = long_ago;
        allowed.allow(":1.5", false);
        assert!(!allowed.trusted(":1.5"), "renewed without asking");
        assert!(allowed.hears(":1.5"));
        assert_eq!(allowed.hearing(), vec![":1.5".to_owned()]);
        // Quiet too long: no longer hears.
        let quiet = Instant::now().checked_sub(HEARS_FOR * 2).unwrap();
        allowed.0.lock().unwrap().get_mut(":1.5").unwrap().seen = quiet;
        assert!(!allowed.hears(":1.5") && allowed.hearing().is_empty());
    }
}
