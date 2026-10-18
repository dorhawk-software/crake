//! The parts sign-in's cards share.

use super::*;

pub(super) fn title(text: String) -> Div {
    div()
        .text_size(px(19.))
        .line_height(px(25.))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

pub(super) fn para(text: String, color: gpui_kit::Rgba) -> Div {
    div().mt(px(6.)).text_color(color).child(text)
}

/// What a gateway's field is called in Crake's sentence: "ACME asks for
/// your group". A name not in the table gets none, and the title keeps the
/// gateway's word (design/proposals/audit-2026-10-17.html, 8).
pub(super) fn asks_for(label: &str) -> Option<&'static str> {
    let l = label.trim().trim_end_matches(':').to_ascii_lowercase();
    Some(match l.as_str() {
        "group" => "your group",
        "username" | "user name" | "user" | "login" => "your username",
        "password" => "your password",
        "passcode" | "otp" | "token" | "code" | "one-time password" | "one-time code"
        | "verification code" => "a code",
        "second password" | "secondary password" | "password 2" => "your second password",
        "pin" => "your PIN",
        _ => return None,
    })
}

pub(super) fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_lowercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Reached the gateway, signed in, tunnel up: done (two-stroke tick), now
/// (dots, bold), later (empty circle); or failed there (a red cross, bold).
/// The gateway's name is a machine fact: mono.
/// `signing`: the sign-in step's words while it's under way ("Waiting for
/// you in Firefox"), "Signing in" otherwise.
pub(super) fn steps(
    at: usize,
    failed: Option<(usize, String)>,
    signing: Option<String>,
    spec: &Spec,
    t: &Theme,
) -> Div {
    let failed_row = failed.as_ref().map(|(_, row)| row.clone());
    let failed = failed.map(|(at, _)| at);
    let gateway = || {
        div()
            .font_family(fonts::MONO)
            .text_size(px(13.))
            .child(spec.gateway.clone())
    };
    let rows = [
        (
            div().flex().gap(px(4.)).child("Reached").child(gateway()),
            div().flex().gap(px(4.)).child("Reaching").child(gateway()),
        ),
        (
            div().child("Signed in"),
            div().child(signing.unwrap_or_else(|| "Signing in".into())),
        ),
        (
            div().child("Tunnel up"),
            div().child("Bringing the tunnel up"),
        ),
    ];
    let mut list = div()
        .mt(px(18.))
        .border_1()
        .border_color(t.line)
        .rounded(px(8.));
    for (i, (done, doing)) in rows.into_iter().enumerate() {
        if failed.is_some_and(|f| i > f) {
            break;
        }
        let (mark, text, bold, color) = if failed == Some(i) {
            (
                "crake/cross.svg",
                div().child(failed_row.clone().unwrap_or_default()),
                true,
                t.danger,
            )
        } else if i < at {
            ("crake/mark-routed.svg", done, false, t.accent)
        } else if i == at {
            ("crake/dots.svg", doing, true, t.accent)
        } else {
            ("crake/circle.svg", done, false, t.line_strong)
        };
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(11.))
                .px(px(12.))
                .when(i > 0, |r| r.border_t_1().border_color(t.line))
                .when(i > at && failed.is_none(), |r| r.text_color(t.muted))
                .when(bold, |r| r.font_weight(FontWeight::BOLD))
                .child(svg().path(mark).size(px(18.)).flex_none().text_color(color))
                .child(text),
        );
    }
    list
}

pub(super) fn buttons() -> Div {
    div().flex().items_center().gap(px(8.)).mt(px(18.))
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Look {
    /// Teal, the action that moves on.
    Primary,
    /// Hairline outline, a secondary action.
    Outline,
    /// Text only (Cancel).
    Text,
}

pub(super) fn button(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    look: Look,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(40.))
        .px(px(if look == Look::Primary { 18. } else { 14. }))
        .rounded(px(6.))
        .font_weight(if look == Look::Primary {
            FontWeight::BOLD
        } else {
            FontWeight::SEMIBOLD
        })
        .cursor_pointer()
        .when(look == Look::Primary, |b| {
            b.bg(t.accent).text_color(t.accent_ink)
        })
        .when(look == Look::Outline, |b| {
            b.border_1()
                .border_color(t.line)
                .bg(t.panel)
                .text_color(t.ink)
                .hover(|s| s.bg(t.surface))
        })
        .when(look == Look::Text, |b| {
            b.text_color(t.ink).hover(|s| s.bg(t.surface))
        })
        .child(label)
        .on_mouse_down(gpui_kit::MouseButton::Left, keep_press)
}

/// A small accent link that acts (sign-in.html's "Copy for helpdesk",
/// beside the words it copies).
pub(super) fn link(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.accent)
        .cursor_pointer()
        .hover(|s| s.underline())
        .child(label)
        .on_mouse_down(gpui_kit::MouseButton::Left, keep_press)
}

/// A press on a control is its own: the card's window drag doesn't take it.
fn keep_press(_: &gpui_kit::MouseDownEvent, _: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    cx.stop_propagation();
}

pub(super) fn checkbox(
    id: &'static str,
    on: bool,
    label: &'static str,
    t: &Theme,
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
                .flex()
                .items_center()
                .justify_center()
                .when(on, |b| b.bg(t.accent))
                .when(!on, |b| b.border_1().border_color(t.line_strong))
                .when(on, |b| {
                    b.child(
                        svg()
                            .path("crake/check.svg")
                            .size(px(14.))
                            .text_color(t.accent_ink),
                    )
                }),
        )
        .child(label)
        .on_mouse_down(gpui_kit::MouseButton::Left, keep_press)
}
