//! New tunnel, step 2: how it signs in (browser, password, certificate).

use super::*;

impl NewTunnel {
    pub(super) fn sign_in(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        if self.protocol == "ipsec" {
            return self.ipsec_sign_in(t, cx);
        }
        if self.protocol == "openvpn" {
            return div()
                .child(heading("How do you sign in?"))
                .child(
                    div()
                        .mt(px(18.))
                        .py(px(14.))
                        .px(px(16.))
                        .rounded(px(8.))
                        .bg(t.surface)
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("As the profile says"),
                        )
                        .child(div().text_color(t.muted).child(
                            "Crake asks what the profile needs, a password, a code or a sign-in in your browser, when you switch the tunnel on.",
                        )),
                )
                .into_any_element();
        }
        let no_browser = self.no_browser();
        let mut methods = div().flex().flex_col().gap(px(8.)).mt(px(18.));
        for (i, (id, name, sub)) in METHODS.into_iter().enumerate() {
            let on = self.method == id;
            let unavailable = id == "browser" && no_browser.is_some();
            methods = methods.child(
                card(("method", i), on, name, sub, None, t)
                    .when(unavailable, |c| {
                        c.child(super::server::tag("Not available", t))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.method = id;
                        this.problem = None;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        let detail = self.method_detail(t, cx);
        let why = no_browser.filter(|_| self.method == "browser");
        div()
            .child(heading("How do you sign in?"))
            .child(methods)
            .when_some(why, |d, why| {
                d.child(
                    div()
                        .mt(px(10.))
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(t.ink)
                        .child(why),
                )
            })
            .child(detail)
            .child(
                div()
                    .mt(px(22.))
                    .pt(px(14.))
                    .border_t_1()
                    .border_color(t.line)
                    .child(
                        div()
                            .id("advanced")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .child(
                                svg()
                                    .path(if self.advanced {
                                        "crake/disclose-down.svg"
                                    } else {
                                        "crake/disclose-right.svg"
                                    })
                                    .size(px(10.))
                                    .text_color(t.ink),
                            )
                            .child("Advanced")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.advanced = !this.advanced;
                                cx.notify();
                            })),
                    )
                    .when(self.advanced, |d| {
                        d.child(
                            checkbox(
                                "client-version",
                                self.client_version,
                                "Report a different client version",
                                t,
                                cx.listener(|this, _, _, cx| {
                                    this.client_version = !this.client_version;
                                    cx.notify();
                                }),
                            )
                            .mt(px(12.)),
                        )
                        .child(
                            checkbox(
                                "weak-ciphers",
                                self.weak_ciphers,
                                "Allow weak ciphers",
                                t,
                                cx.listener(|this, _, _, cx| {
                                    this.weak_ciphers = !this.weak_ciphers;
                                    cx.notify();
                                }),
                            )
                            .mt(px(10.))
                            .child(
                                div()
                                    .text_color(t.muted)
                                    .child("(only if your gateway is very old)"),
                            ),
                        )
                    }),
            )
            .into_any_element()
    }

    /// What the chosen method needs: a username, where the certificate comes
    /// from, or how the browser sign-in goes.
    fn method_detail(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        match self.method {
            "password" => div()
                .mt(px(18.))
                .child(label("Username"))
                .child(div().max_w(px(320.)).child(field(&self.user, false, t).text_size(px(15.))))
                .child(
                    div()
                        .mt(px(8.))
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child("Your password goes to the system keyring, never into Crake\u{2019}s files."),
                )
                .into_any_element(),
            "certificate" => {
                let chosen = self.cert_file.as_ref().map(|(n, _)| n.clone());
                div()
                    .mt(px(18.))
                    .child(label("Certificate from"))
                    .child(div().max_w(px(320.)).child(self.dropdown(
                        "cert",
                        vec![
                            ("pkcs11".into(), "Smart card (PKCS#11)".into()),
                            ("keyring".into(), "System keyring".into()),
                            ("file".into(), "A file\u{2026}".into()),
                        ],
                        self.cert_source.to_owned(),
                        Some(px(320.)),
                        t,
                        cx,
                        |this, v, _| {
                            this.cert_source = match v.as_str() {
                                "pkcs11" => "pkcs11",
                                "keyring" => "keyring",
                                _ => "file",
                            }
                        },
                    )))
                    .when(self.cert_source == "file", |d| {
                        d.child(
                            div()
                                .mt(px(10.))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .id("choose-cert")
                                        .h(px(36.))
                                        .px(px(12.))
                                        .flex()
                                        .items_center()
                                        .border_1()
                                        .border_color(t.line)
                                        .rounded(px(6.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .cursor_pointer()
                                        .child(if chosen.is_some() { "Choose another\u{2026}" } else { "Choose a file\u{2026}" })
                                        .on_click(cx.listener(|this, _, _, cx| this.choose_certificate(cx))),
                                )
                                .when_some(chosen, |d, name| {
                                    d.child(
                                        div()
                                            .font_family(fonts::MONO)
                                            .text_size(px(13.))
                                            .text_color(t.muted)
                                            .child(name),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .mt(px(8.))
                                .text_size(px(13.))
                                .text_color(t.muted)
                                .child("Crake keeps a copy in the system keyring, so you can delete the file."),
                        )
                    })
                    .into_any_element()
            }
            _ => div()
                .child(checkbox(
                    "keep-signed-in",
                    self.keep_signed_in,
                    "Stay signed in for 12 hours, if the gateway allows it",
                    t,
                    cx.listener(|this, _, _, cx| {
                        this.keep_signed_in = !this.keep_signed_in;
                        cx.notify();
                    }),
                )
                .mt(px(18.)))
                .into_any_element(),
        }
    }
}
