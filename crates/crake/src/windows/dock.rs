//! The Dock on a Mac: Crake is a menu bar app with no Dock icon (its
//! Info.plist's LSUIElement) until a window of its own opens (Settings, the
//! log, a sign-in, tunnel detail…). Then it's in the Dock and the app
//! switcher like any app with a window open, and leaves both once the last
//! one closes; the panel alone never brings it there. Until 2026-10-27
//! Crake was in the Dock all the time: GPUI makes every Mac app a Dock app
//! once it has launched, LSUIElement or not.

use gpui_kit::{ActivationPolicy, App, Global};

use crate::Shell;

/// Whether Crake is in the Dock now; tests read it (GPUI's test platform
/// has no Dock).
#[derive(Default)]
pub(crate) struct InDock(pub bool);

impl Global for InDock {}

/// The app as GPUI starts it: on a Mac, without a Dock icon (GPUI's
/// "headless" there is only that; its windows open as ever).
pub(crate) fn application() -> gpui_kit::Application {
    let app = gpui_kit::application();
    if cfg!(target_os = "macos") {
        app.with_windowing(gpui_kit::WindowingRequest::Headless)
    } else {
        app
    }
}

/// Out of the Dock once only the panel is left open.
pub(crate) fn follow_windows(cx: &mut App) {
    cx.set_global(InDock(false));
    cx.on_window_closed(|cx, _| {
        let panel = cx.global::<Shell>().panel;
        if cx.windows().iter().all(|w| Some(*w) == panel) {
            set(false, cx);
        }
    })
    .detach();
}

/// A window of Crake's own is opening: into the Dock (before `activate`,
/// which GPUI asks for after this switch, or the menu bar may not show).
pub(super) fn window_opens(cx: &mut App) {
    set(true, cx);
}

fn set(in_dock: bool, cx: &mut App) {
    if cx.try_global::<InDock>().is_some_and(|d| d.0 == in_dock) {
        return;
    }
    cx.set_global(InDock(in_dock));
    if cfg!(target_os = "macos") {
        cx.set_activation_policy(if in_dock {
            ActivationPolicy::Regular
        } else {
            ActivationPolicy::Accessory
        });
    }
}
