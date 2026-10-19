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
fn names_not_kept_private_say_why(cx: &mut TestAppContext) {
    let w = open(cx, Section::Names);
    assert!(
        !w.shown("private-trouble", cx),
        "nothing to say while they're kept"
    );
    w.model.update(cx, |m, _| {
        m.settings.insert(
            "keep_private_trouble".into(),
            "Another DNS server on this computer answers on every address.".into(),
        );
    });
    assert!(w.shown("private-trouble", cx), "said under the setting");
    w.model.update(cx, |m, _| {
        m.settings.insert("keep_private".into(), "false".into());
    });
    assert!(
        !w.shown("private-trouble", cx),
        "not when the setting is off"
    );
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
    // The bird row's "?" says what each look of the bird means.
    assert!(!w.shown("bird-legend", cx));
    w.click(tray_row::TITLE, cx);
    assert!(w.shown("bird-legend", cx));
    w.quiet();
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

/// Nothing shows tray icons: the bird's row says why, with what would help
/// under it (here the command, copied); the switch still works.
#[gpui_kit::test]
fn the_bird_row_says_what_would_bring_it_back(cx: &mut TestAppContext) {
    use crate::no_tray::{EXTENSION, Remedy};
    use crate::tray_host::Event;
    let w = open(cx, Section::General);
    assert!(!w.shown("no-tray-switch", cx));
    w.model.update(cx, |m, cx| {
        m.no_tray = Some(Remedy::SwitchOn(EXTENSION.into()));
        cx.notify();
    });
    w.click("no-tray-switch", cx);
    assert!(matches!(
        w.request(),
        Request::NoTray(Event::SwitchOn(id)) if id == EXTENSION
    ));
    w.model.update(cx, |m, cx| {
        m.no_tray = Some(Remedy::Refused(crate::no_tray::Why::Locked));
        cx.notify();
    });
    w.click("no-tray-extensions", cx);
    assert!(matches!(
        w.request(),
        Request::NoTray(Event::OpenExtensions)
    ));
    let command = "sudo apt install gnome-shell-extension-appindicator";
    w.model.update(cx, |m, cx| {
        m.no_tray = Some(Remedy::Install(Some(command.into())));
        cx.notify();
    });
    w.click("no-tray-copy", cx);
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
        Some(command)
    );
    w.quiet();
    let bird = cx.read(|cx| cx.global::<Prefs>().show_bird);
    w.click("bird", cx);
    assert!(matches!(w.request(), Request::ShowBird(on) if on == !bird));
}

/// A "?" only where its card says more than the line under the title
/// (design/proposals/audit-2026-10-17.html, 7).
#[test]
fn a_question_mark_only_where_its_card_says_more() {
    for kept in [
        super::tray_row::TITLE,
        "When Crake routes a host",
        "Wait for a direct answer",
        "When two tunnels answer",
        "Forget hosts unused for",
        "Keep internal names private",
        "Keep the log for",
    ] {
        assert!(super::help(kept).is_some(), "{kept}");
    }
    for dropped in [
        "Start when you log in",
        "Switch on the tunnels you left on",
        "Tell me when there\u{2019}s a new version",
        "Never send through a tunnel",
        "Appearance",
    ] {
        assert!(super::help(dropped).is_none(), "{dropped}");
    }
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
    let w = open(cx, Section::Diagnostics);
    let craked = stand_in(cx);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("crake-report-2026-10-18.txt");
    w.click("report", cx);
    assert!(cx.did_prompt_for_new_path(), "asks where to");
    let chosen = file.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    // craked is being asked: the button waits, as it is (nothing in the
    // row moves), and the box under it says so.
    assert!(matches!(
        w.view.read_with(cx, |s, _| s.report.clone()),
        diagnostics::Report::Saving(_)
    ));
    assert!(w.shown("report-status", cx), "the box under the row");
    assert!(w.shown("report", cx), "the button stays");
    w.click("report", cx);
    assert!(!cx.did_prompt_for_new_path(), "one report at a time");

    craked.send_blocking(gathered()).unwrap();
    cx.run_until_parked();
    let text = std::fs::read_to_string(&file).expect("the report written");
    assert!(text.starts_with("Crake report, "), "{text}");
    assert!(text.contains("\nThis computer\n  The internet: one.one.one.one"));
    assert_eq!(
        w.view.read_with(cx, |s, _| s.report.clone()),
        diagnostics::Report::Saved(file.clone())
    );
    assert!(w.shown("report-said", cx), "said under the row");
    assert!(w.shown("report-open", cx), "with Open, to read it first");
    assert!(w.shown("report-show", cx), "and Show in folder");
    // Still said after looking at another section.
    w.click(("section", 0usize), cx);
    w.click(("section", Section::Diagnostics.position()), cx);
    assert!(w.shown("report-said", cx), "kept until the window closes");
}

#[gpui_kit::test]
fn a_report_that_cant_be_written_says_why(cx: &mut TestAppContext) {
    let w = open(cx, Section::Diagnostics);
    let craked = stand_in(cx);
    let dir = tempfile::tempdir().unwrap();
    let nowhere = dir.path().join("gone").join("crake-report.txt");
    w.click("report", cx);
    cx.simulate_new_path_selection(move |_| Some(nowhere));
    craked.send_blocking(gathered()).unwrap();
    cx.run_until_parked();
    match w.view.read_with(cx, |s, _| s.report.clone()) {
        diagnostics::Report::Failed(why) => assert!(!why.is_empty()),
        other => panic!("{other:?}"),
    }
    assert!(!w.shown("report-show", cx), "nothing to show in a folder");
    assert!(!w.shown("report-open", cx), "nor to open");
}

#[gpui_kit::test]
fn a_report_not_saved_after_all_changes_nothing(cx: &mut TestAppContext) {
    let w = open(cx, Section::Diagnostics);
    let _craked = stand_in(cx);
    w.click("report", cx);
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert_eq!(
        w.view.read_with(cx, |s, _| s.report.clone()),
        diagnostics::Report::None
    );
}
