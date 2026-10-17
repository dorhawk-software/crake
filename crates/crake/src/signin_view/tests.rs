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
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("openconnect");
    std::fs::write(&path, GATEWAY).unwrap();
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
        protocol: "anyconnect".into(),
        user: String::new(),
        auth_args: vec![],
        extra_args: vec![],
        user_agent: None,
        keyring: false,
        openconnect: Some(openconnect),
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

    w.click("retry", cx);
    assert!(w.signing_in(cx));
    w.until("asked again", cx, |s| asked(s, &groups));
    w.click("cancel", cx);
    assert!(w.closed.get(), "Cancel closes the window");
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
