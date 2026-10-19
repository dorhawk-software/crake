//! Small controls shared by the windows.

use gpui_kit::prelude::*;
use gpui_kit::{Div, ElementId, FontWeight, Pixels, SharedString, Stateful, div, px};

use crate::theme::Theme;

/// One segment of a segmented control (`count` of them, `chosen` filled
/// teal). Each segment draws its own edges and the chosen one is framed in
/// its own colour: a grey rim around dark teal reads as blur in light mode.
pub fn segment(
    id: impl Into<ElementId>,
    i: usize,
    count: usize,
    chosen: Option<usize>,
    height: Pixels,
    t: &Theme,
) -> Stateful<Div> {
    let on = chosen == Some(i);
    let last = i + 1 == count;
    // The line between two segments belongs to the chosen one: it draws both
    // its sides, the one after it no left edge.
    let after_chosen = i > 0 && chosen == Some(i - 1);
    div()
        .id(id)
        .h(height)
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

/// A small action or link (DESIGN.md, "Small actions and links";
/// design/proposals/text-actions.html, A): ink, 13 px, semibold, with an ink
/// underline, the hand when pointed at. Never teal: the only teal you click
/// is the one primary action.
pub fn action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.ink)
        .underline()
        .cursor_pointer()
        .child(label.into())
}

/// "Copied", after what was copied: an acknowledgement, so muted, with the
/// app's check (DESIGN.md: never teal).
pub fn copied(t: &Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .ml(px(8.))
        .text_size(px(12.))
        .text_color(t.muted)
        .child(
            gpui_kit::svg()
                .path("crake/check.svg")
                .size(px(12.))
                .text_color(t.muted),
        )
        .child("Copied")
}
