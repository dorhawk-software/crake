//! The connecting head (the tray's, `tray.rs`, and
//! assets/icons/tray-head-connecting.svg) drawn in a window: dashed, its
//! dashes going round while Crake reaches a gateway
//! (design/proposals/newtunnel-asking.html). A GPUI path, sharp at any
//! scale. GPUI's dashes have no offset, so the pattern is spelled out from
//! part-way through a dash instead.

use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{Path, PathBuilder, Pixels, Point, point, px};

/// One dash and gap goes by in this long, as in the tray and on
/// dorhawk.software/crake.
pub const ROUND: Duration = Duration::from_millis(700);

// literal: the head's drawing (assets/icons/tray-head-connecting.svg)
const VIEW_X: f32 = 7.1;
// literal: the head's drawing (assets/icons/tray-head-connecting.svg)
const VIEW_Y: f32 = 6.6;
// literal: the head's drawing (assets/icons/tray-head-connecting.svg)
const VIEW: f32 = 15.2;
// literal: the head's drawing (assets/icons/tray-head-connecting.svg)
const DASH: f32 = 2.4;
// literal: the head's drawing (assets/icons/tray-head-connecting.svg)
const GAP: f32 = 1.9;
const STROKE: f32 = 2.;
/// Longer than the outline (about 44 units), with a dash and gap to spare.
const OUTLINE: f32 = 50.;

// literal: the head's drawing (assets/icons/tray-head-connecting.svg)
/// The dashed head in a `size` square at `origin`, its dashes moved
/// `phase` (0–1) of a dash and gap along the outline.
pub fn path(origin: Point<Pixels>, size: f32, phase: f32) -> Option<Path<Pixels>> {
    let s = size / VIEW;
    let at = |x: f32, y: f32| {
        point(
            origin.x + px((x - VIEW_X) * s),
            origin.y + px((y - VIEW_Y) * s),
        )
    };
    let mut p = PathBuilder::stroke(px(STROKE * s)).dash_array(&dashes(phase, s));
    p.move_to(at(8.5, 20.5));
    p.line_to(at(8.5, 12.));
    p.arc_to(
        point(px(3.75 * s), px(3.75 * s)),
        px(0.),
        false,
        true,
        at(15.95, 11.4),
    );
    p.line_to(at(16., 10.4));
    p.line_to(at(20.6, 11.6));
    p.line_to(at(16., 12.8));
    p.cubic_bezier_to(at(13.4, 20.5), at(16., 15.5), at(15.1, 17.8));
    p.close();
    p.build().ok()
}

/// The dashes moved `phase` of a dash and gap along the outline (SVG's
/// negative dash offset), at scale `s`, written out for the whole outline.
fn dashes(phase: f32, s: f32) -> Vec<Pixels> {
    let period = DASH + GAP;
    // Where in the pattern the outline starts.
    let at = (period - phase.rem_euclid(1.) * period) % period;
    let mut v = if at < DASH {
        vec![DASH - at, GAP]
    } else {
        vec![0., period - at]
    };
    while v.iter().sum::<f32>() < OUTLINE {
        v.extend([DASH, GAP]);
    }
    v.into_iter().map(|d| px(d * s)).collect()
}

/// The second something under way is in, as it's counted beside the head:
/// quiet for two seconds, the first number shown is 3, not a 2 the person
/// has just sat through (the user, 2026-10-19).
pub fn second_counted(for_: Duration) -> u64 {
    for_.as_secs() + 1
}

/// The head at 16 px, its dashes going round since `since`: Crake's sign
/// that something is under way (New tunnel asking the gateway, a report
/// being saved). Every frame until it's gone, which also keeps a caller's
/// seconds counting. Still with reduce motion on.
pub fn under_way(color: gpui_kit::Rgba, since: std::time::Instant) -> impl IntoElement {
    const SIZE: f32 = 16.;
    gpui_kit::canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let phase = if crate::motion::enabled() {
                since.elapsed().as_secs_f32()
                    / (ROUND.as_secs_f32() * crate::motion::slowed() as f32)
            } else {
                0.
            };
            if let Some(p) = path(bounds.origin, SIZE, phase) {
                window.paint_path(p, color);
            }
            window.request_animation_frame();
        },
    )
    .size(px(SIZE))
    .flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(phase: f32) -> Vec<f32> {
        dashes(phase, 1.).into_iter().map(f32::from).collect()
    }

    #[test]
    fn the_dashes_move_along_the_outline() {
        // At rest: the connecting icon's own pattern.
        assert_eq!(&units(0.)[..4], &[2.4, 1.9, 2.4, 1.9]);
        // A quarter round on, the first dash has moved forward along the
        // outline: it starts with a gap that long (an empty dash first)...
        let a = units(0.25);
        assert_eq!(a[0], 0.);
        assert!((a[1] - 4.3 * 0.25).abs() < 1e-4, "{a:?}");
        // ...half a round on, with the end of the dash before it.
        let b = units(0.5);
        assert!((b[0] - (2.4 - 4.3 * 0.5)).abs() < 1e-4, "{b:?}");
        assert_eq!(b[1], 1.9);
        // A whole round is where it started; the pattern covers the outline.
        assert_eq!(units(1.), units(0.));
        assert!(units(0.7).iter().sum::<f32>() >= OUTLINE);
        assert!(path(point(px(0.), px(0.)), 16., 0.3).is_some());
    }
}
