//! The panel in a headless window, with the design's sample (model::demo):
//! each control asks the app for what it says.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, TestAppContext, WindowOptions};

use super::*;
use crate::prefs::Prefs;

struct Opened {
    window: AnyWindowHandle,
    model: Entity<Model>,
    requests: async_channel::Receiver<Request>,
}

fn open(cx: &mut TestAppContext, prefs: Prefs) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(Prefs {
            scratch: true,
            ..prefs
        });
    });
    let model = cx.new(|_| crate::model::demo());
    let (tx, requests) = async_channel::unbounded();
    let shown = model.clone();
    let (window, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            let motion = Motion::new(shown.read(cx));
            // Downward: the window fits itself (upward asks the app).
            cx.new(|cx| Panel::new(shown, tx, motion, false, window, cx))
        })
        .expect("a window")
    });
    cx.run_until_parked();
    Opened {
        window,
        model,
        requests,
    }
}

impl Opened {
    fn click(&self, id: impl Into<gpui_kit::ElementId>, cx: &mut TestAppContext) -> Request {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
        self.requests.try_recv().expect("a request")
    }
}

#[gpui_kit::test]
fn each_switch_turns_its_own_tunnel(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    let tunnels = w.model.read_with(cx, |m, _| m.tunnels.clone());
    assert!(tunnels.len() >= 2, "the sample has tunnels on and off");
    for (i, t) in tunnels.iter().enumerate() {
        let on = matches!(t.state.as_str(), "connected" | "connecting");
        match w.click(("switch", i), cx) {
            Request::Switch(name, to) => {
                assert_eq!(name, t.name);
                assert_eq!(to, !on, "{} goes the other way", t.name);
            }
            other => panic!("switch {i}: {other:?}"),
        }
    }
}

#[gpui_kit::test]
fn a_decisions_button_does_what_it_says(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    let decisions = w.model.read_with(cx, |m, _| m.decisions(DECISIONS));
    let mut pressed = 0;
    for (i, d) in decisions.iter().enumerate() {
        let Some(action) = d.action.clone() else {
            continue;
        };
        let tunnel = d.entry.tunnel.clone().unwrap();
        let hosts = d.entry.hosts.clone();
        let want = match action {
            Action::Undo => Request::Undo(tunnel, hosts),
            Action::Redo => Request::Redo(tunnel, hosts),
            Action::Send => Request::Send(tunnel, hosts),
        };
        let got = w.click(("act", i), cx);
        assert_eq!(format!("{got:?}"), format!("{want:?}"));
        pressed += 1;
    }
    assert!(pressed > 0, "the sample has a decision to undo");
}

#[gpui_kit::test]
fn the_footer_and_header_open_windows(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    assert!(matches!(w.click("full-log", cx), Request::Log));
    assert!(matches!(w.click("settings", cx), Request::Settings));
}

/// "Copy for helpdesk" puts the summary on the clipboard, without host
/// names when Settings says so.
#[gpui_kit::test]
fn copy_for_helpdesk(cx: &mut TestAppContext) {
    for hide in [false, true] {
        let w = open(
            cx,
            Prefs {
                hide_hosts: hide,
                ..Prefs::default()
            },
        );
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("helpdesk", cx);
        })
        .unwrap();
        let copied = cx
            .read_from_clipboard()
            .and_then(|c| c.text())
            .expect("text on the clipboard");
        let want = w.model.read_with(cx, |m, _| m.helpdesk(hide));
        assert_eq!(copied, want);
        assert!(w.model.read_with(cx, |m, _| m.copied_at.is_some()));
    }
}

/// The AnyConnect notice under a tunnel, and its "Got it".
#[gpui_kit::test]
fn the_anyconnect_notice_can_be_dismissed(cx: &mut TestAppContext) {
    let model = crate::model::demo();
    let (i, t) = model
        .tunnels
        .iter()
        .enumerate()
        .find(|(_, t)| !t.gateway.is_empty())
        .expect("a tunnel with a gateway");
    let prefs = Prefs {
        user_agents: [(t.gateway.clone(), crate::signin::ANYCONNECT.into())].into(),
        ..Prefs::default()
    };
    let w = open(cx, prefs);
    match w.click(("got-it", i), cx) {
        Request::Dismiss(key) => assert_eq!(key, format!("anyconnect:{}", t.name)),
        other => panic!("{other:?}"),
    }
}

/// The "?" after "What Crake did" opens its card; a click (after the hover
/// that opened it) keeps it open instead of closing it.
#[gpui_kit::test]
fn what_crake_did_explains_itself(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    let card_shown = |cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find("what-crake-did-card").is_some()
        })
        .unwrap()
    };
    assert!(!card_shown(cx));
    for _ in 0..2 {
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("what-crake-did-help", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(card_shown(cx), "open, and it stays open");
    }
    assert!(
        w.requests.try_recv().is_err(),
        "it asks the app for nothing"
    );
}
