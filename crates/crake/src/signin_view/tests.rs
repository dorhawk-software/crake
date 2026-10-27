//! Sign-in in a headless window against a fake openconnect (a shell script
//! that asks what a gateway asks): the questions show, the answers reach
//! it, and a session goes to the app.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, ElementId, TestAppContext, WindowOptions};

use super::*;

/// A gateway that wants a group, then a password ("hunter2"); what it was
/// told goes to `answers` next to it.
const GATEWAY: &str = r#"#!/bin/sh
here=$(dirname "$0")
echo "Connected to 203.0.113.10:443" >&2
echo "Please choose your group." >&2
printf 'GROUP: [Employees|Contractors]:' >&2
read group
echo "group=$group" >> "$here/answers"
echo "Please enter your username and password." >&2
printf 'Password:' >&2
read pw
if [ "$pw" = "hunter2" ]; then
    echo "COOKIE='c00kie'"
    echo "CONNECT_URL='https://vpn.acme.example/'"
    exit 0
fi
echo "Login failed." >&2
echo "Failed to complete authentication" >&2
exit 1
"#;

fn fake(dir: &Path) -> PathBuf {
    fake_with(dir, GATEWAY)
}

fn fake_with(dir: &Path, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("openconnect");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

struct Opened {
    window: AnyWindowHandle,
    view: Entity<SignIn>,
    model: Entity<Model>,
    requests: async_channel::Receiver<Request>,
    closed: std::rc::Rc<std::cell::Cell<bool>>,
}

fn open(cx: &mut TestAppContext, openconnect: PathBuf) -> Opened {
    open_with(cx, openconnect, "anyconnect")
}

fn open_with(cx: &mut TestAppContext, openconnect: PathBuf, protocol: &str) -> Opened {
    // Real I/O: openconnect's answers arrive from the sign-in thread.
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let model = cx.new(|_| {
        let mut m = crate::model::demo();
        for t in &mut m.tunnels {
            t.state = "off".into();
        }
        m
    });
    let (tx, requests) = async_channel::unbounded();
    let spec = Spec {
        tunnel: "acme".into(),
        display: "ACME".into(),
        gateway: "vpn.acme.example".into(),
        protocol: protocol.into(),
        user: String::new(),
        auth_args: vec![],
        extra_args: vec![],
        user_agent: None,
        keyring: false,
        openconnect: Some(openconnect),
        trusted: None,
    };
    let shown = model.clone();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| SignIn::new(spec, shown, tx, window, cx))
        })
        .expect("a window")
    });
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = closed.clone();
    cx.update(|cx| {
        cx.subscribe(&view, move |_, _: &Close, _| flag.set(true))
            .detach()
    });
    Opened {
        window,
        view,
        model,
        requests,
        closed,
    }
}

impl Opened {
    /// The sign-in runs openconnect on its own thread: let its events in
    /// until `done` holds (5 s at most).
    fn until(&self, what: &str, cx: &mut TestAppContext, done: impl Fn(&Stage) -> bool) {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            if self.view.read_with(cx, |s, _| done(&s.stage)) {
                return;
            }
            let stage = self.view.read_with(cx, |s, _| s.stage.clone());
            assert!(Instant::now() < end, "never saw {what}; at {stage:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn click(&self, id: impl Into<ElementId>, cx: &mut TestAppContext) {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn type_answer(&self, text: &str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.input(text, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn signing_in(&self, cx: &mut TestAppContext) -> bool {
        self.model
            .read_with(cx, |m, _| m.signing_in.contains("acme"))
    }
}

/// OpenVPN (design/proposals/openvpn-tunnel.html, 6): craked's questions,
/// shown as craked asks them (`openvpn::asked`): a page opened in the
/// browser, with "Open the page again"; after a sign-in the gateway refused,
/// the username asked again, saying so; the answer goes back to craked with
/// its question's id.
#[gpui_kit::test]
fn openvpns_questions_come_from_craked(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let w = open_with(cx, fake(dir.path()), "openvpn");
    let ask = |pairs: &[(&str, &str)]| -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    };
    let page = ask(&[
        ("id", "7"),
        ("kind", "browser"),
        ("url", "https://vpn.acme.example/sso"),
    ]);
    cx.update(|cx| crate::openvpn::asked("acme", &page, cx));
    w.until(
        "the page",
        cx,
        |s| matches!(s, Stage::Browser { url, .. } if url == "https://vpn.acme.example/sso"),
    );
    w.click("open-again", cx);

    let user = ask(&[("id", "8"), ("kind", "user"), ("retry", "true")]);
    cx.update(|cx| crate::openvpn::asked("acme", &user, cx));
    w.until(
        "the username, again",
        cx,
        |s| matches!(s, Stage::Asking(p) if p.refused && p.label == "Username"),
    );
    w.type_answer("alice", cx);
    w.click("continue", cx);
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        match w.requests.try_recv() {
            Ok(Request::Answer(tunnel, id, text)) => {
                assert_eq!((tunnel.as_str(), id, text.as_str()), ("acme", 8, "alice"));
                break;
            }
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < end, "the answer never went to craked");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    // Up: the window says so and closes.
    w.model.update(cx, |m, cx| {
        if let Some(t) = m.tunnels.iter_mut().find(|t| t.name == "acme") {
            t.state = "connected".into();
        }
        cx.notify();
    });
    w.until("up", cx, |s| *s == Stage::Up);
}

fn asked(stage: &Stage, ask: &Ask) -> bool {
    matches!(stage, Stage::Asking(p) if p.ask == *ask)
}

#[gpui_kit::test]
fn a_group_and_a_password_give_a_session(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let w = open(cx, fake(dir.path()));
    assert!(w.signing_in(cx), "connecting, for the tray and panel");

    let groups = Ask::Choice(vec!["Employees".into(), "Contractors".into()]);
    w.until("the group question", cx, |s| asked(s, &groups));
    w.click(("choice", 1usize), cx);

    w.until("the password question", cx, |s| asked(s, &Ask::Secret));
    // It says the password is kept, ticked; unticked, it isn't
    // (design/proposals/ipsec-tunnel.html, 5).
    assert!(
        w.view.read_with(cx, |s, _| s.remember),
        "ticked as it starts"
    );
    w.click("remember", cx);
    assert!(!w.view.read_with(cx, |s, _| s.remember), "unticked");
    w.click("remember", cx);
    w.type_answer("hunter2", cx);
    w.click("continue", cx);

    w.until("signed in", cx, |s| *s == Stage::Connecting);
    match w.requests.try_recv() {
        Ok(Request::ConnectWithSession(tunnel, session)) => {
            assert_eq!(tunnel, "acme");
            assert_eq!(session.cookie, "c00kie");
            assert_eq!(session.connect_url, "https://vpn.acme.example/");
            assert_eq!(session.user_agent, None);
        }
        other => panic!("expected the session, got {other:?}"),
    }
    let answers = std::fs::read_to_string(dir.path().join("answers")).unwrap();
    assert_eq!(answers, "group=Contractors\n");

    // craked brings it up: the window says so.
    w.model.update(cx, |m, cx| {
        if let Some(t) = m.tunnels.iter_mut().find(|t| t.name == "acme") {
            t.state = "connected".into();
        }
        cx.notify();
    });
    w.until("up", cx, |s| *s == Stage::Up);
}

/// A wrong password says why; Try again asks again; Cancel closes.
#[gpui_kit::test]
fn a_wrong_password_can_be_tried_again(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let w = open(cx, fake(dir.path()));
    let groups = Ask::Choice(vec!["Employees".into(), "Contractors".into()]);
    w.until("the group question", cx, |s| asked(s, &groups));
    w.click(("choice", 0usize), cx);
    w.until("the password question", cx, |s| asked(s, &Ask::Secret));
    w.type_answer("letmein", cx);
    w.click("continue", cx);

    w.until("the failure", cx, |s| matches!(s, Stage::Failed { .. }));
    let reason = w.view.read_with(cx, |s, _| match &s.stage {
        Stage::Failed { reason, .. } => reason.clone(),
        _ => unreachable!(),
    });
    assert!(!reason.is_empty());
    assert!(!w.signing_in(cx), "not connecting any more");
    assert!(w.requests.try_recv().is_err(), "no session");

    // Copy for helpdesk: what happened, for someone else to read.
    w.click("helpdesk", cx);
    let copied = cx
        .read_from_clipboard()
        .and_then(|c| c.text())
        .unwrap_or_default();
    assert!(copied.contains("Couldn\u{2019}t sign in to"), "{copied}");
    assert!(copied.contains(&reason), "{copied}");
    // And it says so, as every Copy does (it used to say nothing).
    assert!(
        w.view.read_with(cx, |s, _| s.copied.is_some()),
        "Copied is shown after Copy for helpdesk"
    );

    w.click("retry", cx);
    assert!(w.signing_in(cx));
    w.until("asked again", cx, |s| asked(s, &groups));
    w.click("cancel", cx);
    assert!(w.closed.get(), "Cancel closes the window");
}

/// openconnect with a certificate whose password is "hunter2": a wrong one
/// and it asks again, as openconnect does.
const CERTIFICATE: &str = r#"#!/bin/sh
echo "Connected to 203.0.113.10:443" >&2
while :; do
    printf 'Enter PKCS#12 pass phrase:' >&2
    read pw
    [ "$pw" = "hunter2" ] && break
    echo "Failed to decrypt PKCS#12 certificate file" >&2
done
echo "COOKIE='c00kie'"
echo "CONNECT_URL='https://vpn.acme.example/'"
"#;

/// A typed certificate password that doesn't open it says so when it's
/// asked again (it came back as the same question, 2026-10-26).
#[gpui_kit::test]
fn a_certificate_password_that_doesnt_open_it_says_so(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let w = open(cx, fake_with(dir.path(), CERTIFICATE));
    let first = Ask::CertificatePassword {
        from_file: None,
        retry: false,
        kept: false,
    };
    w.until("the certificate's password", cx, |s| asked(s, &first));
    w.type_answer("letmein", cx);
    w.click("continue", cx);
    let again = Ask::CertificatePassword {
        from_file: None,
        retry: true,
        kept: false,
    };
    w.until("asked again, saying why", cx, |s| asked(s, &again));
    w.type_answer("hunter2", cx);
    w.click("continue", cx);
    w.until("signed in", cx, |s| *s == Stage::Connecting);
}

/// A gateway's field name in Crake's words; one it doesn't know keeps
/// the gateway's word (design/proposals/audit-2026-10-17.html, 8).
#[test]
fn a_gateways_field_is_asked_for_in_words() {
    use super::widgets::asks_for;
    assert_eq!(asks_for("GROUP"), Some("your group"));
    assert_eq!(asks_for("Group:"), Some("your group"));
    assert_eq!(asks_for("Password"), Some("your password"));
    assert_eq!(asks_for("OTP"), Some("a code"));
    assert_eq!(asks_for("Second password"), Some("your second password"));
    assert_eq!(asks_for("PIN"), Some("your PIN"));
    assert_eq!(asks_for("Realm"), None);
}

/// IPsec (design/proposals/ipsec-tunnel.html): a key the gateway turns
/// down ends in the failure shape with Change the key…, not Try again,
/// and that opens the tunnel's Edit and closes the window.
#[gpui_kit::test]
fn a_refused_key_offers_to_change_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let w = open_with(cx, fake(dir.path()), "ipsec");
    w.model.update(cx, |m, cx| {
        if let Some(t) = m.tunnels.iter_mut().find(|t| t.name == "acme") {
            t.state = "trouble".into();
            t.why = format!(
                "{} ACME\u{2019}s gateway. Your IT team can tell you the right one; change it in ACME\u{2019}s Edit.",
                crake_proto::trouble::KEY_MISMATCH
            );
        }
        cx.notify();
    });
    w.until("the failure", cx, |s| matches!(s, Stage::Failed { .. }));
    w.click("change-key", cx);
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        match w.requests.try_recv() {
            Ok(Request::NewTunnel(Some(tunnel))) => {
                assert_eq!(tunnel, "acme");
                break;
            }
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < end, "Edit was never asked for");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    assert!(w.closed.get(), "the window closes");
}

/// IPsec's trust question (design/proposals/ipsec-certificates.html, 3),
/// asked by craked: whose certificate it is and who signed it; changed
/// since it was trusted, it says so and the keyboard starts on "Don't
/// connect"; "Trust it" goes back to craked with the question's id.
#[gpui_kit::test]
fn ipsecs_trust_question_comes_from_craked(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let w = open_with(cx, fake(dir.path()), "ipsec");
    let ask: std::collections::HashMap<String, String> = [
        ("id", "9"),
        ("kind", "trust"),
        ("fingerprint", "pin-sha256:AbC="),
        ("for", "gw.nordbank.example"),
        ("issuer", "Nordbank Root CA"),
        ("trusted_on", "2020-10-21"),
        ("retry", "true"),
    ]
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();
    cx.update(|cx| crate::openvpn::asked("acme", &ask, cx));
    w.until("the trust question", cx, |s| {
        matches!(s, Stage::Asking(p) if matches!(&p.ask, Ask::TrustServer { about: Some(a), trusted_on: Some(d), .. }
            if a.contains("gw.nordbank.example") && a.contains("Nordbank Root CA") && d == "21 October 2020"))
    });
    let starts_on_dont_connect = cx
        .update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            w.view.read(cx).dont_connect.is_focused(window)
        })
        .unwrap();
    assert!(
        starts_on_dont_connect,
        "a changed certificate: a second look"
    );
    w.click("yes", cx);
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        match w.requests.try_recv() {
            Ok(Request::Answer(tunnel, id, text)) => {
                assert_eq!((tunnel.as_str(), id, text.as_str()), ("acme", 9, "yes"));
                break;
            }
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < end, "the answer never went to craked");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// The window asks for all the card holds with its padding, however much
/// of it shows, and never for more than its screen's room.
#[test]
fn the_window_asks_for_the_whole_card() {
    use super::{CARD_BOTTOM, CARD_TOP, wanted_height};
    let edges = crate::chrome::CARD_BORDERS;
    assert_eq!(
        wanted_height(226., 900.),
        226. + CARD_TOP + CARD_BOTTOM + edges,
        "the padding counted once"
    );
    assert_eq!(
        wanted_height(1000., 700.),
        700. + edges,
        "a screen's room at most"
    );
}

/// A window shorter than its card (one the window manager didn't grow):
/// the card stays inside it and scrolls, its buttons still there. (Not
/// the cause of 2026-10-21's half-cut Cancel after a restart: there the
/// card was drawn for a taller window than the one on screen.)
#[gpui_kit::test]
fn a_window_left_short_keeps_its_card_inside(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let w = open_with(cx, fake(dir.path()), "openvpn");
    let page: std::collections::HashMap<String, String> = [
        ("id", "7"),
        ("kind", "browser"),
        ("url", "https://vpn.acme.example/sso"),
    ]
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();
    cx.update(|cx| crate::openvpn::asked("acme", &page, cx));
    w.until("the page", cx, |s| matches!(s, Stage::Browser { .. }));
    // Shorter than the card; the test platform never grants the resize.
    cx.simulate_window_resize(
        w.window,
        gpui_kit::size(gpui_kit::px(WIDTH), gpui_kit::px(220.)),
    );
    let (card, viewport, cancel) = cx
        .update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.render_frame(cx);
            (
                window.find("sign-in-card").bounds(),
                window.viewport_size(),
                window.try_find("cancel").is_some(),
            )
        })
        .unwrap();
    assert!(
        card.bottom() <= viewport.height,
        "the card ends at {:?}, the window at {:?}",
        card.bottom(),
        viewport.height
    );
    assert!(cancel, "Cancel is still there, a scroll away");
}
