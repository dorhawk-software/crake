//! "Last 5 minutes" under the graph: the panel's chart, shorter and finer:
//! a bar every 5 s, through tunnels below and direct on top, both grey,
//! sliding left as time passes; a teal tick where Crake routed a host
//! (DESIGN.md, Charts: teal is a decision, never volume;
//! design/proposals/critique-2026-10-15, 2).

use std::collections::VecDeque;
use std::time::Instant;

use gpui_kit::{Bounds, Pixels, Rgba, Window};

pub const BARS: usize = 60;
const BAR_SECS: f32 = 5.;
pub const HEIGHT: f32 = 56.;
const BAR_WIDTH: f32 = 9.;
/// The tallest bar leaves this much above it.
const HEADROOM: f32 = 2.;
/// A tick's two strokes (the panel chart's): their width, heights and the
/// space between them; the gap in the panel's colour around it.
const TICK: f32 = 3.2;
const TICK_HEIGHTS: [f32; 2] = [42., 32.];
const TICK_APART: f32 = 1.8;
const TICK_GAP: f32 = 1.5;
/// Seconds in a minute: the chart's marks count minutes ago.
const SECS_PER_MINUTE: f32 = 60.;

/// The strip's colours.
#[derive(Clone, Copy)]
pub struct Colours {
    pub tunnels: Rgba,
    pub direct: Rgba,
    pub baseline: Rgba,
    pub accent: Rgba,
    /// An undone decision's tick.
    pub undone: Rgba,
    pub panel: Rgba,
}

/// Each bar's tick: none, or whether every decision under it was undone
/// (drawn grey, as in the panel), from the chart's marks (minutes ago).
pub fn ticks(marks: &[crate::model::Mark]) -> Vec<Option<bool>> {
    let mut ticks = vec![None; BARS];
    for m in marks.iter().filter(|m| m.routed) {
        let i = (m.ago * SECS_PER_MINUTE / BAR_SECS) as usize;
        if i < BARS {
            let t = &mut ticks[BARS - 1 - i];
            *t = Some(t.unwrap_or(true) && m.undone);
        }
    }
    ticks
}

/// The marks to ask the model for: the strip's five minutes, merged per bar.
pub const SPAN_MINUTES: f32 = BARS as f32 * BAR_SECS / SECS_PER_MINUTE;
pub const MERGE_MINUTES: f32 = BAR_SECS / SECS_PER_MINUTE;

/// Each bar's average (direct, through tunnels) bytes/s, oldest first,
/// the last one ending now.
pub fn bars(history: &VecDeque<(Instant, f64, f64)>, now: Instant) -> Vec<(f64, f64)> {
    let mut sums = vec![(0., 0., 0u32); BARS];
    for (at, direct, tunnels) in history {
        let ago = now.saturating_duration_since(*at).as_secs_f32();
        let i = (ago / BAR_SECS) as usize;
        if i < BARS {
            let s = &mut sums[BARS - 1 - i];
            s.0 += direct;
            s.1 += tunnels;
            s.2 += 1;
        }
    }
    sums.into_iter()
        .map(|(d, t, n)| {
            if n == 0 {
                (0., 0.)
            } else {
                (d / f64::from(n), t / f64::from(n))
            }
        })
        .collect()
}

/// The bars, scaled to the busiest: through tunnels on the baseline,
/// direct on top, both grey; then each tick, a gap in the panel's colour
/// around it, standing at the right edge of its bar's slot so no bar is
/// hidden (a tick is wider than the space between bars).
pub fn paint(
    bars: &[(f64, f64)],
    ticks: &[Option<bool>],
    bounds: Bounds<Pixels>,
    c: Colours,
    window: &mut Window,
) {
    let w = f32::from(bounds.size.width);
    let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
    let base = top + HEIGHT - 1.;
    let step = (w - 2. * crate::chart::INSET) / BARS as f32;
    let max = bars.iter().map(|(d, t)| d + t).fold(0., f64::max);
    if max > 0. {
        let scale = (HEIGHT - 1. - HEADROOM) as f64 / max;
        for (i, (d, t)) in bars.iter().enumerate() {
            // literal: a bar stands in the middle of its step
            let x = left + crate::chart::INSET + step * (i as f32 + 0.5);
            let (dh, th) = ((d * scale) as f32, (t * scale) as f32);
            if th > 0.
                && let Some(p) = crate::chart::bar(x, BAR_WIDTH, base, th, window.scale_factor())
            {
                window.paint_path(p, c.tunnels);
            }
            if dh > 0.
                && let Some(p) =
                    crate::chart::bar(x, BAR_WIDTH, base - th, dh, window.scale_factor())
            {
                window.paint_path(p, c.direct);
            }
        }
    }
    let strokes = |i: usize| {
        let right = left + crate::chart::INSET + step * (i as f32 + 1.);
        let first = right - (TICK + TICK_APART) / 2.;
        [
            (first, TICK_HEIGHTS[0]),
            (first + TICK + TICK_APART, TICK_HEIGHTS[1]),
        ]
    };
    for (i, _) in ticks.iter().enumerate().filter(|(_, t)| t.is_some()) {
        for (x, h) in strokes(i) {
            if let Some(p) =
                crate::chart::rounded_bar(x, base - h - TICK_GAP, base, TICK + 2. * TICK_GAP)
            {
                window.paint_path(p, c.panel);
            }
        }
    }
    let mut line = gpui_kit::PathBuilder::stroke(gpui_kit::px(1.));
    line.move_to(gpui_kit::point(
        gpui_kit::px(left),
        gpui_kit::px(base + 0.5),
    ));
    line.line_to(gpui_kit::point(
        gpui_kit::px(left + w),
        gpui_kit::px(base + 0.5),
    ));
    if let Ok(p) = line.build() {
        window.paint_path(p, c.baseline);
    }
    for (i, t) in ticks.iter().enumerate() {
        let Some(undone) = t else { continue };
        for (x, h) in strokes(i) {
            if let Some(p) = crate::chart::rounded_bar(x, base - h, base, TICK) {
                window.paint_path(p, if *undone { c.undone } else { c.accent });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn each_bar_is_the_average_of_its_five_seconds() {
        let now = Instant::now();
        let at = |secs: u64| now.checked_sub(Duration::from_secs(secs)).unwrap();
        let history: VecDeque<_> = [
            (at(1), 100., 10.),
            (at(3), 300., 30.),
            (at(7), 50., 0.),
            (at(400), 9., 9.),
        ]
        .into();
        let b = bars(&history, now);
        assert_eq!(b.len(), BARS);
        assert_eq!(b[BARS - 1], (200., 20.), "the last 5 s");
        assert_eq!(b[BARS - 2], (50., 0.));
        assert_eq!(b[0], (0., 0.), "older than five minutes: gone");
    }

    #[test]
    fn a_tick_stands_where_crake_routed() {
        let mark = |ago: f32, routed: bool, undone: bool| crate::model::Mark {
            ago,
            routed,
            count: 1,
            undone,
            ts: String::new(),
            time: String::new(),
            text: String::new(),
        };
        // Routed 7 s ago and 2 min ago (undone), left alone 1 min ago.
        let t = ticks(&[
            mark(7. / 60., true, false),
            mark(2., true, true),
            mark(1., false, false),
            mark(6., true, false),
        ]);
        assert_eq!(t.len(), BARS);
        assert_eq!(t[BARS - 2], Some(false), "7 s ago: the bar before the last");
        assert_eq!(t[BARS - 1 - 24], Some(true), "undone: grey");
        assert_eq!(
            t.iter().filter(|x| x.is_some()).count(),
            2,
            "left alone and too old: none"
        );
    }
}
