//! Driving the app as a person would, on GPUI's test platform: waiting,
//! clicking, typing, the panel and the sign-in window. The Linux journey
//! (`journey_tests.rs`) and Windows' (`windows_journey.rs`) are made of
//! these.

use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, Render, TestAppContext};

use crate::model::Model;
use crate::{Request, Shell, panel, signin_view};

/// Let the app and craked work until `done` holds; `what` names the step.
pub(crate) fn until(
    cx: &mut TestAppContext,
    what: &str,
    wait: Duration,
    done: impl Fn(&mut TestAppContext) -> bool,
) {
    let end = Instant::now() + wait;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn model(cx: &mut TestAppContext) -> Model {
    cx.read(|cx| cx.global::<Shell>().model.read(cx).clone())
}

pub(crate) fn request(cx: &mut TestAppContext, r: Request) {
    cx.update(|cx| cx.global::<Shell>().requests.send_blocking(r).unwrap());
    cx.run_until_parked();
}

/// The open window whose view is a `V`, and that view.
pub(crate) fn window<V: 'static + Render>(
    cx: &mut TestAppContext,
) -> Option<(AnyWindowHandle, Entity<V>)> {
    cx.update(|cx| {
        cx.windows().into_iter().find_map(|w| {
            let typed = w.downcast::<V>()?;
            Some((w, typed.root(cx).ok()?))
        })
    })
}

pub(crate) fn wait_window<V: 'static + Render>(
    cx: &mut TestAppContext,
    what: &str,
) -> (AnyWindowHandle, Entity<V>) {
    until(cx, what, Duration::from_secs(10), |cx| {
        window::<V>(cx).is_some()
    });
    window::<V>(cx).unwrap()
}

pub(crate) fn click(
    cx: &mut TestAppContext,
    w: AnyWindowHandle,
    id: impl Into<gpui_kit::ElementId>,
) {
    let id = id.into();
    settle(cx, w);
    if std::env::var_os("CRAKE_JOURNEY_DEBUG").is_some() {
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            let found = window
                .try_find(id.clone())
                .map(|e| (e.bounds(), e.visible()));
            eprintln!(
                "click {id:?}: viewport {:?}, element {found:?}",
                window.viewport_size()
            );
        })
        .unwrap();
    }
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// A few frames, as a screen would draw them: a window that fits itself
/// to its content (New tunnel) resizes on the frame after it measures.
/// GPUI's test platform takes a resize without telling the window, as a
/// window system's configure event would: it's delivered here.
pub(crate) fn settle(cx: &mut TestAppContext, w: AnyWindowHandle) {
    for _ in 0..3 {
        let asked = cx
            .update_window(w, |_, window, cx| {
                window.render_frame(cx);
                let asked = window.bounds().size;
                (asked != window.viewport_size()).then_some(asked)
            })
            .unwrap();
        if let Some(size) = asked {
            cx.simulate_window_resize(w, size);
        }
        cx.run_until_parked();
    }
}

pub(crate) fn type_into(
    cx: &mut TestAppContext,
    w: AnyWindowHandle,
    field: Entity<gpui_kit::component::input::InputState>,
    text: &str,
) {
    cx.update_window(w, |_, window, cx| {
        field.update(cx, |f, cx| f.focus(window, cx));
        window.render_frame(cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// The panel, opened the way the tray icon opens it.
pub(crate) fn panel(cx: &mut TestAppContext) -> AnyWindowHandle {
    if window::<panel::Panel>(cx).is_none() {
        // A tray click within half a second of the panel closing on blur
        // means "close" (the click takes focus first; windows.rs): a person
        // clicks later than that.
        let blurred = cx.read(|cx| cx.global::<Shell>().panel_blurred.map(|t| t.elapsed()));
        if let Some(ago) = blurred {
            std::thread::sleep(Duration::from_millis(600).saturating_sub(ago));
        }
        request(cx, Request::Panel(None));
    }
    wait_window::<panel::Panel>(cx, "the panel opens").0
}

pub(crate) fn state(cx: &mut TestAppContext, tunnel: &str) -> String {
    model(cx)
        .tunnels
        .iter()
        .find(|t| t.name == tunnel)
        .map(|t| t.state.clone())
        .unwrap_or_default()
}

pub(crate) fn index(cx: &mut TestAppContext, tunnel: &str) -> usize {
    model(cx)
        .tunnels
        .iter()
        .position(|t| t.name == tunnel)
        .expect("listed")
}

/// The sign-in window of that tunnel (another's may still be open: a
/// window that's done closes on a timer, which the test platform runs only
/// when its clock is moved).
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn sign_in_of(
    cx: &mut TestAppContext,
    tunnel: &str,
) -> (AnyWindowHandle, Entity<signin_view::SignIn>) {
    let find = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            cx.windows().into_iter().find_map(|w| {
                let view = w.downcast::<signin_view::SignIn>()?.root(cx).ok()?;
                (view.read(cx).tunnel() == tunnel).then_some((w, view))
            })
        })
    };
    until(
        cx,
        &format!("{tunnel}'s sign-in opens"),
        Duration::from_secs(10),
        |cx| find(cx).is_some(),
    );
    find(cx).unwrap()
}

/// Not signed in after all, on a gateway trusted before: asked for the
/// password straight away (its certificate is remembered, never asked
/// about again), then Cancel, as a person would. Until 2026-10-27 every
/// sign-in asked about the certificate, and this step said "Don't connect".
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn decline(
    cx: &mut TestAppContext,
    w: AnyWindowHandle,
    view: &Entity<signin_view::SignIn>,
) {
    use crate::signin::Ask;
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let stage = view.read_with(cx, |v, _| v.stage().clone());
        if let signin_view::Stage::Asking(p) = &stage {
            assert_eq!(p.ask, Ask::Secret, "asked before the password");
            break;
        }
        assert!(
            Instant::now() < end && !matches!(stage, signin_view::Stage::Failed { .. }),
            "never asked for the password: {stage:?}"
        );
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    click(cx, w, "cancel");
}

/// Sign in as the window asks: trust the gateway, give the password.
pub(crate) fn sign_in(cx: &mut TestAppContext) {
    use crate::signin::Ask;
    let (w, view) = wait_window::<signin_view::SignIn>(cx, "the sign-in window opens");
    let asked = |cx: &mut TestAppContext, want: fn(&Ask) -> bool| {
        view.read_with(
            cx,
            |v, _| matches!(v.stage(), signin_view::Stage::Asking(p) if want(&p.ask)),
        )
    };
    let trust = |cx: &mut TestAppContext| asked(cx, |a| matches!(a, Ask::TrustServer { .. }));
    let end = Instant::now() + Duration::from_secs(20);
    while !trust(cx) {
        let stage = view.read_with(cx, |v, _| v.stage().clone());
        assert!(
            Instant::now() < end && !matches!(stage, signin_view::Stage::Failed { .. }),
            "never asked to trust the gateway: {stage:?}"
        );
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    click(cx, w, "yes");
    until(
        cx,
        "asked for the password",
        Duration::from_secs(20),
        |cx| asked(cx, |a| *a == Ask::Secret),
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.input("hunter2", cx);
    })
    .unwrap();
    click(cx, w, "continue");
}

/// Settings → Diagnostics → "Save a report…": the file the platform's
/// dialog gives, written once craked has said its part. Its text; Settings
/// closed again.
pub(crate) fn save_a_report(cx: &mut TestAppContext) -> String {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("crake-report.txt");
    request(cx, crate::Request::Settings);
    let (s, _) = wait_window::<crate::settings_view::Settings>(cx, "Settings opens");
    click(
        cx,
        s,
        (
            "section",
            crate::settings_view::Section::Diagnostics.position(),
        ),
    );
    click(cx, s, "report");
    let chosen = file.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    // The internet tried, and each tunnel's name: seconds, more where
    // nothing answers.
    until(cx, "the report saved", Duration::from_secs(90), |_| {
        file.exists()
    });
    // macOS' own window buttons close it there (`chrome`); Crake's × elsewhere.
    if cx.read(crate::chrome::system_buttons) {
        cx.update_window(s, |_, window, _| window.remove_window())
            .unwrap();
    } else {
        click(cx, s, "close");
    }
    until(cx, "Settings closed", Duration::from_secs(10), |cx| {
        window::<crate::settings_view::Settings>(cx).is_none()
    });
    std::fs::read_to_string(&file).unwrap()
}

/// Crake's first route says so under it in the panel, once
/// (tray-panel.html?moment=first): `host`'s, if it's the first this
/// system's craked knows of, and Got it puts it away. Where craked's log
/// already held routes (Windows' service keeps its log across the
/// scenarios run before the journey, 2026-10-29), nothing is said.
pub(crate) fn first_route_got_it(cx: &mut TestAppContext, host: &str) {
    let first = model(cx).first_route().map(|e| e.hosts.clone());
    let p = panel(cx);
    if first.is_none() {
        let shown = cx
            .update_window(p, |_, window, cx| {
                window.render_frame(cx);
                window.try_find("first-route-got-it").is_some()
            })
            .unwrap();
        assert!(!shown, "a first route said where there were others");
        return;
    }
    assert_eq!(
        first.as_deref(),
        Some(&[host.to_owned()][..]),
        "the first route"
    );
    click(cx, p, "first-route-got-it");
    until(
        cx,
        "the first route's notice put away",
        Duration::from_secs(10),
        |cx| {
            cx.read(|cx| {
                cx.global::<crate::prefs::Prefs>()
                    .dismissed
                    .contains(panel::FIRST_ROUTE)
            })
        },
    );
}

/// The log's "Show in ACME" on a routed host's row: tunnel detail opens on
/// that host (log.html?pointed=13:21).
pub(crate) fn show_in(cx: &mut TestAppContext, log: AnyWindowHandle, host: &str) {
    let key = model(cx)
        .log_days(crate::model::Show::Everything, None, "")
        .into_iter()
        // Today's rows: earlier days may be folded away.
        .filter(|d| d.title == "Today")
        .find_map(|d| {
            d.rows.into_iter().find_map(|r| {
                let shown = r.show_in.as_ref().is_some_and(|(_, h)| h == host);
                shown.then(|| format!("{}|{}|{}", d.title, r.time, r.text))
            })
        })
        .expect("a routed row for the host");
    click(
        cx,
        log,
        gpui_kit::SharedString::from(format!("show-in:{key}")),
    );
    let (_, view) =
        wait_window::<crate::tunnel_detail::TunnelDetail>(cx, "tunnel detail opens on the host");
    until(cx, "the host's row shown", Duration::from_secs(10), |cx| {
        view.read_with(cx, |v, _| v.shown_host() == Some(host))
    });
}

/// What `name` resolves to, asked as an app asks the system.
fn look_up(name: &'static str) -> Vec<String> {
    std::thread::spawn(move || {
        use std::net::ToSocketAddrs as _;
        (name, 80)
            .to_socket_addrs()
            .map(|a| a.map(|a| a.ip().to_string()).collect())
            .unwrap_or_default()
    })
    .join()
    .unwrap_or_default()
}

/// A short name: "git", typed on its own, is asked about in "What Crake
/// did"; "Use ACME's" makes it ACME's git.acme.internal (10.40.1.10), and
/// tunnel detail's Short names forgets it.
pub(crate) fn short_name(cx: &mut TestAppContext) {
    let asked = |cx: &mut TestAppContext| {
        model(cx).decisions(panel::DECISIONS).iter().position(|d| {
            d.action == Some(crate::model::Action::Use)
                && d.entry.short.as_ref().is_some_and(|s| s.name == "git")
        })
    };
    until(cx, "asked about git", Duration::from_secs(20), |cx| {
        asked(cx).is_some() || {
            look_up("git");
            false
        }
    });
    let p = panel(cx);
    let i = asked(cx).unwrap();
    click(cx, p, ("act", i));
    until(cx, "git is ACME's", Duration::from_secs(10), |cx| {
        model(cx)
            .log
            .iter()
            .any(|e| e.message == "git is git.acme.internal now.")
    });
    let found = look_up("git");
    assert!(found.iter().any(|a| a == "10.40.1.10"), "git: {found:?}");
    request(cx, Request::OpenDetail("acme".into(), None));
    let (d, _) = wait_window::<crate::tunnel_detail::TunnelDetail>(cx, "tunnel detail opens");
    until(cx, "git in Short names", Duration::from_secs(10), |cx| {
        model(cx)
            .host_rows
            .get("acme")
            .is_some_and(|rows| rows.iter().any(|r| r.kind == "short" && r.target == "git"))
    });
    click(cx, d, ("tab", 3usize));
    click(cx, d, ("forget", 0usize));
    until(cx, "git forgotten", Duration::from_secs(10), |cx| {
        model(cx).log.iter().any(|e| e.message == "Forgot git.")
    });
}
