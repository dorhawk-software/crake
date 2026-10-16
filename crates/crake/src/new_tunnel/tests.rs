//! New tunnel in a headless window: real clicks and typing through the
//! steps, checked against what "Add and connect" asks the app to do.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, TestAppContext, WindowOptions};

use super::*;

struct Opened {
    window: AnyWindowHandle,
    view: Entity<NewTunnel>,
    requests: async_channel::Receiver<Request>,
}

fn open(cx: &mut TestAppContext) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::chrome::Chrome {
            system_buttons: false,
        });
    });
    let model = cx.new(|_| Model::default());
    let (tx, requests) = async_channel::unbounded();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| NewTunnel::new(model, tx, None, window, cx))
        })
        .expect("a window")
    });
    // The profiles and engines (look_around's fixed pair) arrive.
    cx.run_until_parked();
    Opened {
        window,
        view,
        requests,
    }
}

/// Opened as the app opens it (windows.rs): the view itself, no Root, so
/// no key binding of Root's moves the focus on Tab.
fn open_as_the_app(cx: &mut TestAppContext) -> Opened {
    cx.update(gpui_kit::init);
    let model = cx.new(|_| Model::default());
    let (tx, requests) = async_channel::unbounded();
    let (window, view) = cx.update(|cx| {
        let mut view = None;
        let window = cx
            .open_window(WindowOptions::default(), |window, cx| {
                let v = cx.new(|cx| NewTunnel::new(model, tx, None, window, cx));
                view = Some(v.clone());
                v
            })
            .expect("a window");
        (window.into(), view.expect("the view"))
    });
    cx.run_until_parked();
    Opened {
        window,
        view,
        requests,
    }
}

impl Opened {
    fn click(&self, id: impl Into<gpui_kit::ElementId>, cx: &mut TestAppContext) {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    /// Type into the field `field` picks (it gets the focus first).
    fn type_into(
        &self,
        field: impl Fn(&NewTunnel) -> Entity<InputState>,
        text: &str,
        cx: &mut TestAppContext,
    ) {
        let input = self.view.read_with(cx, |this, _| field(this));
        cx.update_window(self.window, |_, window, cx| {
            input.update(cx, |f, cx| f.focus(window, cx));
            window.render_frame(cx);
            window.input(text, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn read<T>(&self, cx: &mut TestAppContext, f: impl Fn(&NewTunnel) -> T) -> T {
        self.view.read_with(cx, |this, _| f(this))
    }

    fn shown(&self, id: &'static str, cx: &mut TestAppContext) -> bool {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    }

    /// The request "Add and connect" sent.
    fn added(&self) -> HashMap<String, String> {
        match self.requests.try_recv() {
            Ok(Request::AddTunnel(fields, None)) => fields,
            other => panic!("expected AddTunnel, got {other:?}"),
        }
    }
}

const OPENVPN3: usize = 8;
const ANYCONNECT: usize = 1;

/// The OpenVPN 3 path, where two bugs were: the chosen profile jumped back
/// to "Detect automatically", and the routing step had no Continue.
#[gpui_kit::test]
fn an_openvpn3_profile_gets_added(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Initech", cx);
    w.click(("protocol", OPENVPN3), cx);
    assert_eq!(w.read(cx, |t| t.protocol), "openvpn3");
    w.click("profile", cx);
    // The last of three: its menu row lies over the protocol list, where a
    // click once went through and picked "Detect automatically".
    w.click(("profile", 2usize), cx);
    assert_eq!(
        w.read(cx, |t| t.profile.clone()).as_deref(),
        Some("initech")
    );
    assert_eq!(w.read(cx, |t| t.protocol), "openvpn3", "the choice stays");

    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 3);
    assert!(w.shown("next", cx), "routing has its button");
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["display_name"], "Initech");
    assert_eq!(fields["protocol"], "openvpn3");
    assert_eq!(
        fields["gateway"], "initech",
        "an OpenVPN 3 tunnel's gateway is its profile"
    );
    assert_eq!(fields["group"], "");
    assert_eq!(fields["mode"], "learn");
}

#[gpui_kit::test]
fn a_gateway_with_a_password_and_ask_me_first(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "ACME", cx);
    w.click(("protocol", ANYCONNECT), cx);
    w.type_into(|t| t.gateway.clone(), "vpn.acme.example", cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2);

    w.click(("method", 1usize), cx);
    w.type_into(|t| t.user.clone(), "jane", cx);
    w.click("next", cx);
    w.click(("mode", 1usize), cx);
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["display_name"], "ACME");
    assert_eq!(fields["protocol"], "anyconnect");
    assert_eq!(fields["gateway"], "vpn.acme.example");
    assert_eq!(fields["sign_in"], "password");
    assert_eq!(fields["user"], "jane");
    assert_eq!(fields["mode"], "ask");
}

#[gpui_kit::test]
fn only_what_i_list_keeps_the_list(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Globex", cx);
    w.click(("protocol", ANYCONNECT), cx);
    w.type_into(|t| t.gateway.clone(), "vpn.globex.example", cx);
    w.click("next", cx);
    w.click("next", cx);
    w.click(("mode", 2usize), cx);
    w.click("add-listed", cx);
    w.type_into(|t| t.list_input.clone(), "*.globex.internal", cx);
    cx.update_window(w.window, |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(w.read(cx, |t| t.list.clone()), ["*.globex.internal"]);
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["mode"], "list");
    assert_eq!(fields["list"], "*.globex.internal");
}

/// Continue on an incomplete step stays there and says what's missing.
#[gpui_kit::test]
fn an_incomplete_step_says_what_is_missing(cx: &mut TestAppContext) {
    let w = open(cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 1);
    assert_eq!(
        w.read(cx, |t| t.problem.clone()).as_deref(),
        Some("Give it a name.")
    );

    w.type_into(|t| t.name.clone(), "Initech", cx);
    w.click(("protocol", OPENVPN3), cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 1);
    assert_eq!(
        w.read(cx, |t| t.problem.clone()).as_deref(),
        Some("Which OpenVPN 3 profile?")
    );

    w.click(("protocol", ANYCONNECT), cx);
    w.click("next", cx);
    assert_eq!(
        w.read(cx, |t| t.problem.clone()).as_deref(),
        Some("Which gateway? Its name or address, like vpn.example.com.")
    );
    assert!(w.requests.try_recv().is_err(), "nothing was added");
}

/// openconnect 8 (Ubuntu 22.04) can't sign in through the browser: that
/// card can't be continued with, and says why; a password can.
#[gpui_kit::test]
fn without_browser_sign_in_a_password_still_works(cx: &mut TestAppContext) {
    let w = open(cx);
    w.view.update(cx, |t, _| {
        t.engines = Some(crate::engines::Engines {
            browser: false,
            openconnect_version: Some((8, 20)),
            ..crate::engines::Engines::ALL
        });
    });
    w.type_into(|t| t.name.clone(), "ACME", cx);
    w.click(("protocol", ANYCONNECT), cx);
    w.type_into(|t| t.gateway.clone(), "vpn.acme.example", cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2);

    w.click(("method", 0usize), cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2, "the browser can't sign in here");
    let problem = w.read(cx, |t| t.problem.clone()).unwrap_or_default();
    assert!(problem.contains("this computer has 8.20"), "{problem}");

    w.click(("method", 1usize), cx);
    w.type_into(|t| t.user.clone(), "jane", cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 3);
    w.click("next", cx);
    assert_eq!(w.added()["sign_in"], "password");
}

/// A small screen (a 768 px laptop: the window gets 648 px): the step body
/// scrolls inside it, and Continue stays in view.
#[gpui_kit::test]
fn on_a_small_screen_continue_stays_in_view(cx: &mut TestAppContext) {
    let w = open(cx);
    w.view.update(cx, |t, _| t.max_height = 648.);
    cx.simulate_window_resize(
        w.window,
        gpui_kit::size(gpui_kit::px(WIDTH), gpui_kit::px(648.)),
    );
    cx.run_until_parked();
    let visible = |cx: &mut TestAppContext, id: gpui_kit::ElementId| {
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some_and(|e| e.visible())
        })
        .unwrap()
    };
    assert!(visible(cx, "next".into()), "Continue in view");
    // The last card shown: Array Networks, the last Beta one (OpenVPN 3
    // sits near the top, with those used with real gateways).
    let last: gpui_kit::ElementId = ("protocol", 7usize).into();
    assert!(!visible(cx, last.clone()), "the list runs past the window");
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        let down =
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(-2000.)));
        window.scroll("body", down, cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(visible(cx, last), "scrolled to the last protocol");
    assert!(visible(cx, "next".into()), "and Continue still in view");
}

/// A .ovpn file a test imports, in a folder of its own: `body`, with a
/// `ca.crt` beside it unless `alone`.
fn ovpn(name: &str, body: &str, alone: bool) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    if !alone {
        std::fs::write(dir.path().join("ca.crt"), "-----BEGIN CERTIFICATE-----\n").unwrap();
    }
    let path = dir.path().join(name);
    std::fs::write(&path, body).unwrap();
    (dir, path)
}

/// What the Profile field says under it after an import.
fn note(w: &Opened, cx: &mut TestAppContext) -> Option<String> {
    w.read(cx, |t| match &t.profile_note {
        Some(import::ProfileNote::Imported(s) | import::ProfileNote::Failed(s)) => Some(s.clone()),
        None => None,
    })
}

/// design/proposals/ovpn-import.html, option A: "Import .ovpn…" in the
/// Profile label's row; the imported file becomes the chosen profile and
/// the tunnel's.
#[gpui_kit::test]
fn a_ovpn_file_becomes_the_profile(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Initech", cx);
    w.click(("protocol", OPENVPN3), cx);
    assert!(w.shown("import-ovpn", cx), "the import, in the label's row");
    assert!(w.shown("profile", cx), "next to the list");
    // The picker is the platform's; the file it gives is imported.
    w.click("import-ovpn", cx);
    let (_dir, path) = ovpn("initech-office.ovpn", "client\nca ca.crt\n", false);
    w.view.update(cx, |t, cx| t.import_ovpn(path, cx));
    cx.run_until_parked();
    assert_eq!(
        w.read(cx, |t| t.profile.clone()).as_deref(),
        Some("initech-office")
    );
    assert!(w.read(cx, |t| t.profiles.contains(&"initech-office".to_owned())));
    assert_eq!(
        note(&w, cx).as_deref(),
        Some("Imported initech-office.ovpn as your profile.")
    );
    w.click("next", cx);
    w.click("next", cx);
    w.click("next", cx);
    assert_eq!(w.added()["gateway"], "initech-office");
}

/// A name a profile has already gets a number; a file that points to one
/// that isn't there, or that openvpn3 refuses, says why and chooses
/// nothing.
#[gpui_kit::test]
fn an_import_says_what_went_wrong(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let w = open(cx);
    w.click(("protocol", OPENVPN3), cx);
    let (_a, taken) = ovpn("initech.ovpn", "client\n", true);
    w.view.update(cx, |t, cx| t.import_ovpn(taken, cx));
    cx.run_until_parked();
    assert_eq!(
        w.read(cx, |t| t.profile.clone()).as_deref(),
        Some("initech 2")
    );
    assert_eq!(
        note(&w, cx).as_deref(),
        Some("Imported initech.ovpn as your profile \u{201c}initech 2\u{201d}.")
    );

    w.view.update(cx, |t, _| t.profile = None);
    let (_b, missing) = ovpn("globex.ovpn", "client\nca ca.crt\n", true);
    w.view.update(cx, |t, cx| t.import_ovpn(missing, cx));
    cx.run_until_parked();
    assert_eq!(
        note(&w, cx).as_deref(),
        Some(
            "Couldn\u{2019}t import globex.ovpn: it points to ca.crt, which isn\u{2019}t next to it. Keep the files your IT team sent together and try again."
        )
    );
    assert_eq!(w.read(cx, |t| t.profile.clone()), None);

    let (_c, refused) = ovpn("bad.ovpn", "# openvpn3 refuses\n", true);
    w.view.update(cx, |t, cx| t.import_ovpn(refused, cx));
    cx.run_until_parked();
    assert_eq!(
        note(&w, cx).as_deref(),
        Some(
            "Couldn\u{2019}t import bad.ovpn: openvpn3 didn\u{2019}t take it (Configuration file contains errors)."
        )
    );
}

/// No profiles on this computer: the list is the import button.
#[gpui_kit::test]
fn with_no_profiles_the_list_is_the_import(cx: &mut TestAppContext) {
    let w = open(cx);
    w.click(("protocol", OPENVPN3), cx);
    w.view.update(cx, |t, cx| {
        t.profiles.clear();
        cx.notify();
    });
    assert!(w.shown("import-ovpn", cx));
    assert!(!w.shown("profile", cx), "no list to choose from");
}

#[gpui_kit::test]
fn tab_goes_from_the_name_to_the_gateway_and_back(cx: &mut TestAppContext) {
    let w = open_as_the_app(cx);
    w.type_into(|t| t.name.clone(), "Globex", cx);
    w.click(("protocol", ANYCONNECT), cx);
    w.type_into(|t| t.name.clone(), "", cx);
    let focused =
        |w: &Opened, cx: &mut TestAppContext, field: fn(&NewTunnel) -> Entity<InputState>| {
            let input = w.read(cx, field);
            cx.update_window(w.window, |_, window, cx| {
                input.read(cx).focus_handle(cx).is_focused(window)
            })
            .unwrap()
        };
    let press = |w: &Opened, cx: &mut TestAppContext, key: &str| {
        cx.update_window(w.window, |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
    };
    press(&w, cx, "tab");
    assert!(focused(&w, cx, |t| t.gateway.clone()), "Tab: the gateway");
    press(&w, cx, "shift-tab");
    assert!(
        focused(&w, cx, |t| t.name.clone()),
        "Shift+Tab: the name again"
    );
}

#[test]
fn asking_the_gateway_says_for_how_long_after_a_moment() {
    use std::time::Duration;
    let said = super::server::asking_said;
    assert_eq!(
        said(Duration::from_millis(900)),
        "Asking the gateway\u{2026}"
    );
    assert_eq!(
        said(Duration::from_millis(5400)),
        "Asking the gateway\u{2026} 5 s"
    );
}

/// Cancel is a button the size of its word, not half the header (its
/// hover background showed it spanning everything left of the title).
#[gpui_kit::test]
fn cancel_is_as_wide_as_its_word(cx: &mut TestAppContext) {
    let w = open(cx);
    let (cancel, header) = cx
        .update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            (window.find("cancel").bounds(), window.bounds())
        })
        .unwrap();
    let width = f32::from(cancel.size.width);
    assert!(
        width < 120.,
        "Cancel is {width:.0} px wide in a {:.0} px window",
        f32::from(header.size.width)
    );
}

/// With macOS's own window buttons (top left): no × of Crake's, and
/// Cancel moves to the right, where the × was.
#[gpui_kit::test]
fn with_the_systems_window_buttons(cx: &mut TestAppContext) {
    let w = open(cx);
    cx.update(|cx| {
        cx.set_global(crate::chrome::Chrome {
            system_buttons: true,
        })
    });
    let (close, cancel, size) = cx
        .update_window(w.window, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            (
                window.try_find("close"),
                window.find("cancel").bounds(),
                window.bounds().size,
            )
        })
        .unwrap();
    assert!(close.is_none(), "Crake's own × is there too");
    assert!(
        f32::from(cancel.origin.x) > f32::from(size.width) / 2.,
        "Cancel is at {:.0} px, on the left under the system's buttons",
        f32::from(cancel.origin.x)
    );
}

/// Once the gateway has answered, the protocols fold behind "Choose the
/// protocol yourself"; opened, they're all there to pick
/// (design/proposals/critique-2026-10-15, 3).
#[gpui_kit::test]
fn the_protocols_fold_once_the_gateway_answered(cx: &mut TestAppContext) {
    let w = open(cx);
    let shows = |id: gpui_kit::ElementId, cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    };
    // Not asked yet: every protocol, no fold.
    assert!(shows(("protocol", 1usize).into(), cx));
    assert!(!shows("protocols-more".into(), cx));
    w.view.update(cx, |v, cx| {
        v.detect = Detect::Done(Found::AnyConnect {
            groups: Vec::new(),
            expires: None,
        });
        cx.notify();
    });
    assert!(shows("protocols-more".into(), cx), "folded");
    assert!(!shows(("protocol", 1usize).into(), cx));
    assert!(
        shows(("protocol", 0usize).into(), cx),
        "Detect automatically stays"
    );
    w.click("protocols-more", cx);
    assert!(shows(("protocol", 8usize).into(), cx), "opened");
    w.click(("protocol", 1usize), cx);
    assert_eq!(w.view.read_with(cx, |v, _| v.protocol), "anyconnect");
}
