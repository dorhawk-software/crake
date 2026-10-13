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
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(
                                                if tunnel.state == "trouble"
                                                    || (tunnel.state == "connected"
                                                        && tunnel.dns_down)
                                                {
                                                    t.danger
                                                } else {
                                                    t.muted
                                                },
                                            )
                                            .child(tunnel_state(tunnel)),
                                    )
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
                                    (
                                        "crake/person.svg",
                                        "Your profile · learning on".to_owned(),
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
                                .text_color(t.accent)
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child("Got it")
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.send(Request::Dismiss(key.clone()));
                                }))
                                .test_support(),
                        ),
                );
            }
        }
        list
    }
}
