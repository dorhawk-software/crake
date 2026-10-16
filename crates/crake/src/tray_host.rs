//! The app's side of `no_tray`: the tray going and coming (`sys::tray`
//! tells), what would bring the bird back (looked at again every few
//! seconds while there's no tray, so installing the extension in a
//! terminal changes the row), and what's said at start: the panel with its
//! note when Crake was started by hand, one notification when it started
//! at login. Linux only: Windows and macOS always have a place for it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui_kit::App;

use crate::no_tray::Remedy;
use crate::{Request, Shell};

/// What the app hears about the tray, and what the user does about it.
#[derive(Clone, Debug)]
pub enum Event {
    /// A tray is there (true) or isn't (false).
    Host(bool),
    /// What would bring the bird back, as last found.
    Found(Remedy),
    /// "Switch on AppIndicator" (the extension's id).
    SwitchOn(String),
    OpenExtensions,
    /// The panel's "Not now": no note until Crake starts again.
    Quiet,
}

/// No tray now (the look loop runs while it is so).
static MISSING: AtomicBool = AtomicBool::new(false);
/// The look loop is running.
static LOOKING: AtomicBool = AtomicBool::new(false);
/// What's said at start has been decided (once per run).
static SAID: AtomicBool = AtomicBool::new(false);

/// How often Crake looks again while there's no tray.
const LOOK_EVERY: Duration = Duration::from_secs(3);
/// At login a tray may come up after Crake: wait this long before saying
/// there's none.
const LOGIN_GRACE: Duration = Duration::from_secs(10);
/// After "Switch on AppIndicator": the tray comes at once when it works.
const SWITCH_WAIT: Duration = Duration::from_secs(2);

pub fn handle(event: Event, cx: &mut App) {
    let shell = cx.global::<Shell>();
    let (model, requests) = (shell.model.clone(), shell.requests.clone());
    match event {
        Event::Host(true) => {
            MISSING.store(false, Ordering::SeqCst);
            model.update(cx, |m, cx| {
                m.no_tray = None;
                cx.notify();
            });
        }
        Event::Host(false) => {
            MISSING.store(true, Ordering::SeqCst);
            if !LOOKING.swap(true, Ordering::SeqCst) {
                std::thread::spawn(move || look_while_missing(requests));
            }
        }
        Event::Found(remedy) => {
            if !MISSING.load(Ordering::SeqCst) {
                return;
            }
            let first = model.read(cx).no_tray.is_none() && !SAID.swap(true, Ordering::SeqCst);
            model.update(cx, |m, cx| {
                m.no_tray = Some(remedy);
                cx.notify();
            });
            if first {
                say_at_start(cx);
            }
        }
        Event::SwitchOn(id) => {
            std::thread::spawn(move || {
                let remedy = switch_on(&id);
                std::thread::sleep(SWITCH_WAIT);
                requests
                    .send_blocking(Request::NoTray(Event::Found(remedy)))
                    .ok();
            });
        }
        Event::OpenExtensions => {
            std::thread::spawn(open_extensions);
        }
        Event::Quiet => model.update(cx, |m, cx| {
            m.no_tray_quiet = true;
            cx.notify();
        }),
    }
}

/// What's said at start is past (the bird switched on later, in Settings).
pub fn said_at_start() {
    SAID.store(true, Ordering::SeqCst);
}

/// Looks every few seconds while there's no tray, telling the app when the
/// answer changes.
fn look_while_missing(requests: async_channel::Sender<Request>) {
    let mut last = None;
    while MISSING.load(Ordering::SeqCst) {
        let found = look();
        if last.as_ref() != Some(&found) {
            requests
                .send_blocking(Request::NoTray(Event::Found(found.clone())))
                .ok();
            last = Some(found);
        }
        std::thread::sleep(LOOK_EVERY);
    }
    LOOKING.store(false, Ordering::SeqCst);
}

/// The first time there's no tray in this run: started by hand, the panel
/// opens with its note; at login, one notification after a grace (once per
/// case, remembered). Nothing when the user hid the bird.
fn say_at_start(cx: &mut App) {
    if !cx.global::<crate::prefs::Prefs>().show_bird {
        return;
    }
    if !cx.global::<Shell>().started_at_login {
        crate::windows::toggle_panel(None, cx);
        return;
    }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(LOGIN_GRACE).await;
        cx.update(|cx| {
            let shell = cx.global::<Shell>();
            let Some(remedy) = shell.model.read(cx).no_tray.clone() else {
                return;
            };
            let requests = shell.requests.clone();
            let key = format!("no-tray:{}", remedy.key());
            let prefs = cx.global_mut::<crate::prefs::Prefs>();
            if prefs.dismissed.insert(key) {
                prefs.save();
                crate::notify::no_tray(&remedy, requests);
            }
        });
    })
    .detach();
}

#[cfg(target_os = "linux")]
use crate::sys::gnome_tray::{look, open_extensions, switch_on};

#[cfg(not(target_os = "linux"))]
fn look() -> Remedy {
    Remedy::NoTray
}
#[cfg(not(target_os = "linux"))]
fn switch_on(_: &str) -> Remedy {
    Remedy::NoTray
}
#[cfg(not(target_os = "linux"))]
fn open_extensions() {}
