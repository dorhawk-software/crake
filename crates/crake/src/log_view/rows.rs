//! The log's rows: one decision or event each, with its reason and time.
//! A routed row, pointed at or reached with the keyboard, shows "Show in
//! ACME" in its badge's place (design/screens/log.html?pointed=13:21).

use super::*;

impl Log {
    /// A day's rows: time, mark, the sentence and its reason, the tunnel.
    pub(super) fn rows(
        &mut self,
        day: &str,
        rows: Vec<Row>,
        empty: &'static str,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut list = div().px(px(LIST_INSET));
        if rows.is_empty() && !empty.is_empty() {
            return list.child(
                div()
                    .py(px(20.))
                    .border_t_1()
                    .border_color(t.line)
                    .text_color(t.muted)
                    .child(empty),
            );
        }
        for r in rows {
            let routed = r.show_in.clone().map(|(tunnel, host)| {
                let key = key(day, &r);
                let focus = self
                    .row_focus
                    .entry(key.clone())
                    .or_insert_with(|| cx.focus_handle())
                    .clone();
                let lit = self.pointed.as_deref() == Some(key.as_str()) || focus.is_focused(window);
                Routed {
                    key,
                    focus,
                    lit,
                    tunnel,
                    host,
                }
            });
            list = list.child(match routed {
                Some(routed) => self.routed_row(r, routed, t, cx),
                None => row(r, None, t).into_any_element(),
            });
        }
        list
    }

    /// A routed row: the pointer over it, or the keyboard on it, tints it to
    /// the window's edges and puts "Show in ACME" where its badge was. The
    /// badge is the row's tab stop, so the keyboard finds it too.
    fn routed_row(
        &mut self,
        r: Row,
        routed: Routed,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Routed {
            key,
            focus,
            lit,
            tunnel,
            host,
        } = routed;
        let stop = if lit {
            crate::controls::action(
                gpui_kit::SharedString::from(format!("show-in:{key}")),
                format!("Show in {}", r.badge),
                t,
            )
            .flex_none()
            .pt(px(1.))
        } else {
            badge(r.badge.clone(), t).id(gpui_kit::SharedString::from(format!("show-in:{key}")))
        }
        .track_focus(&focus)
        .keys()
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.emit(ShowIn(tunnel.clone(), host.clone()));
        }))
        .test_support()
        .into_any_element();
        let pointed = key.clone();
        let nav_hover = t.nav_hover;
        row(r, Some(stop), t)
            .id(gpui_kit::SharedString::from(format!("row:{key}")))
            .when(lit, |row| {
                // The tint runs under the list's inset to the window's edges;
                // the hairline above stays the list's width.
                row.bg(nav_hover).shadow(vec![
                    edge(nav_hover, -LIST_INSET),
                    edge(nav_hover, LIST_INSET),
                ])
            })
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.pointed = Some(pointed.clone());
                } else if this.pointed.as_deref() == Some(pointed.as_str()) {
                    this.pointed = None;
                }
                cx.notify();
            }))
            .into_any_element()
    }
}

/// A routed row's tunnel and host, and whether it's lit (pointed at, or
/// the keyboard on it).
struct Routed {
    key: String,
    focus: FocusHandle,
    lit: bool,
    tunnel: String,
    host: String,
}

/// The rows' inset from the window's edges (px).
const LIST_INSET: f32 = 20.;

/// A row's key, kept while new rows come in above it.
fn key(day: &str, r: &Row) -> String {
    format!("{day}|{}|{}", r.time, r.text)
}

/// The tint's copy beside the row, `dx` px across.
fn edge(color: gpui_kit::Rgba, dx: f32) -> gpui_kit::BoxShadow {
    gpui_kit::BoxShadow {
        color: color.into(),
        offset: gpui_kit::point(px(dx), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(0.),
        inset: false,
    }
}

/// One row; `stop` takes the badge's place (a routed row's badge or "Show in").
pub(super) fn row(r: Row, stop: Option<AnyElement>, t: &Theme) -> Div {
    let (mark, color) = match r.kind {
        LogKind::Routed => ("crake/mark-routed.svg", t.accent),
        LogKind::LeftAlone => ("crake/mark-left-alone.svg", t.muted),
        LogKind::Undone => ("crake/mark-undone.svg", t.muted),
        LogKind::System => ("crake/mark-system.svg", t.muted),
        LogKind::Asked => ("crake/help.svg", t.accent),
    };
    let row = div()
        .flex()
        .items_start()
        .gap(px(12.))
        .py(px(11.))
        .border_t_1()
        .border_color(t.line)
        .child(
            div()
                .w(px(52.))
                .flex_none()
                .pt(px(1.))
                .font_family(fonts::MONO)
                .text_size(px(13.))
                .text_color(t.muted)
                .child(r.time),
        )
        .child(
            div()
                .w(px(20.))
                .flex_none()
                .mt(px(1.))
                .child(svg().path(mark).size(px(18.)).text_color(color)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(crate::theme::breakable(&r.text))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(crate::theme::breakable(&r.why)),
                ),
        );
    if let Some(stop) = stop {
        return row.child(stop);
    }
    // A host left alone has no chip: a badge holding a dash said nothing.
    row.when(r.badge != "\u{2014}", |row| row.child(badge(r.badge, t)))
}

/// The tunnel's badge at a row's end.
fn badge(text: String, t: &Theme) -> Div {
    div()
        .flex_none()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(t.surface)
        .text_color(t.muted)
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}
