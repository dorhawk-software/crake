//! Settings, Names: asking the tunnels about unknown names, keeping internal
//! names private.

use super::*;

impl Settings {
    pub(super) fn names(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut chips = div().flex().flex_wrap().gap(px(6.)).mt(px(10.));
        let mut any = false;
        for tunnel in &model.tunnels {
            for d in &tunnel.dns_domains {
                any = true;
                chips = chips.child(
                    div()
                        .h(px(28.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .bg(t.surface)
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .child(format!("*.{d} \u{2192} {}", tunnel.display)),
                );
            }
        }
        if !any {
            chips = chips.child(
                div()
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child("None yet. Gateways push them when they connect."),
            );
        }
        let private = model.setting("keep_private") == "true";
        let ask = model.setting("ask_unknown") == "true";
        group(t)
            .mt(px(22.))
            .child(
                div()
                    .py(px(14.))
                    .px(px(16.))
                    .child(self.title("Ask a tunnel\u{2019}s DNS for these names", t, cx))
                    .child(sub("Pushed by your gateways when they connect.", t))
                    .child(chips),
            )
            .child(row(
                self.title("Keep internal names private", t, cx),
                Some("Names like *.acme.internal never go to your internet provider\u{2019}s DNS."),
                toggle("private", private, t).on_click(cx.listener(move |this, _, _, _| {
                    this.set("keep_private", (!private).to_string());
                })),
                true,
                t,
            ))
            .child(row(
                self.title("Ask every tunnel when a name is unknown", t, cx),
                Some("How Crake finds internal names nobody told it about."),
                toggle("ask-unknown", ask, t).on_click(cx.listener(move |this, _, _, _| {
                    this.set("ask_unknown", (!ask).to_string());
                })),
                true,
                t,
            ))
    }
}
