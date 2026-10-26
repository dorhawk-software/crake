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

/// The room macOS's three buttons take at a header's left edge, and the
/// gap after them: the safe zone every header leaves on a Mac. 88 as in
/// macos-window-buttons.html's log bar; 76 had left none (Traffic's title
/// touched the green button, 2026-10-26).
pub const BUTTONS_WIDTH: f32 = BUTTONS_LEFT + BUTTONS_SPAN + BUTTONS_CLEAR;
/// A window header's height: the buttons sit centred on it.
pub const HEADER_HEIGHT: f32 = 56.;
/// macOS's buttons are this tall.
const BUTTON: f32 = 14.;
/// From the window's left edge to the first button.
const BUTTONS_LEFT: f32 = 18.;
/// From the first button's left edge to the last one's right (measured on
/// macOS 26).
const BUTTONS_SPAN: f32 = 54.;
/// Kept clear after the last button.
const BUTTONS_CLEAR: f32 = 16.;

/// Where macOS draws its buttons: centred on a header `HEADER_HEIGHT` tall.
pub fn traffic_lights() -> Option<Point<Pixels>> {
    Some(point(px(BUTTONS_LEFT), px((HEADER_HEIGHT - BUTTON) / 2.)))
}

/// A window header on the window buttons' line: on a Mac it's
/// `HEADER_HEIGHT` tall and its row is centred where the buttons are, so
/// nothing in it starts above them (Traffic's title stood taller than the
/// buttons, 2026-10-26: "the buttons should be the topmost thing, or level
/// with it"). Elsewhere a header keeps its own padding.
pub fn on_buttons_line<E: gpui_kit::Styled>(header: E) -> E {
    header.h(px(HEADER_HEIGHT)).py(px(0.))
}

/// What's under a window's card. On Windows it's opaque, and Windows
/// rounds the window and draws its hairline (`sys::rounded_frame`): a card
/// rounding itself on a transparent window left square corners with a
/// light fringe there (DirectX), as the panel's had (platforms.md; a
/// colleague's Windows 11, 2026-10-26). Elsewhere it's transparent and the
/// card rounds itself (`card_edge`).
pub fn window_background() -> gpui_kit::WindowBackgroundAppearance {
    if cfg!(windows) {
        gpui_kit::WindowBackgroundAppearance::Opaque
    } else {
        gpui_kit::WindowBackgroundAppearance::Transparent
    }
}

/// A window card's edge: a hairline in `line` and 12 px corners, except on
/// Windows, which draws the window's own (`window_background`).
pub fn card_edge<E: gpui_kit::Styled>(card: E, line: gpui_kit::Rgba) -> E {
    if cfg!(windows) {
        card
    } else {
        card.border_1().border_color(line).rounded(px(CARD_RADIUS))
    }
}

/// A window card's corners where Crake draws them.
pub const CARD_RADIUS: f32 = 12.;

/// The card's top and bottom hairlines, which a window fitting itself to
/// the card adds back (measured inside them): none on Windows.
pub const CARD_BORDERS: f32 = if cfg!(windows) { 0. } else { 2. };

/// A window's round × (design/proposals/hover-close-and-sections.html,
/// chosen by the user): red while pointed at, the × in the colour that
/// reads on red. Its clicks aren't the header's, which moves the window;
/// the caller says what a click does.
pub fn close_button(t: &crate::theme::Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
    use gpui_kit::prelude::*;
    let on_danger = t.on_danger;
    gpui_kit::div()
        .id("close")
        .group("close")
        .size(px(32.))
        .rounded_full()
        .bg(t.surface)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(t.danger))
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
        .child(
            gpui_kit::svg()
                .path("crake/close.svg")
                .size(px(14.))
                .text_color(t.ink)
                .group_hover("close", move |s| s.text_color(on_danger)),
        )
}
