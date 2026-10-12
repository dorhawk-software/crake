//! Settings, About: the bird, the name, the version.

use super::*;

/// The line bird with its teal call marks.
pub(super) fn bird(
    size: Pixels,
    body: &'static str,
    call: &'static str,
    t: &Theme,
) -> impl IntoElement {
    div()
        .relative()
        .size(size)
        .flex_none()
        .child(svg().path(body).size(size).text_color(t.ink).absolute())
        .child(svg().path(call).size(size).text_color(t.accent).absolute())
}

pub(super) fn about(t: &Theme, cx: &mut Context<Settings>) -> impl IntoElement {
    let link = |id: &'static str, label: &'static str, url: &'static str| {
        div()
            .id(id)
            .text_color(t.accent)
            .font_weight(FontWeight::SEMIBOLD)
            .underline()
            .cursor_pointer()
            .child(label)
            .on_click(move |_, _, cx| cx.open_url(url))
    };
    let _ = cx;
    div()
        .child(
            div()
                .mt(px(22.))
                .flex()
                .gap(px(16.))
                .items_center()
                .child(bird(px(56.), "crake/bird-thin.svg", "crake/call-thin.svg", t))
                .child(
                    div()
                        .child(
                            div()
                                .text_size(px(20.))
                                .line_height(px(28.))
                                .font_weight(FontWeight::BOLD)
                                .child(format!("Crake {}", env!("CARGO_PKG_VERSION"))),
                        )
                        .child(
                            div()
                                .text_color(t.muted)
                                .child("A VPN client that routes only what needs routing. Built on openconnect."),
                        )
                        .child(
                            div()
                                .text_color(t.muted)
                                .child("Because a VPN shouldn\u{2019}t be a chore."),
                        ),
                ),
        )
        .child(
            div()
                .mt(px(18.))
                .flex()
                .flex_wrap()
                .gap(px(10.))
                .child(link("github", "Source on GitHub", "https://github.com/dorhawk-software"))
                .child(div().text_color(t.muted).child("\u{b7}"))
                .child(link("site", "dorhawk.software", "https://dorhawk.software/")),
        )
}
