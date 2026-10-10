//! The controls New tunnel's steps share.

use super::*;

impl NewTunnel {
    /// A drop-down like the design's `<select>`.
    #[allow(clippy::too_many_arguments)] // one control's whole description
    pub(super) fn dropdown(
        &self,
        id: &'static str,
        options: Vec<(String, String)>,
        current: String,
        width: Option<f32>,
        t: &Theme,
        cx: &mut Context<Self>,
        pick: impl Fn(&mut Self, String) + Clone + 'static,
    ) -> Div {
        let chosen = options
            .iter()
            .find(|(v, _)| *v == current)
            .map_or(current.clone(), |(_, l)| l.clone());
        let open = self.open_select == Some(id);
        let mut picker = div().relative().child(
            div()
                .id(id)
                .when_some(width, |d, w| d.w(px(w)))
                .when(width.is_none(), |d| d.w_full())
                .h(px(42.))
                .pl(px(12.))
                .pr(px(10.))
                .flex()
                .items_center()
                .justify_between()
                .border_1()
                .border_color(t.line_strong)
                .rounded(px(6.))
                .bg(t.panel)
                .cursor_pointer()
                .child(chosen)
                .child(
                    svg()
                        .path("crake/chevron-down.svg")
                        .size(px(14.))
                        .text_color(t.ink),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_select = if open { None } else { Some(id) };
                    cx.notify();
                })),
        );
        if open {
            let mut menu = div()
                .absolute()
                .top(px(46.))
                .left_0()
                .min_w(px(180.))
                .py(px(4.))
                .bg(t.panel)
                .border_1()
                .border_color(t.line_strong)
                .rounded(px(6.));
            for (i, (value, label)) in options.into_iter().enumerate() {
                let on = value == current;
                let pick = pick.clone();
                menu = menu.child(
                    div()
                        .id((id, i))
                        .px(px(10.))
                        .py(px(6.))
                        .whitespace_nowrap()
                        .cursor_pointer()
                        .when(on, |b| b.bg(t.accent_soft))
                        .hover(|s| s.bg(t.surface))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_select = None;
                            pick(this, value.clone());
                            cx.notify();
                        })),
                );
            }
            picker = picker.child(gpui_kit::deferred(menu.occlude()).with_priority(1));
        }
        picker
    }
}

pub(super) fn heading(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(22.))
        .line_height(px(32.))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

pub(super) fn label(text: &'static str) -> impl IntoElement {
    div()
        .mb(px(6.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

/// A 42 px text field.
pub(super) fn field(input: &Entity<InputState>, mono: bool, t: &Theme) -> Div {
    div()
        .h(px(42.))
        // The input pads itself; together they make the design's 12 px.
        .px(px(1.))
        .flex()
        .items_center()
        .border_1()
        .border_color(t.line_strong)
        .rounded(px(6.))
        .bg(t.panel)
        .when(mono, |d| d.font_family(fonts::MONO).text_size(px(14.)))
        .child(Input::new(input).appearance(false))
}

pub(super) fn radio(on: bool, t: &Theme) -> impl IntoElement {
    div()
        .size(px(18.))
        .flex_none()
        .rounded_full()
        .border(px(1.5))
        .border_color(if on { t.accent } else { t.line_strong })
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| {
            d.child(div().size(px(8.)).rounded_full().bg(t.accent))
        })
}

/// A choice as a card: radio, name (and tag), explanation.
pub(super) fn card(
    id: impl Into<gpui_kit::ElementId>,
    on: bool,
    name: &'static str,
    sub: &'static str,
    tag: Option<&'static str>,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .min_w_0()
        .flex()
        .items_start()
        .gap(px(12.))
        // The chosen card's 2 px edge takes a pixel from its padding.
        .when(on, |c| {
            c.py(px(13.)).px(px(15.)).border_2().border_color(t.accent)
        })
        .when(!on, |c| {
            c.py(px(14.)).px(px(16.)).border_1().border_color(t.line)
        })
        .rounded(px(8.))
        .bg(t.panel)
        .cursor_pointer()
        .child(div().mt(px(2.)).child(radio(on, t)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        .text_size(px(15.))
                        .line_height(px(22.))
                        .font_weight(FontWeight::BOLD)
                        .child(name)
                        .when_some(tag, |d, tag| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(t.accent)
                                    .child(tag),
                            )
                        }),
                )
                .child(div().text_color(t.muted).child(sub)),
        )
}

/// A checkbox with its label.
pub(super) fn checkbox(
    id: &'static str,
    on: bool,
    text: &'static str,
    t: &Theme,
    toggle: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.))
        .cursor_pointer()
        .child(
            div()
                .size(px(18.))
                .flex_none()
                .rounded(px(4.))
                .border(px(1.5))
                .border_color(if on { t.accent } else { t.line_strong })
                .when(on, |d| d.bg(t.accent))
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| {
                    d.child(
                        svg()
                            .path("crake/check.svg")
                            .size(px(12.))
                            .text_color(t.accent_ink),
                    )
                }),
        )
        .child(text)
        .on_click(toggle)
}
