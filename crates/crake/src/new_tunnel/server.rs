//! New tunnel, step 1: where it connects (name, gateway or OpenVPN 3
//! profile) and the protocol, with what's Beta and what isn't installed.

use super::*;

impl NewTunnel {
    pub(super) fn server(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        // The detection line under the fields.
        let (found, line): (bool, Option<String>) = match &self.detect {
            Detect::Idle => (false, None),
            Detect::Asking => (false, Some("Asking the gateway\u{2026}".into())),
            Detect::Done(Found::AnyConnect { .. }) => {
                (true, Some("Found a Cisco AnyConnect gateway.".into()))
            }
            Detect::Done(Found::Unknown) => (
                false,
                Some("It answered, but not as AnyConnect. Pick its protocol below.".into()),
            ),
            Detect::Done(Found::Unreachable) => (
                false,
                Some(
                    "No answer from that gateway. Check the name, or pick the protocol below."
                        .into(),
                ),
            ),
        };
        let protocols = self.protocol_list(t, cx);
        let group_row = self.group_row(t, cx);
        div()
            .child(heading("Where does it connect?"))
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .mt(px(18.))
                    .child(
                        div()
                            .flex_basis(px(0.))
                            .flex_grow(1.)
                            .min_w_0()
                            .child(label("Name"))
                            .child(field(&self.name, false, t).text_size(px(15.))),
                    )
                    .child(
                        // 1 : 1.4, as the design's grid.
                        div().flex_basis(px(0.)).flex_grow(1.4).min_w_0().child(
                            if self.protocol == "openvpn3" {
                                let options: Vec<(String, String)> = self
                                    .profiles
                                    .iter()
                                    .map(|p| (p.clone(), p.clone()))
                                    .collect();
                                div()
                                    .child(label("Profile"))
                                    .child(self.dropdown(
                                        "profile",
                                        options,
                                        self.profile.clone().unwrap_or_else(|| {
                                            if self.profiles.is_empty() {
                                                "No profiles yet".into()
                                            } else {
                                                "Choose\u{2026}".into()
                                            }
                                        }),
                                        None,
                                        t,
                                        cx,
                                        |this, v| this.profile = Some(v),
                                    ))
                                    .into_any_element()
                            } else {
                                div()
                                    .child(label("Gateway"))
                                    .child(field(&self.gateway, true, t))
                                    .into_any_element()
                            },
                        ),
                    ),
            )
            .when_some(line.filter(|_| self.protocol != "openvpn3"), |d, line| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .mt(px(10.))
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .when(found, |d| {
                            d.child(
                                svg()
                                    .path("crake/mark-routed.svg")
                                    .size(px(16.))
                                    .text_color(t.accent),
                            )
                        })
                        .child(line),
                )
            })
            .child(
                div()
                    .mt(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Protocol"),
            )
            .child(protocols)
            .when_some(self.missing(self.protocol), |d, why| {
                d.child(
                    div()
                        .mt(px(10.))
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(t.ink)
                        .child(why),
                )
            })
            .when_some(
                PROTOCOLS
                    .iter()
                    .find(|p| p.0 == self.protocol && beta(p.0))
                    .map(|p| p.1),
                |d, name| {
                    d.child(
                        div()
                            .mt(px(10.))
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(t.muted)
                            .child(format!(
                                "Beta: Crake hasn\u{2019}t been tried against a real {name} gateway yet. openconnect speaks it, so it should work. If it doesn\u{2019}t, Copy for helpdesk in the sign-in window says what happened."
                            )),
                    )
                },
            )
            .children(group_row)
    }

    /// The protocols, each with what it is, its code, and its tags (Beta,
    /// not installed); "Detect automatically" first.
    fn protocol_list(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let auto_sub = match &self.detect {
            Detect::Done(Found::AnyConnect { .. }) => {
                "Crake asks the gateway. It answered: AnyConnect."
            }
            Detect::Asking => "Crake asks the gateway\u{2026}",
            Detect::Done(_) => "Crake asked; the gateway didn\u{2019}t say.",
            Detect::Idle => "Crake asks the gateway.",
        };
        let mut protocols = div()
            .mt(px(8.))
            .border_1()
            .border_color(t.line)
            .rounded(px(8.))
            .overflow_hidden();
        for (i, (id, name, sub, code)) in PROTOCOLS.into_iter().enumerate() {
            let on = self.protocol == id;
            let sub = if id == "auto" { auto_sub } else { sub };
            protocols = protocols.child(
                div()
                    .id(("protocol", i))
                    .min_h(px(46.))
                    .py(px(8.))
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(i > 0, |r| r.border_t_1().border_color(t.line))
                    .bg(if on { t.accent_soft } else { t.panel })
                    .cursor_pointer()
                    .child(radio(on, t))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap(px(8.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(name)
                                    .when(id == "auto", |d| {
                                        d.child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(t.accent)
                                                .child("Recommended"),
                                        )
                                    })
                                    .when(beta(id), |d| d.child(beta_tag(t)))
                                    .when(id != "auto" && self.missing(id).is_some(), |d| {
                                        d.child(tag("Not installed", t))
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .line_height(px(19.))
                                    .text_color(t.muted)
                                    .child(sub),
                            ),
                    )
                    .child(
                        div()
                            .font_family(fonts::MONO)
                            .text_size(px(12.))
                            .text_color(t.muted)
                            .child(code),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.protocol = id;
                        cx.notify();
                    })),
            );
        }
        protocols
    }

    /// The gateway's sign-in groups to choose from, when it offers more than one.
    fn group_row(&self, t: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let groups = self.groups();
        (groups.len() > 1 && self.protocol != "openvpn3").then(|| {
            let options: Vec<(String, String)> =
                groups.iter().map(|g| (g.clone(), g.clone())).collect();
            div()
                .flex()
                .gap(px(14.))
                .mt(px(18.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(label("Group"))
                        .child(self.dropdown(
                            "group",
                            options,
                            self.group.clone().unwrap_or_default(),
                            None,
                            t,
                            cx,
                            |this, v| this.group = Some(v),
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        // A column, so the sentence wraps.
                        .flex()
                        .flex_col()
                        .justify_end()
                        .pb(px(4.))
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format!(
                            "The gateway offers {} groups. Ask your IT team if you\u{2019}re not sure.",
                            match groups.len() {
                                2 => "two".to_owned(),
                                3 => "three".to_owned(),
                                n => n.to_string(),
                            }
                        )),
                )
        })
    }
}

/// Not yet tried against a real gateway: shown as Beta.
pub fn beta(protocol: &str) -> bool {
    !protocol.is_empty() && protocol != "auto" && !TRIED.contains(&protocol)
}

/// "Beta": a protocol not yet tried against a real gateway.
pub fn beta_tag(t: &Theme) -> impl IntoElement {
    tag("Beta", t)
}

/// A small grey tag beside a name.
pub(super) fn tag(text: &'static str, t: &Theme) -> impl IntoElement {
    div()
        .px(px(6.))
        .rounded(px(4.))
        .bg(t.surface)
        .text_size(px(12.))
        .line_height(px(17.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.muted)
        .child(text)
}
