//! Where the panel goes: next to the tray icon that was clicked. Below it when
//! the icon is in the upper half of its screen (a top bar), above it when in
//! the lower half (a bottom panel such as Dash to Panel), inside that screen.
//!
//! Only the click is trusted: the icon sits in the middle of its bar, so the
//! bar's edge is about half a bar from the click. (The work area X11 reports
//! is unreliable across monitors of different sizes.)

pub use crate::sys::monitors;

/// A rectangle in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// From the click to the edge of the bar the icon sits in (half a bar).
const HALF_BAR: f32 = 24.;
/// Between the panel and that bar, and the screen's edges.
const GAP: f32 = 8.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Top-left of the visible panel.
    pub x: f32,
    pub y: f32,
    /// It opens upward: its bottom edge is the fixed one.
    pub upward: bool,
}

/// `click`: where the icon was clicked; `size`: the visible panel; `screens`:
/// each monitor.
pub fn place(click: (f32, f32), size: (f32, f32), screens: &[Rect]) -> Option<Placement> {
    let s = screens
        .iter()
        .find(|s| s.contains(click.0, click.1))
        .or(screens.first())?;
    let (w, h) = size;
    let x = (click.0 - w / 2.).clamp(s.x + GAP, (s.x + s.w - GAP - w).max(s.x + GAP));
    let upward = click.1 > s.y + s.h / 2.;
    let y = if upward {
        (click.1 - HALF_BAR - GAP - h).max(s.y + GAP)
    } else {
        (click.1 + HALF_BAR + GAP).min((s.y + s.h - GAP - h).max(s.y + GAP))
    };
    Some(Placement { x, y, upward })
}

/// The panel in the middle of the primary monitor, when no click told where
/// the tray icon is.
pub fn centered(size: (f32, f32), screens: &[Rect]) -> Option<Placement> {
    let s = screens.first()?;
    Some(Placement {
        x: s.x + (s.w - size.0) / 2.,
        y: s.y + ((s.h - size.1) / 2.).max(GAP),
        upward: false,
    })
}

/// Where the bar with the tray icons sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub bottom: bool,
    pub thickness: f32,
}

impl Bar {
    /// Stock GNOME: the top bar. With Dash to Panel: its panel, at the
    /// bottom unless it's been moved, at its configured size.
    pub fn of_desktop() -> Bar {
        let read = |key: &str| {
            std::process::Command::new("dconf")
                .args(["read", key])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default()
        };
        let enabled = read("/org/gnome/shell/enabled-extensions");
        if !enabled.contains("dash-to-panel@") {
            return Bar {
                bottom: false,
                thickness: 32.,
            };
        }
        let positions = read("/org/gnome/shell/extensions/dash-to-panel/panel-positions");
        let sizes = read("/org/gnome/shell/extensions/dash-to-panel/panel-sizes");
        Self::dash_to_panel(&positions, &sizes)
    }

    /// From Dash to Panel's settings (JSON objects by monitor).
    fn dash_to_panel(positions: &str, sizes: &str) -> Bar {
        let bottom = !(positions.contains("TOP"));
        // The values follow a colon ("monitor id": size); ids hold digits too.
        let thickness = sizes
            .split(':')
            .skip(1)
            .filter_map(|v| {
                v.trim()
                    .split(|c: char| !c.is_ascii_digit())
                    .next()?
                    .parse::<f32>()
                    .ok()
            })
            .filter(|n| (16. ..=128.).contains(n))
            .fold(None, |m: Option<f32>, n| Some(m.map_or(n, |m| m.max(n))))
            .unwrap_or(48.);
        Bar { bottom, thickness }
    }

    /// Where a tray icon most likely is when the click came without a
    /// position: the right end of the bar on the primary monitor.
    pub fn tray_guess(&self, screens: &[Rect]) -> Option<(f32, f32)> {
        let s = screens.first()?;
        let y = if self.bottom {
            s.y + s.h - self.thickness / 2.
        } else {
            s.y + self.thickness / 2.
        };
        Some((s.x + s.w - 200., y))
    }
}

/// From the click to the edge of the bar, for a bar of this thickness.
pub fn place_at_bar(
    click: (f32, f32),
    size: (f32, f32),
    screens: &[Rect],
    bar: Bar,
) -> Option<Placement> {
    let s = screens
        .iter()
        .find(|s| s.contains(click.0, click.1))
        .or(screens.first())?;
    let (w, h) = size;
    let half = bar.thickness / 2.;
    let x = (click.0 - w / 2.).clamp(s.x + GAP, (s.x + s.w - GAP - w).max(s.x + GAP));
    let upward = click.1 > s.y + s.h / 2.;
    let y = if upward {
        (click.1 - half - GAP - h).max(s.y + GAP)
    } else {
        (click.1 + half + GAP).min((s.y + s.h - GAP - h).max(s.y + GAP))
    };
    Some(Placement { x, y, upward })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0.,
        y: 0.,
        w: 2560.,
        h: 1440.,
    };
    const PANEL: (f32, f32) = (452., 600.);

    #[test]
    fn drops_down_from_a_top_bar() {
        let p = place((2400., 15.), PANEL, &[SCREEN]).unwrap();
        assert!(!p.upward);
        assert_eq!(p.y, 15. + 24. + 8.);
        assert_eq!(p.x, 2560. - 8. - 452., "kept on screen at the right edge");
    }

    #[test]
    fn opens_upward_from_a_bottom_panel() {
        let p = place((2300., 1416.), PANEL, &[SCREEN]).unwrap();
        assert!(p.upward);
        assert_eq!(
            p.y + PANEL.1,
            1416. - 24. - 8.,
            "sits right on top of the panel"
        );
        assert_eq!(p.x, 2300. - 226.);
    }

    #[test]
    fn picks_the_screen_that_was_clicked() {
        let left = Rect {
            x: 0.,
            y: 116.,
            w: 1920.,
            h: 1080.,
        };
        let right = Rect {
            x: 1920.,
            y: 0.,
            w: 2560.,
            h: 1440.,
        };
        let p = place((4400., 1420.), PANEL, &[left, right]).unwrap();
        assert!(p.upward);
        assert!(p.x >= 1920. && p.x + PANEL.0 <= 1920. + 2560.);
        assert_eq!(p.y + PANEL.1, 1420. - 32.);
    }

    #[test]
    fn without_a_click_the_tray_is_guessed_from_the_bar() {
        let laptop = Rect {
            x: 0.,
            y: 116.,
            w: 1920.,
            h: 1200.,
        };
        let other = Rect {
            x: 1920.,
            y: 0.,
            w: 2560.,
            h: 1440.,
        };
        let bar = Bar::dash_to_panel(
            "'{}'",
            r#"'{"DEL-GH85D71K0D4S":42,"SHP-0x00000000":42,"DEL-9LQ00N3":40}'"#,
        );
        assert_eq!(
            bar,
            Bar {
                bottom: true,
                thickness: 42.
            }
        );
        let click = bar.tray_guess(&[laptop, other]).unwrap();
        let p = place_at_bar(click, PANEL, &[laptop, other], bar).unwrap();
        assert!(p.upward);
        assert_eq!(
            p.y + PANEL.1,
            116. + 1200. - 42. - 8.,
            "right on top of the 42 px panel"
        );
        assert_eq!(p.x + PANEL.0, 1920. - 8., "at the right end, on the laptop");

        let top = Bar::dash_to_panel(r#"'{"SHP-0x00000000":"TOP"}'"#, "'{}'");
        assert_eq!(
            top,
            Bar {
                bottom: false,
                thickness: 48.
            }
        );
    }

    #[test]
    fn a_tall_panel_still_fits_on_screen() {
        let p = place((2300., 1416.), (452., 2000.), &[SCREEN]).unwrap();
        assert_eq!(p.y, 8.);
    }
}
