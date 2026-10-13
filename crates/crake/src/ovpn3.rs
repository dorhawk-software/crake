//! OpenVPN 3 (CLAUDE.md, 7g): the user's own openvpn3 profiles, run as the
//! user (who owns them) over openvpn3's D-Bus services. Its questions go to
//! Crake's sign-in window; once connected, craked is handed the session's
//! device and DNS ("attach") and routes per host through it, as with
//! openconnect tunnels. The session is watched until it ends ("detach").
//!
//! D-Bus API: /usr/share/doc/openvpn3/dbus; status codes as in the Python
//! bindings' `openvpn3.constants`.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// OpenVPN 3's backend may still be starting: tried every READY_POLL, up
/// to 10 s. A session missing from GONE_AFTER looks in a row is gone.
const READY_ATTEMPTS: u32 = 50;
const READY_POLL: Duration = Duration::from_millis(200);
const GONE_AFTER: u32 = 3;

use futures_util::StreamExt;
use zbus::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::Request;
use crate::signin::{Ask, Event, Prompt, Reply};

const SESSIONS: &str = "net.openvpn.v3.sessions";
const CONFIGURATION: &str = "net.openvpn.v3.configuration";
const NETCFG: &str = "net.openvpn.v3.netcfg";

/// StatusMinor values (openvpn3.constants).
const CONN_CONNECTED: u32 = 7;
const CONN_DISCONNECTED: u32 = 9;
const CONN_FAILED: u32 = 10;
const CONN_AUTH_FAILED: u32 = 11;
const CONN_DONE: u32 = 16;
const SESS_REMOVED: u32 = 19;
/// The server wants a web sign-in; the status message is its URL.
const SESS_AUTH_URL: u32 = 22;
/// ClientAttentionGroup: a URL to sign in on the web.
const OPEN_URL: u32 = 8;

/// The profiles the user has imported into openvpn3 (their names).
pub fn profiles() -> Vec<String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(rt) = rt else { return Vec::new() };
    rt.block_on(async {
        let Ok(conn) = Connection::system().await else {
            return Vec::new();
        };
        let paths: Vec<OwnedObjectPath> = match call(
            &conn,
            CONFIGURATION,
            "/net/openvpn/v3/configuration",
            CONFIGURATION,
            "FetchAvailableConfigs",
            &(),
        )
        .await
        {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        let mut names = Vec::new();
        for p in paths {
            if let Ok(name) =
                property::<String>(&conn, CONFIGURATION, p.as_str(), CONFIGURATION, "name").await
            {
                names.push(name);
            }
        }
        names.sort();
        names.dedup();
        names
    })
}

/// Give openvpn3 a profile: `config` (self-contained, `ovpn_file::inline`)
/// under `name`, kept (not single use), owned by the user. Blocking: call
/// it off the UI thread. The error is openvpn3's, as it said it.
pub fn import(name: &str, config: &str) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let conn = Connection::system().await.map_err(|e| e.to_string())?;
        let _profile: OwnedObjectPath = call(
            &conn,
            CONFIGURATION,
            "/net/openvpn/v3/configuration",
            CONFIGURATION,
            "Import",
            // Not single use; persistent (kept across reboots).
            &(name, config, false, true),
        )
        .await
        .map_err(|e| match e {
            zbus::Error::MethodError(_, Some(said), _) => said,
            other => other.to_string(),
        })?;
        Ok(())
    })
}

async fn call<B, R>(
    conn: &Connection,
    dest: &str,
    path: &str,
    iface: &str,
    method: &str,
    body: &B,
) -> zbus::Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let reply = conn
        .call_method(Some(dest), path, Some(iface), method, body)
        .await?;
    reply.body().deserialize()
}

async fn property<T>(
    conn: &Connection,
    dest: &str,
    path: &str,
    iface: &str,
    name: &str,
) -> zbus::Result<T>
where
    T: TryFrom<OwnedValue>,
{
    let v: OwnedValue = call(
        conn,
        dest,
        path,
        "org.freedesktop.DBus.Properties",
        "Get",
        &(iface, name),
    )
    .await?;
    T::try_from(v).map_err(|_| zbus::Error::Failure(format!("{name}: unexpected type")))
}

/// Start the profile's session for `tunnel`. Events go to the sign-in
/// window; replies come back from it. After it connected, the replies
/// channel stays the session's control: `Reply::Cancel` disconnects it.
pub fn start(
    tunnel: String,
    profile: String,
    events: async_channel::Sender<Event>,
    requests: async_channel::Sender<Request>,
) -> async_channel::Sender<Reply> {
    let (replies, incoming) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("openvpn3".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(async {
                match run(&tunnel, &profile, &events, &incoming, &requests).await {
                    Ok(()) => {}
                    Err(e) => {
                        let _ = events.send(Event::Failed(e)).await;
                    }
                }
            });
        })
        .expect("a thread for openvpn3");
    replies
}

async fn run(
    tunnel: &str,
    profile: &str,
    events: &async_channel::Sender<Event>,
    replies: &async_channel::Receiver<Reply>,
    requests: &async_channel::Sender<Request>,
) -> Result<(), String> {
    let conn = Connection::system()
        .await
        .map_err(|e| format!("Couldn\u{2019}t reach OpenVPN 3: {e}"))?;
    // openvpn3 runs one session per profile: one already running (started
    // from openvpn3's own indicator, its command line, or an earlier Crake)
    // is taken over, not refused.
    let running = session_of(&conn, profile).await;
    let adopted = running.is_some();
    let path = match running {
        Some(path) => path,
        None => {
            let configs: Vec<OwnedObjectPath> = call(
                &conn,
                CONFIGURATION,
                "/net/openvpn/v3/configuration",
                CONFIGURATION,
                "LookupConfigName",
                &(profile,),
            )
            .await
            .map_err(|e| format!("Couldn\u{2019}t look up the profile {profile}: {e}"))?;
            let config = configs
                .first()
                .ok_or_else(|| format!("OpenVPN 3 has no profile called {profile} for you."))?;
            let session: OwnedObjectPath = call(
                &conn,
                SESSIONS,
                "/net/openvpn/v3/sessions",
                SESSIONS,
                "NewTunnel",
                &(config,),
            )
            .await
            .map_err(|e| format!("OpenVPN 3 couldn\u{2019}t start a session: {e}"))?;
            session.as_str().to_owned()
        }
    };
    let _ = events.send(Event::Reached).await;
    let mut signals = subscribe(&conn, &path).await?;

    // Web sign-in pages opened so far (each once).
    let mut opened = HashSet::new();
    // Another openvpn3 front-end (openvpn3-indicator) may connect a new
    // session before Crake does; then it's waiting for the web sign-in.
    let mut connect = !adopted;
    if !adopted {
        // Ready? If not, it wants answers first; then connect.
        let mut attempts = 0;
        loop {
            match call::<_, ()>(&conn, SESSIONS, &path, SESSIONS, "Ready", &()).await {
                Ok(()) => break,
                Err(zbus::Error::MethodError(_, Some(text), _))
                    if text.contains("web authentication") =>
                {
                    answer(&conn, &path, events, replies, &mut opened).await?;
                    connect = false;
                    break;
                }
                Err(zbus::Error::MethodError(_, Some(text), _))
                    if text.contains("credentials") || text.contains("input") =>
                {
                    answer(&conn, &path, events, replies, &mut opened).await?;
                }
                Err(e) => {
                    // The backend may still be starting.
                    attempts += 1;
                    if attempts > READY_ATTEMPTS {
                        disconnect(&conn, &path).await;
                        return Err(format!("OpenVPN 3 wasn\u{2019}t ready: {e}"));
                    }
                    tokio::time::sleep(READY_POLL).await;
                }
            }
        }
    }
    if connect {
        match call::<_, ()>(&conn, SESSIONS, &path, SESSIONS, "Connect", &()).await {
            Ok(()) => {}
            // Connected meanwhile by another front-end: wait for it as well.
            Err(zbus::Error::MethodError(_, Some(text), _))
                if text.contains("already") || text.contains("web authentication") => {}
            Err(e) => return Err(format!("OpenVPN 3 couldn\u{2019}t connect: {e}")),
        }
    }

    // Until it's connected (or not), answering what it asks on the way
    // (a taken-over session's questions are for whoever started it).
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut unseen = 0;
    loop {
        tokio::select! {
            msg = signals.next() => {
                let Some(Ok(msg)) = msg else { return Err("OpenVPN 3 went quiet.".into()) };
                if !adopted && msg.header().member().map(|m| m.as_str()) == Some("AttentionRequired") {
                    answer(&conn, &path, events, replies, &mut opened).await?;
                }
            }
            _ = tick.tick() => {}
            r = replies.recv() => {
                if !matches!(r, Ok(Reply::Answer { .. })) {
                    disconnect(&conn, &path).await;
                    return Err("Cancelled.".into());
                }
            }
        }
        let Some((minor, text)) = status(&conn, &path).await else {
            unseen += 1;
            if unseen >= GONE_AFTER {
                return Err("The OpenVPN 3 session is gone.".into());
            }
            continue;
        };
        unseen = 0;
        match minor {
            CONN_CONNECTED => break,
            SESS_AUTH_URL => open_once(&text, &mut opened),
            CONN_AUTH_FAILED => {
                disconnect(&conn, &path).await;
                return Err(
                    format!("The gateway didn\u{2019}t accept the sign-in. {text}")
                        .trim()
                        .to_owned(),
                );
            }
            CONN_FAILED | CONN_DISCONNECTED | CONN_DONE | SESS_REMOVED => {
                disconnect(&conn, &path).await;
                return Err(if text.is_empty() {
                    "The connection failed.".into()
                } else {
                    text
                });
            }
            _ => {}
        }
    }

    let fields = attached(&conn, &path).await?;
    let _ = events.send(Event::Attached(path.clone(), fields)).await;
    follow(&conn, &path, tunnel, signals, replies, requests).await;
    Ok(())
}

/// A session already connected outside Crake, for a tunnel Crake has:
/// hand it to craked and watch it, as if Crake had started it.
pub fn adopt(tunnel: String, path: String, requests: async_channel::Sender<Request>) {
    std::thread::Builder::new()
        .name("openvpn3-adopt".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async {
                let Ok(conn) = Connection::system().await else {
                    return;
                };
                let Ok(signals) = subscribe(&conn, &path).await else {
                    return;
                };
                if !matches!(status(&conn, &path).await, Some((CONN_CONNECTED, _))) {
                    return;
                }
                let Ok(fields) = attached(&conn, &path).await else {
                    return;
                };
                let (control, replies) = async_channel::unbounded();
                let _ = requests
                    .send(Request::Attach(
                        tunnel.clone(),
                        path.clone(),
                        fields,
                        control,
                    ))
                    .await;
                follow(&conn, &path, &tunnel, signals, &replies, &requests).await;
            });
        })
        .ok();
}

/// The user's connected sessions: (profile, session path).
pub fn connected() -> Vec<(String, String)> {
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
            if matches!(status(&conn, &path).await, Some((CONN_CONNECTED, _)))
                && let Ok(name) =
                    property::<String>(&conn, SESSIONS, &path, SESSIONS, "config_name").await
            {
                found.push((name, path));
            }
        }
        found
    })
}

async fn sessions(conn: &Connection) -> Vec<String> {
    call::<_, Vec<OwnedObjectPath>>(
        conn,
        SESSIONS,
        "/net/openvpn/v3/sessions",
        SESSIONS,
        "FetchAvailableSessions",
        &(),
    )
    .await
    .map(|v| v.into_iter().map(|p| p.as_str().to_owned()).collect())
    .unwrap_or_default()
}

/// The running session of `profile`, if there is one (connected or on its way).
async fn session_of(conn: &Connection, profile: &str) -> Option<String> {
    for path in sessions(conn).await {
        let name = property::<String>(conn, SESSIONS, &path, SESSIONS, "config_name").await;
        let live = status(conn, &path).await.is_some_and(|(minor, _)| {
            !matches!(
                minor,
                CONN_FAILED | CONN_DISCONNECTED | CONN_DONE | SESS_REMOVED | CONN_AUTH_FAILED
            )
        });
        if live && name.as_deref() == Ok(profile) {
            return Some(path);
        }
    }
    None
}

/// The session's signals: requests for input from the session manager,
/// status changes from net.openvpn.v3.log (forwarded only once asked for,
/// with LogForward). A signal only wakes the driver: what counts is the
/// session manager's own `status`, also read every second or two in case
/// a signal never comes.
async fn subscribe(conn: &Connection, path: &str) -> Result<zbus::MessageStream, String> {
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path(path.to_owned())
        .map_err(|e| e.to_string())?
        .build();
    let signals = zbus::MessageStream::for_match_rule(rule, conn, None)
        .await
        .map_err(|e| e.to_string())?;
    let _ = call::<_, ()>(conn, SESSIONS, path, SESSIONS, "LogForward", &(true,)).await;
    Ok(signals)
}

/// Connected: what craked needs to route through it. (DNS and domains
/// too, where openvpn3 lets the user read them; craked otherwise takes
/// them from resolved.)
async fn attached(conn: &Connection, path: &str) -> Result<HashMap<String, String>, String> {
    let device: String = property(conn, SESSIONS, path, SESSIONS, "device_name")
        .await
        .map_err(|e| e.to_string())?;
    let mut fields = HashMap::new();
    fields.insert("device".to_owned(), device);
    if let Ok((_, address, _)) =
        property::<(String, String, u32)>(conn, SESSIONS, path, SESSIONS, "connected_to").await
    {
        fields.insert("gateway".to_owned(), address);
    }
    if let Ok(device_path) = property::<String>(conn, SESSIONS, path, SESSIONS, "device_path").await
    {
        if let Ok(dns) =
            property::<Vec<String>>(conn, NETCFG, &device_path, NETCFG, "dns_name_servers").await
        {
            fields.insert("dns".to_owned(), dns.join(","));
        }
        if let Ok(domains) =
            property::<Vec<String>>(conn, NETCFG, &device_path, NETCFG, "dns_search_domains").await
        {
            fields.insert("domains".to_owned(), domains.join(","));
        }
    }
    Ok(fields)
}

/// Watch a connected session until it ends: switched off here, or dropped
/// (or disconnected from openvpn3's own tools).
async fn follow(
    conn: &Connection,
    path: &str,
    tunnel: &str,
    mut signals: zbus::MessageStream,
    replies: &async_channel::Receiver<Reply>,
    requests: &async_channel::Sender<Request>,
) {
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    let mut unseen = 0;
    // openvpn3 says so in its log when the server ends the session.
    let mut expired = false;
    let created = created(conn, path).await;
    loop {
        tokio::select! {
            msg = signals.next() => {
                let Some(Ok(msg)) = msg else {
                    let _ = requests.send(Request::Detached(tunnel.to_owned(), path.to_owned(), Some("OpenVPN 3 went quiet.".into()))).await;
                    return;
                };
                expired |= says_expired(&msg);
            }
            _ = tick.tick() => {}
            r = replies.recv() => match r {
                // Switched off in the app.
                Ok(Reply::Cancel) => {
                    disconnect(conn, path).await;
                    let _ = requests.send(Request::Detached(tunnel.to_owned(), path.to_owned(), None)).await;
                    return;
                }
                // No one holds this session's control any more (another
                // follower has it): stop following, touch nothing. Losing
                // the channel never ends a session.
                Err(_) => return,
                Ok(_) => {}
            }
        }
        let ended = match status(conn, path).await {
            None => {
                unseen += 1;
                (unseen >= GONE_AFTER).then(|| Some("The OpenVPN 3 session is gone.".to_owned()))
            }
            Some((minor, text)) => {
                unseen = 0;
                matches!(
                    minor,
                    CONN_FAILED | CONN_DISCONNECTED | CONN_DONE | SESS_REMOVED | CONN_AUTH_FAILED
                )
                .then(|| (!text.is_empty()).then_some(text))
            }
        };
        if let Some(why) = ended {
            let why = if expired {
                Some(expired_said(
                    created.map(|c| c.elapsed().unwrap_or_default()),
                ))
            } else {
                why
            };
            disconnect(conn, path).await;
            let _ = requests
                .send(Request::Detached(tunnel.to_owned(), path.to_owned(), why))
                .await;
            return;
        }
    }
}

/// When openvpn3 created the session (its `session_created`, seconds
/// since 1970), if it says.
async fn created(conn: &Connection, path: &str) -> Option<std::time::SystemTime> {
    let secs: u64 = property(conn, SESSIONS, path, SESSIONS, "session_created")
        .await
        .ok()?;
    Some(std::time::UNIX_EPOCH + Duration::from_secs(secs))
}

/// A log line openvpn3 forwards for the session saying the server ended
/// it ("[SESSION_EXPIRED]"): a sign-in that ran out, a time limit.
fn says_expired(msg: &zbus::Message) -> bool {
    if msg.header().member().map(|m| m.as_str()) != Some("Log") {
        return false;
    }
    let body = msg.body();
    // (group, category, message), with the session's token in newer ones.
    let text = body
        .deserialize::<(u32, u32, String, String)>()
        .map(|(_, _, a, b)| a + &b)
        .or_else(|_| body.deserialize::<(u32, u32, String)>().map(|(_, _, m)| m))
        .unwrap_or_default();
    text.contains("SESSION_EXPIRED")
}

/// Why it stopped, when the server ended the session after `lasted`.
fn expired_said(lasted: Option<Duration>) -> String {
    const MINUTE: u64 = 60;
    let after = lasted
        .map(|d| d.as_secs() / MINUTE)
        .filter(|m| *m > 0)
        .map(|m| {
            format!(
                " (this one after {m} minute{})",
                if m == 1 { "" } else { "s" }
            )
        })
        .unwrap_or_default();
    format!(
        "Its session expired: the server ends sessions after a while{after}. Switch it on to sign in again."
    )
}

/// Open a web sign-in page in the browser, once per URL.
fn open_once(url: &str, opened: &mut HashSet<String>) {
    let url = url.trim();
    if (url.starts_with("https://") || url.starts_with("http://")) && opened.insert(url.to_owned())
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// The session's StatusMinor and message, as the session manager has it.
async fn status(conn: &Connection, path: &str) -> Option<(u32, String)> {
    property::<(u32, u32, String)>(conn, SESSIONS, path, SESSIONS, "status")
        .await
        .ok()
        .map(|(_, minor, text)| (minor, text))
}

async fn disconnect(conn: &Connection, path: &str) {
    let _ = call::<_, ()>(conn, SESSIONS, path, SESSIONS, "Disconnect", &()).await;
}

/// Everything the session is waiting for: each question goes to the
/// window, its answer back to openvpn3. A web sign-in opens the browser.
async fn answer(
    conn: &Connection,
    path: &str,
    events: &async_channel::Sender<Event>,
    replies: &async_channel::Receiver<Reply>,
    opened: &mut HashSet<String>,
) -> Result<(), String> {
    let groups: Vec<(u32, u32)> = call(
        conn,
        SESSIONS,
        path,
        SESSIONS,
        "UserInputQueueGetTypeGroup",
        &(),
    )
    .await
    .map_err(|e| e.to_string())?;
    for (kind, group) in groups {
        let ids: Vec<u32> = call(
            conn,
            SESSIONS,
            path,
            SESSIONS,
            "UserInputQueueCheck",
            &(kind, group),
        )
        .await
        .map_err(|e| e.to_string())?;
        for id in ids {
            let (_, _, _, name, description, hidden): (u32, u32, u32, String, String, bool) = call(
                conn,
                SESSIONS,
                path,
                SESSIONS,
                "UserInputQueueFetch",
                &(kind, group, id),
            )
            .await
            .map_err(|e| e.to_string())?;
            if group == OPEN_URL {
                // The description is the URL to sign in at.
                open_once(&description, opened);
                continue;
            }
            let label = if description.is_empty() {
                name.clone()
            } else {
                description.clone()
            };
            let prompt = Prompt {
                ask: if hidden { Ask::Secret } else { Ask::Text },
                label: label.trim_end_matches(':').trim().to_owned(),
                words: None,
            };
            let _ = events.send(Event::Prompt(prompt)).await;
            match replies.recv().await {
                Ok(Reply::Answer { text, .. }) => {
                    call::<_, ()>(
                        conn,
                        SESSIONS,
                        path,
                        SESSIONS,
                        "UserInputProvide",
                        &(kind, group, id, text.as_str()),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                }
                _ => {
                    disconnect(conn, path).await;
                    return Err("Cancelled.".into());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(body: impl serde::Serialize + zbus::zvariant::DynamicType) -> zbus::Message {
        zbus::Message::signal(
            "/net/openvpn/v3/sessions/s1",
            "net.openvpn.v3.backends",
            "Log",
        )
        .unwrap()
        .build(&body)
        .unwrap()
    }

    #[test]
    fn the_server_ending_the_session_is_recognised() {
        // As openvpn3 forwards it, with or without the session's token.
        assert!(says_expired(&log((
            16u32,
            2u32,
            "{tag:1} [SESSION_EXPIRED] "
        ))));
        assert!(says_expired(&log((
            16u32,
            2u32,
            "token",
            "[SESSION_EXPIRED] "
        ))));
        assert!(!says_expired(&log((16u32, 2u32, "Disconnected"))));
        assert_eq!(
            expired_said(Some(Duration::from_secs(58 * 60 + 12))),
            "Its session expired: the server ends sessions after a while (this one after 58 minutes). Switch it on to sign in again."
        );
        assert_eq!(
            expired_said(None),
            "Its session expired: the server ends sessions after a while. Switch it on to sign in again."
        );
    }
}
