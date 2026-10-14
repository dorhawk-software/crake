//! "Last 5 minutes" under the graph: the panel's chart, shorter and finer:
//! a bar every 5 s, direct below and through tunnels on top, sliding left
//! as time passes.

use std::collections::VecDeque;
use std::time::Instant;

use gpui_kit::{Bounds, Pixels, Rgba, Window};

pub const BARS: usize = 60;
const BAR_SECS: f32 = 5.;
pub const HEIGHT: f32 = 56.;
const BAR_WIDTH: f32 = 9.;
/// The tallest bar leaves this much above it.
const HEADROOM: f32 = 2.;

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

/// The bars, scaled to the busiest: direct in `grey`, tunnels in `accent`
/// on top.
pub fn paint(
    bars: &[(f64, f64)],
    bounds: Bounds<Pixels>,
    grey: Rgba,
    accent: Rgba,
    window: &mut Window,
) {
    let w = f32::from(bounds.size.width);
    let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
    let base = top + HEIGHT;
    let max = bars.iter().map(|(d, t)| d + t).fold(0., f64::max);
    if max <= 0. {
        return;
    }
    let scale = (HEIGHT - HEADROOM) as f64 / max;
    let step = (w - 2. * crate::chart::INSET) / BARS as f32;
    for (i, (d, t)) in bars.iter().enumerate() {
        // literal: a bar stands in the middle of its step
        let x = left + crate::chart::INSET + step * (i as f32 + 0.5);
        let (dh, th) = ((d * scale) as f32, (t * scale) as f32);
        if dh > 0.
            && let Some(p) = crate::chart::bar(x, BAR_WIDTH, base, dh, window.scale_factor())
        {
            window.paint_path(p, grey);
        }
        if th > 0.
            && let Some(p) = crate::chart::bar(x, BAR_WIDTH, base - dh, th, window.scale_factor())
        {
            window.paint_path(p, accent);
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
}
