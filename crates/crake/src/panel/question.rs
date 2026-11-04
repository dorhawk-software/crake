//! A short name's question while it waits for you
//! (design/proposals/waiting-for-you.html; `model::waiting`): the notice
//! that needs you, first under the header, in leftover-warning.html's
//! amber (3). "Use ACME's" for each tunnel still up that knows the name,
//! in try order, none recommended; "Not now" puts it off (the log's row
//! can still answer it that day).

use super::*;

/// Room for this many tunnels' "Use …" per question in their ids.
pub(crate) const CHOICES: usize = 8;

impl Panel {
    pub(super) fn question_notices(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        model
            .waiting()
            .into_iter()
            .enumerate()
            .map(|(i, w)| question_notice(i, &w, model, t, cx).into_any_element())
            .collect()
    }
}

fn question_notice(
    i: usize,
    w: &crate::model::Waiting,
    model: &Model,
    t: &Theme,
    cx: &mut Context<Panel>,
) -> impl IntoElement + use<> {
    let mut actions = div()
        .mt(px(8.))
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(10.))
        .gap_y(px(8.));
    for (j, (tunnel, full)) in w.choices.iter().enumerate() {
        let display = model.display_of(Some(tunnel));
        let request = Request::UseShortName(tunnel.clone(), w.name.clone(), full.clone());
        actions = actions.child(
            div()
                .id(("question-use", i * CHOICES + j))
                .px(px(12.))
                .py(px(6.))
                .rounded(px(6.))
                .bg(t.panel)
                .border_1()
                .border_color(t.warning_line)
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .child(format!(
                    "Use {}",
                    crate::model::waiting::possessive(&display)
                ))
                .keys()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.send(request.clone());
                    cx.notify();
                }))
                .test_support(),
        );
    }
    let key = w.key.clone();
    actions = actions.child(
        div()
            .id(("question-not-now", i))
            .px(px(4.))
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
    );
    let why = w.entry.reason.clone().unwrap_or_default();
    div()
        .id(("question", i))
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
                        .child(crate::theme::breakable(&w.entry.message)),
                )
                .child(
                    div()
                        .mt(px(3.))
                        .text_color(t.muted)
                        .child(crate::theme::breakable(&why)),
                )
                .child(actions),
        )
        .test_support()
}
