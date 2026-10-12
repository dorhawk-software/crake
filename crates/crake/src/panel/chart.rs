//! The panel's “Last hour”: direct traffic, and a tick where Crake routed.

use super::*;

impl Panel {
    /// "Last hour" (DESIGN.md, Charts): grey bars are how much traffic went
    /// direct per minute, the marks Crake's decisions (`crate::timeline`,
    /// shared with tunnel detail). A new tick grows from the baseline, one
    /// stroke after the other.
    pub(super) fn chart(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let marks = model.marks();
        let now_minute = jiff::Timestamp::now().as_second() as u64 / 60;
        let bars: Vec<(f32, u64)> = model
            .traffic
            .iter()
            .filter(|(m, _)| now_minute.saturating_sub(*m) <= 60)
            .map(|(m, bytes)| ((now_minute - m) as f32, *bytes))
            .collect();
        let grown = marks
            .iter()
            .map(|m| match self.motion.arrival(&m.ts) {
                Some(at) => [phase(at, 0, 260, window), phase(at, 90, 260, window)],
                None => [1., 1.],
            })
            .collect();
        let timeline = crate::timeline::Timeline {
            span: 60.,
            bars,
            bin: 1.,
            marks,
            grown,
            words: Box::new(|b| format!("{} went direct", crate::model::human_bytes(b))),
            nothing: "Nothing went direct".into(),
            axis: vec![
                (0., crate::timeline::clock(60.)),
                (0.5, crate::timeline::clock(30.)),
                (1., "now".into()),
            ],
            shape: crate::timeline::LAST_HOUR,
        };
        div()
            .px(px(18.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Last hour"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .text_color(t.muted)
                            .child(div().size(px(10.)).mr(px(5.)).bg(t.bars))
                            .child("direct")
                            .child(div().ml(px(8.)).w(px(3.)).h(px(11.)).bg(t.accent))
                            .child(div().ml(px(2.)).w(px(3.)).h(px(8.)).bg(t.accent))
                            .child(div().ml(px(5.)).child("routed")),
                    ),
            )
            .child(timeline.element(&self.chart_hover, t, cx, |p| &mut p.chart_hover))
    }
}
