//! The panel's “What Crake did”: the latest decisions, with Undo or Redo.

use crake_proto::LogEntry;

use super::*;

/// A new decision's row slides in over this many ms.
const SLIDES_IN: u64 = 220;

impl Panel {
    pub(super) fn decisions(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = model.decisions(DECISIONS);
        let mut list = div()
            .mt(px(14.))
            .border_t_1()
            .border_color(t.line)
            .child(self.decisions_heading(model, t, cx));
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
            // Crake's first route says so, once; a route another tunnel
            // answered for too keeps its question (moments A and C).
            let notice = self.first_route_notice(&d, model, t, cx);
            let asked = (d.action == Some(Action::Undo))
                .then(|| model.also_answered(&d.entry))
                .flatten()
                .filter(|a| {
                    !cx.global::<crate::prefs::Prefs>()
                        .dismissed
                        .contains(&a.keep_key)
                });
            let row = self.decision(i, d, asked, t, cx);
            let row = match notice {
                Some(notice) => div().child(row).child(notice).into_any_element(),
                None => row,
            };
            list = list.child(match arrived {
                Some(at) => {
                    let p = phase(at, 0, SLIDES_IN, window);
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

    /// "What Crake did", and its "?": a host shows up there once, and how
    /// long it is kept (hover or click it; Settings' pattern).
    fn decisions_heading(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> Div {
        let open = self.help_open;
        let mut q = div().relative().flex_none().child(
            div()
                .id("what-crake-did-help")
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .cursor_pointer()
                .child(
                    svg()
                        .path("crake/help.svg")
                        .size(px(15.))
                        .text_color(if open { t.accent } else { t.muted }),
                )
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.help_open = *hovered;
                    cx.notify();
                }))
                // A click lands after the hover that opened it: it keeps
                // it open (moving away closes it).
                .keys()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.help_open = true;
                    cx.notify();
                }))
                .test_support(),
        );
        if open {
            let paragraphs = model.what_crake_did_help();
            let example = paragraphs.last().is_some_and(|p| p.starts_with("Example:"));
            let count = paragraphs.len();
            let mut card = div()
                .id("what-crake-did-card")
                .absolute()
                .top(px(24.))
                .left(px(-96.))
                .w(px(340.))
                .py(px(10.))
                .px(px(12.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .bg(t.panel)
                .border_1()
                .border_color(t.line_strong)
                .rounded(px(8.))
                .font_weight(FontWeight::NORMAL)
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(t.ink);
            for (i, para) in paragraphs.into_iter().enumerate() {
                let muted = example && i + 1 == count;
                card = card.child(div().when(muted, |d| d.text_color(t.muted)).child(para));
            }
            q = q.child(gpui_kit::deferred(card.occlude().test_support()).with_priority(2));
        }
        div()
            .pt(px(12.))
            .pb(px(4.))
            .px(px(18.))
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .child("What Crake did")
            .child(q)
    }

    pub(super) fn decision(
        &self,
        i: usize,
        d: Decision,
        asked: Option<crate::model::AlsoAnswered>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let asked = asked.map(|a| self.also_answered(i, &d, a, t, cx));
        use crake_proto::LogKind;
        let step = d.entry.short.as_ref().map(|s| s.step);
        let (mark, color) = match d.entry.kind {
            // A short name chosen is kept until forgotten, as a pin is.
            _ if step == Some(crake_proto::ShortStep::Chosen) => ("crake/mark-pinned.svg", t.ink),
            _ if step == Some(crake_proto::ShortStep::Forgotten) => {
                ("crake/mark-undone.svg", t.muted)
            }
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
        // A short name two tunnels know: which one, under the reason.
        let choices = d.entry.short.as_ref().map_or(0, |s| s.full.len());
        let choose = (d.action == Some(Action::Use) && choices > 1)
            .then(|| self.short_choices(i, &d.entry, t, cx));
        if let (Some(action), Some(tunnel)) = (d.action.clone(), d.entry.tunnel.clone())
            && choose.is_none()
        {
            let label = match action {
                Action::Undo => "Undo".to_owned(),
                Action::Redo => "Redo".to_owned(),
                Action::Send => format!(
                    "Send through {}",
                    self.model.read(cx).display_of(Some(&tunnel))
                ),
                Action::Use => format!(
                    "Use {}\u{2019}s",
                    self.model.read(cx).display_of(Some(&tunnel))
                ),
            };
            let entry = d.entry.clone();
            right = right.child(
                button(("act", i), label, t)
                    .px(px(10.))
                    .h(px(30.))
                    .keys()
                    .on_click(cx.listener(move |this, _, _, _| {
                        if let Some(r) = request(&action, &entry, 0) {
                            this.send(r);
                        }
                    }))
                    .test_support(),
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
                    .child(
                        div()
                            .text_size(px(14.))
                            .child(crate::theme::breakable(&d.entry.message)),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(12.5))
                            .line_height(px(17.))
                            .text_color(t.muted)
                            .child(crate::theme::breakable(
                                d.entry.reason.as_deref().unwrap_or_default(),
                            )),
                    )
                    .children(asked)
                    .children(choose),
            )
            .child(right)
            .into_any_element()
    }
}

impl Panel {
    /// "Use ACME's", "Use Nordbank's": one for each tunnel that knows a
    /// short name, as moment C offers "Use Nordbank".
    fn short_choices(&self, i: usize, entry: &LogEntry, t: &Theme, cx: &mut Context<Self>) -> Div {
        let mut actions = div().flex().items_center().gap(px(16.)).mt(px(6.));
        let full = entry
            .short
            .as_ref()
            .map(|s| s.full.clone())
            .unwrap_or_default();
        for (j, (tunnel, _)) in full.iter().enumerate() {
            let display = self.model.read(cx).display_of(Some(tunnel));
            let entry = entry.clone();
            actions = actions.child(
                crate::controls::action(
                    ("use-short", i * CHOICES + j),
                    format!("Use {display}\u{2019}s"),
                    t,
                )
                .keys()
                .on_click(cx.listener(move |this, _, _, _| {
                    if let Some(r) = request(&Action::Use, &entry, j) {
                        this.send(r);
                    }
                }))
                .test_support(),
            );
        }
        actions
    }
}

/// Room for this many tunnels' "Use …" per row in their ids.
const CHOICES: usize = 8;

/// What a decision's button asks of craked: about its hosts, or about its
/// short name (`choice`: which tunnel's full name, for Use; Redo uses the
/// one it had).
pub(super) fn request(action: &Action, entry: &LogEntry, choice: usize) -> Option<Request> {
    let tunnel = entry.tunnel.clone()?;
    let hosts = entry.hosts.clone();
    Some(match (action, &entry.short) {
        (Action::Use | Action::Redo, Some(s)) => {
            let (t, full) = s.full.get(choice)?.clone();
            Request::UseShortName(t, s.name.clone(), full)
        }
        (Action::Undo, Some(s)) => Request::Forget(tunnel, s.name.clone()),
        (Action::Undo, None) => Request::Undo(tunnel, hosts),
        (Action::Redo, None) => Request::Redo(tunnel, hosts),
        (Action::Send, _) => Request::Send(tunnel, hosts),
        (Action::Use, None) => return None,
    })
}
