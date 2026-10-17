//! The log's rows: one decision or event each, with its reason and time.

use super::*;

/// A day's rows: time, mark, the sentence and its reason, the tunnel.
pub(super) fn rows(rows: Vec<Row>, empty: &'static str, t: &Theme) -> Div {
    let mut list = div().px(px(20.));
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
        list = list.child(row(r, t));
    }
    list
}

pub(super) fn row(r: Row, t: &Theme) -> AnyElement {
    let (mark, color) = match r.kind {
        LogKind::Routed => ("crake/mark-routed.svg", t.accent),
        LogKind::LeftAlone => ("crake/mark-left-alone.svg", t.muted),
        LogKind::Undone => ("crake/mark-undone.svg", t.muted),
        LogKind::System => ("crake/mark-system.svg", t.muted),
        LogKind::Asked => ("crake/help.svg", t.accent),
    };
    div()
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
                .child(r.text)
                .child(div().text_size(px(13.)).text_color(t.muted).child(r.why)),
        )
        // A host left alone has no chip: a badge holding a dash said nothing.
        .when(r.badge != "\u{2014}", |row| {
            row.child(
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
                    .child(r.badge),
            )
        })
        .into_any_element()
}
