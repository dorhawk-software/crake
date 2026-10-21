//! OpenVPN tunnels' sign-in (CLAUDE.md, 7i): craked runs openvpn and asks
//! (D-Bus `Asking`, `Update::Asking`); the sign-in window shows the
//! question, the browser opens a sign-in page, and the answer goes back
//! (`Answer`). Switched on, an OpenVPN tunnel just connects: the window
//! opens only when craked asks (a kept token asks nothing), and not for a
//! username or password kept from a sign-in that worked (`kept`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui_kit::App;

use crate::Request;
use crate::signin::{Ask, Event, Prompt, Reply};

mod kept;

/// The question a sign-in window shows: craked's id for it, and its kind.
type Shown = Arc<Mutex<(u32, String)>>;

/// A sign-in window taking a tunnel's questions: where they go, what it
/// shows, and where its answers go.
struct Window {
    events: async_channel::Sender<Event>,
    shown: Shown,
    requests: async_channel::Sender<Request>,
}

/// The sign-in windows taking OpenVPN tunnels' questions, by tunnel.
#[derive(Default)]
pub struct Asks {
    windows: HashMap<String, Window>,
    /// The last page opened in the browser, by tunnel (its question's id).
    opened: HashMap<String, u32>,
    /// What the user typed, by tunnel, kept once the tunnel is up.
    typed: Arc<Mutex<HashMap<String, kept::Typed>>>,
}

impl gpui_kit::Global for Asks {}

/// A sign-in window's side: its questions arrive through `asked`, its
/// answers go to craked. Returns where the window sends its replies.
pub fn start(
    tunnel: String,
    events: async_channel::Sender<Event>,
    requests: async_channel::Sender<Request>,
    asks: &mut Asks,
) -> async_channel::Sender<Reply> {
    let shown: Shown = Arc::default();
    asks.windows.insert(
        tunnel.clone(),
        Window {
            events,
            shown: Arc::clone(&shown),
            requests: requests.clone(),
        },
    );
    let typed = Arc::clone(&asks.typed);
    let (replies, incoming) = async_channel::unbounded::<Reply>();
    std::thread::spawn(move || {
        while let Ok(reply) = incoming.recv_blocking() {
            let request = match reply {
                Reply::Answer { text, remember, .. } => {
                    let (id, kind) = shown.lock().unwrap().clone();
                    let mut typed = typed.lock().unwrap();
                    let entry = typed.entry(tunnel.clone()).or_default();
                    match kind.as_str() {
                        "user" => entry.user = Some(text.clone()),
                        // Kept only when "Remember the password in your
                        // keyring" is (OpenVPN's and IPsec's alike).
                        "password" => entry.password = remember.then(|| text.clone()),
                        _ => {}
                    }
                    Request::Answer(tunnel.clone(), id, text)
                }
                // Cancelled: the tunnel goes off, its sign-in with it.
                Reply::Cancel => Request::Switch(tunnel.clone(), false),
            };
            if requests.send_blocking(request).is_err() {
                return;
            }
        }
    });
    replies
}

/// craked asks this (`Update::Asking`): a username or password kept from
/// a sign-in that worked is given at once; anything else is shown in the
/// tunnel's sign-in window, opened if it isn't; a page opens in the
/// browser, once.
pub fn asked(tunnel: &str, ask: &HashMap<String, String>, cx: &mut App) {
    let Some(id) = ask.get("id").and_then(|i| i.parse::<u32>().ok()) else {
        return;
    };
    let kind = ask.get("kind").map(String::as_str).unwrap_or("");
    // Demo mode keeps nothing (CRAKE_DEMO never touches the keyring).
    let demo = cx.try_global::<crate::Shell>().is_some_and(|s| s.demo);
    if let Some(k) = kept::kind(kind).filter(|_| !demo) {
        if ask.get("retry").map(String::as_str) == Some("true") {
            // Refused: what was kept, and what was typed, goes.
            cx.default_global::<Asks>()
                .typed
                .lock()
                .unwrap()
                .remove(tunnel);
            let t = tunnel.to_owned();
            cx.background_executor()
                .spawn(async move { kept::forget(&t) })
                .detach();
        } else {
            let (tunnel, ask) = (tunnel.to_owned(), ask.clone());
            cx.spawn(async move |cx| {
                let t = tunnel.clone();
                let found = cx
                    .background_executor()
                    .spawn(async move { kept::read(&t, k) })
                    .await;
                cx.update(|cx| match found {
                    Some(text) => answer(&tunnel, id, text, cx),
                    None => show(&tunnel, id, &ask, cx),
                })
            })
            .detach();
            return;
        }
    }
    show(tunnel, id, ask, cx);
}

/// A kept answer, to craked, as the window would have sent it.
fn answer(tunnel: &str, id: u32, text: String, cx: &mut App) {
    let request = Request::Answer(tunnel.to_owned(), id, text);
    let window = cx
        .default_global::<Asks>()
        .windows
        .get(tunnel)
        .filter(|w| !w.events.is_closed())
        .map(|w| w.requests.clone());
    match window {
        Some(requests) => {
            let _ = requests.send_blocking(request);
        }
        None => {
            if let Some(shell) = cx.try_global::<crate::Shell>() {
                let _ = shell.requests.send_blocking(request);
            }
        }
    }
}

/// Up: what the user typed for its sign-in is kept.
pub fn up(cx: &mut App) {
    let Some(shell) = cx.try_global::<crate::Shell>() else {
        return;
    };
    if shell.demo {
        return;
    }
    let up: Vec<(String, String)> = shell
        .model
        .read(cx)
        .tunnels
        .iter()
        .filter(|t| t.state == "connected")
        .map(|t| (t.name.clone(), t.display.clone()))
        .collect();
    let typed = Arc::clone(&cx.default_global::<Asks>().typed);
    for (tunnel, display) in up {
        let Some(t) = typed.lock().unwrap().remove(&tunnel) else {
            continue;
        };
        cx.background_executor()
            .spawn(async move { kept::write(&tunnel, &display, &t) })
            .detach();
    }
}

/// Removed: what was kept for it goes (with its other keyring items).
pub fn forget(tunnel: &str) {
    kept::forget(tunnel);
}

/// The question in the tunnel's sign-in window.
fn show(tunnel: &str, id: u32, ask: &HashMap<String, String>, cx: &mut App) {
    let open = |cx: &mut App| {
        cx.default_global::<Asks>()
            .windows
            .get(tunnel)
            .is_some_and(|w| !w.events.is_closed())
    };
    if !open(cx) {
        let found = cx
            .global::<crate::Shell>()
            .model
            .read(cx)
            .tunnels
            .iter()
            .find(|t| t.name == tunnel)
            .cloned();
        let Some(found) = found else {
            return;
        };
        crate::windows::open_sign_in(found, cx);
    }
    let kind = ask.get("kind").map(String::as_str).unwrap_or("");
    let url = ask.get("url").cloned().unwrap_or_default();
    let asks = cx.default_global::<Asks>();
    let Some((events, shown)) = asks
        .windows
        .get(tunnel)
        .map(|w| (w.events.clone(), Arc::clone(&w.shown)))
    else {
        return;
    };
    *shown.lock().unwrap() = (id, kind.to_owned());
    if kind == "browser" {
        let fresh = asks.opened.insert(tunnel.to_owned(), id) != Some(id);
        if fresh && (url.starts_with("https://") || url.starts_with("http://")) {
            cx.open_url(&url);
        }
        let _ = events.send_blocking(Event::Browser(url));
        return;
    }
    if let Some(prompt) = prompt(ask) {
        let _ = events.send_blocking(Event::Prompt(prompt));
    }
}

/// craked's question as the sign-in window asks it.
pub fn prompt(ask: &HashMap<String, String>) -> Option<Prompt> {
    let get = |k: &str| ask.get(k).map(String::as_str).unwrap_or("");
    let words = Some(get("label").to_owned()).filter(|w| !w.is_empty());
    let retry = get("retry") == "true";
    let refused = retry && get("kind") != "key-password";
    let (ask, label) = match get("kind") {
        "user" => (Ask::Text, "Username"),
        "password" => (Ask::Secret, "Password"),
        "key-password" => (
            Ask::CertificatePassword {
                from_file: None,
                retry,
            },
            "Password",
        ),
        "code" => (Ask::Secret, "Code"),
        "code-visible" => (Ask::Text, "Code"),
        // IPsec: a gateway's certificate no authority here vouches for.
        "trust" => {
            let some = |k: &str| Some(get(k).to_owned()).filter(|v| !v.is_empty());
            let about = some("for").map(|name| match some("issuer") {
                Some(issuer) => format!("For {name}, signed by \u{201c}{issuer}\u{201d}."),
                None => format!("For {name}."),
            });
            let trusted_on = some("trusted_on").and_then(|d| said_day(&d));
            let ask = Ask::TrustServer {
                fingerprint: some("fingerprint"),
                about,
                trusted_on,
            };
            return Some(Prompt {
                ask,
                label: String::new(),
                words: None,
                refused: false,
            });
        }
        _ => return None,
    };
    Some(Prompt {
        ask,
        label: label.to_owned(),
        words,
        refused,
    })
}

/// A day craked gives ("2026-10-21") as the window says it: "21 October",
/// with its year when it isn't this one's.
fn said_day(iso: &str) -> Option<String> {
    let day: jiff::civil::Date = iso.parse().ok()?;
    let this_year = jiff::Zoned::now().year();
    let month = day.strftime("%B");
    Some(if day.year() == this_year {
        format!("{} {month}", day.day())
    } else {
        format!("{} {month} {}", day.day(), day.year())
    })
}

/// A tunnel whose profile OpenVPN 3 keeps locked: the panel's notice
/// ("{}" is the tunnel's name).
pub const LOCKED_TITLE: &str = "{}\u{2019}s profile is locked in OpenVPN 3";
pub const LOCKED_WHY: &str = "Crake can\u{2019}t take it over, so {} stays on OpenVPN 3 for now. Import its .ovpn file to move it.";

/// Why an OpenVPN 3 profile can't be taken over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotTaken {
    /// OpenVPN 3 keeps its text locked (`locked_down`).
    Locked,
    /// No profile by that name now.
    Missing,
    /// Crake can't run it (what it says, of the profile's name).
    Refused(String),
    /// openvpn3 couldn't be asked.
    Unreadable(String),
}

/// An OpenVPN 3 profile, read (as the user, who owns it) and checked as a
/// file's would be: its text and the gateway it names. Blocking: off the
/// UI thread. Tests never touch this computer's openvpn3 (their profiles:
/// globex is locked, acme and initech can be taken over).
pub fn from_openvpn3(name: &str) -> Result<(String, String), NotTaken> {
    use crate::ovpn3::profile::Fetched;
    let fetched = if cfg!(test) {
        match name {
            "globex" => Fetched::Locked,
            "acme" | "initech" => Fetched::Text(format!(
                "client\nremote vpn.{name}.example 1194\n<ca>\nx\n</ca>\n"
            )),
            _ => Fetched::Missing,
        }
    } else {
        crate::ovpn3::profile::fetch(name).map_err(NotTaken::Unreadable)?
    };
    let text = match fetched {
        Fetched::Text(text) => text,
        Fetched::Locked => return Err(NotTaken::Locked),
        Fetched::Missing => return Err(NotTaken::Missing),
    };
    let profile =
        crake_proto::openvpn_profile::check(&text).map_err(|r| NotTaken::Refused(r.said(name)))?;
    Ok((text, profile.remote))
}

/// How often the app looks for OpenVPN 3 tunnels to move.
const MOVE_LOOK: std::time::Duration = std::time::Duration::from_secs(3);

/// Crake's OpenVPN 3 tunnels move to its own OpenVPN (7i, decided with the
/// user 2026-10-18): each one added in the app, once it's off, gets its
/// profile copied out of OpenVPN 3 (which keeps its own) and runs on Crake's
/// OpenVPN from then on (craked says so in the log). One whose profile is
/// locked, or that Crake can't run, stays on OpenVPN 3, and the panel says
/// what to do (`model::Staying`).
///
/// Only where openvpn is installed: a tunnel moved to an engine that isn't
/// there would stop working (it stays on OpenVPN 3 until it is).
pub fn move_from_openvpn3(cx: &mut App) {
    cx.spawn(async move |cx| {
        let mut tried = std::collections::HashSet::new();
        loop {
            cx.background_executor().timer(MOVE_LOOK).await;
            let installed = cx
                .background_executor()
                .spawn(async { crate::engines::openvpn_installed() })
                .await;
            if !installed {
                continue;
            }
            let off: Vec<crate::client::Tunnel> = cx.update(|cx| {
                let model = cx.global::<crate::Shell>().model.clone();
                model.update(cx, |m, cx| {
                    // Moved meanwhile (imported by hand): nothing to say.
                    let before = m.staying.len();
                    let still: Vec<String> = m
                        .tunnels
                        .iter()
                        .filter(|t| t.protocol == "openvpn3")
                        .map(|t| t.name.clone())
                        .collect();
                    m.staying.retain(|s| still.contains(&s.tunnel));
                    if m.staying.len() != before {
                        cx.notify();
                    }
                    m.tunnels
                        .iter()
                        .filter(|t| t.protocol == "openvpn3" && !t.own.is_empty())
                        .filter(|t| t.state == "off" && !m.signing_in.contains(&t.name))
                        .filter(|t| !tried.contains(&t.name))
                        .cloned()
                        .collect()
                })
            });
            for t in off {
                tried.insert(t.name.clone());
                let profile = t.gateway.clone();
                let taken = cx
                    .background_executor()
                    .spawn(async move { from_openvpn3(&profile) })
                    .await;
                cx.update(|cx| moved(&t, taken, cx));
            }
        }
    })
    .detach();
}

/// One OpenVPN 3 tunnel looked at: moved (craked takes the profile), or
/// said to stay.
fn moved(t: &crate::client::Tunnel, taken: Result<(String, String), NotTaken>, cx: &mut App) {
    let shell = cx.global::<crate::Shell>();
    let (commands, model) = (shell.commands.clone(), shell.model.clone());
    let (title, why) = match taken {
        Ok((text, gateway)) => {
            let mut fields = t.own.clone();
            fields.insert("protocol".into(), "openvpn".into());
            fields.insert("gateway".into(), gateway);
            fields.insert("profile".into(), text);
            commands
                .send_blocking(crate::client::Command::UpdateTunnel(t.name.clone(), fields))
                .ok();
            return;
        }
        Err(NotTaken::Locked) => (
            LOCKED_TITLE.replace("{}", &t.display),
            LOCKED_WHY.replace("{}", &t.display),
        ),
        Err(NotTaken::Refused(said)) => (
            format!("{} stays on OpenVPN 3", t.display),
            format!("{said} Import a .ovpn file Crake can run to move it."),
        ),
        // Gone, or openvpn3 not answering: tried again at the next start.
        Err(NotTaken::Missing | NotTaken::Unreadable(_)) => return,
    };
    model.update(cx, |m, cx| {
        if !m.staying.iter().any(|s| s.tunnel == t.name) {
            m.staying.push(crate::model::Staying {
                tunnel: t.name.clone(),
                title,
                why,
            });
            cx.notify();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// The next request from the window's side, or what's there now.
    fn next(rx: &async_channel::Receiver<Request>, cx: &mut gpui_kit::TestAppContext) -> Request {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            if let Ok(r) = rx.try_recv() {
                return r;
            }
            assert!(std::time::Instant::now() < end, "nothing went to craked");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[gpui_kit::test]
    fn a_kept_username_and_password_ask_nothing_until_refused(cx: &mut gpui_kit::TestAppContext) {
        cx.executor().allow_parking();
        let tunnel = "kept-test";
        kept::write(
            tunnel,
            "Kept",
            &kept::Typed {
                user: Some("alice".into()),
                password: Some("hunter2".into()),
            },
        );
        let (events, shown) = async_channel::unbounded();
        let (requests, to_craked) = async_channel::unbounded();
        let replies = cx.update(|cx| {
            let asks = cx.default_global::<Asks>();
            start(tunnel.into(), events, requests, asks)
        });
        // Asked again: answered from what was kept, the window shows nothing.
        for (id, kind, kept) in [("1", "user", "alice"), ("2", "password", "hunter2")] {
            cx.update(|cx| asked(tunnel, &ask(&[("id", id), ("kind", kind)]), cx));
            match next(&to_craked, cx) {
                Request::Answer(t, i, text) => {
                    assert_eq!(
                        (t.as_str(), i.to_string(), text.as_str()),
                        (tunnel, id.into(), kept)
                    );
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(shown.try_recv().is_err(), "the window asked nothing");
        // Refused: forgotten, and the window asks.
        cx.update(|cx| {
            asked(
                tunnel,
                &ask(&[("id", "3"), ("kind", "password"), ("retry", "true")]),
                cx,
            )
        });
        cx.run_until_parked();
        let Ok(Event::Prompt(p)) = shown.try_recv() else {
            panic!("the window wasn't asked");
        };
        assert!(p.refused, "it says the password wasn't accepted");
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while kept::read(tunnel, crate::keyring::Kind::Password).is_some() {
            assert!(
                std::time::Instant::now() < end,
                "the refused password stayed"
            );
            cx.run_until_parked();
        }
        assert_eq!(kept::read(tunnel, crate::keyring::Kind::Username), None);
        // What the user types is noted, to be kept once the tunnel is up
        // ("Remember the password in your keyring", ticked as it starts).
        replies
            .send_blocking(Reply::Answer {
                text: "hunter3".into(),
                remember: true,
                keep_certificate: false,
            })
            .unwrap();
        assert!(matches!(next(&to_craked, cx), Request::Answer(_, 3, ref t) if t == "hunter3"));
        let typed = cx.update(|cx| {
            cx.default_global::<Asks>()
                .typed
                .lock()
                .unwrap()
                .get(tunnel)
                .cloned()
        });
        assert_eq!(
            typed,
            Some(kept::Typed {
                user: None,
                password: Some("hunter3".into())
            })
        );
    }

    #[test]
    fn crakeds_questions_as_the_window_asks_them() {
        let p = prompt(&ask(&[("id", "3"), ("kind", "password")])).unwrap();
        assert_eq!(
            (p.ask, p.label.as_str(), p.words),
            (Ask::Secret, "Password", None)
        );
        // A code, in the gateway's own words.
        let p = prompt(&ask(&[
            ("kind", "code-visible"),
            ("label", "Code we texted you"),
        ]))
        .unwrap();
        assert_eq!(p.ask, Ask::Text);
        assert_eq!(p.words.as_deref(), Some("Code we texted you"));
        let p = prompt(&ask(&[("kind", "key-password"), ("retry", "true")])).unwrap();
        assert_eq!(
            p.ask,
            Ask::CertificatePassword {
                from_file: None,
                retry: true
            }
        );
        // IPsec's trust question: whose certificate, who signed it, and
        // the day the one it replaces was trusted.
        let p = prompt(&ask(&[
            ("kind", "trust"),
            ("fingerprint", "pin-sha256:AbC="),
            ("for", "gw.nordbank.example"),
            ("issuer", "Nordbank Root CA"),
            ("trusted_on", "2020-10-21"),
        ]))
        .unwrap();
        assert_eq!(
            p.ask,
            Ask::TrustServer {
                fingerprint: Some("pin-sha256:AbC=".into()),
                about: Some(
                    "For gw.nordbank.example, signed by \u{201c}Nordbank Root CA\u{201d}.".into()
                ),
                trusted_on: Some("21 October 2020".into()),
            }
        );
        let p = prompt(&ask(&[("kind", "trust"), ("for", "gw.nordbank.example")])).unwrap();
        assert!(
            matches!(p.ask, Ask::TrustServer { about: Some(a), trusted_on: None, .. } if a == "For gw.nordbank.example.")
        );
        // A page isn't a question in the window.
        assert!(prompt(&ask(&[("kind", "browser"), ("url", "https://a.example/")])).is_none());
    }
}
