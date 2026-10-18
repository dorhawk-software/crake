//! OpenVPN tunnels' sign-in (CLAUDE.md, 7i): craked runs openvpn and asks
//! (D-Bus `Asking`, `Update::Asking`); the sign-in window shows the
//! question, the browser opens a sign-in page, and the answer goes back
//! (`Answer`). Switched on, an OpenVPN tunnel just connects: the window
//! opens only when craked asks (a kept token asks nothing).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use gpui_kit::App;

use crate::Request;
use crate::signin::{Ask, Event, Prompt, Reply};

/// The sign-in windows taking OpenVPN tunnels' questions, by tunnel: where
/// a question goes, and the id of the one it shows.
#[derive(Default)]
pub struct Asks {
    windows: HashMap<String, (async_channel::Sender<Event>, Arc<AtomicU32>)>,
    /// The last page opened in the browser, by tunnel (its question's id).
    opened: HashMap<String, u32>,
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
    let shown = Arc::new(AtomicU32::new(0));
    asks.windows
        .insert(tunnel.clone(), (events, Arc::clone(&shown)));
    let (replies, incoming) = async_channel::unbounded::<Reply>();
    std::thread::spawn(move || {
        while let Ok(reply) = incoming.recv_blocking() {
            let request = match reply {
                Reply::Answer { text, .. } => {
                    Request::Answer(tunnel.clone(), shown.load(Ordering::Relaxed), text)
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

/// craked asks this (`Update::Asking`): shown in the tunnel's sign-in
/// window, opened if it isn't; a page opens in the browser, once.
pub fn asked(tunnel: &str, ask: &HashMap<String, String>, cx: &mut App) {
    let Some(id) = ask.get("id").and_then(|i| i.parse::<u32>().ok()) else {
        return;
    };
    let open = |cx: &mut App| {
        cx.default_global::<Asks>()
            .windows
            .get(tunnel)
            .is_some_and(|(events, _)| !events.is_closed())
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
    let Some((events, shown)) = asks.windows.get(tunnel).cloned() else {
        return;
    };
    shown.store(id, Ordering::Relaxed);
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
        _ => return None,
    };
    Some(Prompt {
        ask,
        label: label.to_owned(),
        words,
        refused,
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
        // A page isn't a question in the window.
        assert!(prompt(&ask(&[("kind", "browser"), ("url", "https://a.example/")])).is_none());
    }
}
