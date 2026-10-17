//! Sign-in: a question from the gateway or openconnect, and its answer.

use super::widgets::asks_for;
use super::*;

impl SignIn {
    pub(super) fn question(
        &self,
        prompt: &Prompt,
        t: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        use gpui_kit::Focusable;
        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let display = &self.spec.display;
        let mut body = div().flex().flex_col();
        match &prompt.ask {
            Ask::TrustServer { fingerprint } => {
                body = body
                    .child(title(format!("Crake can\u{2019}t vouch for {display}\u{2019}s gateway")))
                    .child(para(
                        "Its certificate isn\u{2019}t signed by an authority this computer knows. Trust it only if your IT says this is the right one."
                            .into(),
                        t.muted,
                    ))
                    .when_some(fingerprint.clone(), |b, f| {
                        b.child(div().mt(px(12.)).font_family(fonts::MONO).text_size(px(12.)).text_color(t.muted).child(f))
                    })
                    .child(
                        buttons().child(div().flex_1())
                            .child(button("no", "Don\u{2019}t connect", Look::Text, t).on_click(cx.listener(|this, _, _, cx| {
                                this.reply("no".into(), cx)
                            })).test_support())
                            .child(button("yes", "Trust it", Look::Primary, t).on_click(cx.listener(|this, _, _, cx| {
                                this.reply("yes".into(), cx)
                            })).test_support()),
                    );
                return body;
            }
            Ask::Choice(options) => {
                body = body
                    .child(title(match asks_for(&prompt.label) {
                        Some(what) => format!("{display} asks for {what}"),
                        None => format!("{display} asks: {}", prompt.label),
                    }))
                    .when_some(prompt.words.clone(), |b, w| {
                        b.child(para(
                            format!("The gateway\u{2019}s own words: \u{201c}{w}\u{201d}"),
                            t.muted,
                        ))
                    });
                let mut list = div().flex().flex_col().gap(px(8.)).mt(px(16.));
                for (i, option) in options.iter().enumerate() {
                    let o = option.clone();
                    list = list.child(
                        button(("choice", i), "", Look::Outline, t)
                            .h(px(40.))
                            .child(option.clone())
                            .keys()
                            .on_click(cx.listener(move |this, _, _, cx| this.reply(o.clone(), cx)))
                            .test_support(),
                    );
                }
                return body.child(list).child(
                    buttons().child(div().flex_1()).child(
                        button("cancel", "Cancel", Look::Text, t)
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
                            .test_support(),
                    ),
                );
            }
            Ask::CertificatePassword { retry, .. } => {
                body = body
                    .child(title("Unlock your certificate".into()))
                    .child(para(
                        if *retry {
                            "The password in your keyring didn\u{2019}t open it. Type it again."
                                .into()
                        } else {
                            format!(
                                "{display} signs you in with a certificate. Its password opens it."
                            )
                        },
                        t.muted,
                    ));
            }
            Ask::Secret | Ask::Text => {
                body = body
                    .child(title(format!(
                        "{display} asks for {}",
                        asks_for(&prompt.label)
                            .map_or_else(|| lower_first(&prompt.label), str::to_owned)
                    )))
                    .when_some(prompt.words.clone(), |b, w| {
                        b.child(para(
                            format!("The gateway\u{2019}s own words: \u{201c}{w}\u{201d}"),
                            t.muted,
                        ))
                    });
            }
        }
        body = body
            .child(
                div()
                    .mt(px(16.))
                    .mb(px(8.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(prompt.label.clone()),
            )
            .child(
                div()
                    .h(px(44.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    // As in the design: 1 px line, 2 px teal while typing in it.
                    .when(focused, |b| b.border_2().border_color(t.accent))
                    .when(!focused, |b| b.border_1().border_color(t.line_strong))
                    .rounded(px(6.))
                    .child(Input::new(&self.field).appearance(false)),
            );
        if let Ask::CertificatePassword { from_file, .. } = &prompt.ask {
            body = body.child(
                checkbox(
                    "remember",
                    self.remember,
                    "Remember the password in your keyring",
                    t,
                )
                .mt(px(14.))
                .keys()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.remember = !this.remember;
                    cx.notify();
                }))
                .test_support(),
            );
            if let Some(path) = from_file {
                let home = std::env::var("HOME").unwrap_or_default();
                let shown = path.display().to_string().replacen(&home, "~", 1);
                body = body.child(
                    checkbox("keep", self.keep_certificate, "Keep a copy of the certificate in your keyring", t)
                        .mt(px(8.))
                        .keys()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.keep_certificate = !this.keep_certificate;
                            cx.notify();
                        })).test_support(),
                )
                .when(self.keep_certificate, |b| {
                    b.child(
                        div()
                            .ml(px(28.))
                            .mt(px(2.))
                            .text_size(px(13.))
                            .text_color(t.muted)
                            .child("Crake keeps its own encrypted copy, so this file can then be deleted:")
                            .child(div().font_family(fonts::MONO).text_size(px(12.)).child(shown)),
                    )
                });
            }
        }
        body.child(
            buttons()
                .child(div().flex_1())
                .child(
                    button("cancel", "Cancel", Look::Text, t)
                        .keys()
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
                        .test_support(),
                )
                .child(
                    button("continue", "Continue", Look::Primary, t)
                        .keys()
                        .on_click(cx.listener(|this, _, _, cx| this.answer(cx)))
                        .test_support(),
                ),
        )
    }
}
