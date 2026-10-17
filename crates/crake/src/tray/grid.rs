//! The bird at 16 px, placed pixel by pixel: a 1× bar's size, where the
//! drawing scaled down goes grey at the edges. The bill is one closed
//! wedge, the throat two pixels deep, the eye a 2×2 hole clear of the
//! badge's gap, the base off the bottom edge
//! (design/proposals/bird-calling.html).

use std::collections::BTreeSet;

use super::{Badge, Shape};

/// A pixel: column, row.
type Px = (i32, i32);

/// The grid's side.
const SIDE: i32 = 16;
const ROWS: usize = 16;

// literal: the 16 px bird, pixel by pixel (design/proposals/bird-calling.html)
/// Awake: `#` ink, `o` the eye, `.` the bar.
const AWAKE: [&str; ROWS] = [
    "................",
    "...########.....",
    "..##########....",
    "..#####oo###....",
    "..#####oo######.",
    "..############..",
    "..###########...",
    "..###########...",
    "..##########....",
    "..#########.....",
    "..#########.....",
    "..##########....",
    "..###########...",
    ".############...",
    ".#############..",
    ".#############..",
];
// literal: the 16 px bird, pixel by pixel (design/proposals/bird-calling.html)
/// Asleep: sunk two rows, the bill tipped down, the eye a slit.
const ASLEEP: [&str; ROWS] = [
    "................",
    "................",
    "................",
    "...########.....",
    "..##########....",
    "..##########....",
    "..#####ooo##....",
    "..############..",
    "..#############.",
    "..###########...",
    "..##########....",
    "..#########.....",
    "..#########.....",
    ".###########....",
    ".############...",
    ".#############..",
];
// literal: the 16 px bird, pixel by pixel (design/proposals/bird-calling.html)
/// The two call marks, in front of the bill.
const MARKS: [Px; 4] = [(12, 1), (13, 0), (15, 1), (15, 2)];
/// The bill's tip: kept in the dashed outline, whose every other pixel
/// would drop it.
const BILL_TIP: Px = (14, 4);
// literal: the 16 px bird, pixel by pixel (design/proposals/bird-calling.html)
/// The badges behind the head: the red one a square (a disc with its
/// corners off read as a "+"), the amber one a triangle.
const TROUBLE: [Px; 16] = [
    (1, 1),
    (2, 1),
    (3, 1),
    (4, 1),
    (1, 2),
    (2, 2),
    (3, 2),
    (4, 2),
    (1, 3),
    (2, 3),
    (3, 3),
    (4, 3),
    (1, 4),
    (2, 4),
    (3, 4),
    (4, 4),
];
const WARNING: [Px; 12] = [
    (2, 1),
    (3, 1),
    (2, 2),
    (3, 2),
    (1, 3),
    (2, 3),
    (3, 3),
    (4, 3),
    (1, 4),
    (2, 4),
    (3, 4),
    (4, 4),
];
/// How bright an asleep bird is.
const ASLEEP_INK: f32 = 0.45;

fn pixels(rows: &[&str; ROWS], of: char) -> BTreeSet<Px> {
    let mut set = BTreeSet::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            if c == of {
                set.insert((x as i32, y as i32));
            }
        }
    }
    set
}

/// The pixels next to `of` (eight ways), not in it: the gap round a mark.
fn ring(of: &BTreeSet<Px>) -> BTreeSet<Px> {
    let mut r = BTreeSet::new();
    for &(x, y) in of {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let p = (x + dx, y + dy);
                if !of.contains(&p) {
                    r.insert(p);
                }
            }
        }
    }
    r
}

/// The pixels of `of` with a side on the outside.
fn edge(of: &BTreeSet<Px>) -> BTreeSet<Px> {
    of.iter()
        .copied()
        .filter(|&(x, y)| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| !of.contains(&(x + dx, y + dy)))
        })
        .collect()
}

fn rects(px: &BTreeSet<Px>, fill: &str, opacity: f32) -> String {
    let op = if opacity < 1. {
        format!(r#" opacity="{opacity}""#)
    } else {
        String::new()
    };
    px.iter()
        .map(|(x, y)| format!(r#"<rect x="{x}" y="{y}" width="1" height="1" fill="{fill}"{op}/>"#))
        .collect()
}

/// The resting look of `shape` at 16 px, white for a dark bar.
pub fn svg(shape: Shape) -> String {
    let (base, badge) = shape.badged();
    let rows = if base == Shape::Off { &ASLEEP } else { &AWAKE };
    let mut body = pixels(rows, '#');
    let mut over = String::new();
    if base == Shape::Routed {
        let marks: BTreeSet<Px> = MARKS.into_iter().collect();
        for p in ring(&marks) {
            body.remove(&p);
        }
        over += &rects(&marks, "#2DD4BF", 1.);
    }
    if let Some(badge) = badge {
        let (px, colour): (BTreeSet<Px>, &str) = match badge {
            Badge::Trouble => (TROUBLE.into_iter().collect(), "#F97066"),
            Badge::Warning => (WARNING.into_iter().collect(), "#F5B740"),
        };
        for p in ring(&px).union(&px) {
            body.remove(p);
        }
        over += &rects(&px, colour, 1.);
    }
    let art = match base {
        Shape::Connecting => {
            // The solid bird's own edge, every other pixel, the bill's tip kept.
            let solid: BTreeSet<Px> = body.union(&pixels(rows, 'o')).copied().collect();
            let mut dots: BTreeSet<Px> = edge(&solid)
                .into_iter()
                .filter(|(x, y)| (x + y) % 2 == 0)
                .collect();
            dots.insert(BILL_TIP);
            rects(&dots, "#ffffff", 1.)
        }
        Shape::Off => rects(&body, "#ffffff", ASLEEP_INK),
        _ => rects(&body, "#ffffff", 1.),
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{SIDE}" height="{SIDE}" viewBox="0 0 {SIDE} {SIDE}" shape-rendering="crispEdges">{art}{over}</svg>"##
    )
}
