//! The panel's “Last hour”: direct traffic, and a tick where Crake routed.

use super::*;

impl Panel {
    /// "Last hour" (DESIGN.md, Charts): grey bars are how much traffic went
    /// direct per minute; teal two-stroke ticks the moments Crake routed a
    /// host; hollow circles hosts it tried and left alone. Pointing at it
    /// reads it: a dotted hairline at the minute, one line about it under
    /// the chart.
    pub(super) fn chart(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let marks = model.marks();
        let now_ts = jiff::Timestamp::now();
        let now_minute = now_ts.as_second() as u64 / 60;
        let bars: Vec<(f32, u64)> = model
            .traffic
            .iter()
            .filter(|(m, _)| now_minute.saturating_sub(*m) <= 60)
            .map(|(m, bytes)| ((now_minute - m) as f32, *bytes))
            .collect();
        let max = bars.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32;

        // What the pointer is on: a mark close by, else that minute's bar.
        let pointed = self.hover.map(|h| {
            match marks
                .iter()
                .filter(|m| (m.ago - h).abs() <= 0.75)
                .min_by(|a, b| (a.ago - h).abs().total_cmp(&(b.ago - h).abs()))
            {
                Some(m) => {
                    // The hairline stops above what stands there (in the
                    // chart's 72 px drawing): the count, the tick and its gap,
                    // or the circle.
                    let top = match (m.routed, m.count) {
                        (true, n) if n > 1 => 12.,
                        (true, _) => 30. - 1.5,
                        (false, _) => 72. - 10. - 4.5,
                    };
                    (m.ago, None, m.time.clone(), m.text.clone(), top)
                }
                None => {
                    let ago = h.round();
                    let minute = now_minute.saturating_sub(ago as u64);
                    let time = jiff::Timestamp::from_second(minute as i64 * 60)
                        .map(|t| {
                            t.to_zoned(jiff::tz::TimeZone::system())
                                .strftime("%H:%M")
                                .to_string()
                        })
                        .unwrap_or_default();
                    let bytes = model
                        .traffic
                        .iter()
                        .find(|(m, _)| *m == minute)
                        .map(|(_, b)| *b);
                    // Above the bar, or down to the baseline when there is none.
                    let top = match bytes {
                        Some(b) if b > 0 => 72. - (3. + 22. * (b as f32 / max).sqrt()),
                        _ => 72. + 3.,
                    };
                    let text = match model.traffic.iter().find(|(m, _)| *m == minute) {
                        Some((_, 0)) => "Nothing went direct".to_owned(),
                        Some((_, b)) => format!("{} went direct", crate::model::human_bytes(*b)),
                        None => "Crake wasn\u{2019}t counting yet".to_owned(),
                    };
                    (ago, Some(ago), time, text, top)
                }
            }
        });
        let hover_x = pointed.as_ref().map(|p| (p.0, p.4));
        let hover_bar = pointed.as_ref().and_then(|p| p.1);

        // A new tick grows from the baseline, one stroke after the other.
        let paint_marks: Vec<(f32, bool, [f32; 2])> = marks
            .iter()
            .map(|m| {
                let grown = match self.motion.arrival(&m.ts) {
                    Some(at) => [phase(at, 0, 260, window), phase(at, 90, 260, window)],
                    None => [1., 1.],
                };
                (m.ago, m.routed, grown)
            })
            .collect();
        let now = jiff::Zoned::now();
        let label = |mins: i64| {
            now.checked_sub(jiff::SignedDuration::from_mins(mins))
                .map(|z| z.strftime("%H:%M").to_string())
                .unwrap_or_default()
        };
        let mono = |s: String| {
            div()
                .font_family(fonts::MONO)
                .text_size(px(11.))
                .text_color(t.muted)
                .child(s)
        };
        let paint = HourPaint {
            bars,
            max,
            marks: paint_marks,
            hover_x,
            hover_bar,
            accent: t.accent,
            muted: t.muted,
            panel: t.panel,
            base: t.line_strong,
            grey: t.bars,
        };
        let geometry = self.chart_geometry.clone();

        // Counts above merged ticks, in mono 11 px accent.
        let counts = merged_counts(&marks, t);

        let under = match &pointed {
            Some((_, _, time, text, _)) => div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .min_w_0()
                .child(mono(time.clone()))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .text_color(t.ink)
                        .child(text.clone()),
                ),
            None => div()
                .flex()
                .justify_between()
                .w_full()
                .child(mono(label(60)))
                .child(mono(label(30)))
                .child(mono("now".into())),
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
            .child(
                div()
                    .id("chart")
                    .relative()
                    .w_full()
                    .h(px(73.))
                    .mt(px(8.))
                    .on_mouse_move(cx.listener(|this, e: &gpui_kit::MouseMoveEvent, _, cx| {
                        let (left, width) = this.chart_geometry.get();
                        if width <= 4. {
                            return;
                        }
                        let x = f32::from(e.position.x) - left;
                        let ago = (60. * (1. - (x - 2.) / (width - 4.))).clamp(0., 60.);
                        if this.hover != Some(ago) {
                            this.hover = Some(ago);
                            cx.notify();
                        }
                    }))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !*hovered {
                            this.hover = None;
                            cx.notify();
                        }
                    }))
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                geometry
                                    .set((f32::from(bounds.left()), f32::from(bounds.size.width)));
                            },
                            move |bounds, _, window, _| paint_hour(bounds, window, &paint),
                        )
                        .size_full(),
                    )
                    .child(counts),
            )
            .child(
                div()
                    .h(px(18.))
                    .mt(px(4.))
                    .flex()
                    .items_baseline()
                    .child(under),
            )
    }
}

/// One stroke of the two-stroke tick in a chart: flat bottom on the
/// baseline, rounded top (design/DESIGN.md, "The signature").
pub(super) fn tick_bar(x: f32, top: f32, base: f32, w: f32) -> Option<gpui_kit::Path<Pixels>> {
    let r = w / 2.;
    let mut p = PathBuilder::fill();
    p.move_to(point(px(x - r), px(base)));
    p.line_to(point(px(x - r), px(top + r)));
    p.arc_to(
        point(px(r), px(r)),
        px(0.),
        false,
        true,
        point(px(x + r), px(top + r)),
    );
    p.line_to(point(px(x + r), px(base)));
    p.close();
    p.build().ok()
}

/// The 1.5 px gap around one stroke of a tick: the same shape, widened by
/// the gap on each side, painted in the panel's colour before the tick.
pub(super) fn tick_gap(x: f32, top: f32, base: f32, w: f32) -> Option<gpui_kit::Path<Pixels>> {
    const GAP: f32 = 1.5;
    tick_bar(x, top - GAP, base, w + 2. * GAP)
}

/// A circle, filled or (with a width) stroked.
pub(super) fn circle(
    cx: f32,
    cy: f32,
    r: f32,
    stroke: Option<f32>,
) -> Option<gpui_kit::Path<Pixels>> {
    let mut p = match stroke {
        Some(w) => PathBuilder::stroke(px(w)),
        None => PathBuilder::fill(),
    };
    let radii = point(px(r), px(r));
    p.move_to(point(px(cx - r), px(cy)));
    p.arc_to(radii, px(0.), false, true, point(px(cx + r), px(cy)));
    p.arc_to(radii, px(0.), false, true, point(px(cx - r), px(cy)));
    p.close();
    p.build().ok()
}

/// What the panel's "Last hour" draws: direct traffic per minute, ticks
/// where Crake routed (growing in), rings where it left a host alone, and
/// the pointer's minute.
struct HourPaint {
    bars: Vec<(f32, u64)>,
    max: f32,
    marks: Vec<(f32, bool, [f32; 2])>,
    hover_x: Option<(f32, f32)>,
    hover_bar: Option<f32>,
    accent: gpui_kit::Rgba,
    muted: gpui_kit::Rgba,
    panel: gpui_kit::Rgba,
    base: gpui_kit::Rgba,
    grey: gpui_kit::Rgba,
}

/// Draw the last hour into `bounds`.
fn paint_hour(bounds: gpui_kit::Bounds<gpui_kit::Pixels>, window: &mut Window, p: &HourPaint) {
    let HourPaint {
        max,
        hover_x,
        hover_bar,
        accent,
        muted,
        panel,
        base,
        grey,
        ..
    } = *p;
    let (bars, paint_marks) = (&p.bars, &p.marks);
    let w = f32::from(bounds.size.width);
    let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
    // The design's 416 x 72 drawing, at the panel's width.
    let baseline = top + 72.;
    let x_of = |ago: f32| left + 2. + (w - 4.) * (60. - ago) / 60.;
    for (ago, bytes) in bars {
        let x = x_of(*ago);
        if *bytes == 0 {
            continue;
        }
        let h = 3. + 22. * (*bytes as f32 / max).sqrt();
        if let Some(p) = crate::chart::bar(x, 3.6, baseline, h, window.scale_factor()) {
            let on = hover_bar.is_some_and(|b| (b - ago).abs() < 0.01);
            window.paint_path(p, if on { muted } else { grey });
        }
    }
    // The hairline never draws over a bar, a tick or a circle:
    // it ends 3 px above whatever stands at that minute.
    if let Some((ago, stop)) = hover_x {
        let x = x_of(ago);
        let end = top + stop - 3.;
        if end - (top + 2.) >= 4. {
            let mut line = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(3.)]);
            line.move_to(point(px(x), px(top + 2.)));
            line.line_to(point(px(x), px(end)));
            if let Ok(p) = line.build() {
                window.paint_path(p, base);
            }
        }
    }
    // A tick keeps clear of the bars: a gap in the panel's
    // colour around it, drawn before the baseline so the
    // baseline stays whole (DESIGN.md, Charts).
    // A stroke grown `g` of the way up from the baseline.
    let stroke_top = |top: f32, g: f32| (baseline - (72. - top) * g).min(baseline - 1.6);
    for (ago, routed, grown) in paint_marks {
        if !*routed {
            continue;
        }
        let x = x_of(*ago);
        for ((dx, top), g) in [(0., 30.), (5., 40.)].into_iter().zip(grown) {
            if *g <= 0. {
                continue;
            }
            if let Some(p) = tick_gap(x + dx, stroke_top(top, *g), baseline, 3.2) {
                window.paint_path(p, panel);
            }
        }
    }
    let mut line = PathBuilder::stroke(px(1.));
    line.move_to(point(px(left), px(baseline + 0.5)));
    line.line_to(point(px(left + w), px(baseline + 0.5)));
    if let Ok(p) = line.build() {
        window.paint_path(p, base);
    }
    for (ago, routed, grown) in paint_marks {
        let x = x_of(*ago);
        if *routed {
            for ((dx, top), g) in [(0., 30.), (5., 40.)].into_iter().zip(grown) {
                if *g <= 0. {
                    continue;
                }
                if let Some(p) = tick_bar(x + dx, stroke_top(top, *g), baseline, 3.2) {
                    window.paint_path(p, accent);
                }
            }
        } else {
            if let Some(p) = circle(x, baseline - 10., 4.5, None) {
                window.paint_path(p, panel);
            }
            if let Some(p) = circle(x, baseline - 10., 4.5, Some(1.6)) {
                window.paint_path(p, muted);
            }
        }
    }
}

/// Above ticks that stand for several decisions: how many, in accent.
fn merged_counts(marks: &[crate::model::Mark], t: &Theme) -> Div {
    let mut counts = div().absolute().top_0().left_0().size_full();
    for m in marks.iter().filter(|m| m.routed && m.count > 1) {
        let x = 2. + (CHART_WIDTH - 4.) * (60. - m.ago) / 60. + 2.5;
        counts = counts.child(
            div()
                .absolute()
                .top(px(12.))
                .left(px(x - 10.))
                .w(px(20.))
                .flex()
                .justify_center()
                .font_family(fonts::MONO)
                .text_size(px(11.))
                .line_height(px(12.))
                .text_color(t.accent)
                .child(m.count.to_string()),
        );
    }
    counts
}
