//! The log in a headless window, with the design's sample
//! (model::demo_log_whole_day): its filters show what the model says they should,
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
    open_with(cx, crate::model::demo_log_whole_day())
}

fn open_with(cx: &mut TestAppContext, sample: Model) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::chrome::Chrome {
            system_buttons: false,
        });
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
    let mut sample = crate::model::demo_log_whole_day();
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

/// With macOS's own window buttons (top left): no × of Crake's, and the
/// title starts after them.
#[gpui_kit::test]
fn with_the_systems_window_buttons(cx: &mut TestAppContext) {
    let w = open(cx);
    cx.update(|cx| {
        cx.set_global(crate::chrome::Chrome {
            system_buttons: true,
        })
    });
    let (close, title) = cx
        .update_window(w.window, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            (window.try_find("close"), window.find("log-title").bounds())
        })
        .unwrap();
    assert!(close.is_none(), "Crake's own × is there too");
    assert!(
        f32::from(title.origin.x) >= crate::chrome::BUTTONS_WIDTH,
        "\"Log\" starts at {:.0} px, under the system's buttons",
        f32::from(title.origin.x)
    );
}

/// craked for Save a report…, as a test has it: what it says arrives when
/// the test sends it.
fn stand_in(cx: &mut TestAppContext) -> async_channel::Sender<crate::report::Gathered> {
    let (tx, rx) = async_channel::unbounded();
    cx.update(|cx| {
        cx.set_global(crate::report::Gatherer(std::rc::Rc::new(move || {
            rx.clone()
        })));
    });
    tx
}

fn gathered() -> crate::report::Gathered {
    crate::report::Gathered {
        service: String::new(),
        tunnels: vec!["ACME: off".into()],
        decisions: Vec::new(),
        computer: Ok(crake_proto::report::Computer {
            checks: vec!["The internet: one.one.one.one is 1.1.1.1".into()],
            ..Default::default()
        }),
    }
}

#[gpui_kit::test]
fn save_a_report_says_where_it_went(cx: &mut TestAppContext) {
    let w = open(cx);
    let craked = stand_in(cx);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("crake-report-2026-10-18.txt");
    w.click("report", cx);
    assert!(cx.did_prompt_for_new_path(), "asks where to");
    let chosen = file.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    // craked is being asked: the button waits.
    assert_eq!(
        w.view.read_with(cx, |l, _| l.report.clone()),
        Report::Saving
    );
    w.click("report", cx);
    assert!(!cx.did_prompt_for_new_path(), "one report at a time");

    craked.send_blocking(gathered()).unwrap();
    cx.run_until_parked();
    let text = std::fs::read_to_string(&file).expect("the report written");
    assert!(text.starts_with("Crake report, "), "{text}");
    assert!(text.contains("\nThis computer\n  The internet: one.one.one.one"));
    assert_eq!(
        w.view.read_with(cx, |l, _| l.report.clone()),
        Report::Saved(file.clone())
    );
    let row = cx
        .update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            (
                window.try_find("report-said").is_some(),
                window.try_find("report-show").is_some(),
            )
        })
        .unwrap();
    assert_eq!(
        row,
        (true, true),
        "said under the title, with Show in folder"
    );
}

#[gpui_kit::test]
fn a_report_that_cant_be_written_says_why(cx: &mut TestAppContext) {
    let w = open(cx);
    let craked = stand_in(cx);
    let dir = tempfile::tempdir().unwrap();
    let nowhere = dir.path().join("gone").join("crake-report.txt");
    w.click("report", cx);
    cx.simulate_new_path_selection(move |_| Some(nowhere));
    craked.send_blocking(gathered()).unwrap();
    cx.run_until_parked();
    match w.view.read_with(cx, |l, _| l.report.clone()) {
        Report::Failed(why) => assert!(!why.is_empty()),
        other => panic!("{other:?}"),
    }
    let show = cx
        .update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find("report-show").is_some()
        })
        .unwrap();
    assert!(!show, "nothing to show in a folder");
}

#[gpui_kit::test]
fn a_report_not_saved_after_all_changes_nothing(cx: &mut TestAppContext) {
    let w = open(cx);
    let _craked = stand_in(cx);
    w.click("report", cx);
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert_eq!(w.view.read_with(cx, |l, _| l.report.clone()), Report::None);
}
