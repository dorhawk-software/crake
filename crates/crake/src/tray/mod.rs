//! The top-bar icon. States change shape, never colour (design/DESIGN.md):
//! off, connecting, listening, routed (a few seconds), and a badge on the
//! bird as it is (asleep, dashed or awake): trouble's red one, or an amber
//! triangle when something needs the user (red wins). A click opens
//! the panel; the menu is the plain fallback: tunnels with check marks, Open
//! Crake, Quit. What it shows and how it's drawn lives here (`art`: the
//! bird, `motion`: its frames); each system's tray shows it (`sys::tray`).

use std::sync::LazyLock;

pub use crate::sys::tray::{Handle, start};

pub(crate) mod art;
mod motion;

pub use art::Art;
pub use motion::{connecting_loop, transition};

#[cfg(test)]
mod family;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Off,
    Connecting,
    Listening,
    Routed,
    /// The badge on the bird awake: a tunnel stopped, another is up.
    Trouble,
    /// The badge on the bird asleep: a tunnel stopped, nothing is up.
    OffTrouble,
    /// The badge on the dashed bird: a tunnel stopped, another connects.
    ConnectingTrouble,
    /// The amber triangle on the bird awake: something needs you (an
    /// OpenVPN 3 process left running), a tunnel is up.
    Warning,
    /// The amber triangle on the bird asleep.
    OffWarning,
    /// The amber triangle on the dashed bird.
    ConnectingWarning,
}

/// Every shape, in `Shape::index` order.
const SHAPES: [Shape; 10] = [
    Shape::Off,
    Shape::Connecting,
    Shape::Listening,
    Shape::Routed,
    Shape::Trouble,
    Shape::OffTrouble,
    Shape::ConnectingTrouble,
    Shape::Warning,
    Shape::OffWarning,
    Shape::ConnectingWarning,
];

/// A badge on the bird: red, something stopped; amber, it needs you.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    Trouble,
    Warning,
}

impl Shape {
    fn index(self) -> usize {
        self as usize
    }

    /// The shape without its badge, and the badge.
    pub fn badged(self) -> (Shape, Option<Badge>) {
        match self {
            Shape::Trouble => (Shape::Listening, Some(Badge::Trouble)),
            Shape::OffTrouble => (Shape::Off, Some(Badge::Trouble)),
            Shape::ConnectingTrouble => (Shape::Connecting, Some(Badge::Trouble)),
            Shape::Warning => (Shape::Listening, Some(Badge::Warning)),
            Shape::OffWarning => (Shape::Off, Some(Badge::Warning)),
            Shape::ConnectingWarning => (Shape::Connecting, Some(Badge::Warning)),
            other => (other, None),
        }
    }

    /// This shape with `badge` on the bird as it is (a routed bird shows
    /// the badge instead of its marks).
    pub fn with(self, badge: Badge) -> Shape {
        match (badge, self.badged().0) {
            (Badge::Trouble, Shape::Off) => Shape::OffTrouble,
            (Badge::Trouble, Shape::Connecting) => Shape::ConnectingTrouble,
            (Badge::Trouble, _) => Shape::Trouble,
            (Badge::Warning, Shape::Off) => Shape::OffWarning,
            (Badge::Warning, Shape::Connecting) => Shape::ConnectingWarning,
            (Badge::Warning, _) => Shape::Warning,
        }
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
/// order), drawn white for a dark bar; and the same drawn at each size a
/// tray shows icons at (`SMALL`), so no tray has to shrink it (a shrunk
/// 64 px bird looked choppy on Windows and soft on GNOME).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    /// (size, ARGB) at each of `SMALL`.
    pub small: Vec<(u32, Vec<u8>)>,
    /// The Mac's menu bar's own drawings, (size, ARGB) at 1x and 2x
    /// (`art::MAC_POINTS`, `art::MAC_SIZE`); none elsewhere.
    pub mac: Vec<(u32, Vec<u8>)>,
}

/// The largest image sent to the tray.
const SIZE: u32 = 64;
/// The sizes trays show icons at: 16 to 24 px (1x and 1.25x and 1.5x
/// notification areas, GNOME's and KDE's bars), 32 to 48 (2x and more);
/// the Mac's menu bar has a drawing of its own (`art::MAC_SIZE`).
const SMALL: [u32; 9] = [16, 20, 22, 24, 28, 32, 36, 40, 48];

static ICONS: LazyLock<[Icon; 10]> = LazyLock::new(|| {
    // The bird alone, solid with the eye cut out: it holds its own next to
    // app icons in a small bar (DESIGN.md, Tray icon). CRAKE_TRAY=bird: the
    // whole bird in the grass, as in design/icons/crake-*-symbolic.svg.
    if std::env::var("CRAKE_TRAY").as_deref() != Ok("bird") {
        return SHAPES.map(|shape| rest(shape).icon());
    }
    // The whole bird has one trouble drawing, the bird with "!", and no
    // warning drawing (the plain bird; CRAKE_TRAY=bird is a fallback).
    let trouble = draw(include_str!(
        "../../../../assets/icons/crake-trouble-symbolic.svg"
    ));
    let off = draw(include_str!(
        "../../../../assets/icons/crake-off-symbolic.svg"
    ));
    let connecting = draw(include_str!(
        "../../../../assets/icons/crake-connecting-symbolic.svg"
    ));
    let listening = draw(include_str!(
        "../../../../assets/icons/crake-listening-symbolic.svg"
    ));
    [
        off.clone(),
        connecting.clone(),
        listening.clone(),
        draw(include_str!(
            "../../../../assets/icons/crake-routed-symbolic.svg"
        )),
        trouble.clone(),
        trouble.clone(),
        trouble,
        listening,
        off,
        connecting,
    ]
});

/// A symbolic SVG drawn white, for the top bar (dark on Ubuntu in both light
/// and dark).
fn draw(svg: &str) -> Icon {
    // White, as the top bar is dark. The grass bolder and the drawing
    // cropped for the bar's small size (DESIGN.md: icons at 12–16 px get
    // thicker strokes): GNOME scales the whole image into the icon slot, so
    // empty margin would only make the bird smaller.
    let svg = svg
        .replace("#2e3436", "#ffffff")
        .replace("stroke-width=\"1.9\"", "stroke-width=\"2.3\"")
        .replace("viewBox=\"0 0 24 24\"", "viewBox=\"2.2 2.6 21.6 19.2\"");
    render(&svg)
}

/// One SVG as the tray's ARGB images: scaled to `SIZE` px, and to each of
/// `SMALL`.
fn render(svg: &str) -> Icon {
    let tree = tree(svg);
    Icon {
        width: SIZE,
        height: SIZE,
        data: argb(&tree, SIZE),
        small: SMALL.iter().map(|s| (*s, argb(&tree, *s))).collect(),
        mac: Vec::new(),
    }
}

fn tree(svg: &str) -> resvg::usvg::Tree {
    resvg::usvg::Tree::from_data(svg.as_bytes(), &resvg::usvg::Options::default())
        .expect("tray icons are valid SVG")
}

/// An SVG drawn at `size` px square, as the ARGB the tray wants.
fn raster(svg: &str, size: u32) -> Vec<u8> {
    argb(&tree(svg), size)
}

/// The drawing at `size` px square, as the ARGB StatusNotifierItem wants
/// (from tiny-skia's premultiplied RGBA).
fn argb(tree: &resvg::usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("non-zero size");
    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // Four bytes a pixel: alpha, red, green, blue.
    const ARGB: u32 = 4;
    let mut data = Vec::with_capacity((size * size * ARGB) as usize);
    for p in pixmap.pixels() {
        let c = p.demultiply();
        data.extend_from_slice(&[c.alpha(), c.red(), c.green(), c.blue()]);
    }
    data
}

/// The marks' colours on a light bar (Windows' light taskbar, macOS'
/// light menu bar), in place of the dark bar's: DESIGN.md's light danger
/// and warning, and what goes on them (the dark ones are 2.4:1 and 1.5:1
/// against a light bar; design/proposals/tray-badge.html), and the light
/// theme's accent for the call (the dark one was 1.6:1 there, found
/// 2026-10-25 by the Mac's proposal).
const ON_LIGHT_BAR: [(&str, &str); 5] = [
    (art::CALL, "#0F766E"),
    (art::TROUBLE, "#B42318"),
    (art::ON_TROUBLE, "#FFFFFF"),
    (art::WARNING, "#BF7E12"),
    (art::ON_WARNING, "#1A1C1E"),
];

/// An SVG of the bird for a light bar: `ink` for the bird, the badges in
/// their light-bar colours.
pub fn for_light_bar(svg: &str, ink: &str) -> String {
    ON_LIGHT_BAR
        .iter()
        .fold(svg.replace("#ffffff", ink), |svg, (dark, light)| {
            svg.replace(dark, light)
        })
}

/// How far a pixel's colour may be from a mark's and still be taken for
/// it: an anti-aliased edge comes back from its alpha a few steps off
/// (most of a 2 px call mark is edge; with exact matches only, most of it
/// stayed the dark bar's teal, found 2026-10-25).
#[cfg(any(windows, target_os = "macos", test))]
const NEAR: i16 = 24;

/// A pixel of the tray's image for a light bar: the grey bird drawn
/// `ink`, a mark's colour (or one near it, an edge's) its light-bar one.
#[cfg(any(windows, target_os = "macos", test))]
// literal: "#rrggbb" read as its three bytes
pub fn light_bar_pixel([r, g, b]: [u8; 3], ink: [u8; 3]) -> [u8; 3] {
    if r == g && g == b {
        return ink;
    }
    let hex = |h: &str| {
        let v = u32::from_str_radix(&h[1..], 16).unwrap_or(0);
        v.to_be_bytes()
    };
    let near = |a: u8, b: u8| (i16::from(a) - i16::from(b)).abs() <= NEAR;
    for (dark, light) in ON_LIGHT_BAR {
        let [_, dr, dg, db] = hex(dark);
        if near(dr, r) && near(dg, g) && near(db, b) {
            let [_, lr, lg, lb] = hex(light);
            return [lr, lg, lb];
        }
    }
    [r, g, b]
}

/// The resting look of `shape` as an SVG at the drawing's own size, the
/// bird white (Settings shows each look in the bird row's "?").
pub fn resting_svg(shape: Shape) -> String {
    rest(shape).svg()
}

/// The resting look of each state, as `Art`.
fn rest(shape: Shape) -> Art {
    let (shape, badge) = shape.badged();
    let art = rest_unbadged(shape);
    match badge {
        Some(b) => Art {
            badge: Some(1.),
            warning: b == Badge::Warning,
            ..art
        },
        None => art,
    }
}

fn rest_unbadged(shape: Shape) -> Art {
    let awake = Art {
        fill: 1.,
        eye: 1.,
        ..Art::default()
    };
    match shape {
        Shape::Off => Art {
            eye: 0.,
            sleep: 1.,
            ..awake
        },
        Shape::Connecting => Art {
            dashes: Some(0.),
            fill: 0.,
            ..Art::default()
        },
        Shape::Listening => awake,
        Shape::Routed => Art {
            bill: 1.,
            calls: Some((1., 1., 1.)),
            ..awake
        },
        Shape::Trouble
        | Shape::OffTrouble
        | Shape::ConnectingTrouble
        | Shape::Warning
        | Shape::OffWarning
        | Shape::ConnectingWarning => rest_unbadged(shape.badged().0),
    }
}
