//! The panel's moments (design/screens/tray-panel.html?moment=…): Crake's
//! first route, said once, and a question that stays under its route.

use super::*;
use crate::model::AlsoAnswered;

/// The first-route notice's key among the notices dismissed.
pub(crate) const FIRST_ROUTE: &str = "first-route";

impl Panel {
    /// Under Crake's first route, once: what just happened, in plain
    /// words, until "Got it".
    pub(super) fn first_route_notice(
        &self,
        d: &Decision,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let first = model.first_route()?;
        let standing = d.action == Some(Action::Undo) && first.ts == d.entry.ts;
        let dismissed = cx
            .global::<crate::prefs::Prefs>()
            .dismissed
            .contains(FIRST_ROUTE);
        if !standing || dismissed {
            return None;
        }
        let host = d
            .entry
            .message
            .strip_prefix("Sent ")
            .and_then(|m| m.split(" through ").next());
        let host = host.or(d.entry.hosts.first().map(String::as_str))?;
        let tunnel = model.display_of(d.entry.tunnel.as_deref());
        Some(
            div()
                .id("first-route")
                .mx(px(18.))
                .mb(px(10.))
                .px(px(14.))
                .py(px(12.))
                .rounded(px(8.))
                .bg(t.surface)
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Crake\u{2019}s first route."),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format!(
                            "Only {host} goes through {tunnel}. Everything else still goes \
                             direct. Each time Crake sends a host through a tunnel, it shows \
                             here, and Undo puts it back."
                        )),
                )
                .child(
                    div().flex().mt(px(10.)).child(
                        div()
                            .id("first-route-got-it")
                            .h(px(30.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(t.panel)
                            .border_1()
                            .border_color(t.line_strong)
                            .hover(|s| s.border_color(t.muted))
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .child("Got it")
                            .keys()
                            .on_click(cx.listener(|this, _, _, _| {
                                this.send(Request::Dismiss(FIRST_ROUTE.into()));
                            }))
                            .test_support(),
                    ),
                ),
        )
    }

    /// Under a route's reason, while another tunnel answered for its host
    /// too: "Nordbank answers for it too.", Use Nordbank and Keep ACME.
    pub(super) fn also_answered(
        &self,
        i: usize,
        d: &Decision,
        asked: AlsoAnswered,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let names: Vec<&str> = asked
            .others
            .iter()
            .map(|(_, display)| display.as_str())
            .collect();
        let verb = if names.len() == 1 {
            "answers"
        } else {
            "answer"
        };
        let said = format!("{} {verb} for it too.", names.join(" and "));
        let from = d.entry.tunnel.clone().unwrap_or_default();
        let hosts = d.entry.hosts.clone();
        let mut actions = div().flex().items_center().gap(px(16.)).mt(px(4.));
        for (j, (name, display)) in asked.others.into_iter().enumerate() {
            let (from, hosts) = (from.clone(), hosts.clone());
            actions = actions.child(
                crate::controls::action(("use", i * MOVE_ROWS + j), format!("Use {display}"), t)
                    .keys()
                    .on_click(cx.listener(move |this, _, _, _| {
                        this.send(Request::Move(from.clone(), name.clone(), hosts.clone()));
                    }))
                    .test_support(),
            );
        }
        let keep = asked.keep_key;
        let display = self.model.read(cx).display_of(Some(&from));
        div()
            .child(div().mt(px(6.)).text_size(px(13.)).child(said))
            .child(
                actions.child(
                    div()
                        .id(("keep", i))
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .cursor_pointer()
                        .child(format!("Keep {display}"))
                        .keys()
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.send(Request::Dismiss(keep.clone()));
                        }))
                        .test_support(),
                ),
            )
    }
}

/// Room for this many "Use …" actions per row in their ids.
const MOVE_ROWS: usize = 8;
