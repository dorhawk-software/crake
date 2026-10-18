//! An OpenVPN 3 tunnel stays on OpenVPN 3 (design/proposals/openvpn-tunnel.html,
//! 5, in leftover-warning.html's amber, 3: it needs you, nothing has broken
//! yet): its profile is locked there, or Crake can't run it. "Import .ovpn…"
//! opens the tunnel's Edit, where its file moves it; "Not now" until the
//! next start.

use super::*;

impl Panel {
    pub(super) fn staying_notices(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let dismissed = &cx.global::<crate::prefs::Prefs>().dismissed;
        let shown: Vec<_> = model
            .staying
            .iter()
            .filter(|s| !dismissed.contains(&s.dismiss_key()))
            .cloned()
            .collect();
        shown
            .into_iter()
            .enumerate()
            .map(|(i, s)| staying_notice(i, &s, t, cx).into_any_element())
            .collect()
    }
}

fn staying_notice(
    i: usize,
    s: &crate::model::Staying,
    t: &Theme,
    cx: &mut Context<Panel>,
) -> impl IntoElement + use<> {
    let (tunnel, key) = (s.tunnel.clone(), s.dismiss_key());
    div()
        .id(("staying", i))
        .mx(px(18.))
        .mb(px(10.))
        .px(px(12.))
        .py(px(10.))
        .rounded(px(8.))
        .bg(t.warning_soft)
        .border_1()
        .border_color(t.warning_line)
        .flex()
        .gap(px(10.))
        .text_size(px(13.))
        .child(super::leftover::warning_icon(t))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(s.title.clone()),
                )
                .child(div().mt(px(3.)).text_color(t.muted).child(s.why.clone()))
                .child(
                    div()
                        .mt(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(
                            div()
                                .id(("staying-import", i))
                                .px(px(12.))
                                .py(px(6.))
                                .rounded(px(6.))
                                .bg(t.panel)
                                .border_1()
                                .border_color(t.warning_line)
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child("Import .ovpn\u{2026}")
                                .keys()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.send(Request::NewTunnel(Some(tunnel.clone())));
                                    cx.notify();
                                }))
                                .test_support(),
                        )
                        .child(
                            div()
                                .id(("staying-not-now", i))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.muted)
                                .cursor_pointer()
                                .child("Not now")
                                .keys()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.send(Request::Dismiss(key.clone()));
                                    cx.notify();
                                }))
                                .test_support(),
                        ),
                ),
        )
        .test_support()
}
