//! The log in a headless window, with the design's sample
//! (model::demo_log): its filters show what the model says they should,
//! and Copy for helpdesk copies what's shown.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, ElementId, TestAppContext, WindowOptions};

use super::*;
use crate::model::Show;

struct Opened {
    window: AnyWindowHandle,
    view: Entity<Log>,
    model: Entity<Model>,
}

fn open(cx: &mut TestAppContext) -> Opened {
    open_with(cx, crate::model::demo_log())
}

fn open_with(cx: &mut TestAppContext, sample: Model) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::prefs::Prefs {
            scratch: true,
            ..Default::default()
        });
    });
    let model = cx.new(|_| sample);
    let shown = model.clone();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Log::new(shown, window, cx))
        })
        .expect("a window")
    });
    cx.run_until_parked();
    Opened {
        window,
        view,
        model,
    }
}

/// Days as titles and row texts, to compare.
fn texts(days: &[Day]) -> Vec<(String, Vec<String>)> {
    days.iter()
        .map(|d| {
            (
                d.title.clone(),
                d.rows.iter().map(|r| r.text.clone()).collect(),
            )
        })
        .collect()
}

impl Opened {
    fn click(&self, id: impl Into<ElementId>, cx: &mut TestAppContext) {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn shown(&self, cx: &mut TestAppContext) -> Vec<(String, Vec<String>)> {
        self.view.update(cx, |log, cx| texts(&log.days(cx)))
    }

    fn expected(
        &self,
        cx: &mut TestAppContext,
        show: Show,
        tunnel: Option<&str>,
        search: &str,
    ) -> Vec<(String, Vec<String>)> {
        self.model
            .read_with(cx, |m, _| texts(&m.log_days(show, tunnel, search)))
    }
}

#[gpui_kit::test]
fn each_filter_shows_its_kind(cx: &mut TestAppContext) {
    let w = open(cx);
    let everything = w.expected(cx, Show::Everything, None, "");
    assert!(everything.len() >= 2, "the sample spans days");
    assert_eq!(w.shown(cx), everything);
    for (i, show) in Show::ALL.into_iter().enumerate() {
        w.click(("show", i), cx);
        assert_eq!(w.view.read_with(cx, |l, _| l.show), show);
        assert_eq!(w.shown(cx), w.expected(cx, show, None, ""), "{show:?}");
    }
}

#[gpui_kit::test]
fn one_tunnel_and_a_search(cx: &mut TestAppContext) {
    let w = open(cx);
    let first = w.model.read_with(cx, |m, _| m.tunnels[0].name.clone());
    w.click("tunnel", cx);
    w.click(("tunnel-option", 1usize), cx);
    assert_eq!(
        w.view.read_with(cx, |l, _| l.tunnel.clone()),
        Some(first.clone())
    );
    let only = w.expected(cx, Show::Everything, Some(&first), "");
    assert_eq!(w.shown(cx), only);
    assert_ne!(
        only,
        w.expected(cx, Show::Everything, None, ""),
        "it narrows"
    );

    // Back to all, then search.
    w.click("tunnel", cx);
    w.click(("tunnel-option", 0usize), cx);
    assert_eq!(w.view.read_with(cx, |l, _| l.tunnel.clone()), None);
    let input = w.view.read_with(cx, |l, _| l.search.clone());
    cx.update_window(w.window, |_, window, cx| {
        input.update(cx, |f, cx| f.focus(window, cx));
        window.render_frame(cx);
        window.input("git", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let found = w.expected(cx, Show::Everything, None, "git");
    assert_eq!(w.shown(cx), found);
    assert!(
        found.iter().any(|(_, rows)| !rows.is_empty()),
        "the sample has a git host"
    );
}

/// One earlier day at first; the rest on request.
#[gpui_kit::test]
fn earlier_days_on_request(cx: &mut TestAppContext) {
    // The sample, and the same first entry three and four days ago.
    let mut sample = crate::model::demo_log();
    let first = sample.log[0].clone();
    for days in [4, 3] {
        let at = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * days);
        let mut e = first.clone();
        e.ts = at.to_string();
        sample.log.insert(0, e);
    }
    let w = open_with(cx, sample);
    assert!(w.shown(cx).len() > 2, "today and more than one earlier day");
    assert!(!w.view.read_with(cx, |l, _| l.earlier_days));
    // At the end of the list: scrolled to, as a user would.
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        let down = gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-4000.)));
        window.scroll("days", down, cx);
    })
    .unwrap();
    cx.run_until_parked();
    w.click("earlier", cx);
    assert!(w.view.read_with(cx, |l, _| l.earlier_days));
}

#[gpui_kit::test]
fn copy_for_helpdesk_and_close(cx: &mut TestAppContext) {
    let w = open(cx);
    w.click("helpdesk", cx);
    let copied = cx
        .read_from_clipboard()
        .and_then(|c| c.text())
        .expect("text on the clipboard");
    let want = w.view.update(cx, |log, cx| log.as_text(cx));
    // The first line is the time it was copied.
    let body = |s: &str| s.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert_eq!(body(&copied), body(&want));
    assert!(copied.starts_with("Crake log, "));

    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = closed.clone();
    cx.update(|cx| {
        cx.subscribe(&w.view, move |_, _: &Close, _| flag.set(true))
            .detach()
    });
    w.click("close", cx);
    assert!(closed.get());
}
