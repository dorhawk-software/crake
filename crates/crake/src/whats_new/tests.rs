//! When the panel speaks after an update, what the window shows, and the
//! window itself in a headless test window.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, WindowOptions};

use super::changelog::parse;
use super::view::{Close, WhatsNew};
use super::*;

/// Releases enough to tell the rules apart: one with only what's
/// underneath, one with three changes, one with one, the first in prose.
const SAMPLE: &str = "# Changelog\n\n## Unreleased\n\n## 0.4.0 (2026-04-01)\n\n### Underneath\n\n\
    - **Libraries updated.** Nothing you'd notice.\n\n## 0.3.0 (2026-03-01)\n\n### New\n\n\
    - **A.** a\n- **B.** b\n\n### Fixed\n\n- **C.** c\n\n## 0.2.0 (2026-02-01)\n\n### Fixed\n\n\
    - **D.** d\n\n## 0.1.0 (2026-01-01)\n\nThe first release.\n";

#[test]
fn the_panel_tells_what_changed_since_the_version_seen() {
    let r = parse(SAMPLE);
    assert_eq!(
        notice_in(&r, "0.3.0", Some("0.1.0")).as_deref(),
        Some("Crake is now 0.3.0: 4 changes since\u{a0}0.1.0.")
    );
    assert_eq!(
        notice_in(&r, "0.2.0", Some("0.1.0")).as_deref(),
        Some("Crake is now 0.2.0: 1 change since\u{a0}0.1.0.")
    );
    // No version kept (an update from before Crake kept it): this one's.
    assert_eq!(
        notice_in(&r, "0.3.0", None).as_deref(),
        Some("Crake is now 0.3.0.")
    );
    // Seen already, or a version back: nothing.
    assert_eq!(notice_in(&r, "0.3.0", Some("0.3.0")), None);
    assert_eq!(notice_in(&r, "0.3.0", Some("0.4.0")), None);
    // Only what's underneath changed: nothing to tell.
    assert_eq!(notice_in(&r, "0.4.0", Some("0.3.0")), None);
    assert_eq!(notice_in(&r, "0.4.0", None), None);
    // ... but counted with the rest it's left out of the number.
    assert_eq!(
        notice_in(&r, "0.4.0", Some("0.2.0")).as_deref(),
        Some("Crake is now 0.4.0: 3 changes since\u{a0}0.2.0.")
    );
}

#[test]
fn the_window_shows_the_versions_since_then_and_the_rest_on_asking() {
    let r = parse(SAMPLE);
    let versions =
        |v: &[&changelog::Release]| v.iter().map(|r| r.version.clone()).collect::<Vec<_>>();
    let s = shown_in(&r, "0.3.0", Some("0.1.0"));
    assert_eq!(s.since.as_deref(), Some("0.1.0"));
    assert_eq!(versions(&s.full), ["0.3.0", "0.2.0"]);
    assert_eq!(versions(&s.earlier), ["0.1.0"]);
    // From About: this version, the rest behind "Show earlier versions".
    let s = shown_in(&r, "0.3.0", None);
    assert_eq!(s.since, None);
    assert_eq!(versions(&s.full), ["0.3.0"]);
    assert_eq!(versions(&s.earlier), ["0.2.0", "0.1.0"]);
    // Many versions skipped: three in full.
    let s = shown_in(&r, "0.4.0", Some("0.0.1"));
    assert_eq!(versions(&s.full), ["0.4.0", "0.3.0", "0.2.0"]);
    assert_eq!(versions(&s.earlier), ["0.1.0"]);
    // Nothing newer than what was seen: this version, no "since".
    let s = shown_in(&r, "0.3.0", Some("0.3.0"));
    assert_eq!(s.since, None);
    assert_eq!(versions(&s.full), ["0.3.0"]);
}

#[test]
fn seen_keeps_the_highest_version_and_a_fresh_install_has_seen_this_one() {
    let this = versions::this();
    let mut p = Prefs {
        scratch: true,
        ..Default::default()
    };
    // A fresh install: nothing to tell.
    first_start(&mut p);
    assert_eq!(p.seen_version.as_deref(), Some(this));
    assert_eq!(notice(&p), None);
    // app.json without a version seen: an update, told.
    let mut p = Prefs {
        scratch: true,
        existed: true,
        ..Default::default()
    };
    first_start(&mut p);
    assert_eq!(p.seen_version, None);
    mark_seen(&mut p);
    assert_eq!(p.seen_version.as_deref(), Some(this));
    // A version back keeps the newer one seen.
    p.seen_version = Some("99.0.0".into());
    mark_seen(&mut p);
    assert_eq!(p.seen_version.as_deref(), Some("99.0.0"));
}

/// The newest released version's id in the window: always shown in full,
/// where an older one may be behind "Show earlier versions" (three in
/// full: an Unreleased with changes in it is one of them).
fn newest_release() -> String {
    let newest = changelog::releases()
        .iter()
        .find(|r| r.version != changelog::UNRELEASED)
        .unwrap();
    format!("release-{}", newest.version)
}

/// The first version in the window, at its top: Unreleased in a
/// development build with changes in it (a long one put the newest release
/// below the fold, 2026-10-29), the newest release otherwise.
fn top_release() -> String {
    let top = changelog::releases()
        .iter()
        .find(|r| r.groups.iter().any(|g| !g.changes.is_empty()))
        .unwrap();
    format!("release-{}", top.version)
}

struct Opened {
    window: AnyWindowHandle,
    view: Entity<WhatsNew>,
}

fn open(cx: &mut TestAppContext, since: Option<&str>) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::chrome::Chrome {
            system_buttons: false,
        });
    });
    let since = since.map(str::to_owned);
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| WhatsNew::new(since, window, cx))
        })
        .expect("a window")
    });
    cx.run_until_parked();
    Opened { window, view }
}

impl Opened {
    fn shows(&self, id: &str, cx: &mut TestAppContext) -> bool {
        let id = gpui_kit::ElementId::Name(id.to_owned().into());
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    }

    fn click(&self, id: &str, cx: &mut TestAppContext) {
        let id = gpui_kit::ElementId::Name(id.to_owned().into());
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
}

/// From the panel's notice after an update from 0.13.1: since then, the
/// rest on asking; Escape closes it.
#[gpui_kit::test]
fn the_window_shows_what_changed_since_the_version_you_had(cx: &mut TestAppContext) {
    let w = open(cx, Some("0.13.1"));
    assert!(w.shows("whats-new-title", cx));
    assert!(w.shows("since", cx), "Since 0.13.1, the version you had");
    assert!(w.shows(&newest_release(), cx));
    assert!(
        !w.shows("release-0.13.1", cx),
        "seen already: behind the button"
    );
    // The button is at the end: End scrolls there.
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        window.press("end", cx);
    })
    .unwrap();
    cx.run_until_parked();
    w.click("earlier", cx);
    assert!(w.shows("release-0.13.1", cx));
    assert!(
        w.shows("release-0.1.0", cx),
        "every version, the first included"
    );
    assert!(!w.shows("earlier", cx), "nothing left to show");

    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = closed.clone();
    cx.update(|cx| {
        cx.subscribe(&w.view, move |_, _: &Close, _| seen.set(true))
            .detach()
    });
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(closed.get(), "Escape closes it");
}

/// ⌘W closes a window on a Mac, as Escape does; elsewhere it's nothing
/// of Crake's.
#[gpui_kit::test]
fn command_w_closes_it_on_a_mac(cx: &mut TestAppContext) {
    let w = open(cx, None);
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = closed.clone();
    cx.update(|cx| {
        cx.subscribe(&w.view, move |_, _: &Close, _| seen.set(true))
            .detach()
    });
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        window.press("cmd-w", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(closed.get(), cfg!(target_os = "macos"));
}

/// From About: this version, no "since".
#[gpui_kit::test]
fn from_about_it_opens_at_this_version(cx: &mut TestAppContext) {
    let w = open(cx, None);
    assert!(!w.shows("since", cx));
    assert!(w.shows("earlier", cx));
    assert!(!w.shows("release-0.13.1", cx));
}

/// Pointing at the versions shows their scrollbar (hover), and moves
/// nothing: the window stays at its top.
#[gpui_kit::test]
fn pointing_at_the_versions_moves_nothing(cx: &mut TestAppContext) {
    let w = open(cx, Some("0.13.1"));
    cx.update(|cx| crate::theme::Theme::dark().sync_components(cx));
    let top = |cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window
                .find(gpui_kit::ElementId::Name(newest_release().into()))
                .bounds()
                .origin
                .y
        })
        .unwrap()
    };
    let before = top(cx);
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        window.hover("versions", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(top(cx), before);
}

/// The bar is there while the pointer is over the versions, and gone a
/// moment after it leaves (not while it's elsewhere, as gpui-kit's own
/// hover had it: the user, 2026-10-21).
#[gpui_kit::test]
fn the_scrollbar_follows_the_pointer_over_what_scrolls(cx: &mut TestAppContext) {
    let w = open(cx, Some("0.13.1"));
    assert!(!w.shows("versions-bar", cx), "not pointed at: no bar");
    let point = |id: &'static str, cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.hover(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    point("versions", cx);
    assert!(w.shows("versions-bar", cx), "pointed at: the bar");
    // Moving on over what scrolls, the bar shown over it: it stays (the
    // bar's own layer once took the pointer away, and it flickered). The
    // version at the top: a later one may be below the fold.
    point(Box::leak(top_release().into_boxed_str()), cx);
    assert!(
        w.shows("versions-bar", cx),
        "still pointed at: still the bar"
    );
    point("whats-new-title", cx);
    assert!(w.shows("versions-bar", cx), "held a moment after leaving");
    // The hold and the fade, then the bar goes.
    std::thread::sleep(std::time::Duration::from_millis(700));
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(700));
    cx.run_until_parked();
    assert!(!w.shows("versions-bar", cx), "gone once faded");
}
