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
            Ask::TrustServer {
                fingerprint,
                about,
                trusted_on,
            } => {
                return self.trust_question(
                    (
                        fingerprint.as_deref(),
                        about.as_deref(),
                        trusted_on.as_deref(),
                    ),
                    t,
                    cx,
                );
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
            Ask::CertificatePassword { retry, kept, .. } => {
                // A password that didn't open it says so in ink, as a
                // refused one does.
                let (said, colour) = match (retry, kept) {
                    (true, true) => (
                        "The password in your keyring didn\u{2019}t open it. Type it again.".into(),
                        t.ink,
                    ),
                    (true, false) => (
                        "That password didn\u{2019}t open it. Type it again.".into(),
                        t.ink,
                    ),
                    _ => (
                        format!(
                            "{display} signs you in with a certificate. Its password opens it."
                        ),
                        t.muted,
                    ),
                };
                body = body
                    .child(title("Unlock your certificate".into()))
                    .child(para(said, colour));
            }
            Ask::Secret | Ask::Text => {
                body = body
                    .child(title(format!(
                        "{display} asks for {}",
                        asks_for(&prompt.label)
                            .map_or_else(|| lower_first(&prompt.label), str::to_owned)
                    )))
                    // Asked again (OpenVPN): the gateway refused the last one.
                    .when(prompt.refused, |b| {
                        b.child(para(
                            format!("{display} didn\u{2019}t accept that username or password."),
                            t.ink,
                        ))
                    })
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
                    .child({
                        // A password can be shown while typed (the eye).
                        let input = Input::new(&self.field).appearance(false);
                        let secret =
                            matches!(prompt.ask, Ask::Secret | Ask::CertificatePassword { .. });
                        if secret { input.mask_toggle() } else { input }
                    }),
            );
        // Every password says whether it's kept (it was kept without a word
        // but a certificate's; design/proposals/ipsec-tunnel.html, 5); a
        // one-time code is never kept.
        if matches!(prompt.ask, Ask::CertificatePassword { .. })
            || crate::signin::asks_password(prompt)
        {
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
        }
        if let Ask::CertificatePassword {
            from_file: Some(path),
            ..
        } = &prompt.ask
        {
            {
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

    /// The gateway's certificate, no authority here vouching for it: trust
    /// it? (`fingerprint`, `about` whose it is, `trusted_on` the day the
    /// one it replaces was trusted.)
    fn trust_question(
        &self,
        (fingerprint, about, trusted_on): (Option<&str>, Option<&str>, Option<&str>),
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let display = &self.spec.display;
        // A certificate that changed since it was trusted says so, in ink
        // (design/proposals/ipsec-certificates.html, 3).
        let (why, colour) = match trusted_on {
            Some(day) => (
                format!(
                    "It shows a different certificate from the one you trusted on {day}. Trust it only if your IT says it was renewed."
                ),
                t.ink,
            ),
            None => (
                "Its certificate isn\u{2019}t signed by an authority this computer knows. Trust it only if your IT says this is the right one."
                    .into(),
                t.muted,
            ),
        };
        div()
            .flex()
            .flex_col()
            .child(title(format!(
                "Crake can\u{2019}t vouch for {display}\u{2019}s gateway"
            )))
            .child(para(why, colour))
            .when_some(about, |b, a| b.child(para(a.to_owned(), t.ink).mt(px(12.))))
            .when_some(fingerprint, |b, f| {
                b.child(
                    div()
                        .mt(px(12.))
                        .font_family(fonts::MONO)
                        .text_size(px(12.))
                        .text_color(t.muted)
                        .child(f.to_owned()),
                )
            })
            .child(
                buttons()
                    .child(div().flex_1())
                    .child(
                        button("no", "Don\u{2019}t connect", Look::Text, t)
                            .track_focus(&self.dont_connect)
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| this.reply("no".into(), cx)))
                            .test_support(),
                    )
                    .child(
                        button("yes", "Trust it", Look::Primary, t)
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| this.reply("yes".into(), cx)))
                            .test_support(),
                    ),
            )
    }
}
