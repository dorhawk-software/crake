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
// literal: the bird's drawing (design/proposals/tray-badge.html, B)
/// The badge at the bottom right, over the breast, on each size's whole
/// pixels: 7 px across at 16 px.
pub(super) const BADGE: Place = Place {
    centre: (25., 25.),
    radius: 7.,
    snapped: true,
};
/// The badges' colours for a dark bar (DESIGN.md's dark danger and
/// warning, and what goes on them); `tray::for_light_bar` swaps them.
pub(super) const TROUBLE: &str = "#F97066";
pub(super) const ON_TROUBLE: &str = "#0b1514";
pub(super) const WARNING: &str = "#F5B740";
pub(super) const ON_WARNING: &str = "#1a1408";
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

/// Where a badge sits on the bird and how large it is: its centre and
/// radius in the drawing, and whether each size gets it on whole pixels
/// (its edge, its gap and its "!"), as the eye is.
#[derive(Clone, Copy, Debug)]
pub(super) struct Place {
    pub centre: P,
    pub radius: f32,
    pub snapped: bool,
}

/// The badge at scale `s`, grown to `size`: the gap cut, the badge in
/// `colour`, and its "!" in `on` (once it's nearly grown).
#[cfg(test)]
pub(super) fn badge(
    s: f32,
    size: f32,
    warning: bool,
    colour: &str,
    on: &str,
) -> (String, String, String) {
    badge_at(BADGE, s, size, warning, colour, on)
}

/// The "!": its stem's top, width and height, and its dot's top.
struct Bang {
    top: f32,
    width: f32,
    height: f32,
    dot: f32,
}

/// A badge's shape at scale `s`: its centre, radius and gap, and its "!".
// literal: the badge's drawing (design/proposals/bird-calling.html,
// tray-badge.html)
fn shape(place: Place, s: f32, warning: bool) -> ((f32, f32), f32, f32, Option<Bang>) {
    let ((x, y), r) = ((place.centre.0 * s, place.centre.1 * s), place.radius * s);
    if !place.snapped {
        let bar = 2. * s;
        let bang = if warning {
            Bang {
                top: y - r * 0.35,
                width: bar,
                height: r * 0.6,
                dot: y + r * 0.45,
            }
        } else {
            Bang {
                top: y - r * 0.62,
                width: bar,
                height: r * 0.75,
                dot: y + r * 0.3,
            }
        };
        return ((x, y), r, 1.2 * s, Some(bang));
    }
    // On whole pixels: an even width centred on a pixel boundary, an odd
    // one on a pixel's middle; the "!" a stem, a gap and a dot, each whole.
    let across = (2. * r).round().max(4.);
    let at = |c: f32| {
        if across % 2. == 0. {
            c.round()
        } else {
            c.floor() + 0.5
        }
    };
    let (x, y, r) = (at(x), at(y), across / 2.);
    let gap = (1.2 * s).round().max(1.);
    let bang = if warning {
        // The triangle's: low in it, where it's wide enough.
        let (width, space) = (whole(across / 8., across), (across / 10.).round().max(1.));
        let base = (y + r * 0.9).round();
        let height = (across * 0.3).round().max(2.);
        let dot = base - space - width;
        Some(Bang {
            top: dot - space - height,
            width,
            height,
            dot,
        })
    } else {
        bang(across, y)
    };
    ((x, y), r, gap, bang)
}

/// The nearest whole width to `w` that centres in `across` (odd in an odd
/// width, even in an even one).
fn whole(w: f32, across: f32) -> f32 {
    let odd = across % 2. == 1.;
    let w = if odd {
        ((w - 1.) / 2.).round() * 2. + 1.
    } else {
        (w / 2.).round() * 2.
    };
    w.max(if odd { 1. } else { 2. })
}

/// The circle's "!" on whole pixels, `across` wide about `y`: a stem a
/// sixth of the circle wide, a space and a dot as wide as the stem, with
/// a margin above and below; none where the circle has no room for one.
// literal: the badge's drawing (design/proposals/tray-badge.html)
fn bang(across: f32, y: f32) -> Option<Bang> {
    let width = whole(across / 6., across);
    let space = (across / 8.).round().max(1.);
    let margin = (across / 8.).round().max(1.);
    let room = across - 2. * margin;
    let height = (across * 0.375).round().min(room - space - width);
    // A stem half again as tall as it's wide, or it reads as a block.
    if height < 1.5 * width {
        return None;
    }
    let top = (y - (height + space + width) / 2.).round();
    Some(Bang {
        top,
        width,
        height,
        dot: top + height + space,
    })
}

/// The badge `place` puts on the bird, as `badge` draws it.
// literal: the badge's drawing (design/proposals/bird-calling.html)
pub(super) fn badge_at(
    place: Place,
    s: f32,
    size: f32,
    warning: bool,
    colour: &str,
    on: &str,
) -> (String, String, String) {
    let ((x, y), r, gap, bang) = shape(place, s, warning);
    let grow = format!(
        r#" transform="translate({x:.2} {y:.2}) scale({size:.3}) translate({:.2} {:.2})""#,
        -x, -y
    );
    let mark = if let Some(bang) = bang.filter(|_| size > 0.8) {
        let left = if place.snapped {
            (x - bang.width / 2.).round()
        } else {
            x - bang.width / 2.
        };
        format!(
            r##"<rect x="{left:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{on}"/><rect x="{left:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{on}"/>"##,
            bang.top, bang.width, bang.height, bang.dot, bang.width, bang.width
        )
    } else {
        String::new()
    };
    if warning {
        let triangle = format!(
            "M{x:.2} {:.2}L{:.2} {:.2}H{:.2}Z",
            y - r,
            x + r,
            if place.snapped {
                (y + r * 0.9).round()
            } else {
                y + r * 0.9
            },
            x - r
        );
        (
            format!(
                r##"<path d="{triangle}" fill="#000" stroke="#000" stroke-width="{:.2}" stroke-linejoin="round"{grow}/>"##,
                2. * gap
            ),
            format!(
                r##"<path d="{triangle}" fill="{colour}" stroke="{colour}" stroke-width="{s:.2}" stroke-linejoin="round"{grow}/>"##
            ),
            mark,
        )
    } else {
        (
            format!(
                r##"<circle cx="{x:.2}" cy="{y:.2}" r="{:.2}" fill="#000"/>"##,
                (r + gap) * size
            ),
            format!(
                r##"<circle cx="{x:.2}" cy="{y:.2}" r="{:.2}" fill="{colour}"/>"##,
                r * size
            ),
            mark,
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

    pub(super) fn parts(&self, n: u32, ink: &str) -> Parts {
        self.parts_at(n, ink, BADGE)
    }

    /// The parts with the badge at `place`.
    // literal: the bird's drawing (design/proposals/bird-calling.html)
    pub(super) fn parts_at(&self, n: u32, ink: &str, place: Place) -> Parts {
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
                badge_at(place, s, size, true, WARNING, ON_WARNING)
            } else {
                badge_at(place, s, size, false, TROUBLE, ON_TROUBLE)
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
