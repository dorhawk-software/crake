//! The panel's “What Crake did”: the latest decisions, with Undo or Redo.

use super::*;

impl Panel {
    pub(super) fn decisions(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = model.decisions(DECISIONS);
        let mut list = section("What Crake did", t).mt(px(14.));
        if rows.is_empty() {
            list = list.child(
                div()
                    .px(px(18.))
                    .pb(px(12.))
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child("Nothing yet. Crake routes nothing until something needs a tunnel."),
            );
        }
        for (i, d) in rows.into_iter().enumerate() {
            // A decision made while the panel is open slides in from above.
            let arrived = self.motion.arrival(&d.entry.ts);
            let row = self.decision(i, d, t, cx);
            list = list.child(match arrived {
                Some(at) => {
                    let p = phase(at, 0, 220, window);
                    div()
                        .relative()
                        .top(px(-6. * (1. - p)))
                        .opacity(p)
                        .child(row)
                        .into_any_element()
                }
                None => row,
            });
        }
        list
    }

    pub(super) fn decision(
        &self,
        i: usize,
        d: Decision,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crake_proto::LogKind;
        let (mark, color) = match d.entry.kind {
            LogKind::Routed if d.entry.message.starts_with("Pinned") => {
                ("crake/mark-pinned.svg", t.ink)
            }
            LogKind::Routed => ("crake/mark-routed.svg", t.accent),
            LogKind::Undone => ("crake/mark-undone.svg", t.muted),
            LogKind::Asked => ("crake/help.svg", t.accent),
            _ => ("crake/mark-left-alone.svg", t.muted),
        };
        let mut right = div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(4.))
            .flex_none()
            .child(
                div()
                    .font_family(fonts::MONO)
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child(d.time.clone()),
            );
        if let (Some(action), Some(tunnel)) = (d.action.clone(), d.entry.tunnel.clone()) {
            let hosts = d.entry.hosts.clone();
            let label = match action {
                Action::Undo => "Undo".to_owned(),
                Action::Redo => "Redo".to_owned(),
                Action::Send => format!(
                    "Send through {}",
                    self.model.read(cx).display_of(Some(&tunnel))
                ),
            };
            right = right.child(
                button(("act", i), label, t)
                    .px(px(10.))
                    .h(px(30.))
                    .on_click(cx.listener(move |this, _, _, _| {
                        let r = match action {
                            Action::Undo => Request::Undo(tunnel.clone(), hosts.clone()),
                            Action::Redo => Request::Redo(tunnel.clone(), hosts.clone()),
                            Action::Send => Request::Send(tunnel.clone(), hosts.clone()),
                        };
                        this.send(r);
                    })),
            );
        }
        div()
            .flex()
            .items_start()
            .gap(px(10.))
            .pt(px(8.))
            .pb(px(10.))
            .px(px(18.))
            .child(
                svg()
                    .path(mark)
                    .size(px(18.))
                    .flex_none()
                    .mt(px(2.))
                    .text_color(color),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_size(px(14.)).child(d.entry.message.clone()))
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(12.5))
                            .line_height(px(17.))
                            .text_color(t.muted)
                            .child(d.entry.reason.clone().unwrap_or_default()),
                    ),
            )
            .child(right)
            .into_any_element()
    }
}
