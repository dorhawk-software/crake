//! OpenVPN 3 left a connection running (design/proposals/leftover-openvpn3.html,
//! A2, in leftover-warning.html's amber, 3): the session ended, its process
//! didn't. Crake doesn't end another program's process: it says how, with
//! the command to copy, and "Got it" beside it. Gone by itself once the
//! process has ended.

use super::*;

/// How long "Copied" stays after Copy.
const COPIED_FOR: Duration = Duration::from_secs(3);

impl Panel {
    pub(super) fn leftover_notices(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let dismissed = &cx.global::<crate::prefs::Prefs>().dismissed;
        let shown: Vec<_> = model
            .leftovers
            .iter()
            .filter(|l| !dismissed.contains(&l.dismiss_key()))
            .cloned()
            .collect();
        shown
            .into_iter()
            .enumerate()
            .map(|(i, l)| self.leftover_notice(i, &l, t, cx).into_any_element())
            .collect()
    }

    fn leftover_notice(
        &self,
        i: usize,
        l: &crate::model::Leftover,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let command = format!("sudo kill -9 {}", l.pid);
        let copied = self
            .leftover_copied
            .is_some_and(|(pid, at)| pid == l.pid && at.elapsed() < COPIED_FOR);
        let key = l.dismiss_key();
        let pid = l.pid;
        div()
            .id(("leftover", i))
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
            .child(warning_icon(t))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(crake_proto::LEFT_RUNNING),
                    )
                    .child(div().mt(px(3.)).text_color(t.muted).child(format!(
                        "Its {} session ended, but the process kept going: it reconnects on its \
                         own and changes your other tunnels\u{2019} DNS. Stop it in a terminal:",
                        l.session
                    )))
                    .child(
                        div()
                            .mt(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(self.command_chip(i, &command, pid, copied, t, cx))
                            .child(
                                div()
                                    .id(("leftover-got-it", i))
                                    .flex_none()
                                    .px(px(12.))
                                    .py(px(6.))
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(t.warning_line)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .cursor_pointer()
                                    .child("Got it")
                                    .keys()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.send(Request::Dismiss(key.clone()));
                                        cx.notify();
                                    }))
                                    .test_support(),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(6.))
                            .text_color(t.muted)
                            .child("This note goes away by itself once the process ends."),
                    ),
            )
            .test_support()
    }
}

impl Panel {
    /// The command, with Copy inside it: an icon and a word, "Copied" with
    /// a check for a moment after.
    fn command_chip(
        &self,
        i: usize,
        command: &str,
        pid: u32,
        copied: bool,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let text = command.to_owned();
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .bg(t.panel)
            .border_1()
            .border_color(t.warning_line)
            .rounded(px(6.))
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(9.))
                    .py(px(6.))
                    .font_family(fonts::MONO)
                    .child(text.clone()),
            )
            .child(
                div()
                    .id(("leftover-copy", i))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .border_l_1()
                    .border_color(t.warning_line)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if copied { t.muted } else { t.ink })
                    .cursor_pointer()
                    .child(
                        svg()
                            .path(if copied {
                                "crake/check.svg"
                            } else {
                                "crake/clipboard.svg"
                            })
                            .size(px(14.))
                            .text_color(if copied { t.muted } else { t.ink }),
                    )
                    .child(if copied { "Copied" } else { "Copy" })
                    .keys()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                        this.leftover_copied = Some((pid, Instant::now()));
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(COPIED_FOR).await;
                            this.update(cx, |_, cx| cx.notify()).ok();
                        })
                        .detach();
                    }))
                    .test_support(),
            )
    }
}

/// The bird's head with an amber triangle: three layers, each one colour.
pub(super) fn warning_icon(t: &Theme) -> impl IntoElement + use<> {
    let layer = |path: &'static str, color| {
        svg()
            .absolute()
            .top_0()
            .left_0()
            .size(px(22.))
            .path(path)
            .text_color(color)
    };
    div()
        .relative()
        .flex_none()
        .size(px(22.))
        .child(layer("crake/head-warning.svg", t.ink))
        .child(layer("crake/warning-triangle.svg", t.warning))
        .child(layer("crake/warning-mark.svg", t.on_warning))
}
