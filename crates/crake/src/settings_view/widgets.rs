//! The controls Settings' sections share.

use super::*;

/// A bordered group of rows.
pub(super) fn group(t: &Theme) -> Div {
    div().border_1().border_color(t.line).rounded(px(8.))
}

pub(super) fn sub(text: &str, t: &Theme) -> impl IntoElement {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(t.muted)
        .child(text.to_owned())
}

/// A setting: its label and explanation on the left, the control on the right.
pub(super) fn row(
    label: impl IntoElement,
    explain: Option<&str>,
    control: impl IntoElement,
    divided: bool,
    t: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .py(px(14.))
        .px(px(16.))
        .when(divided, |r| r.border_t_1().border_color(t.line))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(label))
                .when_some(explain, |c, e| c.child(sub(e, t))),
        )
        .child(div().flex_none().child(control))
}

pub(super) fn toggle(id: &'static str, on: bool, t: &Theme) -> Stateful<Div> {
    crate::panel::switch(id, on, t)
}

/// A button with a coloured outline.
pub(super) fn outline(
    id: &'static str,
    label: &'static str,
    border: gpui_kit::Rgba,
    ink: gpui_kit::Rgba,
    icon: Option<&'static str>,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(38.))
        .px(px(14.))
        .flex()
        .items_center()
        .gap(px(8.))
        .when_some(icon, |b, path| {
            b.child(svg().path(path).size(px(16.)).text_color(ink))
        })
        .border_1()
        .border_color(border)
        .rounded(px(6.))
        .text_color(ink)
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .child(label)
}

/// A 32 px button with an outline (Edit, Remove…), maybe with an icon.
pub(super) fn small_button(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    icon: Option<&'static str>,
    border: gpui_kit::Rgba,
    ink: gpui_kit::Rgba,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(32.))
        .px(px(12.))
        .flex()
        .items_center()
        .gap(px(6.))
        .border_1()
        .border_color(border)
        .rounded(px(6.))
        .bg(t.panel)
        .text_color(ink)
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
        .when_some(icon, |b, path| {
            b.child(svg().path(path).size(px(14.)).text_color(ink))
        })
        .child(label)
}

/// A drop-down like the design's `<select>`: 38 px, its options below.
#[allow(clippy::too_many_arguments)] // one control's whole description
pub(super) fn select(
    this: &Settings,
    id: &'static str,
    width: Pixels,
    options: &[(&'static str, &'static str)],
    current: &str,
    t: &Theme,
    cx: &mut Context<Settings>,
    pick: impl Fn(&mut Settings, String, &mut Context<Settings>) + Clone + 'static,
) -> Div {
    let chosen = options
        .iter()
        .find(|(v, _)| *v == current)
        .map_or(current.to_owned(), |(_, l)| (*l).to_owned());
    let open = this.open_select == Some(id);
    let mut picker = div().relative().child(
        div()
            .id(id)
            .w(width)
            .h(px(38.))
            .pl(px(14.))
            .pr(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
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
            }))
            .test_support(),
    );
    if open {
        let mut menu = div()
            .absolute()
            .top(px(42.))
            .right_0()
            .min_w(px(180.))
            .py(px(4.))
            .bg(t.panel)
            .border_1()
            .border_color(t.line_strong)
            .rounded(px(6.));
        for (i, (value, label)) in options.iter().enumerate() {
            let on = *value == current;
            let pick = pick.clone();
            let value = (*value).to_owned();
            menu = menu.child(
                div()
                    .id((id, i))
                    .px(px(10.))
                    .py(px(6.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .when(on, |b| b.bg(t.accent_soft))
                    .hover(|s| s.bg(t.surface))
                    .child(*label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_select = None;
                        pick(this, value.clone(), cx);
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        picker = picker.child(gpui_kit::deferred(menu.occlude()).with_priority(1));
    }
    picker
}
