//! The top-bar icon. States change shape, never colour (design/DESIGN.md):
//! off, connecting, listening, routed (a few seconds), trouble. A click opens
//! the panel; the menu is the plain fallback: tunnels with check marks, Open
//! Crake, Quit. What it shows and how it's drawn lives here; each system's
//! tray shows it (`sys::tray`).

use std::sync::LazyLock;

pub use crate::sys::tray::{Handle, start};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Off,
    Connecting,
    Listening,
    Routed,
    Trouble,
}

impl Shape {
    fn index(self) -> usize {
        self as usize
    }
}

/// One tunnel in the fallback menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuTunnel {
    pub name: String,
    pub display: String,
    pub on: bool,
}

/// What the tray shows.
#[derive(Clone, Debug)]
pub struct State {
    pub shape: Shape,
    /// A frame of an animation, shown instead of the shape's icon.
    pub frame: Option<Icon>,
    pub summary: String,
    pub tunnels: Vec<MenuTunnel>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            shape: Shape::Off,
            frame: None,
            summary: "Crake".into(),
            tunnels: Vec::new(),
        }
    }
}

impl State {
    /// The image to show now: the animation's frame, or the shape's.
    pub fn icon(&self) -> Icon {
        self.frame
            .clone()
            .unwrap_or_else(|| ICONS[self.shape.index()].clone())
    }
}

/// An image for the tray: `SIZE` px square, ARGB (StatusNotifierItem's
/// order), drawn white for a dark bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The image size sent to the tray; the desktop scales it into its slot.
const SIZE: u32 = 64;

static ICONS: LazyLock<[Icon; 5]> = LazyLock::new(|| {
    // The head alone, solid with the eye cut out: it holds its own next to app
    // icons in a small bar (DESIGN.md, Tray icon). CRAKE_TRAY=bird: the whole
    // bird, as in design/icons/crake-*-symbolic.svg.
    if std::env::var("CRAKE_TRAY").as_deref() != Ok("bird") {
        return [
            draw_as_is(include_str!("../../../assets/icons/tray-head-off.svg")),
            draw_as_is(include_str!(
                "../../../assets/icons/tray-head-connecting.svg"
            )),
            draw_as_is(include_str!(
                "../../../assets/icons/tray-head-listening.svg"
            )),
            draw_as_is(include_str!("../../../assets/icons/tray-head-routed.svg")),
            draw_as_is(include_str!("../../../assets/icons/tray-head-trouble.svg")),
        ];
    }
    [
        draw(include_str!("../../../assets/icons/crake-off-symbolic.svg")),
        draw(include_str!(
            "../../../assets/icons/crake-connecting-symbolic.svg"
        )),
        draw(include_str!(
            "../../../assets/icons/crake-listening-symbolic.svg"
        )),
        draw(include_str!(
            "../../../assets/icons/crake-routed-symbolic.svg"
        )),
        draw(include_str!(
            "../../../assets/icons/crake-trouble-symbolic.svg"
        )),
    ]
});

/// An icon already cropped and weighted for the bar: only made white.
fn draw_as_is(svg: &str) -> Icon {
    render(&svg.replace("#2e3436", "#ffffff"))
}

/// A symbolic SVG drawn white, for the top bar (dark on Ubuntu in both light
/// and dark).
fn draw(svg: &str) -> Icon {
    // White, as the top bar is dark. Bolder and cropped to the drawing for
    // the bar's small size (DESIGN.md: icons at 12–16 px get thicker
    // strokes): GNOME scales the whole image into the icon slot, so empty
    // margin would only make the bird smaller.
    let svg = svg
        .replace("#2e3436", "#ffffff")
        .replace("stroke-width=\"1.9\"", "stroke-width=\"2.3\"")
        .replace("viewBox=\"0 0 24 24\"", "viewBox=\"2.2 2.6 21.6 19.2\"");
    render(&svg)
}

/// An SVG as the tray's 64 px ARGB image.
fn render(svg: &str) -> Icon {
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &resvg::usvg::Options::default())
        .expect("tray icons are valid SVG");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE).expect("non-zero size");
    let scale = SIZE as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // Premultiplied RGBA to the ARGB StatusNotifierItem wants.
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for p in pixmap.pixels() {
        let c = p.demultiply();
        data.extend_from_slice(&[c.alpha(), c.red(), c.green(), c.blue()]);
    }
    Icon {
        width: SIZE,
        height: SIZE,
        data,
    }
}

/// One frame of the head in motion (DESIGN.md, Tray icon; animations.html).
/// The same geometry as `assets/icons/tray-head-*.svg`, at any point between
/// the states.
#[derive(Clone, Copy, Debug, Default)]
pub struct Art {
    /// The outline drawn so far (0–1), or none.
    pub outline: Option<f32>,
    /// How filled the head is (0–1).
    pub fill: f32,
    /// How open the eye is: 0 closed (a slit), 1 open.
    pub eye: f32,
    /// The two call marks: how far each is drawn (0–1), and their opacity.
    pub calls: Option<(f32, f32, f32)>,
    /// The trouble badge's size (0–1), or none.
    pub badge: Option<f32>,
    /// Asleep (0–1): the head tips down and shrinks a little, so off reads
    /// at a glance (design/proposals/tray-off-vs-on.html, D).
    pub tuck: f32,
}

/// How far an asleep head tips, and how much smaller it gets to stay inside.
const TUCK_DEGREES: f32 = 25.;
const TUCK_SCALE: f32 = 0.82;
/// How bright an asleep head is.
const ASLEEP: f32 = 0.45;

/// The head's transform at `tuck`: about (14, 13), then re-centred in the box.
fn tuck_transform(t: f32) -> String {
    if t <= 0. {
        return String::new();
    }
    let (x, y) = (14. + 1.56 * t, 13. + 0.86 * t);
    let scale = 1. - (1. - TUCK_SCALE) * t;
    format!(
        r#" transform="translate({x:.3} {y:.3}) scale({scale:.3}) rotate({:.2}) translate(-14 -13)""#,
        TUCK_DEGREES * t
    )
}

const SIL: &str = "M8.5 20.5V12A3.75 3.75 0 0 1 15.95 11.4L16 10.4L20.6 11.6L16 12.8C16 15.5 15.1 17.8 13.4 20.5Z";
/// The outline's length, for drawing it progressively.
const SIL_LENGTH: f32 = 44.;
const MARK_1: &str = "M17.95 9.65l1.6-1.8";
const MARK_2: &str = "M19.2 11.25l1.9-.8";
const MARK_LENGTH: f32 = 2.45;

impl Art {
    pub fn svg(&self) -> String {
        let mut holes = String::new();
        if self.eye > 0.02 {
            holes += &format!(
                r##"<circle cx="12.35" cy="11.4" r="{:.3}" fill="#000"/>"##,
                1.55 * self.eye
            );
        }
        if self.eye < 0.98 {
            let w = 3.9 * (1. - self.eye) + 0.1;
            holes += &format!(
                r##"<rect x="{:.3}" y="10.7" width="{w:.3}" height="1.4" rx="0.7" fill="#000"/>"##,
                12.35 - w / 2.
            );
        }
        let mut over = String::new();
        if let Some((a, b, opacity)) = self.calls {
            holes += &format!(
                r##"<path d="{MARK_1}{MARK_2}" fill="none" stroke="#000" stroke-width="3" stroke-linecap="round" opacity="{opacity:.2}"/>"##
            );
            for (d, drawn) in [(MARK_1, a), (MARK_2, b)] {
                if drawn > 0.02 {
                    over += &format!(
                        r##"<path d="{d}" fill="none" stroke="#2DD4BF" stroke-width="1.7" stroke-linecap="round" stroke-dasharray="{MARK_LENGTH}" stroke-dashoffset="{:.3}" opacity="{opacity:.2}"/>"##,
                        MARK_LENGTH * (1. - drawn)
                    );
                }
            }
        }
        if let Some(size) = self.badge {
            holes += &format!(
                r##"<circle cx="19.2" cy="9.75" r="{:.3}" fill="#000"/>"##,
                3.45 * size
            );
            over += &format!(
                r##"<circle cx="19.2" cy="9.75" r="{:.3}" fill="#F97066"/>"##,
                2.75 * size
            );
            if size > 0.8 {
                over += r##"<path d="M19.2 8.25v1.7" fill="none" stroke="#0b1514" stroke-width="1.15" stroke-linecap="round"/><circle cx="19.2" cy="11.35" r="0.6" fill="#0b1514"/>"##;
            }
        }
        let outline = self.outline.map_or(String::new(), |p| {
            format!(
                r##"<path d="{SIL}" fill="none" stroke="#ffffff" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" stroke-dasharray="{SIL_LENGTH}" stroke-dashoffset="{:.2}"/>"##,
                SIL_LENGTH * (1. - p)
            )
        });
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="7.1 6.6 15.2 15.2"><defs><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="30" height="30"><g{}><path d="{SIL}" fill="#fff" stroke="#fff" stroke-width="2.2" stroke-linejoin="round"/>{holes}</g></mask></defs><rect x="0" y="0" width="30" height="30" fill="#ffffff" opacity="{:.2}" mask="url(#m)"/>{outline}{over}</svg>"##,
            tuck_transform(self.tuck),
            self.fill
        )
    }

    pub fn icon(&self) -> Icon {
        render(&self.svg())
    }
}

/// The resting look of each state, as `Art`.
fn rest(shape: Shape) -> Art {
    let head = Art {
        fill: 1.,
        eye: 1.,
        ..Art::default()
    };
    match shape {
        Shape::Off => Art {
            eye: 0.,
            fill: ASLEEP,
            tuck: 1.,
            ..head
        },
        Shape::Connecting => Art {
            outline: Some(1.),
            fill: 0.,
            ..Art::default()
        },
        Shape::Listening => head,
        Shape::Routed => Art {
            calls: Some((1., 1., 1.)),
            ..head
        },
        Shape::Trouble => Art {
            badge: Some(1.),
            ..head
        },
    }
}

/// The frames from one state to the next (every ~70 ms), ending at the
/// resting look of `to`. Connecting has its own loop (`connecting_loop`).
pub fn transition(from: Shape, to: Shape) -> Vec<Art> {
    let head = rest(Shape::Listening);
    let steps = |n: usize| (1..=n).map(move |i| crate::motion::ease_out(i as f32 / n as f32));
    match (from, to) {
        // The outline completes, the head fills in, then the eye opens.
        (Shape::Connecting, Shape::Listening | Shape::Routed) => {
            let mut v: Vec<Art> = steps(3)
                .map(|f| Art {
                    outline: Some(1.),
                    fill: f,
                    eye: 0.,
                    ..Art::default()
                })
                .collect();
            v.extend(steps(3).map(|e| Art { eye: e, ..head }));
            v
        }
        // crex, crex: the marks draw out one after the other, twice.
        (_, Shape::Routed) => {
            let mut v = Vec::new();
            for _ in 0..2 {
                v.extend(steps(2).map(|a| Art {
                    calls: Some((a, 0., 1.)),
                    ..head
                }));
                v.extend(steps(2).map(|b| Art {
                    calls: Some((1., b, 1.)),
                    ..head
                }));
                v.push(Art {
                    calls: Some((1., 1., 1.)),
                    ..head
                });
                v.push(Art {
                    calls: Some((1., 1., 0.25)),
                    ..head
                });
            }
            v.push(Art {
                calls: Some((1., 1., 1.)),
                ..head
            });
            v
        }
        // The marks fade.
        (Shape::Routed, Shape::Listening) => [0.7, 0.4, 0.15]
            .iter()
            .map(|o| Art {
                calls: Some((1., 1., *o)),
                ..head
            })
            .collect(),
        // The badge appears once.
        (_, Shape::Trouble) => steps(4)
            .map(|s| Art {
                badge: Some(s),
                ..head
            })
            .collect(),
        // The eye closes, then the head tips down and dims: asleep.
        (_, Shape::Off) => {
            let mut v: Vec<Art> = steps(3)
                .map(|c| Art {
                    eye: 1. - c,
                    ..head
                })
                .collect();
            v.extend(steps(3).map(|t| Art {
                eye: 0.,
                tuck: t,
                fill: 1. - (1. - ASLEEP) * t,
                ..head
            }));
            v
        }
        // It wakes: the head comes up and brightens, then the eye opens.
        (Shape::Off, Shape::Listening) => {
            let mut v: Vec<Art> = steps(3)
                .map(|t| Art {
                    eye: 0.,
                    tuck: 1. - t,
                    fill: ASLEEP + (1. - ASLEEP) * t,
                    ..head
                })
                .collect();
            v.extend(steps(3).map(|e| Art { eye: e, ..head }));
            v
        }
        _ => Vec::new(),
    }
}

/// Connecting: the outline draws itself, holds a moment, and starts again.
pub fn connecting_loop() -> Vec<Art> {
    let mut v: Vec<Art> = (1..=12)
        .map(|i| Art {
            outline: Some(crate::motion::ease_out(i as f32 / 12.)),
            ..Art::default()
        })
        .collect();
    v.extend(std::iter::repeat_n(
        Art {
            outline: Some(1.),
            ..Art::default()
        },
        5,
    ));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eye_is_a_hole_the_bar_shows_through() {
        // viewBox 7.1 6.6 15.2 15.2 at 64 px: 4.21 px a unit.
        let at = |icon: &Icon, x: f32, y: f32| {
            let (px, py) = (
                ((x - 7.1) * 64. / 15.2) as usize,
                ((y - 6.6) * 64. / 15.2) as usize,
            );
            icon.data[(py * 64 + px) * 4]
        };
        let listening = &ICONS[Shape::Listening.index()];
        assert_eq!(at(listening, 12.35, 11.4), 0, "the eye is see-through");
        assert_eq!(at(listening, 11., 17.), 255, "the head is solid");
        // Routed: teal marks over the corner; trouble: a red badge there.
        let px = |icon: &Icon, x: f32, y: f32| {
            let (px, py) = (
                ((x - 7.1) * 64. / 15.2) as usize,
                ((y - 6.6) * 64. / 15.2) as usize,
            );
            icon.data[(py * 64 + px) * 4..(py * 64 + px) * 4 + 4].to_vec()
        };
        assert_eq!(
            px(&ICONS[Shape::Routed.index()], 19.2, 7.7)[1..],
            [0x2D, 0xD4, 0xBF]
        );
        assert_eq!(
            px(&ICONS[Shape::Trouble.index()], 17.6, 9.75)[1..],
            [0xF9, 0x70, 0x66]
        );
    }

    #[test]
    fn motion_ends_where_the_state_rests() {
        // The last frame of each transition looks like the resting icon.
        let close = |a: &Icon, b: &Icon| {
            a.data
                .iter()
                .zip(&b.data)
                .filter(|(x, y)| x.abs_diff(**y) > 40)
                .count()
                < 64
        };
        for (from, to) in [
            (Shape::Connecting, Shape::Listening),
            (Shape::Listening, Shape::Routed),
            (Shape::Listening, Shape::Trouble),
            (Shape::Listening, Shape::Off),
            (Shape::Off, Shape::Listening),
        ] {
            let last = transition(from, to).last().unwrap().icon();
            assert!(close(&last, &ICONS[to.index()]), "{from:?} -> {to:?}");
        }
        assert!(!connecting_loop().is_empty());
    }

    #[test]
    fn the_coloured_marks_fit_inside_the_icon() {
        // A mark that reaches the edge is cut flat there.
        for shape in [Shape::Routed, Shape::Trouble] {
            let icon = &ICONS[shape.index()];
            // Coloured: drawn, and not the white head (which may touch the
            // bottom: the crop is tight).
            let coloured = |x: usize, y: usize| {
                let p = &icon.data[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
                p[0] > 0 && !(p[1] > 230 && p[2] > 230 && p[3] > 230)
            };
            let edge = (0..64)
                .any(|i| coloured(i, 0) || coloured(63, i) || coloured(i, 63) || coloured(0, i));
            let at: Vec<(usize, usize)> = (0..64)
                .flat_map(|i| [(i, 0), (63, i), (i, 63), (0, i)])
                .filter(|(x, y)| coloured(*x, *y))
                .collect();
            assert!(!edge, "{shape:?} touches the icon's edge at {at:?}");
        }
    }

    #[test]
    fn asleep_is_plain_at_a_glance() {
        // Off is far dimmer than listening, not just a different eye.
        let ink = |i: &Icon| i.data.chunks(4).map(|p| u64::from(p[0])).sum::<u64>();
        let (off, on) = (
            ink(&ICONS[Shape::Off.index()]),
            ink(&ICONS[Shape::Listening.index()]),
        );
        assert!(off * 2 < on, "off {off}, listening {on}");
        // And all of the tipped head stays inside the icon.
        let icon = &rest(Shape::Off).icon();
        let edge = |x: usize, y: usize| icon.data[(y * 64 + x) * 4];
        let (mut x0, mut y0, mut x1, mut y1) = (64, 64, 0, 0);
        for y in 0..64 {
            for x in 0..64 {
                if edge(x, y) > 0 {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        assert!(
            x0 > 0 && y0 > 0 && x1 < 63 && y1 < 63,
            "drawn within {x0},{y0} to {x1},{y1}"
        );
    }

    #[test]
    fn every_state_has_its_own_shape() {
        let icons = &*ICONS;
        for (i, a) in icons.iter().enumerate() {
            assert_eq!(a.data.len(), 64 * 64 * 4);
            assert!(
                a.data.chunks(4).any(|p| p[0] > 0),
                "icon {i} draws something"
            );
            for b in &icons[i + 1..] {
                assert_ne!(a.data, b.data, "shapes differ");
            }
        }
    }
}
