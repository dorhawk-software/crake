//! Settings, About: the bird, the name, the version, and a coffee
//! (design/proposals/about-coffee.html, B, chosen by the user).

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
        .child(coffee(t))
}

/// Where a coffee goes (the same page as Obálka's and the website's).
const COFFEE: &str = "https://buymeacoffee.com/software.dorhawk";

/// Crake is free; a quiet card says a coffee is welcome and unlocks
/// nothing. Only here: no reminders anywhere else. (Crake isn't sold through
/// Apple's App Store, whose rules keep Obálka's out of its app.)
fn coffee(t: &Theme) -> impl IntoElement {
    div()
        .mt(px(22.))
        .py(px(14.))
        .px(px(16.))
        .rounded(px(10.))
        .bg(t.surface)
        // The button as wide as its words.
        .flex()
        .flex_col()
        .items_start()
        .child(div().text_color(t.muted).child({
            // "Crake is free and stays free:" in the ink colour, bold.
            let free = "Crake is free and stays free:";
            gpui_kit::StyledText::new(format!(
                "{free} no ads, no tracking, nothing locked. If it saved you a headache, you can buy its developer a coffee. It\u{2019}s a thank-you, not a subscription."
            ))
            .with_highlights([(
                0..free.len(),
                gpui_kit::HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    color: Some(t.ink.into()),
                    ..Default::default()
                },
            )])
        }))
        .child(
            div()
                .id("coffee")
                .mt(px(10.))
                .h(px(34.))
                .px(px(14.))
                .flex()
                .items_center()
                .border_1()
                .border_color(t.line)
                .rounded(px(6.))
                .bg(t.panel)
                .text_color(t.ink)
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .hover(|s| s.border_color(t.accent))
                .child("Buy me a coffee")
                .on_click(|_, _, cx| cx.open_url(COFFEE))
                .test_support(),
        )
}
