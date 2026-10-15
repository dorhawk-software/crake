//! The graph (design/proposals/traffic-window.html): this computer on the
//! left, the ways out in the middle (each tunnel that's up, then direct),
//! the busiest destinations of each on the right, and packets ticking
//! along every path. A path's dots move as fast as its traffic flows (a
//! trickle crawls, a big download about twice as fast) and come one per
//! ~250 KB, between one every four seconds and two a second: calm enough
//! to keep open beside work (asked for: the first ones were too fast).
//! With animations off, the dots stand still and a path's thickness says
//! its rate instead.

use std::collections::HashMap;
use std::time::Instant;

use gpui_kit::{Bounds, PathBuilder, Pixels, Point, Rgba, Window, point, px, size};

use crate::model::Model;

/// Destinations shown per way; past that the last line is "and N more".
pub const HOSTS_PER_WAY: usize = 4;
/// A destination's line.
pub const ROW: f32 = 21.;
/// The least room a way takes, and the gap between ways.
const MIN_BLOCK: f32 = 64.;
const WAY_GAP: f32 = 26.;
/// Above the first way and below the last.
const MARGIN: f32 = 18.;
/// This computer's box, centred on ME_X.
pub const ME_X: f32 = 96.;
pub const ME_W: f32 = 132.;
pub const ME_H: f32 = 48.;
/// A way's box, centred on WAY_X.
pub const WAY_X: f32 = 400.;
pub const WAY_W: f32 = 116.;
pub const WAY_H: f32 = 44.;
/// Where destinations' names start, and the paths to them end (a gap
/// before the text).
pub const HOST_X: f32 = 640.;
const HOST_GAP: f32 = 8.;

/// Packets: speed in px/s is SPEED_BASE + SPEED_PER_DECADE × log10(1 +
/// KB/s); one dot per DOT_KB, at least DOTS_MIN and at most DOTS_MAX a
/// second.
const SPEED_BASE: f32 = 22.;
const SPEED_PER_DECADE: f32 = 16.;
const DOT_KB: f64 = 250.;
const DOTS_MIN: f64 = 0.25;
const DOTS_MAX: f64 = 2.;
const KB: f64 = 1000.;
pub const DOT_RADIUS: f32 = 2.6;
/// A path's line; still (animations off), up to this much thicker with
/// its rate.
const LINE: f32 = 1.5;
const STILL_EXTRA: f32 = 3.5;
/// A destination that just got a path through a tunnel has it drawn in,
/// once, over this long.
pub const DRAW_IN: f32 = 1.5;
/// A frame longer than this (the window was hidden) moves dots no further.
const LONGEST_FRAME: f32 = 0.1;
/// Steps a curve's length is measured in.
const STEPS: usize = 24;

/// A way out: a tunnel (by name) or direct (None).
#[derive(Clone, Debug, PartialEq)]
pub struct Way {
    pub tunnel: Option<String>,
    pub label: String,
    /// Bytes per second.
    pub rate: f64,
    pub hosts: Vec<Dest>,
}

/// A destination of a way, or "and N more" (`more`).
#[derive(Clone, Debug, PartialEq)]
pub struct Dest {
    pub label: String,
    pub rate: f64,
    pub more: bool,
}

/// The ways out right now: each tunnel that's up, in the order traffic
/// tries them, then direct; each with its busiest destinations.
pub fn ways(model: &Model) -> Vec<Way> {
    let rates = model.live.rates.clone().unwrap_or_default();
    // The order traffic tries them (Settings → Tunnels); full tunnels,
    // which aren't tried, after them.
    let tried: Vec<&crate::client::Tunnel> = model.try_order();
    let rest = model
        .tunnels
        .iter()
        .filter(|t| !tried.iter().any(|o| o.name == t.name));
    let order: Vec<&crate::client::Tunnel> = tried.iter().copied().chain(rest).collect();
    let mut ways: Vec<Way> = order
        .into_iter()
        .filter(|t| t.state == "connected")
        .map(|t| Way {
            tunnel: Some(t.name.clone()),
            label: t.display.clone(),
            rate: rates.tunnels.get(&t.name).copied().unwrap_or(0.),
            hosts: Vec::new(),
        })
        .collect();
    ways.push(Way {
        tunnel: None,
        label: "Direct".into(),
        rate: rates.direct,
        hosts: Vec::new(),
    });
    for way in &mut ways {
        let mut by_dest: HashMap<String, (f64, f64)> = HashMap::new();
        for (c, rate) in &rates.connections {
            if c.tunnel == way.tunnel {
                let label = c.name.clone().unwrap_or_else(|| c.dst.to_string());
                let d = by_dest.entry(label).or_default();
                d.0 += rate;
                d.1 += model.live.standing(c);
            }
        }
        way.hosts = busiest(by_dest);
    }
    ways
}

/// The busiest destinations first, by their rate over the last twenty
/// seconds or so (by name when equal), so they don't trade places every
/// second; past HOSTS_PER_WAY, the last line stands for the rest. Each
/// destination's (rate shown, rate it's ordered by).
fn busiest(by_dest: HashMap<String, (f64, f64)>) -> Vec<Dest> {
    let mut all: Vec<(Dest, f64)> = by_dest
        .into_iter()
        .map(|(label, (rate, standing))| {
            (
                Dest {
                    label,
                    rate,
                    more: false,
                },
                standing,
            )
        })
        .collect();
    all.sort_by(|(a, a_standing), (b, b_standing)| {
        b_standing
            .total_cmp(a_standing)
            .then_with(|| a.label.cmp(&b.label))
    });
    let mut all: Vec<Dest> = all.into_iter().map(|(d, _)| d).collect();
    if all.len() <= HOSTS_PER_WAY {
        return all;
    }
    let rest = all.split_off(HOSTS_PER_WAY - 1);
    all.push(Dest {
        label: format!("and {} more", rest.len()),
        rate: rest.iter().map(|d| d.rate).sum(),
        more: true,
    });
    all
}

/// Where everything sits, in the graph's own pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub height: f32,
    /// This computer's centre, vertically.
    pub me: f32,
    /// Each way's centre, and each of its destinations' centres.
    pub ways: Vec<(f32, Vec<f32>)>,
}

pub fn layout(ways: &[Way]) -> Layout {
    let mut y = MARGIN;
    let mut placed = Vec::new();
    for way in ways {
        let n = way.hosts.len();
        let block = (n as f32 * ROW).max(MIN_BLOCK);
        let centre = y + block / 2.;
        let hosts = (0..n)
            .map(|i| centre + (i as f32 - (n as f32 - 1.) / 2.) * ROW)
            .collect();
        placed.push((centre, hosts));
        y += block + WAY_GAP;
    }
    let height = y - WAY_GAP + MARGIN;
    let me = height / 2.;
    Layout {
        height,
        me,
        ways: placed,
    }
}

/// A path between two boxes: a cubic curve, flat at both ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    from: (f32, f32),
    to: (f32, f32),
}

impl Curve {
    pub fn new(from: (f32, f32), to: (f32, f32)) -> Self {
        Self { from, to }
    }

    fn at_t(&self, t: f32) -> (f32, f32) {
        let mid = (self.from.0 + self.to.0) / 2.;
        let (p0, p1, p2, p3) = (self.from, (mid, self.from.1), (mid, self.to.1), self.to);
        let u = 1. - t;
        // literal: the cubic Bézier's binomial coefficients
        let (a, b, c, d) = (u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t);
        (
            a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
            a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
        )
    }

    /// Lengths along the curve at each of STEPS + 1 points.
    fn lengths(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(STEPS + 1);
        let (mut total, mut last) = (0., self.from);
        out.push(0.);
        for i in 1..=STEPS {
            let p = self.at_t(i as f32 / STEPS as f32);
            total += ((p.0 - last.0).powi(2) + (p.1 - last.1).powi(2)).sqrt();
            out.push(total);
            last = p;
        }
        out
    }

    pub fn length(&self) -> f32 {
        self.lengths().last().copied().unwrap_or(0.)
    }

    /// The point `d` along the curve.
    pub fn at(&self, d: f32) -> (f32, f32) {
        let lengths = self.lengths();
        let i = lengths.partition_point(|l| *l < d).clamp(1, STEPS);
        let (a, b) = (lengths[i - 1], lengths[i]);
        let f = if b > a { (d - a) / (b - a) } else { 0. };
        self.at_t(((i - 1) as f32 + f.clamp(0., 1.)) / STEPS as f32)
    }

    /// The curve as a stroke, offset by `o`, up to `part` of its length.
    pub fn stroke(
        &self,
        o: Point<Pixels>,
        width: f32,
        part: f32,
    ) -> Option<gpui_kit::Path<Pixels>> {
        let p = |(x, y): (f32, f32)| point(o.x + px(x), o.y + px(y));
        let mut b = PathBuilder::stroke(px(width));
        b.move_to(p(self.from));
        let end = (STEPS as f32 * part.clamp(0., 1.)).round() as usize;
        for i in 1..=end {
            b.line_to(p(self.at_t(i as f32 / STEPS as f32)));
        }
        b.build().ok()
    }
}

/// A path to draw: its curve, its traffic and colour.
pub struct Path {
    pub key: String,
    pub curve: Curve,
    pub rate: f64,
    pub color: Rgba,
}

/// The packets on every path, moved on each frame.
#[derive(Default)]
pub struct Flows {
    /// Per path: dots owed (fractions accumulate) and each dot's distance.
    dots: HashMap<String, (f64, Vec<f32>)>,
    last: Option<Instant>,
    /// When each path first appeared (a tunnel's new destination is drawn in).
    appeared: HashMap<String, Instant>,
}

/// How fast a path's dots move (px/s) and how many start a second.
pub fn pace(rate: f64) -> (f32, f64) {
    let kb = rate / KB;
    let speed = SPEED_BASE + SPEED_PER_DECADE * (1. + kb).log10() as f32;
    let every = if rate > 0. {
        (kb / DOT_KB).clamp(DOTS_MIN, DOTS_MAX)
    } else {
        0.
    };
    (speed, every)
}

impl Flows {
    /// Moves every path's dots on to `now`; `moving` false (animations off)
    /// keeps none.
    pub fn step(&mut self, paths: &[Path], now: Instant, moving: bool) {
        let dt = self
            .last
            .map_or(0., |t| now.saturating_duration_since(t).as_secs_f32())
            .min(LONGEST_FRAME);
        self.last = Some(now);
        self.dots.retain(|k, _| paths.iter().any(|p| &p.key == k));
        for p in paths {
            self.appeared.entry(p.key.clone()).or_insert(now);
            let (speed, every) = pace(p.rate);
            let (owed, dots) = self.dots.entry(p.key.clone()).or_default();
            if !moving {
                dots.clear();
                continue;
            }
            *owed += f64::from(dt) * every;
            while *owed >= 1. {
                *owed -= 1.;
                dots.push(0.);
            }
            let len = p.curve.length();
            for d in dots.iter_mut() {
                *d += speed * dt;
            }
            dots.retain(|d| *d <= len);
        }
    }

    pub fn dots(&self, key: &str) -> &[f32] {
        self.dots.get(key).map_or(&[], |(_, d)| d.as_slice())
    }

    /// How much of a path is drawn in: 1 once it's been there DRAW_IN.
    pub fn drawn(&self, key: &str, now: Instant) -> f32 {
        self.appeared.get(key).map_or(1., |at| {
            (now.saturating_duration_since(*at).as_secs_f32() / DRAW_IN).min(1.)
        })
    }

    /// The paths that appeared first (the window opening) are there
    /// already: only later ones are drawn in.
    pub fn settle(&mut self, now: Instant) {
        let long_ago = now
            .checked_sub(std::time::Duration::from_secs_f32(DRAW_IN))
            .unwrap_or(now);
        for at in self.appeared.values_mut() {
            *at = long_ago;
        }
    }

    pub fn any_moving(&self) -> bool {
        self.dots
            .values()
            .any(|(owed, d)| !d.is_empty() || *owed > 0.)
    }
}

/// A path as this frame paints it: its dots, and how much of it is drawn.
pub struct Painted {
    pub path: Path,
    pub dots: Vec<f32>,
    pub drawn: f32,
}

impl Flows {
    /// The paths with their dots now, for the canvas (which paints after
    /// the view has let go of them).
    pub fn painted(&self, paths: Vec<Path>, now: Instant) -> Vec<Painted> {
        paths
            .into_iter()
            .map(|path| Painted {
                dots: self.dots(&path.key).to_vec(),
                drawn: self.drawn(&path.key, now),
                path,
            })
            .collect()
    }
}

/// Paints the paths and their dots. `still`: no motion, thickness by rate.
pub fn paint(
    painted: &[Painted],
    line: Rgba,
    accent: Rgba,
    origin: Point<Pixels>,
    still: bool,
    window: &mut Window,
) {
    let busiest = painted
        .iter()
        .map(|p| p.path.rate)
        .fold(0., f64::max)
        .max(1.);
    // Every line first, then every dot: a path drawn later crosses the
    // ones before it, and its line went over their dots.
    for Painted { path: p, drawn, .. } in painted {
        let drawn = *drawn;
        if still {
            let w = LINE + STILL_EXTRA * (p.rate / busiest) as f32;
            if let Some(path) = p.curve.stroke(origin, w, 1.) {
                window.paint_path(path, if p.rate > 0. { p.color } else { line });
            }
            continue;
        }
        if let Some(path) = p.curve.stroke(origin, LINE, 1.) {
            window.paint_path(path, line);
        }
        if drawn < 1.
            && let Some(path) = p.curve.stroke(origin, LINE, drawn)
        {
            window.paint_path(path, accent);
        }
    }
    if still {
        return;
    }
    for Painted { path: p, dots, .. } in painted {
        for d in dots {
            let (x, y) = p.curve.at(*d);
            let r = DOT_RADIUS;
            let b = Bounds::new(
                point(origin.x + px(x - r), origin.y + px(y - r)),
                size(px(2. * r), px(2. * r)),
            );
            window.paint_quad(gpui_kit::fill(b, p.color).corner_radii(px(r)));
        }
    }
}

/// Bytes per second in words: "1.1 MB/s", "85 KB/s", "0 KB/s".
pub fn rate(bytes: f64) -> String {
    if bytes >= KB * KB {
        format!("{:.1} MB/s", bytes / (KB * KB))
    } else {
        format!("{:.0} KB/s", bytes / KB)
    }
}

/// The paths for `ways` laid out as `l`: computer to way, way to each
/// destination, in `tunnel`/`direct` colours.
pub fn paths(ways: &[Way], l: &Layout, tunnel: Rgba, direct: Rgba) -> Vec<Path> {
    let mut out = Vec::new();
    for (way, (cy, hosts)) in ways.iter().zip(&l.ways) {
        let color = if way.tunnel.is_some() { tunnel } else { direct };
        out.push(Path {
            key: format!("way:{}", way.label),
            curve: Curve::new((ME_X + ME_W / 2., l.me), (WAY_X - WAY_W / 2., *cy)),
            rate: way.rate,
            color,
        });
        for (dest, hy) in way.hosts.iter().zip(hosts) {
            out.push(Path {
                key: format!("dest:{}:{}", way.label, dest.label),
                curve: Curve::new((WAY_X + WAY_W / 2., *cy), (HOST_X - HOST_GAP, *hy)),
                rate: dest.rate,
                color,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_busiest_come_first_and_the_rest_is_one_line() {
        // "d" is busier this second, "c" over the last twenty: "c" first.
        let few: HashMap<String, (f64, f64)> = [
            ("b".into(), (5., 5.)),
            ("a".into(), (5., 5.)),
            ("c".into(), (9., 9.)),
            ("d".into(), (20., 6.)),
        ]
        .into();
        let labels: Vec<String> = busiest(few).into_iter().map(|d| d.label).collect();
        assert_eq!(labels, ["c", "d", "a", "b"]);
        let many: HashMap<String, (f64, f64)> = (0..10)
            .map(|i| (format!("h{i}"), (f64::from(i), f64::from(i))))
            .collect();
        let shown = busiest(many);
        assert_eq!(shown.len(), HOSTS_PER_WAY);
        assert_eq!(shown[0].label, "h9");
        let rest = shown.last().unwrap();
        assert!(rest.more);
        assert_eq!(rest.label, "and 7 more");
        assert_eq!(rest.rate, (0..7).map(f64::from).sum::<f64>());
    }

    #[test]
    fn dots_are_calm_and_follow_the_traffic() {
        let (crawl, rare) = pace(12_000.);
        let (fast, often) = pace(1_100_000.);
        assert!(
            crawl > 35. && crawl < 45.,
            "a trickle crawls ({crawl} px/s)"
        );
        assert!(
            fast > 65. && fast < 75.,
            "a download about twice as fast ({fast})"
        );
        assert_eq!(rare, DOTS_MIN, "one every four seconds at least");
        assert_eq!(often, DOTS_MAX, "two a second at most");
        assert_eq!(pace(0.).1, 0., "nothing flowing, no dots");
    }

    #[test]
    fn dots_start_move_along_and_leave_at_the_end() {
        let path = Path {
            key: "p".into(),
            curve: Curve::new((0., 0.), (100., 0.)),
            rate: 1_100_000.,
            color: gpui_kit::rgb(0),
        };
        let mut f = Flows::default();
        let t0 = Instant::now();
        f.step(std::slice::from_ref(&path), t0, true);
        // Two a second: after a second (in 0.1 s frames), two dots.
        for i in 1..=10 {
            f.step(
                std::slice::from_ref(&path),
                t0 + std::time::Duration::from_millis(i * 100),
                true,
            );
        }
        assert_eq!(f.dots("p").len(), 2, "{:?}", f.dots("p"));
        assert!(f.any_moving());
        // Animations off: none.
        f.step(
            std::slice::from_ref(&path),
            t0 + std::time::Duration::from_millis(1100),
            false,
        );
        assert!(f.dots("p").is_empty());
        // A path that's gone takes its dots along.
        f.step(&[], t0 + std::time::Duration::from_millis(1200), true);
        assert!(f.dots("p").is_empty());
    }

    #[test]
    fn a_curve_is_measured_and_walked() {
        let c = Curve::new((0., 0.), (100., 0.));
        assert!((c.length() - 100.).abs() < 0.5);
        let (x, y) = c.at(50.);
        assert!((x - 50.).abs() < 1. && y.abs() < 0.01, "({x}, {y})");
        let bent = Curve::new((0., 0.), (100., 80.));
        assert!(bent.length() > 128., "longer than the straight line");
        assert_eq!(bent.at(0.), (0., 0.));
        let end = bent.at(bent.length());
        assert!((end.0 - 100.).abs() < 0.5 && (end.1 - 80.).abs() < 0.5);
    }

    #[test]
    fn ways_get_room_for_their_destinations() {
        let way = |n: usize| Way {
            tunnel: None,
            label: "w".into(),
            rate: 0.,
            hosts: (0..n)
                .map(|i| Dest {
                    label: i.to_string(),
                    rate: 0.,
                    more: false,
                })
                .collect(),
        };
        let l = layout(&[way(4), way(0)]);
        let (c0, h0) = &l.ways[0];
        assert_eq!(h0.len(), 4);
        assert!((h0[3] - h0[0] - 3. * ROW).abs() < 0.01, "one row apart");
        assert!(
            (h0.iter().sum::<f32>() / 4. - c0).abs() < 0.01,
            "centred on the way"
        );
        assert!(l.ways[1].0 > c0 + MIN_BLOCK, "the next way below");
        assert_eq!(l.me, l.height / 2.);
    }

    #[test]
    fn rates_in_words() {
        assert_eq!(rate(1_100_000.), "1.1 MB/s");
        assert_eq!(rate(85_400.), "85 KB/s");
        assert_eq!(rate(0.), "0 KB/s");
    }
}
