//! The log's day chart: traffic and decisions through the day, zoomable.

use super::*;

use crate::units::{HOURS_PER_DAY, MINUTES_PER_HOUR, minutes_today as now_minutes};

/// Within half a minute of now (or of the whole day) counts as "now".
const NOW_SLACK: f32 = 0.5;
/// The chart's width until it's been measured.
const MIN_WIDTH: f32 = 900.;
/// Round times to label the axis with, in minutes: 6 h down to 1 min.
const LABEL_STEPS: [f32; 8] = [360., 120., 60., 30., 15., 10., 5., 1.];
/// A wheel step narrows or widens the shown span by this much.
const ZOOM_IN: f32 = 0.8;
const ZOOM_OUT: f32 = 1.25;
/// Bar sizes to choose from, in minutes, finest first; bars stay at least
/// BAR_SPACING pixels apart.
const BIN_SIZES: [f32; 6] = [1., 2., 5., 10., 15., 30.];
const BAR_SPACING: f32 = 6.;

impl Log {
    /// The minutes shown: (from, to) since midnight; the whole day when not zoomed.
    pub(super) fn span(&self) -> (f32, f32) {
        self.view.unwrap_or((0., now_minutes()))
    }

    /// Zoom by `factor` around minute `at`, keeping it under the pointer.
    pub(super) fn zoom(&mut self, at: f32, factor: f32) {
        let now = now_minutes();
        let (a, b) = self.span();
        let span = ((b - a) * factor).clamp(MIN_SPAN.min(now), now);
        let from = (at - (at - a) * span / (b - a)).clamp(0., (now - span).max(0.));
        self.view = if span >= now - NOW_SLACK {
            None
        } else {
            Some((from, from + span))
        };
    }

    /// "Today", its summary, and when zoomed what's shown with Whole day.
    pub(super) fn today_head(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (a, b) = self.span();
        let mut head = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::BOLD)
                    .child("Today"),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(model.today_summary()),
            )
            .child(div().flex_1());
        if self.view.is_some() {
            let bin = bin_minutes(b - a, self.chart_geometry.get().1.max(MIN_WIDTH));
            let bars = if bin as i32 == 1 {
                "1-minute bars".to_owned()
            } else {
                format!("{bin}-minute bars")
            };
            head = head
                .child(
                    div()
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format!("{}\u{2013}{} \u{b7} {bars}", hm(a), hm(b))),
                )
                .child(
                    outline_button("whole-day", "Whole day", px(30.), t)
                        .px(px(10.))
                        .text_size(px(13.))
                        .keys()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.view = None;
                            cx.notify();
                        })),
                );
        }
        head
    }

    /// Today (DESIGN.md, Charts): grey bars are all traffic, finer as it
    /// zooms; two-stroke ticks the moments Crake routed a host (merged when
    /// close, with a count); hollow circles hosts it left alone. Scroll zooms,
    /// drag moves, double-click goes back; pointing reads it.
    pub(super) fn day_chart(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (a, b) = self.span();
        let span = (b - a).max(1.);
        let width = self.chart_geometry.get().1.max(MIN_WIDTH);
        let x_frac = |m: f32| (m - a) / span;
        let bin = bin_minutes(span, width);

        let bins = day_bins(model, a, b, bin);
        let max = bins.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32;

        let marks = day_marks(model, a, b, span, width);

        let pointed = self.hover.map(|h| {
            pointed_at(
                h,
                &marks,
                &bins,
                Scale {
                    span,
                    width,
                    bin,
                    max,
                },
            )
        });

        let paint = DayPaint {
            from: a,
            span,
            bin,
            max,
            bins: bins.clone(),
            marks: marks.iter().map(|m| (m.0, m.1)).collect(),
            hover: pointed.as_ref().map(|p| (p.0, p.1, p.4)),
            accent: t.accent,
            muted: t.muted,
            panel: t.panel,
            base: t.line_strong,
            grey: t.bars,
        };
        let geometry = self.chart_geometry.clone();
        let mono = |s: String| {
            div()
                .font_family(fonts::MONO)
                .text_size(px(11.))
                .text_color(t.muted)
                .child(s)
        };

        let counts = merged_counts(&marks, a, span, t);

        // Under the chart: round times, or one line about what's pointed at.
        let now = now_minutes();
        let label = |m: f32| {
            if (m - now).abs() < NOW_SLACK {
                "now".to_owned()
            } else {
                hm(m)
            }
        };
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
            None => {
                let step = LABEL_STEPS
                    .into_iter()
                    .find(|s| span / s >= 2.)
                    .unwrap_or(1.);
                let mid = ((a + b) / 2. / step).round() * step;
                div()
                    .relative()
                    .w_full()
                    .h(px(16.))
                    .child(div().absolute().left_0().child(mono(label(a))))
                    .child(
                        div()
                            .absolute()
                            .left(gpui_kit::relative(x_frac(mid)))
                            .ml(px(-20.))
                            .w(px(40.))
                            .flex()
                            .justify_center()
                            .child(mono(hm(mid))),
                    )
                    .child(div().absolute().right_0().child(mono(label(b))))
            }
        };

        div()
            .mt(px(10.))
            .child(
                div()
                    .id("day-chart")
                    .relative()
                    .w_full()
                    .h(px(CHART_HEIGHT - 20.))
                    .cursor_grab()
                    .on_scroll_wheel(cx.listener(|this, e: &gpui_kit::ScrollWheelEvent, _, cx| {
                        let (left, width) = this.chart_geometry.get();
                        if width <= 2. * crate::chart::INSET {
                            return;
                        }
                        let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
                        if dy == 0. {
                            return;
                        }
                        let (a, b) = this.span();
                        let at =
                            a + (b - a) * ((f32::from(e.position.x) - left) / width).clamp(0., 1.);
                        this.zoom(at, if dy > 0. { ZOOM_IN } else { ZOOM_OUT });
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, e: &gpui_kit::MouseDownEvent, _, cx| {
                            if e.click_count >= 2 {
                                this.view = None;
                                this.drag = None;
                            } else {
                                this.drag = Some((f32::from(e.position.x), this.span()));
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, _, _, _| this.drag = None),
                    )
                    .on_mouse_move(cx.listener(|this, e: &gpui_kit::MouseMoveEvent, _, cx| {
                        let (left, width) = this.chart_geometry.get();
                        if width <= 2. * crate::chart::INSET {
                            return;
                        }
                        let x = f32::from(e.position.x);
                        if let (Some((x0, (a0, b0))), true) = (this.drag, e.dragging()) {
                            let span = b0 - a0;
                            let now = now_minutes();
                            let from =
                                (a0 - span * (x - x0) / width).clamp(0., (now - span).max(0.));
                            if this.view.is_some() {
                                this.view = Some((from, from + span));
                            }
                        }
                        let (a, b) = this.span();
                        this.hover = Some(a + (b - a) * ((x - left) / width).clamp(0., 1.));
                        cx.notify();
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
                            move |bounds, _, window, _| paint_day(bounds, window, &paint),
                        )
                        .size_full(),
                    )
                    .child(counts),
            )
            .child(
                div()
                    .h(px(18.))
                    .mt(px(2.))
                    .flex()
                    .items_baseline()
                    .child(under),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child("Scroll to zoom \u{b7} drag to move"),
            )
    }
}

/// "09:14".
pub(super) fn hm(minutes: f32) -> String {
    let m = minutes.max(0.).round() as u32;
    let per_hour = MINUTES_PER_HOUR as u32;
    let last_hour = HOURS_PER_DAY as u32 - 1;
    format!("{:02}:{:02}", (m / per_hour).min(last_hour), m % per_hour)
}

/// The finest bar size that keeps bars about 6 px apart.
pub(super) fn bin_minutes(span: f32, width: f32) -> f32 {
    BIN_SIZES
        .into_iter()
        .find(|n| n / span * width >= BAR_SPACING)
        .unwrap_or(BIN_SIZES[BIN_SIZES.len() - 1])
}

// literal: the chart's drawing (DESIGN.md, Charts; design/screens/log.html)
pub(super) fn ring(
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

/// What the day chart draws: the span shown, the traffic in bins, the
/// decisions as ticks (routed) or rings (left alone), the pointer's minute.
struct DayPaint {
    from: f32,
    span: f32,
    bin: f32,
    max: f32,
    bins: Vec<(f32, u64)>,
    marks: Vec<(f32, bool)>,
    hover: Option<(f32, Option<f32>, f32)>,
    accent: gpui_kit::Rgba,
    muted: gpui_kit::Rgba,
    panel: gpui_kit::Rgba,
    base: gpui_kit::Rgba,
    grey: gpui_kit::Rgba,
}

// literal: the chart's drawing (DESIGN.md, Charts; design/screens/log.html)
/// Draw the day chart into `bounds`.
fn paint_day(bounds: gpui_kit::Bounds<gpui_kit::Pixels>, window: &mut Window, p: &DayPaint) {
    let DayPaint {
        from: a,
        span,
        bin,
        max,
        accent,
        muted,
        panel,
        base,
        grey,
        hover,
        ..
    } = *p;
    let (paint_bins, paint_marks) = (&p.bins, &p.marks);
    let w = f32::from(bounds.size.width);
    let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
    let baseline = top + BASELINE;
    let x_of = |m: f32| left + 2. + (w - 4.) * (m - a) / span;
    let scale = window.scale_factor();
    // Bars about half their spacing wide, 2 to 8 px.
    let bw = (bin / span * w * 0.55).clamp(2., 8.);
    for (start, bytes) in paint_bins {
        if *bytes == 0 {
            continue;
        }
        let h = 2. + 21. * (*bytes as f32 / max).sqrt();
        let x = x_of(start + bin / 2.);
        if x < left || x > left + w {
            continue;
        }
        if let Some(p) = crate::chart::bar(x, bw, baseline, h, scale) {
            let on = hover.is_some_and(|(_, s, _)| s == Some(*start));
            window.paint_path(p, if on { muted } else { grey });
        }
    }
    // Ticks keep clear of the bars (DESIGN.md, Charts).
    for (m, routed) in paint_marks {
        if *routed {
            let x = x_of(*m);
            for (dx, tip) in [(0., 24.), (4.5, 33.)] {
                if let Some(p) = crate::chart::rounded_bar(x + dx, top + tip - 1.5, baseline, 6.) {
                    window.paint_path(p, panel);
                }
            }
        }
    }
    // The pointer's minute: a dotted hairline that stops above
    // whatever stands there.
    if let Some((m, _, stop)) = hover {
        let x = x_of(m);
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
    let mut line = PathBuilder::stroke(px(1.));
    line.move_to(point(px(left), px(baseline + 0.5)));
    line.line_to(point(px(left + w), px(baseline + 0.5)));
    if let Ok(p) = line.build() {
        window.paint_path(p, base);
    }
    for (m, routed) in paint_marks {
        let x = x_of(*m);
        if *routed {
            for (dx, tip) in [(0., 24.), (4.5, 33.)] {
                if let Some(p) = crate::chart::rounded_bar(x + dx, top + tip, baseline, 3.) {
                    window.paint_path(p, accent);
                }
            }
        } else {
            for (fill, stroke) in [(Some(panel), None), (None, Some(1.6))] {
                if let Some(p) = ring(x, baseline - 8., 4., stroke) {
                    window.paint_path(p, fill.unwrap_or(muted));
                }
            }
        }
    }
}

/// Today's traffic between minutes `a` and `b`, in bins of `bin` minutes:
/// (start minute, bytes).
fn day_bins(model: &Model, a: f32, b: f32, bin: f32) -> Vec<(f32, u64)> {
    let mut bins: Vec<(f32, u64)> = Vec::new();
    for (m, bytes) in model.today_traffic() {
        if m < a - bin || m > b {
            continue;
        }
        let start = (m / bin).floor() * bin;
        match bins.last_mut() {
            Some((s, sum)) if *s == start => *sum += bytes,
            _ => bins.push((start, bytes)),
        }
    }
    bins
}

// literal: the chart's drawing (DESIGN.md, Charts; design/screens/log.html)
/// Today's decisions between minutes `a` and `b`: marks less than 8 px apart
/// share one, with its entries (minute, routed, entries).
fn day_marks(
    model: &Model,
    a: f32,
    b: f32,
    span: f32,
    width: f32,
) -> Vec<(f32, bool, Vec<LogEntry>)> {
    let mut marks: Vec<(f32, bool, Vec<LogEntry>)> = Vec::new();
    for (m, e) in model.today_events() {
        if m < a || m > b {
            continue;
        }
        let routed = e.kind == LogKind::Routed;
        match marks.iter_mut().rev().find(|x| x.1 == routed) {
            Some(x) if (m - x.0) / span * width < 8. => {
                x.0 = m;
                x.2.push(e);
            }
            _ => marks.push((m, routed, vec![e])),
        }
    }
    marks
}

/// The chart's scale: minutes shown, its width, the bin size, the busiest bin.
#[derive(Clone, Copy)]
struct Scale {
    span: f32,
    width: f32,
    bin: f32,
    max: f32,
}

// literal: the chart's drawing (DESIGN.md, Charts; design/screens/log.html)
/// What the pointer at minute `h` is on: a mark close by, else that bin.
/// (minute, the bin's start, time label, sentence, where the hairline stops)
fn pointed_at(
    h: f32,
    marks: &[(f32, bool, Vec<LogEntry>)],
    bins: &[(f32, u64)],
    scale: Scale,
) -> (f32, Option<f32>, String, String, f32) {
    let Scale {
        span,
        width,
        bin,
        max,
    } = scale;
    let near = marks
        .iter()
        .filter(|x| ((x.0 - h) / span * width).abs() <= 6.)
        .min_by(|p, q| (p.0 - h).abs().total_cmp(&(q.0 - h).abs()));
    match near {
        Some((m, routed, entries)) => {
            let refs: Vec<&LogEntry> = entries.iter().collect();
            let top = match (*routed, entries.len()) {
                (true, n) if n > 1 => 6.,
                (true, _) => 24. - 1.5,
                (false, _) => BASELINE - 8. - 4.,
            };
            (*m, None, hm(*m), crate::model::mark_text(&refs), top)
        }
        None => {
            let start = (h / bin).floor() * bin;
            let bytes = bins.iter().find(|x| x.0 == start).map(|x| x.1).unwrap_or(0);
            let time = if bin > 1. {
                format!("{}\u{2013}{}", hm(start), hm(start + bin))
            } else {
                hm(start)
            };
            let text = match (bytes, bin > 1.) {
                (0, _) => "No traffic".to_owned(),
                (b, true) => format!(
                    "{} of traffic in {} minutes",
                    crate::model::human_bytes(b),
                    bin
                ),
                (b, false) => format!("{} of traffic", crate::model::human_bytes(b)),
            };
            let top = if bytes > 0 {
                BASELINE - (2. + 21. * (bytes as f32 / max).sqrt())
            } else {
                BASELINE + 3.
            };
            (start + bin / 2., Some(start), time, text, top)
        }
    }
}

/// Above ticks that stand for several decisions: how many, in accent.
fn merged_counts(marks: &[(f32, bool, Vec<LogEntry>)], from: f32, span: f32, t: &Theme) -> Div {
    let mut counts = div().absolute().top_0().left_0().size_full();
    for (m, routed, entries) in marks {
        if *routed && entries.len() > 1 {
            counts = counts.child(
                div()
                    .absolute()
                    .top(px(4.))
                    .left(gpui_kit::relative(((m - from) / span).clamp(0., 1.)))
                    .ml(px(-8.))
                    .w(px(20.))
                    .flex()
                    .justify_center()
                    .font_family(fonts::MONO)
                    .text_size(px(11.))
                    .line_height(px(12.))
                    .text_color(t.accent)
                    .child(entries.len().to_string()),
            );
        }
    }
    counts
}
