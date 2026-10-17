//! Settings, About: the bird, the name, the version, "Is it safe?"
//! (design/proposals/how-secure.html, A, chosen by the user), and a coffee
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
            .keys()
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
                .child(bird(px(56.), "crake/bird.svg", "crake/call.svg", t))
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
        .child(safe(t))
        .child(coffee(t))
}

/// A colleague's question before trying Crake ("Is it safe? Can it break
/// my traffic?"), answered where people look when they wonder what an app
/// does. True on every system Crake runs on.
const SAFE: [(&str, &str); 4] = [
    (
        "Does anything leave this computer?",
        "No. Crake decides everything here: there\u{2019}s no Crake server, no account and no telemetry. Only your VPN\u{2019}s own sign-in talks to your company.",
    ),
    (
        "Can Crake break my internet?",
        "If Crake stops, its routes and rules go with it, and your traffic goes straight out as it would without Crake. A tunnel that drops takes only what went through it.",
    ),
    (
        "What does my company see?",
        "Only what goes through its tunnel: the hosts that need it. Everything else goes straight out, not through the company.",
    ),
    (
        "Who can change what?",
        "Changes need you, at this computer. A tunnel your IT set up can be switched on and off, but its rules can\u{2019}t be widened.",
    ),
];

fn safe(t: &Theme) -> impl IntoElement {
    div()
        .mt(px(30.))
        .child(
            div()
                .pb(px(8.))
                .border_b_1()
                .border_color(t.line)
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD)
                .child("Is it safe?"),
        )
        .children(SAFE.iter().map(|(q, a)| {
            div()
                .py(px(11.))
                .border_b_1()
                .border_color(t.line)
                .child(div().font_weight(FontWeight::SEMIBOLD).child(*q))
                .child(div().mt(px(2.)).text_color(t.muted).child(*a))
        }))
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
                .keys()
                .on_click(|_, _, cx| cx.open_url(COFFEE))
                .test_support(),
        )
}
