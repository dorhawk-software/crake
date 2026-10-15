//! Telling a new session to connect, once. openvpn3's session manager
//! gives a session's process 5 s to answer Connect; two front-ends telling
//! the same new session to connect at once (openvpn3-indicator did it to
//! every session, Crake's too) ran out those 5 s, and openvpn3 then lost
//! the process: its session went, the process didn't, and it kept
//! reconnecting on its own for days, rewriting other tunnels' DNS.

use zbus::Connection;

use super::bus::call;
use super::{CONN_CONNECTED, SESS_AUTH_URL, SESSIONS, status};

// openvpn3's StatusMinor values of a session already told to connect.
const CONN_INIT: u32 = 5;
const CONN_CONNECTING: u32 = 6;
const CONN_RECONNECTING: u32 = 12;
const CONN_RESUMING: u32 = 15;
const SESS_AUTH_USERPASS: u32 = 20;
const SESS_AUTH_CHALLENGE: u32 = 21;

/// The session is on its way already (or there): someone else told it to
/// connect, and telling it again only makes openvpn3 wait on itself.
pub(super) fn under_way(minor: u32) -> bool {
    matches!(
        minor,
        CONN_INIT
            | CONN_CONNECTING
            | CONN_CONNECTED
            | CONN_RECONNECTING
            | CONN_RESUMING
            | SESS_AUTH_USERPASS
            | SESS_AUTH_CHALLENGE
            | SESS_AUTH_URL
    )
}

/// openvpn3's refusal of Connect that still leaves the session on its way:
/// connected meanwhile by another front-end, waiting for the web sign-in,
/// or its process slow to answer (it may connect all the same; the
/// session's status says).
pub(super) fn still_on_its_way(refusal: &str) -> bool {
    refusal.contains("already")
        || refusal.contains("web authentication")
        || refusal.contains("Timeout was reached")
}

/// Connect, unless the session is under way already.
pub(super) async fn connect(conn: &Connection, path: &str) -> Result<(), String> {
    if status(conn, path)
        .await
        .is_some_and(|(minor, _)| under_way(minor))
    {
        return Ok(());
    }
    match call::<_, ()>(conn, SESSIONS, path, SESSIONS, "Connect", &()).await {
        Ok(()) => Ok(()),
        Err(zbus::Error::MethodError(_, Some(text), _)) if still_on_its_way(&text) => Ok(()),
        Err(e) => Err(format!("OpenVPN 3 couldn\u{2019}t connect: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_already_told_to_connect_isnt_told_again() {
        for minor in [5, 6, 7, 12, 15, 20, 21, 22] {
            assert!(under_way(minor), "{minor}");
        }
        // New, configured, asking for its profile's answers, or over.
        for minor in [2, 4, 9, 10, 16, 17, 19] {
            assert!(!under_way(minor), "{minor}");
        }
    }

    #[test]
    fn a_slow_answer_to_connect_is_waited_out() {
        // As openvpn3's session manager words it when its 5 s run out.
        assert!(still_on_its_way(
            "[Proxy::Client('net.openvpn.v3.backends.be2415678', '/net/openvpn/v3/backends/session', 'net.openvpn.v3.backends', 'Connect')] Timeout was reached"
        ));
        assert!(still_on_its_way("Session is already connected"));
        assert!(still_on_its_way("Pending web authentication"));
        assert!(!still_on_its_way("Backend VPN process is not ready"));
    }
}
