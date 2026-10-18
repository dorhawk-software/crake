//! New tunnel, step 3: what goes through it, how long hosts are kept, and
//! the sentence that sums the tunnel up.

use super::*;

impl NewTunnel {
    pub(super) fn routing(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        // Two by two, as the design's grid.
        let mut modes = div().flex().flex_col().gap(px(8.)).mt(px(18.));
        for pair in [0, 2] {
            let mut row = div().flex().gap(px(8.));
            for (i, (id, name, sub, tag)) in MODES.into_iter().enumerate().skip(pair).take(2) {
                let on = self.mode == id;
                row = row.child(
                    card(
                        ("mode", i),
                        on,
                        name,
                        sub,
                        Some(tag).filter(|t| !t.is_empty()),
                        t,
                    )
                    .flex_basis(px(0.))
                    .flex_grow(1.)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.mode = id;
                        cx.notify();
                    }))
                    .test_support(),
                );
            }
            modes = modes.child(row);
        }
        // "Only what I list": what goes through.
        let list_box = (self.mode == "list").then(|| {
            let mut chips = div().flex().flex_wrap().gap(px(6.)).mt(px(8.));
            for (i, item) in self.list.iter().enumerate() {
                chips = chips.child(
                    div()
                        .h(px(28.))
                        .pl(px(8.))
                        .pr(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .rounded(px(4.))
                        .bg(t.panel)
                        .border_1()
                        .border_color(t.line)
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .child(item.clone())
                        .child(
                            div()
                                .id(("unlist", i))
                                .size(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(svg().path("crake/close.svg").size(px(10.)).text_color(t.muted))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.list.remove(i);
                                    cx.notify();
                                })),
                        ),
                );
            }
            chips = chips.child(if self.adding_list {
                div()
                    .w(px(260.))
                    .h(px(28.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(t.accent)
                    .rounded(px(4.))
                    .bg(t.panel)
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .child(Input::new(&self.list_input).appearance(false))
                    .into_any_element()
            } else {
                div()
                    .id("add-listed")
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_dashed()
                    .border_color(t.line_strong)
                    .rounded(px(4.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child("+ Add")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.adding_list = true;
                        this.list_input.update(cx, |f, cx| f.focus(window, cx));
                        cx.notify();
                    }))
                    .test_support()
                    .into_any_element()
            });
            div()
                .mt(px(18.))
                .py(px(14.))
                .px(px(16.))
                .rounded(px(8.))
                .bg(t.surface)
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Send these through"))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child("Names (*.example.internal) and networks (10.40.0.0/16). Nothing else goes through it."),
                )
                .child(chips)
        });
        // What the gateway pushed when it last connected.
        let suggests = (!self.suggested.is_empty()).then(|| {
            let mut chips = div().flex().flex_wrap().gap(px(6.)).mt(px(8.));
            for s in &self.suggested {
                chips = chips.child(
                    div()
                        .py(px(3.))
                        .px(px(8.))
                        .rounded(px(4.))
                        .bg(t.panel)
                        .border_1()
                        .border_color(t.line)
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .child(s.clone()),
                );
            }
            div()
                .mt(px(18.))
                .py(px(14.))
                .px(px(16.))
                .rounded(px(8.))
                .bg(t.surface)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("The gateway suggests"),
                )
                .child(chips)
                .child(
                    checkbox(
                        "send-suggested",
                        self.send_suggested,
                        "Send these through from the start, without waiting",
                        t,
                        cx.listener(|this, _, _, cx| {
                            this.send_suggested = !this.send_suggested;
                            cx.notify();
                        }),
                    )
                    .mt(px(12.)),
                )
        });
        let summary = self.summary(cx);
        div()
            .child(heading("What should go through it?"))
            .child(modes)
            .children(list_box)
            .children(suggests)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .mt(px(18.))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Forget hosts unused for"),
                    )
                    .child(self.dropdown(
                        "forget",
                        vec![
                            ("1".into(), "1 day".into()),
                            ("7".into(), "7 days".into()),
                            ("30".into(), "30 days".into()),
                            ("never".into(), "Never".into()),
                        ],
                        self.forget.clone(),
                        Some(px(92.)),
                        t,
                        cx,
                        |this, v, _| this.forget = v,
                    )),
            )
            .child(
                div()
                    .mt(px(22.))
                    .pt(px(16.))
                    .border_t_1()
                    .border_color(t.line)
                    .text_size(px(15.))
                    .line_height(px(22.))
                    .child(summary),
            )
    }

    /// "Nordbank will connect with AnyConnect, sign you in through your
    /// browser, and learn which hosts need it as it goes."
    pub(super) fn summary(&self, cx: &Context<Self>) -> String {
        let name = match self.display_name(cx) {
            n if n.is_empty() => "It".to_owned(),
            n => n,
        };
        let protocol = match self.protocol().unwrap_or("anyconnect") {
            "anyconnect" => "AnyConnect",
            "gp" => "GlobalProtect",
            "pulse" => "Ivanti Connect Secure",
            "nc" => "Network Connect",
            "fortinet" => "Fortinet SSL VPN",
            "f5" => "F5 BIG-IP",
            "openvpn3" => "OpenVPN 3",
            "openvpn" => "OpenVPN",
            _ => "Array Networks",
        };
        let sign_in = match self.method {
            _ if self.protocol.starts_with("openvpn") => "ask what the profile needs",
            "password" => "ask for your password",
            "certificate" => "use your certificate",
            _ => "sign you in through your browser",
        };
        let route = match self.mode {
            "ask" => "ask you before sending any host through",
            "list" => "only send what you listed",
            "full" => "send everything through it",
            _ => "learn which hosts need it as it goes",
        };
        format!("{name} will connect with {protocol}, {sign_in}, and {route}.")
    }
}
