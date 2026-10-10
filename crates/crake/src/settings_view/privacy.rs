//! Settings, Privacy & log: retention, helpdesk copies, clearing the log.

use super::*;

impl Settings {
    pub(super) fn privacy(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hide = cx.global::<Prefs>().hide_hosts;
        let keep = select(
            self,
            "keep",
            142.,
            &[
                ("1", "1 day"),
                ("7", "7 days"),
                ("30", "30 days"),
                ("never", "Don\u{2019}t keep a log"),
            ],
            model.setting("log_days"),
            t,
            cx,
            |this, value, _| this.set("log_days", value),
        );
        let clear: AnyElement = if self.confirm_clear {
            div()
                .flex()
                .gap(px(8.))
                .child(
                    outline("cancel-clear", "Keep it", t.line_strong, t.ink, None).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.confirm_clear = false;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    outline(
                        "clear",
                        "Clear log",
                        t.danger,
                        t.panel,
                        Some("crake/broom.svg"),
                    )
                    .bg(t.danger)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.requests.send_blocking(Request::ClearLog).ok();
                        this.confirm_clear = false;
                        cx.notify();
                    })),
                )
                .into_any_element()
        } else {
            outline(
                "clear-log",
                "Clear log\u{2026}",
                t.danger,
                t.danger,
                Some("crake/broom.svg"),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.confirm_clear = true;
                cx.notify();
            }))
            .into_any_element()
        };
        div()
            .child(
                div()
                    .mt(px(22.))
                    .flex()
                    .gap(px(12.))
                    .items_start()
                    .py(px(14.))
                    .px(px(16.))
                    .rounded(px(8.))
                    .bg(t.accent_soft)
                    .child(
                        svg()
                            .path("crake/section-privacy.svg")
                            .size(px(20.))
                            .flex_none()
                            .mt(px(1.))
                            .text_color(t.accent),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Everything Crake decides stays on this machine."),
                            )
                            .child(sub(
                                "It sends nothing to anyone, including us. Your tunnels\u{2019} gateways see what passes through them, as with any VPN.",
                                t,
                            )),
                    ),
            )
            .child(
                group(t)
                    .mt(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .py(px(14.))
                            .px(px(16.))
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.title("Keep the log for", t, cx))
                                    .child(
                                        div()
                                            .font_family(fonts::MONO)
                                            .text_size(px(12.))
                                            .text_color(t.muted)
                                            .child("~/.local/share/crake/log"),
                                    ),
                            )
                            .child(keep),
                    )
                    .child(row(self.title("Hide host names in helpdesk copies", t, cx),
                        Some("\u{201c}Copy for helpdesk\u{201d} replaces names with tunnel and timing only."),
                        toggle("hide-hosts", hide, t).on_click(cx.listener(move |_, _, _, cx| {
                            Self::pref(cx, |p| p.hide_hosts = !hide);
                        })),
                        true,
                        t,
                    ))
                    .child(row(self.title("Clear the log", t, cx),
                        Some("Learned hosts stay. Only the history goes."),
                        clear,
                        true,
                        t,
                    )),
            )
    }
}
