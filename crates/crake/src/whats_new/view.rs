//! The What's new window: every window's title row, then each version's
//! number and date and its changes under their kinds, newest first.

use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, FocusHandle, FontWeight, ScrollHandle, Subscription, TestSupportExt,
    Window, div, point, px,
};

use super::changelog::{KINDS, Release, releases};
use crate::focus::Keys as _;
use crate::theme::{Theme, fonts};

pub const WIDTH: f32 = 600.;
pub const HEIGHT: f32 = 640.;

/// An arrow key's scroll.
const STEP: f32 = 40.;
/// What a page's scroll keeps of the page before.
const OVERLAP: f32 = 40.;

pub struct WhatsNew {
    /// The version seen before, when opened from the panel's notice.
    since: Option<String>,
    earlier: bool,
    scroll: ScrollHandle,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for WhatsNew {}

impl WhatsNew {
    pub fn new(since: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let subscriptions = vec![cx.observe_window_appearance(window, |_, _, cx| {
            Theme::current(cx).sync_components(cx);
            cx.notify();
        })];
        Self {
            since,
            earlier: false,
            scroll: ScrollHandle::new(),
            focus,
            _subscriptions: subscriptions,
        }
    }

    /// The arrow keys, Page Up and Down, Home and End scroll the versions.
    fn scroll_by_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let page = (self.scroll.bounds().size.height.as_f32() - OVERLAP).max(STEP);
        let max = self.scroll.max_offset().y.as_f32();
        let y = self.scroll.offset().y.as_f32();
        let to = match key {
            "up" => y + STEP,
            "down" => y - STEP,
            "pageup" => y + page,
            "pagedown" | "space" => y - page,
            "home" => 0.,
            "end" => -max,
            _ => return,
        };
        self.scroll
            .set_offset(point(px(0.), px(to.clamp(-max, 0.))));
        cx.notify();
    }

    fn header(&self, since: Option<String>, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        // macOS draws its own window buttons top left (`chrome`).
        let system = crate::chrome::system_buttons(cx);
        div()
            .id("header")
            .h(px(crate::chrome::HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(if system {
                crate::chrome::BUTTONS_WIDTH
            } else {
                20.
            }))
            .pr(px(12.))
            .border_b_1()
            .border_color(t.line)
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(
                div()
                    .id("whats-new-title")
                    .text_size(px(17.))
                    .font_weight(FontWeight::BOLD)
                    .child("What's new")
                    .test_support(),
            )
            .children(since.map(|s| {
                div()
                    .id("since")
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(format!("Since {s}, the version you had"))
                    .test_support()
            }))
            .child(div().flex_1())
            .when(!system, |header| {
                header.child(
                    crate::chrome::close_button(t)
                        .keys()
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))
                        .test_support(),
                )
            })
    }
}

/// One version: its number and date, then its changes under their kinds.
fn release(r: &Release, first: bool, t: &Theme) -> AnyElement {
    let mut groups: Vec<_> = r.groups.iter().filter(|g| !g.changes.is_empty()).collect();
    // Kinds in their order; an early release's own headings as written.
    if groups.iter().all(|g| KINDS.contains(&g.title.as_str())) {
        groups.sort_by_key(|g| KINDS.iter().position(|k| *k == g.title));
    }
    let mut el = div()
        .id(gpui_kit::ElementId::Name(
            format!("release-{}", r.version).into(),
        ))
        .when(!first, |d| d.mt(px(24.)).border_t_1().border_color(t.line))
        .child(
            div()
                .pt(px(20.))
                .flex()
                .items_baseline()
                .gap(px(10.))
                .child(
                    div()
                        .text_size(px(17.))
                        .font_weight(FontWeight::BOLD)
                        .child(r.version.clone()),
                )
                .children(r.date.clone().map(|d| {
                    div()
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(d)
                })),
        );
    for g in groups {
        if !g.title.is_empty() {
            el = el.child(
                div()
                    .mt(px(14.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.muted)
                    .child(g.title.clone()),
            );
        }
        for (i, c) in g.changes.iter().enumerate() {
            el = el.child(
                div()
                    .pt(px(if i == 0 { 6. } else { 12. }))
                    .children(c.headline.clone().map(|h| {
                        div()
                            .text_size(px(15.))
                            .line_height(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(h)
                    }))
                    .when(!c.body.is_empty(), |d| {
                        d.child(div().text_color(t.muted).child(c.body.clone()))
                    }),
            );
        }
    }
    el.test_support().into_any_element()
}

impl Render for WhatsNew {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let shown = super::shown_in(releases(), crate::versions::this(), self.since.as_deref());
        let mut body = div().px(px(20.)).pb(px(20.));
        for (i, r) in shown.full.iter().enumerate() {
            body = body.child(release(r, i == 0, &t));
        }
        if self.earlier {
            for r in &shown.earlier {
                body = body.child(release(r, false, &t));
            }
        } else if !shown.earlier.is_empty() {
            body = body.child(
                div().flex().child(
                    div()
                        .id("earlier")
                        .mt(px(22.))
                        // A small action's look (DESIGN.md): ink, semibold, underlined.
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .underline()
                        .cursor_pointer()
                        .child("Show earlier versions")
                        .keys()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.earlier = true;
                            // The button goes: the window keeps the keys
                            // (Escape, the scroll keys).
                            window.focus(&this.focus, cx);
                            cx.notify();
                        }))
                        .test_support(),
                ),
            );
        }
        div()
            .id("whats-new")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(t.panel)
            .map(|card| crate::chrome::card_edge(card, t.line))
            .overflow_hidden()
            .text_color(t.ink)
            .font_family(fonts::UI)
            .text_size(px(14.))
            .line_height(px(20.))
            .on_key_down(crate::focus::tab_moves)
            .on_key_down(cx.listener(|this, e: &gpui_kit::KeyDownEvent, _, cx| {
                match e.keystroke.key.as_str() {
                    _ if crate::focus::closes_window(e) => cx.emit(Close),
                    "escape" => cx.emit(Close),
                    key => this.scroll_by_key(key, cx),
                }
            }))
            .child(self.header(shown.since.clone(), &t, cx))
            .child(
                crate::controls::over(
                    "versions",
                    div()
                        .id("versions")
                        .flex_1()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .child(body)
                        .test_support(),
                    &self.scroll,
                    window,
                    cx,
                )
                .flex_1(),
            )
    }
}
