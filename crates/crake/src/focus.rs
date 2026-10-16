//! The keyboard (critique 2026-10-15, design/proposals/critique-2026-10-15):
//! Tab and Shift+Tab move between a window's controls, Enter or Space acts
//! (GPUI clicks a focused element itself), and the control in focus wears a
//! 2 px ring in the accent, 2 px out, only when the focus came from the
//! keyboard: the gap in the panel's colour keeps it apart from a teal control.

use std::sync::Mutex;

use gpui_kit::prelude::*;
use gpui_kit::{App, BoxShadow, KeyDownEvent, Rgba, Window, point, px};

/// The ring's colour and its gap's: the theme's accent and panel, as last
/// looked up (`Theme::current`); a focus style can't ask while it's drawn.
const CLEAR: Rgba = Rgba {
    r: 0.,
    g: 0.,
    b: 0.,
    a: 0.,
};
static RING: Mutex<(Rgba, Rgba)> = Mutex::new((CLEAR, CLEAR));

/// Remember the theme's accent and panel for the ring.
pub fn remember(accent: Rgba, panel: Rgba) {
    *RING.lock().unwrap() = (accent, panel);
}

/// The ring's gap and its width, px.
const GAP: f32 = 2.;
const WIDTH: f32 = 2.;

/// A ring `spread` px around the control.
fn ring(color: Rgba, spread: f32) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(spread),
        inset: false,
    }
}

/// A control the keyboard reaches: a tab stop in the window's order, with
/// the ring while it has the keyboard's focus.
pub trait Keys: InteractiveElement + Sized {
    fn keys(self) -> Self {
        self.tab_index(0).focus_visible(|s| {
            let (accent, panel) = *RING.lock().unwrap();
            s.shadow(vec![ring(accent, GAP + WIDTH), ring(panel, GAP)])
        })
    }
}

impl<E: InteractiveElement> Keys for E {}

/// Tab and Shift+Tab, on a window's root: the next or previous control.
pub fn tab_moves(e: &KeyDownEvent, window: &mut Window, cx: &mut App) {
    if e.keystroke.key == "tab" {
        if e.keystroke.modifiers.shift {
            window.focus_prev(cx);
        } else {
            window.focus_next(cx);
        }
        cx.stop_propagation();
    }
}
