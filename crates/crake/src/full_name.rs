//! A host's whole name, wherever a list cuts it short with "…"
//! (design/proposals/host-full-name.html, option A). Pointing at a name
//! underlines it, and after a moment a card shows the whole name with
//! "Click to copy" (only "Click to copy" when it fits); a click copies the
//! whole name. Every name behaves alike, long or short. Two departures
//! from the proposal, both GPUI's: the underline is a thin solid one (GPUI
//! draws solid or wavy ones only, not dotted), and "Copied" shows right
//! after the name for a moment, not in the card (GPUI closes a card when
//! the button goes down).

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{
    App, ClipboardItem, ElementId, Global, Pixels, SharedString, TestSupportExt, TextRun, Window,
    canvas, div, font, px,
};

use crate::focus::Keys as _;
use crate::theme::{Theme, fonts};

/// How long "Copied" shows after a click.
const COPIED_FOR: Duration = Duration::from_secs(3);
/// The card's whole name at most this size (px), as the proposal draws it.
const CARD_NAME_SIZE: f32 = 13.;
/// The card's widest: a long name wraps inside it.
const CARD_WIDTH: f32 = 560.;

/// The name copied last, and when ("Copied" shows after it).
struct Copied(SharedString, Instant);
impl Global for Copied {}

/// `name` in a list row: monospace, cut with "…" when it doesn't fit,
/// with the card and the copy described above. `id` is unique in its
/// window (a row's index, say).
pub fn host_name(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    size: f32,
    t: &Theme,
    cx: &App,
) -> impl IntoElement {
    let name: SharedString = name.into();
    let copied = just_copied(&name, cx);
    // How wide the name's box is, as last drawn: the card says the whole
    // name only when it's wider than that.
    let shown = Rc::new(Cell::new(Pixels::MAX));
    let (tip, copy, measure) = (name.clone(), name.clone(), Rc::clone(&shown));
    let name_box = div()
        .id(id)
        .relative()
        .min_w_0()
        .font_family(fonts::MONO)
        .text_size(px(size))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|s| s.text_decoration_1().text_decoration_color(t.muted))
        .child(name)
        .child(
            canvas(
                move |bounds, _, _| measure.set(bounds.size.width),
                |_, (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .tooltip(move |window, cx| {
            let cut = width_of(&tip, size, window) > shown.get();
            let name = tip.clone();
            cx.new(|_| Card { name, cut, size }).into()
        })
        .keys()
        .on_click(move |_, window, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string()));
            cx.set_global(Copied(copy.clone(), Instant::now()));
            window.refresh();
            // And gone again.
            cx.spawn(async move |cx| {
                cx.background_executor().timer(COPIED_FOR).await;
                cx.update(App::refresh_windows);
            })
            .detach();
        })
        .test_support();
    // Sized to the name (and "Copied"), at most the room there is: the
    // name takes what's left, so it's cut with "…" only when it must, and
    // "Copied" sits right after it. (A name that's a plain flex item kept
    // its whole width and lost the "…".)
    let row = div()
        .flex()
        .items_baseline()
        .max_w_full()
        .child(name_box.flex_1().min_w_0())
        .when(copied, |d| d.child(crate::controls::copied(t)));
    div().flex().flex_col().items_start().min_w_0().child(row)
}

/// How wide `text` is drawn in the monospace font at `size`.
fn width_of(text: &SharedString, size: f32, window: &mut Window) -> Pixels {
    let run = TextRun {
        len: text.len(),
        font: font(fonts::MONO),
        color: gpui_kit::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.clone(), px(size), &[run], None)
        .width
}

/// Whether `name` was copied a moment ago.
pub(crate) fn just_copied(name: &SharedString, cx: &App) -> bool {
    cx.try_global::<Copied>()
        .is_some_and(|c| c.0 == *name && c.1.elapsed() < COPIED_FOR)
}

/// The card under the pointer.
struct Card {
    name: SharedString,
    /// The name is cut short where it's shown: the card says it whole.
    cut: bool,
    size: f32,
}

impl Render for Card {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        div()
            .max_w(px(CARD_WIDTH))
            .px(px(10.))
            .py(px(7.))
            .rounded(px(6.))
            .border_1()
            .border_color(t.line_strong)
            .bg(t.surface)
            .shadow_md()
            .when(self.cut, |d| {
                d.child(
                    div()
                        .font_family(fonts::MONO)
                        .text_size(px(self.size.min(CARD_NAME_SIZE)))
                        .text_color(t.ink)
                        .child(self.name.clone()),
                )
            })
            .child(
                div()
                    .text_size(px(12.))
                    .when(self.cut, |d| d.mt(px(2.)))
                    .text_color(t.muted)
                    .child("Click to copy"),
            )
    }
}
