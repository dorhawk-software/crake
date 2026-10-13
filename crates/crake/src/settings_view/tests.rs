//! Settings in a headless window, with the design's sample (model::demo):
//! every section renders, and its controls ask for what they say.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, ElementId, TestAppContext, WindowOptions};

use super::*;

struct Opened {
    window: AnyWindowHandle,
    view: Entity<Settings>,
    model: Entity<Model>,
    requests: async_channel::Receiver<Request>,
}

fn open(cx: &mut TestAppContext, section: Section) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::chrome::Chrome {
            system_buttons: false,
        });
        cx.set_global(Prefs {
            scratch: true,
            ..Prefs::default()
        });
    });
    let model = cx.new(|_| crate::model::demo());
    let (tx, requests) = async_channel::unbounded();
    let shown = model.clone();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Settings::new(shown, tx, section, window, cx))
        })
        .expect("a window")
    });
    cx.run_until_parked();
    Opened {
        window,
        view,
        model,
        requests,
    }
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

    fn shown(&self, id: impl Into<ElementId>, cx: &mut TestAppContext) -> bool {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    }

    fn request(&self) -> Request {
        self.requests.try_recv().expect("a request")
    }

    /// The setting a control asked craked to change.
    fn setting(&self) -> (String, String) {
        match self.request() {
            Request::Setting(key, value) => (key, value),
            other => panic!("expected a setting, got {other:?}"),
        }
    }

    fn quiet(&self) {
        if let Ok(r) = self.requests.try_recv() {
            panic!("nothing should be asked, got {r:?}");
        }
    }
}

fn pair(key: &str, value: &str) -> (String, String) {
    (key.to_owned(), value.to_owned())
}

/// The navigation reaches every section, and each renders.
#[gpui_kit::test]
fn every_section_opens(cx: &mut TestAppContext) {
    let w = open(cx, Section::Tunnels);
    for (i, section) in Section::ALL.into_iter().enumerate() {
        w.click(("section", i), cx);
        assert_eq!(w.view.read_with(cx, |s, _| s.section), section);
    }
    w.quiet();
}

#[gpui_kit::test]
fn learning(cx: &mut TestAppContext) {
    let w = open(cx, Section::Learning);
    // Wait for a direct answer: 1 s.
    w.click(("wait", 0usize), cx);
    assert_eq!(w.setting(), pair("probe_wait_secs", "1"));

    // When two tunnels answer: the faster one.
    w.click("both", cx);
    w.click(("both", 1usize), cx);
    assert_eq!(w.setting(), pair("both", "faster"));

    // The never list: remove the first, then add one.
    let never = w.model.read_with(cx, |m, _| m.setting_list("never"));
    assert_eq!(never.len(), 2, "the sample's two");
    w.click(("remove", 0usize), cx);
    assert_eq!(w.setting(), pair("never", &never[1]));

    w.click("add-never", cx);
    let input = w.view.read_with(cx, |s, _| s.never_input.clone());
    cx.update_window(w.window, |_, window, cx| {
        input.update(cx, |f, cx| f.focus(window, cx));
        window.render_frame(cx);
        window.input("10.9.0.0/16", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let (key, value) = w.setting();
    assert_eq!(key, "never");
    assert_eq!(value, format!("{}\n10.9.0.0/16", never.join("\n")));
}

#[gpui_kit::test]
fn names(cx: &mut TestAppContext) {
    let w = open(cx, Section::Names);
    let private = w
        .model
        .read_with(cx, |m, _| m.setting("keep_private") == "true");
    w.click("private", cx);
    assert_eq!(w.setting(), pair("keep_private", &(!private).to_string()));
    let ask = w
        .model
        .read_with(cx, |m, _| m.setting("ask_unknown") == "true");
    w.click("ask-unknown", cx);
    assert_eq!(w.setting(), pair("ask_unknown", &(!ask).to_string()));
}

/// Clearing the log asks first; "Keep it" keeps it.
#[gpui_kit::test]
fn privacy(cx: &mut TestAppContext) {
    let w = open(cx, Section::Privacy);
    w.click("clear-log", cx);
    w.click("cancel-clear", cx);
    w.quiet();
    assert!(w.shown("clear-log", cx));

    w.click("clear-log", cx);
    w.click("clear", cx);
    assert!(matches!(w.request(), Request::ClearLog));

    w.click("keep", cx);
    w.click(("keep", 3usize), cx);
    assert_eq!(w.setting(), pair("log_days", "never"));

    // The app's own preference: kept in Prefs, craked isn't asked.
    let before = cx.read(|cx| cx.global::<Prefs>().hide_hosts);
    w.click("hide-hosts", cx);
    assert_eq!(cx.read(|cx| cx.global::<Prefs>().hide_hosts), !before);
    w.quiet();
}

#[gpui_kit::test]
fn general(cx: &mut TestAppContext) {
    let w = open(cx, Section::General);
    let bird = cx.read(|cx| cx.global::<Prefs>().show_bird);
    w.click("bird", cx);
    assert!(matches!(w.request(), Request::ShowBird(on) if on == !bird));
}

/// A tunnel added in the app can be edited and removed (after asking);
/// New tunnel opens empty.
#[gpui_kit::test]
fn tunnels(cx: &mut TestAppContext) {
    let w = open(cx, Section::Tunnels);
    let own: Vec<String> = w.model.read_with(cx, |m, _| {
        m.tunnels
            .iter()
            .filter(|t| t.source == "app")
            .map(|t| t.name.clone())
            .collect()
    });
    let at = (0..8usize)
        .find(|i| w.shown(("edit", *i), cx))
        .expect("an Edit button");
    w.click(("edit", at), cx);
    let Request::NewTunnel(Some(name)) = w.request() else {
        panic!("Edit opens New tunnel on the tunnel");
    };
    assert!(own.contains(&name), "{name} was added in the app");

    w.click(("remove", at), cx);
    w.quiet();
    w.click("keep-tunnel", cx);
    w.quiet();
    w.click(("remove", at), cx);
    w.click("remove-tunnel", cx);
    assert!(matches!(w.request(), Request::RemoveTunnel(n) if n == name));

    w.click("new-tunnel", cx);
    assert!(matches!(w.request(), Request::NewTunnel(None)));

    // Each tunnel's switch, as in the panel: on asks to switch it off, off
    // to switch it on.
    let states: Vec<(String, bool)> = w.model.read_with(cx, |m, _| {
        m.tunnels
            .iter()
            .map(|t| {
                (
                    t.name.clone(),
                    matches!(t.state.as_str(), "connected" | "connecting"),
                )
            })
            .collect()
    });
    for (name, on) in states {
        w.click(ElementId::Name(format!("switch-{name}").into()), cx);
        assert!(
            matches!(w.request(), Request::Switch(n, to) if n == name && to == !on),
            "{name}'s switch"
        );
    }
}

#[gpui_kit::test]
fn about_offers_a_coffee(cx: &mut TestAppContext) {
    let w = open(cx, Section::About);
    assert!(w.shown("coffee", cx), "About has the coffee card's button");
}

/// With macOS's own window buttons (over the sidebar's top): no × of
/// Crake's, and the sidebar's heading starts below them.
#[gpui_kit::test]
fn with_the_systems_window_buttons(cx: &mut TestAppContext) {
    let w = open(cx, Section::Tunnels);
    cx.update(|cx| {
        cx.set_global(crate::chrome::Chrome {
            system_buttons: true,
        })
    });
    let (close, first) = cx
        .update_window(w.window, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            (window.try_find("close"), window.find("nav-title").bounds())
        })
        .unwrap();
    assert!(close.is_none(), "Crake's own × is there too");
    assert!(
        f32::from(first.origin.y) >= crate::chrome::HEADER_HEIGHT,
        "The heading starts at {:.0} px, under the system's buttons",
        f32::from(first.origin.y)
    );
}
