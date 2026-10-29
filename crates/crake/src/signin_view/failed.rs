//! "When it can't connect" (design/screens/sign-in.html): what happened,
//! where it stopped (the steps, or the checks of a gateway that didn't
//! answer), Copy for helpdesk, Close and Try again.

use super::checks::{CheckRow, Mark};
use super::*;

impl SignIn {
    pub(super) fn failed(
        &mut self,
        reason: &str,
        at: usize,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let why = explain(reason, at, &self.spec);
        let mut card = div().child(title(why.title));
        card = match self.checks.clone() {
            // The checks say it in Crake's words; openconnect's own are in
            // the helpdesk copy.
            Some(checks) => card
                .child(para(checks.sentence(&self.spec), t.ink))
                .child(check_list(checks.rows(&self.spec.gateway), t))
                .children(self.others_line(t, cx)),
            None => card
                .child(para(why.sentence, t.ink))
                .child(steps(at, Some((at, why.row)), None, &self.spec, t))
                // openconnect's (or openvpn3's) own words, for whoever helps.
                .child(
                    div()
                        .mt(px(12.))
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format!("{} said: {reason}", self.spec.program())),
                ),
        };
        card
            // Beside the words it copies: the buttons' row has room for
            // two (design/proposals/signin-error-buttons.html, A).
            .child(
                div()
                    .mt(px(6.))
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .child(
                        link("helpdesk", "Copy for helpdesk", t)
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| this.copy_for_helpdesk(cx)))
                            .test_support(),
                    )
                    .when(self.copied.is_some_and(|c| c.elapsed() < COPIED_FOR), |d| {
                        d.child(crate::controls::copied(t))
                    }),
            )
            .child(
                buttons()
                    .child(div().flex_1())
                    .child(
                        button("close", "Close", Look::Text, t)
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
                            .test_support(),
                    )
                    .child(if reason.starts_with(crake_proto::trouble::KEY_MISMATCH) {
                        // A key the gateway turns down only IT can fix:
                        // trying again can't help, changing it can
                        // (design/proposals/ipsec-tunnel.html).
                        button("change-key", "Change the key\u{2026}", Look::Primary, t)
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| this.change_key(cx)))
                            .test_support()
                    } else {
                        button("retry", "Try again", Look::Primary, t)
                            .keys()
                            .on_click(cx.listener(|this, _, window, cx| this.retry(window, cx)))
                            .test_support()
                    }),
            )
    }

    /// "Other tunnels are unaffected. ACME is still connected.", while
    /// another tunnel is up.
    fn others_line(&self, t: &Theme, cx: &Context<Self>) -> Option<Div> {
        let up: Vec<String> = self
            .model
            .read(cx)
            .tunnels
            .iter()
            .filter(|x| x.name != self.spec.tunnel && x.state == "connected")
            .map(|x| x.display.clone())
            .collect();
        let verb = match up.len() {
            0 => return None,
            1 => "is",
            _ => "are",
        };
        Some(
            div()
                .mt(px(12.))
                .text_size(px(13.))
                .text_color(t.muted)
                .child(format!(
                    "Other tunnels are unaffected. {} {verb} still connected.",
                    crate::browser_dns::and_list(&up)
                )),
        )
    }
}

/// The checks, as the steps are drawn: done (a plain check, ink), failed
/// (a red cross, bold), under way (dots, bold); machine facts in mono.
fn check_list(rows: Vec<CheckRow>, t: &Theme) -> Div {
    let mut list = div()
        .mt(px(18.))
        .border_1()
        .border_color(t.line)
        .rounded(px(8.));
    for (i, r) in rows.into_iter().enumerate() {
        let (mark, color) = match r.mark {
            Mark::Done => ("crake/check.svg", t.ink),
            Mark::Failed => ("crake/cross.svg", t.danger),
            Mark::UnderWay => ("crake/dots.svg", t.ink),
        };
        list =
            list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .py(px(11.))
                    .px(px(12.))
                    .when(i > 0, |d| d.border_t_1().border_color(t.line))
                    .when(r.mark != Mark::Done, |d| d.font_weight(FontWeight::BOLD))
                    .child(svg().path(mark).size(px(18.)).flex_none().text_color(color))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(4.))
                            .child(r.words)
                            .children(r.fact.map(|f| {
                                div().font_family(fonts::MONO).text_size(px(13.)).child(f)
                            })),
                    ),
            );
    }
    list
}
