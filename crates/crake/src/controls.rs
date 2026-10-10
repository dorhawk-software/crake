//! Small controls shared by the windows.

use gpui_kit::prelude::*;
use gpui_kit::{Div, ElementId, Stateful, div, px};

use crate::theme::Theme;

/// One segment of a segmented control (`count` of them, `chosen` filled
/// teal). Each segment draws its own edges and the chosen one is framed in
/// its own colour: a grey rim around dark teal reads as blur in light mode.
pub fn segment(
    id: impl Into<ElementId>,
    i: usize,
    count: usize,
    chosen: Option<usize>,
    height: f32,
    t: &Theme,
) -> Stateful<Div> {
    let on = chosen == Some(i);
    let last = i + 1 == count;
    // The line between two segments belongs to the chosen one: it draws both
    // its sides, the one after it no left edge.
    let after_chosen = i > 0 && chosen == Some(i - 1);
    div()
        .id(id)
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .border_t_1()
        .border_b_1()
        .when(!after_chosen, |b| b.border_l_1())
        .when(last || on, |b| b.border_r_1())
        .border_color(if on { t.accent } else { t.line_strong })
        .when(i == 0, |b| b.rounded_l(px(6.)))
        .when(last, |b| b.rounded_r(px(6.)))
        .bg(if on { t.accent } else { t.panel })
        .text_color(if on { t.accent_ink } else { t.ink })
        .cursor_pointer()
}
