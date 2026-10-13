//! Who draws a window's buttons. macOS draws its own (red, yellow and
//! green, top left), so there Crake's windows leave them room and draw no
//! close button of their own; elsewhere Crake draws its ×
//! (design/proposals/macos-window-buttons.html, option A, chosen by the
//! user: the system draws the frame, as on Windows).

use gpui_kit::{App, Global, Pixels, Point, point, px};

/// Whether the system draws the window buttons. Without it set, it's the
/// system's own way; tests set either.
pub struct Chrome {
    pub system_buttons: bool,
}

impl Global for Chrome {}

pub fn system_buttons(cx: &App) -> bool {
    cx.try_global::<Chrome>()
        .map_or(cfg!(target_os = "macos"), |c| c.system_buttons)
}

/// The room macOS's three buttons take at a header's left edge.
pub const BUTTONS_WIDTH: f32 = 76.;
/// A window header's height: the buttons sit centred on it.
pub const HEADER_HEIGHT: f32 = 56.;
/// macOS's buttons are this tall.
const BUTTON: f32 = 14.;
/// From the window's left edge to the first button.
const BUTTONS_LEFT: f32 = 18.;

/// Where macOS draws its buttons: centred on a header `HEADER_HEIGHT` tall.
pub fn traffic_lights() -> Option<Point<Pixels>> {
    Some(point(px(BUTTONS_LEFT), px((HEADER_HEIGHT - BUTTON) / 2.)))
}
