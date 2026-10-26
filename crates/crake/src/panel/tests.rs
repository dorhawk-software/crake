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

/// A tunnel that stopped on its own has a Dismiss next to why
/// (design/proposals/dismiss-trouble.html); it asks craked to make it Off.
#[gpui_kit::test]
fn a_stopped_tunnel_can_be_dismissed(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    let name = w.model.update(cx, |m, cx| {
        m.tunnels[1].state = "trouble".into();
        cx.notify();
        m.tunnels[1].name.clone()
    });
    cx.run_until_parked();
    match w.click(("dismiss", 1_usize), cx) {
        Request::DismissTrouble(t) => assert_eq!(t, name),
        other => panic!("{other:?}"),
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

#[gpui_kit::test]
fn with_no_tunnel_the_panel_offers_to_add_one(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    w.model.update(cx, |m, cx| {
        m.tunnels.clear();
        cx.notify();
    });
    cx.run_until_parked();
    assert!(matches!(
        w.click("add-tunnel", cx),
        Request::NewTunnel(None)
    ));
    // Nothing to chart or report yet: no footer.
    let footer = cx
        .update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find("full-log").is_some()
        })
        .unwrap();
    assert!(!footer, "the first-run panel has no footer");
}

/// Browsers that look names up themselves (design/proposals/browser-dns.html,
/// A): said under the first connected tunnel that learns, until "Got it",
/// which remembers each browser it named.
#[gpui_kit::test]
fn a_browser_that_looks_names_up_itself_is_said_until_got_it(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    w.model.update(cx, |m, cx| {
        m.own_dns_browsers = vec!["Firefox".into(), "Chrome".into()];
        cx.notify();
    });
    let mut asked = vec![w.click("browser-dns-got-it", cx)];
    while let Ok(r) = w.requests.try_recv() {
        asked.push(r);
    }
    let keys: Vec<String> = asked
        .into_iter()
        .map(|r| match r {
            Request::Dismiss(key) => key,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(keys, ["browser-dns:Firefox", "browser-dns:Chrome"]);
}

#[test]
fn the_browser_notice_names_the_browsers_not_dismissed() {
    use std::collections::BTreeSet;
    let mut m = crate::model::demo();
    m.own_dns_browsers = vec!["Firefox".into(), "Chrome".into()];
    let (at, names, text) = tunnels::browser_notice(&m, &BTreeSet::new()).expect("shown");
    assert_eq!(
        m.tunnels[at].display, "ACME",
        "the first connected tunnel that learns"
    );
    assert_eq!(names, ["Firefox", "Chrome"]);
    assert_eq!(
        text,
        "Firefox and Chrome look names up themselves (DNS over HTTPS), so the first visit there to a site only ACME knows waits a few seconds. Turning DNS over HTTPS off in their settings fixes it."
    );
    let firefox: BTreeSet<String> = ["browser-dns:Firefox".to_owned()].into();
    let (_, names, text) = tunnels::browser_notice(&m, &firefox).expect("Chrome still");
    assert_eq!(names, ["Chrome"]);
    assert_eq!(
        text,
        "Chrome looks names up itself (DNS over HTTPS), so the first visit there to a site only ACME knows waits a few seconds. Turning DNS over HTTPS off in Chrome\u{2019}s settings fixes it."
    );
    let both: BTreeSet<String> = [
        "browser-dns:Firefox".to_owned(),
        "browser-dns:Chrome".to_owned(),
    ]
    .into();
    assert!(tunnels::browser_notice(&m, &both).is_none(), "both read");
    for t in &mut m.tunnels {
        t.state = "off".into();
    }
    assert!(
        tunnels::browser_notice(&m, &BTreeSet::new()).is_none(),
        "no tunnel up"
    );
}

/// On a Mac, craked waits for the user to switch it on in System Settings
/// (it's registered by the app, SMAppService): the panel says so, and its
/// button asks the app to open that page.
#[gpui_kit::test]
fn the_service_waiting_for_approval_takes_the_user_there(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    w.model.update(cx, |m, cx| {
        m.reachable = Some(false);
        m.service = Some(crate::service::Wanted::Approval);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(matches!(
        w.click("open-login-items", cx),
        Request::OpenServiceSettings
    ));
}

/// Windows wouldn't start craked (an antivirus refused it): the panel says
/// so, and what to do, instead of only "isn't running".
#[gpui_kit::test]
fn a_service_windows_wouldnt_start_says_why(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    w.model.update(cx, |m, cx| {
        m.reachable = Some(false);
        m.stopped = Some(crate::service::Stopped::Denied);
        cx.notify();
    });
    cx.run_until_parked();
    let (title, _) = crate::service::said(&crate::service::Stopped::Denied, "");
    assert!(w.shows_element(gpui_kit::ElementId::Name(title.into()), cx));
    // Without a reason: as before.
    w.model.update(cx, |m, cx| {
        m.stopped = None;
        cx.notify();
    });
    cx.run_until_parked();
    let plain = gpui_kit::ElementId::Name("Crake's service isn't running.".into());
    assert!(w.shows_element(plain, cx));
}

impl Opened {
    fn shows_element(&self, id: impl Into<gpui_kit::ElementId>, cx: &mut TestAppContext) -> bool {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    }

    fn shows(&self, id: &'static str, cx: &mut TestAppContext) -> bool {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    }
}

/// A package updated Crake: craked (restarted by the package) is newer
/// than this app, which asks to be restarted (design/proposals/new-version.html).
#[gpui_kit::test]
fn an_updated_service_asks_for_a_restart(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    assert!(!w.shows("version", cx), "nothing to say yet");
    w.model.update(cx, |m, cx| {
        m.service_version = Some("99.0.0".into());
        cx.notify();
    });
    assert!(matches!(w.click("Restart", cx), Request::Restart));
}

/// A new version is out (asked of dorhawk.software): Download fetches it;
/// Not now remembers that version; switched off in Settings, it's
/// not said.
#[gpui_kit::test]
fn a_new_version_is_downloaded_until_not_now(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    w.model.update(cx, |m, cx| {
        m.newest = Some("99.1.0".into());
        cx.notify();
    });
    match w.click("Download", cx) {
        // This computer's installer, or the page (versions::tests).
        Request::OpenUrl(url) => assert_eq!(url, crate::versions::download("99.1.0")),
        other => panic!("{other:?}"),
    }
    match w.click("Not now", cx) {
        Request::Dismiss(key) => assert_eq!(key, "version:99.1.0"),
        other => panic!("{other:?}"),
    }
    cx.update(|cx| cx.global_mut::<Prefs>().check_versions = false);
    assert!(!w.shows("version", cx), "switched off");
}

/// Updated from 0.13.1 (design/proposals/whats-new.html, 1): the panel
/// says so with the number of changes; What's new opens the window, Not
/// now puts it away; a fresh install says nothing.
#[gpui_kit::test]
fn after_an_update_the_panel_offers_whats_new(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    assert!(!w.shows("What's new", cx), "a fresh install: nothing");
    let w = open(
        cx,
        Prefs {
            seen_version: Some("0.13.1".into()),
            existed: true,
            ..Prefs::default()
        },
    );
    assert!(matches!(w.click("What's new", cx), Request::WhatsNew));
    assert!(matches!(w.click("Not now", cx), Request::SeenVersion));
    // Restart wins: one version notice at a time.
    w.model.update(cx, |m, cx| {
        m.service_version = Some("99.0.0".into());
        cx.notify();
    });
    assert!(!w.shows("What's new", cx));
    assert!(matches!(w.click("Restart", cx), Request::Restart));
}

/// OpenVPN 3 left a connection running (design/proposals/leftover-warning.html):
/// the panel says how to stop it, Copy puts the command on the clipboard,
/// Got it remembers this process; a process that ended isn't said.
#[gpui_kit::test]
fn a_leftover_process_says_how_to_stop_it(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    assert!(
        !w.shows_element(("leftover", 0usize), cx),
        "nothing left running"
    );
    let leftover = crate::model::Leftover {
        pid: 4242,
        started: 100,
        session: "Globex".into(),
    };
    w.model.update(cx, |m, cx| {
        m.leftovers = vec![leftover.clone()];
        cx.notify();
    });
    assert!(w.shows_element(("leftover", 0usize), cx), "said");
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("leftover-copy", 0usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
        Some("sudo kill -9 4242")
    );
    match w.click(("leftover-got-it", 0usize), cx) {
        Request::Dismiss(key) => {
            assert_eq!(key, "leftover:4242:100");
            cx.update(|cx| {
                cx.global_mut::<Prefs>().dismissed.insert(key);
            });
        }
        other => panic!("{other:?}"),
    }
    assert!(!w.shows_element(("leftover", 0usize), cx), "seen: hidden");
    // Another process left running later is said again.
    w.model.update(cx, |m, cx| {
        m.leftovers = vec![crate::model::Leftover {
            pid: 5151,
            ..leftover
        }];
        cx.notify();
    });
    assert!(w.shows_element(("leftover", 0usize), cx));
}

/// The keyboard reaches every control (critique 2026-10-15): Tab moves to
/// the first and on, Shift+Tab back, and Enter acts as a click would.
#[gpui_kit::test]
fn tab_reaches_the_controls_and_enter_acts(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    let focused = |cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| window.focused(cx))
            .unwrap()
    };
    // The panel itself has the keyboard when it opens; no control yet.
    let start = focused(cx);
    let press = |key: &str, cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
    };
    press("tab", cx);
    let first = focused(cx).expect("Tab gives a control the keyboard");
    assert_ne!(Some(first.clone()), start);
    press("tab", cx);
    let second = focused(cx).expect("and Tab again the next");
    assert_ne!(first, second);
    press("shift-tab", cx);
    assert_eq!(focused(cx), Some(first.clone()), "Shift+Tab goes back");
    press("enter", cx);
    assert!(w.requests.try_recv().is_ok(), "Enter acts on it");
}

/// Nothing shows tray icons: the note says what would bring the bird back,
/// its button asks for it, "Not now" puts it off; a hidden bird says nothing.
#[gpui_kit::test]
fn with_no_tray_the_note_says_what_would_help(cx: &mut TestAppContext) {
    use crate::no_tray::{EXTENSION, Remedy};
    use crate::tray_host::Event;
    let w = open(cx, Prefs::default());
    let shown = |cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find("no-tray").is_some()
        })
        .unwrap()
    };
    assert!(!shown(cx), "a tray: no note");
    w.model.update(cx, |m, cx| {
        m.no_tray = Some(Remedy::SwitchOn(EXTENSION.into()));
        cx.notify();
    });
    assert!(shown(cx));
    assert!(matches!(
        w.click("no-tray-switch", cx),
        Request::NoTray(Event::SwitchOn(id)) if id == EXTENSION
    ));
    assert!(matches!(
        w.click("no-tray-later", cx),
        Request::NoTray(Event::Quiet)
    ));
    w.model.update(cx, |m, cx| {
        m.no_tray_quiet = true;
        cx.notify();
    });
    assert!(!shown(cx), "put off until the next start");
    w.model.update(cx, |m, cx| {
        m.no_tray_quiet = false;
        cx.notify();
    });
    cx.update(|cx| cx.global_mut::<Prefs>().show_bird = false);
    assert!(!shown(cx), "the bird hidden: nothing said");
}

/// An OpenVPN 3 tunnel that stays on OpenVPN 3 (design/proposals/
/// openvpn-tunnel.html, 5): said, "Import .ovpn…" opens its Edit, "Not
/// now" hides it.
#[gpui_kit::test]
fn a_tunnel_staying_on_openvpn3_says_what_to_do(cx: &mut TestAppContext) {
    let w = open(cx, Prefs::default());
    assert!(!w.shows_element(("staying", 0usize), cx), "nothing stays");
    w.model.update(cx, |m, cx| {
        *m = crate::model::demo_staying(std::mem::take(m));
        cx.notify();
    });
    assert!(w.shows_element(("staying", 0usize), cx), "said");
    match w.click(("staying-import", 0usize), cx) {
        Request::NewTunnel(Some(tunnel)) => assert_eq!(tunnel, "globex"),
        other => panic!("{other:?}"),
    }
    match w.click(("staying-not-now", 0usize), cx) {
        Request::Dismiss(key) => {
            assert_eq!(key, "openvpn3-stays:globex");
            cx.update(|cx| {
                cx.global_mut::<Prefs>().dismissed.insert(key);
            });
        }
        other => panic!("{other:?}"),
    }
    assert!(!w.shows_element(("staying", 0usize), cx), "not now: hidden");
}

/// Never taller than its screen, however many tunnels: what's between the
/// header and the footer scrolls (four tunnels pushed the footer off a
/// 1366x768 screen, 2026-10-19). With no screen known, a laptop's.
#[test]
fn many_tunnels_keep_the_panel_on_its_screen() {
    let mut m = crate::model::demo();
    let one = m.tunnels[0].clone();
    for i in 0..20 {
        let mut t = one.clone();
        t.name = format!("t{i}");
        m.tunnels.push(t);
    }
    assert!(Panel::height(&m) <= crate::place::max_window_height() + 1.);
    // A few tunnels still get the panel's own height, below the cap.
    let few = crate::model::demo();
    assert!(Panel::height(&few) < Panel::height(&m));
}
