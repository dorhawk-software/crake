//! The bird, drawn: the crake calling (design/proposals/bird-calling.html).
//! One geometry in a 32 px square: the head raised in the call, a short
//! stout bill parted at the tip, the eye a square hole on whole pixels,
//! the folded wing one straight cut across the breast, the base running
//! off the bottom edge. Drawn at any size with the eye snapped to that
//! size's pixels, and at any point between the states for the motion:
//! the bill opens for the call, the head drops to sleep.

use super::{Icon, SIZE, SMALL, raster};

/// A point in the drawing's square.
type P = (f32, f32);

/// How many points the head's outline has.
const POINTS: usize = 16;

/// The drawing's square.
const SQUARE: f32 = 32.;

// literal: the bird's drawing (design/proposals/bird-calling.html)
/// The head with the bill level, in order round its outline: the base of
/// the neck, the back of the head, the curve up to the crown (two control
/// points, then the crown's start), the crown's end, the curve down the
/// forehead to the top of the bill, the upper tip, the gape's upper and
/// lower corners, the lower tip, the bottom of the bill, the throat, the
/// front of the neck.
const AT_REST: [P; POINTS] = [
    (6., 18.),
    (5., 14.),
    (5., 6.5),
    (8.5, 3.),
    (14., 3.),
    (18.5, 3.),
    (20.5, 3.),
    (22.2, 4.5),
    (22.5, 6.6),
    (30., 8.7),
    (23.5, 9.6),
    (23.5, 10.6),
    (29.6, 11.6),
    (22.5, 13.6),
    (21.2, 15.2),
    (20., 19.),
];
// literal: the bird's drawing (design/proposals/bird-calling.html)
/// The call: the lower mandible dropped.
const CALLING: [P; POINTS] = [
    (6., 18.),
    (5., 14.),
    (5., 6.5),
    (8.5, 3.),
    (14., 3.),
    (18.5, 3.),
    (20.5, 3.),
    (22.2, 4.5),
    (22.5, 6.6),
    (30., 8.2),
    (23.5, 9.6),
    (24.2, 11.6),
    (28., 16.6),
    (22.8, 14.6),
    (21.2, 15.2),
    (20., 19.),
];
// literal: the bird's drawing (design/proposals/bird-calling.html)
/// The bill shut: the dashed outline, and asleep.
const SHUT: [P; POINTS] = [
    (6., 18.),
    (5., 14.),
    (5., 6.5),
    (8.5, 3.),
    (14., 3.),
    (18.5, 3.),
    (20.5, 3.),
    (22.2, 4.5),
    (22.5, 6.6),
    (30., 10.1),
    (30., 10.1),
    (30., 10.1),
    (30., 10.1),
    (22.5, 13.6),
    (21.2, 15.2),
    (20., 19.),
];
// literal: the bird's drawing (design/proposals/bird-calling.html)
/// From the front of the neck on: the breast (a curve to the base), the
/// base along the bottom edge, up the back to the base of the neck.
const BODY: [P; 6] = [
    (19.5, 22.5),
    (19.5, 26.5),
    (26., 28.5),
    (29., 32.),
    (2., 32.),
    (4.5, 22.),
];
// literal: the bird's drawing (design/proposals/bird-calling.html)
/// The folded wing: a cut two pixels wide from the shoulder to the base.
const WING: [P; 4] = [(7., 20.), (9., 20.), (15., 32.), (13., 32.)];
/// The eye's centre in the head's frame.
const EYE: P = (18., 8.);
/// The eye's side, and the slit's height, asleep.
const EYE_SIDE: f32 = 3.;
const SLIT: P = (5., 1.2);
/// The head turns about the base of the neck.
const PIVOT: P = (13., 18.);
/// The head raised in the call (degrees; up is negative), and dropped to
/// sleep (DESIGN.md: tipped 25°).
const RAISED: f32 = -20.;
const DROPPED: f32 = 25.;
/// How bright an asleep bird is.
const ASLEEP: f32 = 0.45;
// literal: the bird's drawing (design/proposals/bird-calling.html)
/// The call marks, in front of the open bill, and their width.
const MARK_1: (P, P) = ((27.8, 4.4), (30.4, 1.8));
const MARK_2: (P, P) = ((28.2, 8.4), (30.4, 7.75));
const MARK_WIDTH: f32 = 2.1;
// literal: the bird's drawing (design/proposals/bird-calling.html)
/// The badge behind the head: its centre and radius.
const BADGE: (P, f32) = ((5.5, 5.5), 4.5);
/// The gap cut round a coloured mark, so it reads on any bar.
const GAP: f32 = 1.;
/// Connecting: the outline's stroke, its dash and gap.
const STROKE: f32 = 2.;
const DASH: P = (4., 3.);

/// One frame of the bird (DESIGN.md, Tray icon; animations.html): the
/// resting look of a state, or any point between two.
#[derive(Clone, Copy, Debug, Default)]
pub struct Art {
    /// The outline dashed, as the connecting icon is, its dashes moved
    /// this far along one dash and gap (0–1), or none.
    pub dashes: Option<f32>,
    /// How filled the bird is (0–1).
    pub fill: f32,
    /// How open the eye is: 0 closed (a slit), 1 open.
    pub eye: f32,
    /// How open the bill is: 0 parted (at rest), 1 the call.
    pub bill: f32,
    /// The two call marks: how far each is drawn (0–1), and their opacity.
    pub calls: Option<(f32, f32, f32)>,
    /// The badge's size (0–1), or none.
    pub badge: Option<f32>,
    /// The badge is the amber triangle (needs you), not the red circle.
    pub warning: bool,
    /// Asleep (0–1): the head drops, the bill shuts, the ink dims.
    pub sleep: f32,
}

fn rot(p: P, degrees: f32) -> P {
    let (s, c) = degrees.to_radians().sin_cos();
    let (x, y) = (p.0 - PIVOT.0, p.1 - PIVOT.1);
    (PIVOT.0 + x * c - y * s, PIVOT.1 + x * s + y * c)
}

fn lerp(a: P, b: P, t: f32) -> P {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// A point in the drawing, at scale `s`, for a path.
fn at(p: P, s: f32) -> String {
    format!("{:.2} {:.2}", p.0 * s, p.1 * s)
}

/// The bird's outline at `s` times the drawing: the head at `tilt`
/// degrees, its points `head`, then the body.
// literal: the outline's points in order (AT_REST, BODY)
fn outline(head: &[P; POINTS], tilt: f32, s: f32) -> String {
    let p = |i: usize| at(rot(head[i], tilt), s);
    let b = |i: usize| at(BODY[i], s);
    format!(
        "M{}L{}C{} {} {}L{}C{} {} {}L{}L{}L{}L{}L{}L{}L{}L{}C{} {} {}L{}L{}Z",
        p(0),
        p(1),
        p(2),
        p(3),
        p(4),
        p(5),
        p(6),
        p(7),
        p(8),
        p(9),
        p(10),
        p(11),
        p(12),
        p(13),
        p(14),
        p(15),
        b(0),
        b(1),
        b(2),
        b(3),
        b(4),
        b(5)
    )
}

/// The two call marks at scale `s`: the gap cut for both, and each drawn
/// `a` and `b` of its length in `colour` at `opacity`.
pub(super) fn marks(s: f32, a: f32, b: f32, opacity: f32, colour: &str) -> (String, String) {
    let d = |m: (P, P)| format!("M{}L{}", at(m.0, s), at(m.1, s));
    let cut = format!(
        r##"<path d="{}{}" fill="none" stroke="#000" stroke-width="{:.2}" stroke-linecap="round" opacity="{opacity:.2}"/>"##,
        d(MARK_1),
        d(MARK_2),
        (MARK_WIDTH + 2. * GAP) * s
    );
    let mut over = String::new();
    for (m, drawn) in [(MARK_1, a), (MARK_2, b)] {
        // literal: a mark that short isn't drawn
        if drawn > 0.02 {
            let length = ((m.1.0 - m.0.0).powi(2) + (m.1.1 - m.0.1).powi(2)).sqrt() * s;
            over += &format!(
                r##"<path d="{}" fill="none" stroke="{colour}" stroke-width="{:.2}" stroke-linecap="round" stroke-dasharray="{length:.2}" stroke-dashoffset="{:.2}" opacity="{opacity:.2}"/>"##,
                d(m),
                MARK_WIDTH * s,
                length * (1. - drawn)
            );
        }
    }
    (cut, over)
}

/// The badge at scale `s`, grown to `size`: the gap cut, the badge in
/// `colour`, and its "!" in `on` (once it's nearly grown).
// literal: the badge's drawing (design/proposals/bird-calling.html)
pub(super) fn badge(
    s: f32,
    size: f32,
    warning: bool,
    colour: &str,
    on: &str,
) -> (String, String, String) {
    let ((x, y), r) = ((BADGE.0.0 * s, BADGE.0.1 * s), BADGE.1 * s);
    let grow = format!(
        r#" transform="translate({x:.2} {y:.2}) scale({size:.3}) translate({:.2} {:.2})""#,
        -x, -y
    );
    let bar = 2. * s;
    let mark = |top: f32, height: f32, dot: f32| {
        format!(
            r##"<rect x="{:.2}" y="{:.2}" width="{bar:.2}" height="{height:.2}" fill="{on}"/><rect x="{:.2}" y="{:.2}" width="{bar:.2}" height="{bar:.2}" fill="{on}"/>"##,
            x - bar / 2.,
            top,
            x - bar / 2.,
            dot
        )
    };
    if warning {
        let triangle = format!(
            "M{x:.2} {:.2}L{:.2} {:.2}H{:.2}Z",
            y - r,
            x + r,
            y + r * 0.9,
            x - r
        );
        (
            format!(
                r##"<path d="{triangle}" fill="#000" stroke="#000" stroke-width="{:.2}" stroke-linejoin="round"{grow}/>"##,
                2.4 * s
            ),
            format!(
                r##"<path d="{triangle}" fill="{colour}" stroke="{colour}" stroke-width="{s:.2}" stroke-linejoin="round"{grow}/>"##
            ),
            if size > 0.8 {
                mark(y - r * 0.35, r * 0.6, y + r * 0.45)
            } else {
                String::new()
            },
        )
    } else {
        (
            format!(
                r##"<circle cx="{x:.2}" cy="{y:.2}" r="{:.2}" fill="#000"/>"##,
                (r + 1.2 * s) * size
            ),
            format!(
                r##"<circle cx="{x:.2}" cy="{y:.2}" r="{:.2}" fill="{colour}"/>"##,
                r * size
            ),
            if size > 0.8 {
                mark(y - r * 0.62, r * 0.75, y + r * 0.3)
            } else {
                String::new()
            },
        )
    }
}

/// The parts of the bird as SVG at `n` px: what goes in `<defs>`, the ink
/// (masked for the holes and the gaps), and the coloured marks over it.
pub(super) struct Parts {
    pub defs: String,
    pub ink: String,
    pub over: String,
}

impl Art {
    /// The bird at `n` px, white, for a dark bar.
    pub fn svg_at(&self, n: u32) -> String {
        let Parts { defs, ink, over } = self.parts(n, "#ffffff");
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{n}" height="{n}" viewBox="0 0 {n} {n}"><defs>{defs}</defs>{ink}{over}</svg>"##
        )
    }

    /// The bird at the drawing's own size.
    pub fn svg(&self) -> String {
        self.svg_at(SQUARE as u32)
    }

    // literal: the bird's drawing (design/proposals/bird-calling.html)
    pub(super) fn parts(&self, n: u32, ink: &str) -> Parts {
        let s = n as f32 / SQUARE;
        let tilt = RAISED + (DROPPED - RAISED) * self.sleep;
        let head: [P; POINTS] = if self.dashes.is_some() {
            SHUT
        } else if self.sleep > 0. {
            std::array::from_fn(|i| lerp(AT_REST[i], SHUT[i], self.sleep))
        } else {
            std::array::from_fn(|i| lerp(AT_REST[i], CALLING[i], self.bill))
        };
        let path = outline(&head, tilt, s);
        // The eye: a square hole on this size's whole pixels where the raised
        // head puts it; closing, it flattens to a slit; asleep, the slit lies
        // along the dropped head.
        let (ex, ey) = rot(EYE, tilt);
        let (ex, ey) = (ex * s, ey * s);
        let side = (EYE_SIDE * s).round().max(2.);
        let slit = (SLIT.1 * s).round().max(1.);
        let eye = if self.sleep >= 0.98 {
            let w = (SLIT.0 * s).round();
            format!(
                r##"<rect x="{:.0}" y="{:.0}" width="{w:.0}" height="{slit:.0}" fill="#000" transform="rotate({tilt:.1} {ex:.2} {ey:.2})"/>"##,
                (ex - w / 2.).round(),
                (ey - slit / 2.).round()
            )
        } else {
            let h = slit + (side - slit) * self.eye;
            format!(
                r##"<rect x="{:.0}" y="{:.0}" width="{side:.0}" height="{h:.2}" fill="#000"/>"##,
                (ex - side / 2.).round(),
                (ey - side / 2.).round()
            )
        };
        let wing = format!(
            r##"<path d="M{}L{}L{}L{}Z" fill="#000"/>"##,
            at(WING[0], s),
            at(WING[1], s),
            at(WING[2], s),
            at(WING[3], s)
        );
        // The gaps cut for the coloured marks, and the marks.
        let (mut cut, mut over) = (String::new(), String::new());
        if let Some((a, b, opacity)) = self.calls {
            let (c, o) = marks(s, a, b, opacity, "#2DD4BF");
            cut += &c;
            over += &o;
        }
        if let Some(size) = self.badge {
            let (c, shape, mark) = if self.warning {
                badge(s, size, true, "#F5B740", "#1a1408")
            } else {
                badge(s, size, false, "#F97066", "#0b1514")
            };
            cut += &c;
            over += &shape;
            over += &mark;
        }
        let m = n + 16;
        let defs = format!(
            r##"<mask id="m" maskUnits="userSpaceOnUse" x="-8" y="-8" width="{m}" height="{m}"><rect x="-8" y="-8" width="{m}" height="{m}" fill="#fff"/>{eye}{wing}{cut}</mask><mask id="c" maskUnits="userSpaceOnUse" x="-8" y="-8" width="{m}" height="{m}"><rect x="-8" y="-8" width="{m}" height="{m}" fill="#fff"/>{cut}</mask>"##
        );
        let dim = ((self.sleep - 0.5) * 2.).clamp(0., 1.);
        let opacity = self.fill * (1. - (1. - ASLEEP) * dim);
        let mut art = String::new();
        if opacity > 0.005 {
            art += &format!(
                r##"<g mask="url(#m)" fill="{ink}" opacity="{opacity:.2}"><path d="{path}"/></g>"##
            );
        }
        if let Some(p) = self.dashes {
            art += &format!(
                r##"<g mask="url(#c)"><path d="{path}" fill="none" stroke="{ink}" stroke-width="{:.2}" stroke-linejoin="round" stroke-dasharray="{:.2} {:.2}" stroke-dashoffset="{:.2}"/></g>"##,
                STROKE * s,
                DASH.0 * s,
                DASH.1 * s,
                -(DASH.0 + DASH.1) * s * p
            );
        }
        Parts {
            defs,
            ink: art,
            over,
        }
    }

    /// The images for the tray: the drawing at `SIZE`, and at each of
    /// `SMALL`, each drawn for its own pixels.
    pub fn icon(&self) -> Icon {
        Icon {
            width: SIZE,
            height: SIZE,
            data: raster(&self.svg_at(SIZE), SIZE),
            small: SMALL
                .iter()
                .map(|&n| (n, raster(&self.svg_at(n), n)))
                .collect(),
        }
    }
}
