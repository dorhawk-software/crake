//! The bird in motion (design/proposals/animations.html): the frames from
//! one state to the next, and connecting's loop. Every frame is an `Art`;
//! `updates::animate_tray` shows them 70 ms apart.

use super::{Art, Badge, Shape, rest};

/// The frames from one state to the next, ending at the resting look of
/// `to`. Connecting has its own loop (`connecting_loop`). A badge stays on
/// through the motion, or appears once after it (a red one in an amber
/// one's place too).
pub fn transition(from: Shape, to: Shape) -> Vec<Art> {
    let ((from, had), (to, has)) = (from.badged(), to.badged());
    let mut v = transition_unbadged(from, to);
    match (had, has) {
        (Some(a), Some(b)) if a == b => {
            for art in &mut v {
                art.badge = Some(1.);
                art.warning = b == Badge::Warning;
            }
        }
        (_, Some(b)) => {
            let rest = rest(to);
            v.extend(steps(BADGE_STEPS).map(|s| Art {
                badge: Some(s),
                warning: b == Badge::Warning,
                ..rest
            }));
        }
        _ => {}
    }
    v
}

/// How many frames the badge takes to appear.
const BADGE_STEPS: usize = 4;

/// `n` steps from just past 0 to 1, eased.
fn steps(n: usize) -> impl Iterator<Item = f32> {
    (1..=n).map(move |i| crate::motion::ease_out(i as f32 / n as f32))
}

// literal: the icon's motion, frame by frame (design/proposals/animations.html)
fn transition_unbadged(from: Shape, to: Shape) -> Vec<Art> {
    if from == to {
        return Vec::new();
    }
    let head = rest(Shape::Listening);
    match (from, to) {
        // The dashed bird fills in, then the eye opens.
        (Shape::Connecting, Shape::Listening | Shape::Routed) => {
            let mut v: Vec<Art> = steps(3)
                .map(|f| Art {
                    dashes: Some(0.),
                    fill: f,
                    eye: 0.,
                    ..Art::default()
                })
                .collect();
            v.extend(steps(3).map(|e| Art { eye: e, ..head }));
            v
        }
        // crex, crex: the bill opens as the first mark draws, the second
        // follows; the marks dim and the bill half closes between the two
        // calls.
        (_, Shape::Routed) => {
            let mut v = Vec::new();
            for _ in 0..2 {
                v.extend(steps(2).map(|a| Art {
                    bill: a,
                    calls: Some((a, 0., 1.)),
                    ..head
                }));
                v.extend(steps(2).map(|b| Art {
                    bill: 1.,
                    calls: Some((1., b, 1.)),
                    ..head
                }));
                v.push(Art {
                    bill: 1.,
                    calls: Some((1., 1., 1.)),
                    ..head
                });
                v.push(Art {
                    bill: 0.5,
                    calls: Some((1., 1., 0.25)),
                    ..head
                });
            }
            v.push(rest(Shape::Routed));
            v
        }
        // The marks fade as the bill closes.
        (Shape::Routed, Shape::Listening) => [0.7, 0.4, 0.15]
            .iter()
            .map(|o| Art {
                bill: *o,
                calls: Some((1., 1., *o)),
                ..head
            })
            .chain([head])
            .collect(),
        // The eye closes, then the head drops and dims: asleep.
        (_, Shape::Off) => {
            let mut v: Vec<Art> = steps(3)
                .map(|c| Art {
                    eye: 1. - c,
                    ..head
                })
                .collect();
            v.extend(steps(3).map(|t| Art {
                eye: 0.,
                sleep: t,
                ..head
            }));
            v
        }
        // It wakes: the head comes up and brightens, then the eye opens.
        (Shape::Off, Shape::Listening) => {
            let mut v: Vec<Art> = steps(3)
                .map(|t| Art {
                    eye: 0.,
                    sleep: 1. - t,
                    ..head
                })
                .collect();
            v.extend(steps(3).map(|e| Art { eye: e, ..head }));
            v
        }
        _ => Vec::new(),
    }
}

// literal: the icon's motion, frame by frame (design/proposals/animations.html)
/// Connecting: the dashed bird, its dashes going round the outline, one
/// dash and gap every 0.7 s, as on dorhawk.software/crake
/// (design/proposals/tray-connecting.html). Every frame is the whole
/// outline, so a frame the bar misses only makes the dashes jump a little;
/// a line drawing itself read as blinking there. With `badge`, that badge
/// stays on it.
pub fn connecting_loop(badge: Option<Badge>) -> Vec<Art> {
    const STEPS: u8 = 10;
    (0..STEPS)
        .map(|i| Art {
            dashes: Some(f32::from(i) / f32::from(STEPS)),
            badge: badge.map(|_| 1.),
            warning: badge == Some(Badge::Warning),
            ..Art::default()
        })
        .collect()
}
