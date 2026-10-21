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
    assert!(w.shows("release-0.14.0", cx));
    assert!(
        !w.shows("release-0.13.1", cx),
        "seen already: behind the button"
    );
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

/// From About: this version, no "since".
#[gpui_kit::test]
fn from_about_it_opens_at_this_version(cx: &mut TestAppContext) {
    let w = open(cx, None);
    assert!(!w.shows("since", cx));
    assert!(w.shows("earlier", cx));
    assert!(!w.shows("release-0.13.1", cx));
}
