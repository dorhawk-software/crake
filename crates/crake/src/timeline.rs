//! The timeline both charts are: the panel's “Last hour” and tunnel detail's
//! “Today through X”, each at its own span and size. Grey bars are traffic;
//! teal two-stroke ticks are where Crake routed a host, decisions close
//! together sharing one with how many above it; hollow circles are hosts it
//! tried and left alone. Pointing at it reads it
//! (design/proposals/chart-hover.html, tunnel-detail-chart.html): a dotted
//! hairline at the moment, and in place of the time row one line about it,
//! in the log's words.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, PathBuilder, Window, canvas, div, point, px, relative};

use crate::model::Mark;
use crate::theme::{Theme, fonts};

/// Where the pointer is (minutes ago), and the chart's left edge and width
/// on screen as last painted: kept by the view that shows the chart.
#[derive(Default)]
pub struct Hover {
    pub at: Option<f32>,
    pub geometry: Rc<Cell<(f32, f32)>>,
}

/// A chart's drawing, in px from its top.
#[derive(Clone, Copy)]
pub struct Shape {
    /// Where the baseline is; the chart is a pixel taller.
    pub baseline: f32,
    /// A bar's width, and its height with the least and the most traffic.
    pub bar: f32,
    pub bar_min: f32,
    pub bar_max: f32,
    /// The two strokes of a tick: their width and heights.
    pub tick: f32,
    pub tick_heights: [f32; 2],
    /// The count above a merged tick: its top.
    pub count_top: f32,
}

/// The panel's: tray-panel.html's 416 × 72.
pub const LAST_HOUR: Shape = Shape {
    baseline: 72.,
    bar: 3.6,
    bar_min: 3.,
    bar_max: 25.,
    tick: 3.2,
    tick_heights: [42., 32.],
    count_top: 12.,
};

/// Tunnel detail's: tunnel-detail.html's 840 × 44, with room above the
/// ticks for a count.
pub const TODAY: Shape = Shape {
    baseline: 58.,
    bar: 3.,
    bar_min: 2.,
    bar_max: 18.,
    tick: 3.,
    tick_heights: [34., 26.],
    count_top: 7.,
};

/// What one chart shows.
pub struct Timeline {
    /// Minutes shown, the right edge being now.
    pub span: f32,
    /// Traffic: (minutes ago of the bar's middle, bytes), a bar per `bin`
    /// minutes.
    pub bars: Vec<(f32, u64)>,
    pub bin: f32,
    pub marks: Vec<Mark>,
    /// How far each mark's two strokes have grown in (1 when not arriving).
    pub grown: Vec<[f32; 2]>,
    /// A bar's line: "6.7 MB went direct", "18 MB through ACME".
    pub words: Box<dyn Fn(u64) -> String>,
    /// A moment without traffic.
    pub nothing: String,
    /// The time row: (where, 0 to 1; its text). The ends align to the
    /// chart's edges, the rest centre on their moment.
    pub axis: Vec<(f32, String)>,
    pub shape: Shape,
}

/// "13:21", local time, `ago` minutes ago.
pub fn clock(ago: f32) -> String {
    jiff::Zoned::now()
        .checked_sub(jiff::SignedDuration::from_secs(
            (ago * crate::units::SECS_PER_MINUTE_F) as i64,
        ))
        .map(|z| z.strftime("%H:%M").to_string())
        .unwrap_or_default()
}

use crate::chart::INSET;

/// What the pointer readout and the painting agree on: a "left alone" ring
/// (its centre above the baseline, its radius), the clear space kept around
/// a tick, and how far short of what it points at the hairline stops.
const RING_LIFT: f32 = 10.;
const RING_R: f32 = 4.5;
const CLEARANCE: f32 = 1.5;
const HAIRLINE_GAP: f32 = 3.;
/// The pointer is on a mark within this many pixels of it (on a chart at
/// least MIN_POINT_WIDTH wide).
const NEAR_PX: f32 = 6.;
const MIN_POINT_WIDTH: f32 = 100.;

impl Timeline {
    fn bar_height(&self, bytes: u64, max: f32) -> f32 {
        let s = self.shape;
        s.bar_min + (s.bar_max - s.bar_min) * (bytes as f32 / max).sqrt()
    }

    /// What the pointer at `at` minutes ago is on: a mark close by, else
    /// that moment's bar. (minutes ago, the bar pointed at, time, text, where
    /// the hairline stops.)
    fn pointed(&self, at: f32, width: f32) -> (f32, Option<f32>, String, String, f32) {
        let s = self.shape;
        // Within about NEAR_PX of a mark: the mark.
        let near = self.span * NEAR_PX / width.max(MIN_POINT_WIDTH);
        let mark = self
            .marks
            .iter()
            .filter(|m| (m.ago - at).abs() <= near)
            .min_by(|a, b| (a.ago - at).abs().total_cmp(&(b.ago - at).abs()));
        if let Some(m) = mark {
            // The hairline stops above what stands there: the count, the
            // tick and its gap, or the circle.
            let tallest = s.baseline - s.tick_heights[0];
            let top = match (m.routed, m.count) {
                (true, n) if n > 1 => s.count_top,
                (true, _) => tallest - CLEARANCE,
                (false, _) => s.baseline - RING_LIFT - RING_R,
            };
            return (m.ago, None, m.time.clone(), m.text.clone(), top);
        }
        let max = self.bars.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32;
        let bar = self
            .bars
            .iter()
            .filter(|(ago, _)| (ago - at).abs() <= self.bin / 2.)
            .min_by(|a, b| (a.0 - at).abs().total_cmp(&(b.0 - at).abs()))
            .copied();
        match bar {
            Some((ago, bytes)) if bytes > 0 => (
                ago,
                Some(ago),
                clock(ago),
                (self.words)(bytes),
                s.baseline - self.bar_height(bytes, max),
            ),
            _ => (
                at,
                None,
                clock(at),
                self.nothing.clone(),
                s.baseline + HAIRLINE_GAP,
            ),
        }
    }

    /// The chart and its row underneath. `hover` is the showing view's
    /// (`get` finds it there), which the pointer moves.
    pub fn element<V: 'static>(
        self,
        hover: &Hover,
        t: &Theme,
        cx: &mut Context<V>,
        get: fn(&mut V) -> &mut Hover,
    ) -> Div {
        let shape = self.shape;
        let span = self.span;
        let (_, width) = hover.geometry.get();
        let pointed = hover.at.map(|at| self.pointed(at, width));
        let mono = |s: String| {
            div()
                .font_family(fonts::MONO)
                .text_size(px(11.))
                .line_height(px(14.))
                .text_color(t.muted)
                .whitespace_nowrap()
                .child(s)
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
                let mut row = div().relative().w_full().h_full();
                for (at, text) in &self.axis {
                    row = row.child(if *at <= 0. {
                        div().absolute().left_0().child(mono(text.clone()))
                    } else if *at >= 1. {
                        div().absolute().right_0().child(mono(text.clone()))
                    } else {
                        // Centred under its moment.
                        div()
                            .absolute()
                            .left(relative(*at))
                            .w(px(40.))
                            .ml(px(-20.))
                            .flex()
                            .justify_center()
                            .child(mono(text.clone()))
                    });
                }
                row
            }
        };
        let counts = self.counts(t);
        let paint = Paint {
            hover_x: pointed.as_ref().map(|p| (p.0, p.4)),
            hover_bar: pointed.as_ref().and_then(|p| p.1),
            max: self.bars.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32,
            accent: t.accent,
            muted: t.muted,
            panel: t.panel,
            base: t.line_strong,
            grey: t.bars,
            timeline: self,
        };
        let geometry = hover.geometry.clone();
        div()
            .child(
                div()
                    .id("chart")
                    .relative()
                    .w_full()
                    .h(px(shape.baseline + 1.))
                    .mt(px(8.))
                    .on_mouse_move(
                        cx.listener(move |this, e: &gpui_kit::MouseMoveEvent, _, cx| {
                            let hover = get(this);
                            let (left, width) = hover.geometry.get();
                            if width <= 2. * INSET {
                                return;
                            }
                            let x = f32::from(e.position.x) - left;
                            let plot = width - 2. * INSET;
                            let ago = (span * (1. - (x - INSET) / plot)).clamp(0., span);
                            if hover.at != Some(ago) {
                                hover.at = Some(ago);
                                cx.notify();
                            }
                        }),
                    )
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if !*hovered {
                            get(this).at = None;
                            cx.notify();
                        }
                    }))
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                geometry
                                    .set((f32::from(bounds.left()), f32::from(bounds.size.width)));
                            },
                            move |bounds, _, window, _| paint.paint(bounds, window),
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

    /// Above ticks that stand for several decisions: how many, in accent,
    /// centred over the tick's two strokes.
    fn counts(&self, t: &Theme) -> Div {
        let mut counts = div().absolute().top_0().left_0().size_full();
        for m in self.marks.iter().filter(|m| m.routed && m.count > 1) {
            counts = counts.child(
                div()
                    .absolute()
                    .top(px(self.shape.count_top))
                    .left(relative((self.span - m.ago) / self.span))
                    .w(px(20.))
                    .ml(px(-10. + 2. + self.shape.tick * 0.8))
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
}

/// What the canvas draws: the timeline and the pointer's moment.
struct Paint {
    timeline: Timeline,
    max: f32,
    hover_x: Option<(f32, f32)>,
    hover_bar: Option<f32>,
    accent: gpui_kit::Rgba,
    muted: gpui_kit::Rgba,
    panel: gpui_kit::Rgba,
    base: gpui_kit::Rgba,
    grey: gpui_kit::Rgba,
}

impl Paint {
    // literal: the chart's drawing (DESIGN.md, Charts)
    fn paint(&self, bounds: gpui_kit::Bounds<gpui_kit::Pixels>, window: &mut Window) {
        let tl = &self.timeline;
        let s = tl.shape;
        let w = f32::from(bounds.size.width);
        let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
        let baseline = top + s.baseline;
        let x_of = |ago: f32| left + INSET + (w - 2. * INSET) * (tl.span - ago) / tl.span;
        for (ago, bytes) in &tl.bars {
            if *bytes == 0 {
                continue;
            }
            let h = tl.bar_height(*bytes, self.max);
            if let Some(p) =
                crate::chart::bar(x_of(*ago), s.bar, baseline, h, window.scale_factor())
            {
                let on = self.hover_bar.is_some_and(|b| (b - ago).abs() < 0.01);
                window.paint_path(p, if on { self.muted } else { self.grey });
            }
        }
        // The hairline never draws over a bar, a tick or a circle: it ends
        // 3 px above whatever stands at that moment.
        if let Some((ago, stop)) = self.hover_x {
            let x = x_of(ago);
            let end = top + stop - HAIRLINE_GAP;
            if end - (top + 2.) >= 4. {
                let mut line = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(3.)]);
                line.move_to(point(px(x), px(top + 2.)));
                line.line_to(point(px(x), px(end)));
                if let Ok(p) = line.build() {
                    window.paint_path(p, self.base);
                }
            }
        }
        // A stroke grown `g` of the way up from the baseline.
        let stroke_top =
            |height: f32, g: f32| (baseline - height * g).min(baseline - crate::chart::LINE);
        let strokes = |x: f32| {
            [
                (x, s.tick_heights[0]),
                (x + s.tick + 1.8, s.tick_heights[1]),
            ]
        };
        let grown = |i: usize| tl.grown.get(i).copied().unwrap_or([1., 1.]);
        // A tick keeps clear of the bars: a gap in the panel's colour
        // around it, drawn before the baseline so the baseline stays whole
        // (DESIGN.md, Charts).
        for (i, m) in tl.marks.iter().enumerate().filter(|(_, m)| m.routed) {
            for ((x, height), g) in strokes(x_of(m.ago)).into_iter().zip(grown(i)) {
                if g <= 0. {
                    continue;
                }
                let gap = crate::chart::rounded_bar(
                    x,
                    stroke_top(height, g) - CLEARANCE,
                    baseline,
                    s.tick + 3.,
                );
                if let Some(p) = gap {
                    window.paint_path(p, self.panel);
                }
            }
        }
        let mut line = PathBuilder::stroke(px(1.));
        line.move_to(point(px(left), px(baseline + 0.5)));
        line.line_to(point(px(left + w), px(baseline + 0.5)));
        if let Ok(p) = line.build() {
            window.paint_path(p, self.base);
        }
        for (i, m) in tl.marks.iter().enumerate() {
            let x = x_of(m.ago);
            if m.routed {
                for ((x, height), g) in strokes(x).into_iter().zip(grown(i)) {
                    if g <= 0. {
                        continue;
                    }
                    if let Some(p) =
                        crate::chart::rounded_bar(x, stroke_top(height, g), baseline, s.tick)
                    {
                        window.paint_path(p, self.accent);
                    }
                }
            } else {
                if let Some(p) = crate::chart::circle(x, baseline - RING_LIFT, RING_R, None) {
                    window.paint_path(p, self.panel);
                }
                let ring = Some(crate::chart::LINE);
                if let Some(p) = crate::chart::circle(x, baseline - RING_LIFT, RING_R, ring) {
                    window.paint_path(p, self.muted);
                }
            }
        }
    }
}
