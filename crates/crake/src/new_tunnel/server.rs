//! New tunnel, step 1: where it connects (name, gateway or OpenVPN
//! profile) and the protocol, with what's Beta and what isn't installed.

use super::*;

/// The wider of the two columns grows this much more than the other.
const WIDE_COLUMN: f32 = 1.4;

impl NewTunnel {
    /// OpenVPN's Profile: OpenVPN 3's profiles to take over, with "Import
    /// .ovpn…" in its label's row; with none, the import button in its
    /// place (design/proposals/openvpn-tunnel.html, 2).
    fn profile_field(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        if self.profiles.is_empty() {
            return div().child(label("Profile")).child(
                div()
                    .id("import-ovpn")
                    .h(px(42.))
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_1()
                    .border_color(t.line_strong)
                    .rounded(px(6.))
                    .bg(t.panel)
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child(svg().path("crake/plus.svg").size(px(16.)).text_color(t.ink))
                    .child("Import a .ovpn file\u{2026}")
                    .on_click(cx.listener(|this, _, _, cx| this.choose_ovpn(cx)))
                    .test_support(),
            );
        }
        let options: Vec<(String, String)> = self
            .profiles
            .iter()
            .map(|p| {
                let shown = if self.locked.contains(p) {
                    format!("{p} \u{b7} locked: import its .ovpn file")
                } else {
                    p.clone()
                };
                (p.clone(), shown)
            })
            .collect();
        div()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .child(label("Profile"))
                    .child(
                        crate::controls::action("import-ovpn", "Import .ovpn\u{2026}", t)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_ovpn(cx)))
                            .test_support(),
                    ),
            )
            .child(
                self.dropdown(
                    "profile",
                    options,
                    self.profile
                        .clone()
                        .unwrap_or_else(|| "Choose\u{2026}".into()),
                    None,
                    t,
                    cx,
                    |this, v, cx| this.take_over(v, cx),
                ),
            )
    }

    /// Under OpenVPN's fields: how an import went, or, with no profiles to
    /// take over, where to start.
    fn profile_line(&self, t: &Theme) -> Option<Div> {
        let line = || {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .mt(px(10.))
                .text_size(px(13.))
                .line_height(px(19.))
        };
        match &self.profile_note {
            Some(import::ProfileNote::Imported(said)) => Some(
                line()
                    .text_color(t.muted)
                    .child(
                        svg()
                            .path("crake/check.svg")
                            .size(px(16.))
                            .text_color(t.muted),
                    )
                    .child(said.clone()),
            ),
            Some(import::ProfileNote::Failed(said)) => {
                Some(line().text_color(t.ink).child(said.clone()))
            }
            None if self.profiles.is_empty() => Some(
                line()
                    .text_color(t.muted)
                    .child("Import the .ovpn file your IT team gave you."),
            ),
            None => None,
        }
    }

    pub(super) fn server(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        // The detection line under the fields.
        let asking = match &self.detect {
            Detect::Asking(since) => Some(*since),
            _ => None,
        };
        let (found, line): (bool, Option<String>) = match &self.detect {
            Detect::Idle => (false, None),
            Detect::Asking(since) => (false, Some(asking_said(since.elapsed()))),
            Detect::Done(Found::AnyConnect { expires, .. }) => (
                true,
                Some(crate::detect::certificate_said(
                    *expires,
                    jiff::Timestamp::now(),
                )),
            ),
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
                        div().flex_basis(px(0.)).flex_grow(WIDE_COLUMN).min_w_0().child(
                            match self.protocol {
                                "openvpn" if self.shows_gateway() => {
                                    self.kept_gateway(t, cx).into_any_element()
                                }
                                "openvpn" => self.profile_field(t, cx).into_any_element(),
                                _ => div()
                                    .child(self.import_row("Gateway", t, cx))
                                    .child(field(&self.gateway, true, t))
                                    .into_any_element(),
                            },
                        ),
                    ),
            )
            .when(self.protocol == "openvpn", |d| d.children(self.profile_line(t)))
            .when_some(line.filter(|_| self.protocol != "openvpn"), |d, line| {
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
                                    .path("crake/check.svg")
                                    .size(px(16.))
                                    .text_color(t.muted),
                            )
                        })
                        .when_some(asking, |d, since| d.child(crate::head::under_way(t.ink, since)))
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

    /// The protocols: "Detect automatically" first, then those used with a
    /// real gateway and the Beta ones under their own headings, each with
    /// what it is, its code, and "Not installed" when its engine is missing.
    /// Once the gateway has answered, the list folds away behind "Choose
    /// the protocol yourself" (design/proposals/critique-2026-10-15, 3).
    fn protocol_list(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let auto_sub = match &self.detect {
            Detect::Done(Found::AnyConnect { .. }) => {
                "Crake asks the gateway. It answered: AnyConnect."
            }
            Detect::Asking(_) => "Crake asks the gateway\u{2026}",
            Detect::Done(_) => "Crake asked; the gateway didn\u{2019}t say.",
            Detect::Idle => "Crake asks the gateway.",
        };
        let answered = matches!(self.detect, Detect::Done(Found::AnyConnect { .. }))
            && self.protocol == "auto";
        // A .ovpn imported: OpenVPN, said so, the rest folded away.
        let imported =
            self.protocol == "openvpn" && self.ovpn.as_ref().is_some_and(|o| o.from_file);
        let folded = (answered || imported) && !self.protocols_open;
        let mut protocols = div()
            .mt(px(8.))
            .border_1()
            .border_color(t.line)
            .rounded(px(8.))
            .overflow_hidden();
        if imported {
            let (i, p) = PROTOCOLS
                .into_iter()
                .enumerate()
                .find(|(_, p)| p.0 == "openvpn")
                .unwrap_or((0, PROTOCOLS[0]));
            protocols = protocols
                .child(self.protocol_card(i, p, "From the profile you imported", t, cx))
                .child(self.choose_yourself(folded, t, cx));
            if folded {
                return protocols;
            }
        } else {
            protocols = protocols.child(self.protocol_card(0, PROTOCOLS[0], auto_sub, t, cx));
        }
        if answered && !imported {
            protocols = protocols.child(self.choose_yourself(folded, t, cx));
        }
        if folded {
            return protocols;
        }
        for (heading, tried) in [
            ("Used with real gateways", true),
            ("Beta: not yet tried with a real gateway", false),
        ] {
            protocols = protocols.child(
                div()
                    .px(px(14.))
                    .pt(px(10.))
                    .pb(px(4.))
                    .border_t_1()
                    .border_color(t.line)
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(heading),
            );
            for (i, p) in PROTOCOLS
                .into_iter()
                .enumerate()
                .skip(1)
                .filter(|(_, p)| TRIED.contains(&p.0) == tried)
            {
                protocols = protocols.child(self.protocol_card(i, p, p.2, t, cx));
            }
        }
        protocols
    }

    /// The quiet row that opens (or folds) the list after the gateway answered.
    fn choose_yourself(&self, folded: bool, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("protocols-more")
            .min_h(px(46.))
            .px(px(14.))
            .flex()
            .items_center()
            .gap(px(12.))
            .border_t_1()
            .border_color(t.line)
            .cursor_pointer()
            .child(
                svg()
                    .path(if folded {
                        "crake/chevron-down.svg"
                    } else {
                        "crake/chevron-up.svg"
                    })
                    .size(px(16.))
                    .text_color(t.muted),
            )
            .child(div().flex_1().child("Choose the protocol yourself"))
            .when(folded, |d| {
                d.child(
                    div()
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format!("{} more", PROTOCOLS.len() - 1)),
                )
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.protocols_open = !this.protocols_open;
                cx.notify();
            }))
            .test_support()
    }

    /// One protocol's card. Its id is its place in PROTOCOLS, whatever the
    /// order shown.
    fn protocol_card(
        &self,
        i: usize,
        (id, name, _, code): (&'static str, &'static str, &'static str, &'static str),
        sub: &'static str,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on = self.protocol == id;
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
            }))
            .test_support()
    }

    /// A field's label with "Import .ovpn…" in its row (ovpn-import.html,
    /// A): beside Gateway it picks OpenVPN by itself.
    fn import_row(&self, name: &'static str, t: &Theme, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_baseline()
            .justify_between()
            .child(label(name))
            .child(
                crate::controls::action("import-ovpn", "Import .ovpn\u{2026}", t)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_ovpn(cx)))
                    .test_support(),
            )
    }

    /// OpenVPN shows the gateway its profile names: imported from a file,
    /// or an OpenVPN tunnel edited without a new one.
    pub(super) fn shows_gateway(&self) -> bool {
        match &self.ovpn {
            Some(o) => o.from_file,
            None => self.editing_protocol == "openvpn",
        }
    }

    /// The profile's gateway: shown, not typed (a surface fill, not an
    /// input); a new .ovpn replaces the profile.
    fn kept_gateway(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let gateway = self
            .ovpn
            .as_ref()
            .map_or_else(|| self.kept_gateway.clone(), |o| o.remote.clone());
        div().child(self.import_row("Gateway", t, cx)).child(
            div()
                .id("ovpn-gateway")
                .h(px(42.))
                .px(px(12.))
                .flex()
                .items_center()
                .border_1()
                .border_color(t.line)
                .rounded(px(6.))
                .bg(t.surface)
                .font_family(fonts::MONO)
                .text_size(px(14.))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(gateway)
                .test_support(),
        )
    }

    /// The gateway's sign-in groups to choose from, when it offers more than one.
    fn group_row(&self, t: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let groups = self.groups();
        (groups.len() > 1 && self.protocol != "openvpn").then(|| {
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
                            |this, v, _| this.group = Some(v),
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
                                3 => "three".to_owned(), // literal: a number in words
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

/// After this long asking, the line says for how long.
const SAY_SECONDS_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

/// "Asking the gateway…", and after a moment for how long, so a slow
/// gateway reads as still being asked, not stuck.
pub(super) fn asking_said(asked: std::time::Duration) -> String {
    if asked < SAY_SECONDS_AFTER {
        "Asking the gateway\u{2026}".into()
    } else {
        format!(
            "Asking the gateway\u{2026} {} s",
            crate::head::second_counted(asked)
        )
    }
}
