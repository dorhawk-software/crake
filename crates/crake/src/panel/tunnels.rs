//! The panel's tunnels: each with its switch, state and who decides.

use super::*;

impl Panel {
    pub(super) fn tunnels(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Never empty: with no tunnel the panel shows `first_run` instead.
        let mut list = section("Tunnels", t).mt(px(6.));
        let browsers = browser_notice(model, &cx.global::<crate::prefs::Prefs>().dismissed);
        for (i, tunnel) in model.tunnels.iter().enumerate() {
            let on = matches!(tunnel.state.as_str(), "connected" | "connecting");
            let name = tunnel.name.clone();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(9.))
                    .px(px(18.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if on { t.ink } else { t.muted })
                                            .child(tunnel.display.clone()),
                                    )
                                    .child(self.state_line(tunnel, t))
                                    // Stopped on its own: seen, it's plain Off
                                    // (design/proposals/dismiss-trouble.html, A).
                                    .when(tunnel.state == "trouble", |row| {
                                        row.child(dismiss(i, tunnel.name.clone(), t, cx))
                                    })
                                    .when(tunnel.managed, |row| {
                                        row.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.))
                                                .px(px(7.))
                                                .py(px(1.))
                                                .rounded(px(4.))
                                                .bg(t.surface)
                                                .text_size(px(12.))
                                                .line_height(px(16.))
                                                .text_color(t.muted)
                                                .child(
                                                    svg()
                                                        .path("crake/lock.svg")
                                                        .size(px(12.))
                                                        .text_color(t.muted),
                                                )
                                                .child("Managed"),
                                        )
                                    }),
                            )
                            .child({
                                // Who decides, and what's allowed (DESIGN.md:
                                // policy is visible). A full tunnel says it in ink.
                                let (icon, text, color) = if tunnel.managed {
                                    let icon = if tunnel.full_tunnel {
                                        "crake/block.svg"
                                    } else {
                                        "crake/shield.svg"
                                    };
                                    let color = if tunnel.full_tunnel { t.ink } else { t.muted };
                                    (icon, tunnel.policy.clone(), color)
                                } else {
                                    // The mode as New tunnel names it.
                                    let mode =
                                        tunnel.own.get("mode").map_or("learn", String::as_str);
                                    (
                                        "crake/person.svg",
                                        format!(
                                            "Your profile · {}",
                                            crate::new_tunnel::mode_name(mode)
                                        ),
                                        t.muted,
                                    )
                                };
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .mt(px(2.))
                                    .text_size(px(13.))
                                    .text_color(color)
                                    .child(
                                        svg()
                                            .path(icon)
                                            .size(px(14.))
                                            .flex_none()
                                            .text_color(color),
                                    )
                                    .child(div().min_w_0().truncate().child(text))
                            }),
                    )
                    .child(
                        switch(("switch", i), on, t)
                            .keys()
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.send(Request::Switch(name.clone(), !on));
                            }))
                            .test_support(),
                    ),
            );
            // Said openly, under the tunnel, until the user has read it
            // (design/proposals/anyconnect-notice.html).
            if cx
                .global::<crate::prefs::Prefs>()
                .acting_as(&tunnel.name, &tunnel.gateway)
                .is_some()
            {
                let key = format!("anyconnect:{}", tunnel.name);
                let d = &tunnel.display;
                list = list.child(
                    div()
                        .mx(px(18.))
                        .mb(px(8.))
                        .px(px(12.))
                        .py(px(10.))
                        .rounded(px(8.))
                        .bg(t.surface)
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .text_size(px(13.))
                        .child(div().flex_1().min_w_0().child(format!(
                            "Crake introduces itself to {d} as Cisco AnyConnect, because {d}\u{2019}s gateway only lets AnyConnect in."
                        )))
                        .child(
                            div()
                                .id(("got-it", i))
                                .flex_none()
                                .px(px(12.))
                                .py(px(6.))
                                .rounded(px(6.))
                                .border_1()
                                .border_color(t.line)
                                .bg(t.panel)
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child("Got it")
                                .keys()
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.send(Request::Dismiss(key.clone()));
                                }))
                                .test_support(),
                        ),
                );
            }
            // Browsers that look names up themselves, under the first
            // tunnel that learns (design/proposals/browser-dns.html, A).
            if let Some((at, names, text)) = &browsers
                && *at == i
            {
                let keys: Vec<String> = names.iter().map(|n| browser_key(n)).collect();
                list = list.child(
                    div()
                        .id("browser-dns")
                        .mx(px(18.))
                        .mb(px(8.))
                        .px(px(12.))
                        .py(px(10.))
                        .rounded(px(8.))
                        .bg(t.surface)
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .text_size(px(13.))
                        .child(div().flex_1().min_w_0().child(text.clone()))
                        .child(
                            div()
                                .id("browser-dns-got-it")
                                .flex_none()
                                .px(px(12.))
                                .py(px(6.))
                                .rounded(px(6.))
                                .border_1()
                                .border_color(t.line)
                                .bg(t.panel)
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child("Got it")
                                .keys()
                                .on_click(cx.listener(move |this, _, _, _| {
                                    for key in &keys {
                                        this.send(Request::Dismiss(key.clone()));
                                    }
                                }))
                                .test_support(),
                        )
                        .test_support(),
                );
            }
        }
        list
    }
}

/// A tunnel that stopped on its own, seen: it's plain Off
/// (design/proposals/dismiss-trouble.html, A; a word, not an ×, which
/// could read as removing the tunnel).
fn dismiss(i: usize, name: String, t: &Theme, cx: &mut Context<Panel>) -> impl IntoElement {
    div()
        .id(("dismiss", i))
        .flex_none()
        .px(px(7.))
        .py(px(1.))
        .rounded(px(5.))
        .border_1()
        .border_color(t.line)
        .bg(t.surface)
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(t.ink)
        .cursor_pointer()
        .hover(|d| d.border_color(t.muted))
        .child("Dismiss")
        .keys()
        .on_click(cx.listener(move |this, _, _, _| {
            this.send(Request::DismissTrouble(name.clone()));
        }))
        .test_support()
}

impl Panel {
    /// A tunnel's state beside its name; a tunnel that lost its gateway
    /// wears the dashed head, under way (ink, not teal: a reconnect isn't
    /// Crake's decision).
    fn state_line(&self, tunnel: &crate::client::Tunnel, t: &Theme) -> Div {
        let alarm = tunnel.state == "trouble" || (tunnel.state == "connected" && tunnel.dns_down);
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .when(crate::model::reconnecting(tunnel), |d| {
                d.child(crate::head::under_way(t.ink, self.motion.opened))
            })
            .text_size(px(13.))
            .text_color(if alarm { t.danger } else { t.muted })
            .child(tunnel_state(tunnel))
    }
}

/// What "Got it" remembers for a browser's notice.
fn browser_key(name: &str) -> String {
    format!("browser-dns:{name}")
}

/// The notice about browsers that look names up themselves: under the
/// first connected tunnel that learns, naming the browsers not dismissed
/// yet. Its tunnel's place in the list, the browsers, and what it says.
pub(super) fn browser_notice(
    model: &Model,
    dismissed: &std::collections::BTreeSet<String>,
) -> Option<(usize, Vec<String>, String)> {
    let names: Vec<String> = model
        .own_dns_browsers
        .iter()
        .filter(|n| !dismissed.contains(&browser_key(n)))
        .cloned()
        .collect();
    // A tunnel that learns: a full tunnel sends every name through it anyway.
    let at = model
        .tunnels
        .iter()
        .position(|t| t.state == "connected" && !t.full_tunnel)?;
    let d = &model.tunnels[at].display;
    let (verb, whose) = match names.as_slice() {
        [] => return None,
        [one] => ("looks names up itself", format!("{one}\u{2019}s")),
        _ => ("look names up themselves", "their".to_owned()),
    };
    let who = crate::browser_dns::and_list(&names);
    let text = format!(
        "{who} {verb} (DNS over HTTPS), so the first visit there to a site only {d} knows waits a few seconds. Turning DNS over HTTPS off in {whose} settings fixes it."
    );
    Some((at, names, text))
}
