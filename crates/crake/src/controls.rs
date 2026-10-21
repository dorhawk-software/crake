//! Small controls shared by the windows.

use std::time::{Duration, Instant};

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    Animation, AnimationExt as _, App, DispatchPhase, Div, ElementId, Entity, FontWeight,
    MouseExitEvent, MouseMoveEvent, Pixels, ScrollHandle, SharedString, Stateful,
    TestSupportExt as _, Window, div, px,
};

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

/// A scrolling area's scroll, kept with the window under `id`: the area
/// tracks it (`track_scroll`), and `over` lays its scrollbar over it.
pub fn scroll_handle(id: &'static str, window: &mut Window, cx: &mut App) -> ScrollHandle {
    window
        .use_keyed_state(
            ElementId::Name(format!("{id}-scroll").into()),
            cx,
            |_, _| ScrollHandle::new(),
        )
        .read(cx)
        .clone()
}

/// How a scrollbar comes and goes (DESIGN.md, Scrollbars): in as the
/// pointer enters what scrolls, held a moment after it leaves, then out.
const BAR_IN: Duration = Duration::from_millis(150);
const BAR_HOLD: Duration = Duration::from_millis(300);
const BAR_OUT: Duration = Duration::from_millis(300);

/// Whether the pointer is over a scrolling area, since when, and how many
/// times it came or went (each turn starts its fade afresh).
#[derive(Clone, Copy, Default)]
struct Pointed {
    over: bool,
    at: Option<Instant>,
    turn: u32,
}

/// What scrolls, with its scrollbar over it while the pointer is over it.
/// `area` keeps its own size and scrolls with `scroll`; the container
/// around it holds still, follows the pointer, and carries the bar (inside
/// the area it would scroll away). gpui-kit's own "hover" means pointing at
/// the bar's thin strip, not at what scrolls (the user saw it stay on
/// after leaving the window over it, 2026-10-21): the bar is drawn always
/// (`Theme::sync_components`), and shown here.
pub fn over(
    id: &'static str,
    area: impl IntoElement,
    scroll: &ScrollHandle,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let state = window.use_keyed_state(
        ElementId::Name(format!("{id}-pointed").into()),
        cx,
        |_, _| Pointed::default(),
    );
    let pointed = *state.read(cx);
    let turn = ElementId::Name(format!("{id}-bar-{}", pointed.turn).into());
    let bar = div()
        .absolute()
        .inset_0()
        // Where tests see the bar shown.
        .child(
            div()
                .id(ElementId::Name(format!("{id}-bar").into()))
                .absolute()
                .inset_0()
                .test_support(),
        )
        .vertical_scrollbar(scroll);
    let fading = pointed
        .at
        .is_some_and(|at| at.elapsed() < BAR_HOLD + BAR_OUT);
    let layer = if pointed.over {
        Some(
            bar.with_animation(turn, Animation::new(BAR_IN), |el, t| el.opacity(t))
                .into_any_element(),
        )
    } else if fading {
        let held = BAR_HOLD.as_secs_f32() / (BAR_HOLD + BAR_OUT).as_secs_f32();
        Some(
            bar.with_animation(turn, Animation::new(BAR_HOLD + BAR_OUT), move |el, t| {
                el.opacity(if t < held {
                    1.
                } else {
                    1. - (t - held) / (1. - held)
                })
            })
            .into_any_element(),
        )
    } else {
        None
    };
    div()
        .id(ElementId::Name(format!("{id}-over").into()))
        .relative()
        .flex()
        .flex_col()
        .min_h_0()
        .child(area)
        .children(layer)
        // Where the pointer is, against the area's bounds, on every move,
        // and out when it leaves the window: not hover, which the bar's own
        // layer blocks once shown (it flickered in and out, 2026-10-21).
        .child(
            gpui_kit::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let moved = state.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            pointed_at(&moved, bounds.contains(&e.position), window, cx);
                        }
                    });
                    let left = state.clone();
                    window.on_mouse_event(move |_: &MouseExitEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            pointed_at(&left, false, window, cx);
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// The pointer came over the area (or left it): the bar comes (or holds,
/// fades and goes).
fn pointed_at(state: &Entity<Pointed>, over: bool, window: &mut Window, cx: &mut App) {
    if state.read(cx).over == over {
        return;
    }
    state.update(cx, |p, _| {
        p.over = over;
        p.at = Some(Instant::now());
        p.turn = p.turn.wrapping_add(1);
    });
    window.refresh();
    if !over {
        // Once it has faded, it goes.
        window
            .spawn(cx, async move |cx| {
                cx.background_executor().timer(BAR_HOLD + BAR_OUT).await;
                let _ = cx.update(|window, _| window.refresh());
            })
            .detach();
    }
}

/// A list already pointed at (an open drop-down): its scrollbar shows
/// while it's open and longer than it is tall.
pub fn over_shown(area: impl IntoElement, scroll: &ScrollHandle) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .min_h_0()
        .child(area)
        .vertical_scrollbar(scroll)
}
