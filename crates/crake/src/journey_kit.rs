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
    click(cx, s, "close");
    until(cx, "Settings closed", Duration::from_secs(10), |cx| {
        window::<crate::settings_view::Settings>(cx).is_none()
    });
    std::fs::read_to_string(&file).unwrap()
}
