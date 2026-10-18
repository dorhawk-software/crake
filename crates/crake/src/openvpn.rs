//! OpenVPN tunnels' sign-in (CLAUDE.md, 7i): craked runs openvpn and asks
//! (D-Bus `Asking`, `Update::Asking`); the sign-in window shows the
//! question, the browser opens a sign-in page, and the answer goes back
//! (`Answer`). Switched on, an OpenVPN tunnel just connects: the window
//! opens only when craked asks (a kept token asks nothing).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use gpui_kit::App;

use crate::signin::{Ask, Event, Prompt, Reply};
use crate::{Request, Shell};

/// The sign-in windows taking OpenVPN tunnels' questions, by tunnel: where
/// a question goes, and the id of the one it shows.
#[derive(Default)]
pub struct Asks {
    windows: HashMap<String, (async_channel::Sender<Event>, Arc<AtomicU32>)>,
    /// The last page opened in the browser, by tunnel (its question's id).
    opened: HashMap<String, u32>,
}

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
    let open = |cx: &App| {
        cx.global::<Shell>()
            .openvpn
            .windows
            .get(tunnel)
            .is_some_and(|(events, _)| !events.is_closed())
    };
    if !open(cx) {
        let found = cx
            .global::<Shell>()
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
    let shell = cx.global_mut::<Shell>();
    let Some((events, shown)) = shell.openvpn.windows.get(tunnel).cloned() else {
        return;
    };
    shown.store(id, Ordering::Relaxed);
    if kind == "browser" {
        let fresh = shell.openvpn.opened.insert(tunnel.to_owned(), id) != Some(id);
        if fresh && (url.starts_with("https://") || url.starts_with("http://")) {
            cx.open_url(&url);
        }
        let _ = events.send_blocking(Event::Reached);
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
    })
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
