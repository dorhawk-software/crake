//! Where the panel goes: next to the tray icon that was clicked. Below it when
//! the icon is in the upper half of its screen (a top bar), above it when in
//! the lower half (a bottom panel such as Dash to Panel), inside that screen.
//!
//! Only the click is trusted: the icon sits in the middle of its bar, so the
//! bar's edge is about half a bar from the click. (The work area X11 reports
//! is unreliable across monitors of different sizes.)

pub use crate::sys::monitors;

/// Bars, when they don't say: Windows' taskbar and Dash to Panel's (48),
/// GNOME's top bar (32), macOS' menu bar (24); a size outside BAR_SIZES isn't a bar's; the tray's
/// icons sit about TRAY_FROM_RIGHT from the bar's right end.
const TASKBAR: f32 = 48.;
const TOP_BAR: f32 = 32.;
const MENU_BAR: f32 = 24.;
const BAR_SIZES: std::ops::RangeInclusive<f32> = 16. ..=128.;
const TRAY_FROM_RIGHT: f32 = 200.;

/// A window's tallest: the room windows have (`window_area`), or, with no
/// screen known, a laptop's.
pub fn max_window_height() -> f32 {
    const UNKNOWN_SCREEN: f32 = 1000.;
    max_window_height_or(UNKNOWN_SCREEN)
}

/// The same, with `fallback` when no screen is known.
pub fn max_window_height_or(fallback: f32) -> f32 {
    window_area().map_or(fallback, |a| a.h)
}

/// A window `wanted` tall, or as tall as its screen allows (Traffic and the
/// profile editor opened as tall as the screen until 2026-10-27).
pub fn window_height(wanted: f32) -> f32 {
    max_window_height_or(wanted).min(wanted)
}

/// Where windows open: the primary monitor's work area (the screen less
/// the bars its system reports: GNOME's top bar and Dash to Panel, the
/// menu bar and Dock, the taskbar), WORK_MARGIN kept from them.
pub fn window_area() -> Option<Rect> {
    Some(area(*monitors().first()?, crate::sys::work_area()))
}

/// Kept between a window and the bars, above and below together.
const WORK_MARGIN: f32 = 24.;
/// Taken off a screen whose work area isn't known or believable (until
/// 2026-10-27, every screen's): room for a top bar and a bottom one.
const SCREEN_ROOM: f32 = 120.;

/// The room windows have on `monitor`, given the work area its system
/// reported. A work area outside the monitor, under half of it, or the
/// whole of it isn't believed: X11's spans every monitor (cut to this one
/// here, it may miss a bar on another), and a bar that hides still counts
/// on the safe side.
fn area(monitor: Rect, work: Option<Rect>) -> Rect {
    let believable = |w: &Rect| {
        w.x >= monitor.x
            && w.y >= monitor.y
            && w.x + w.w <= monitor.x + monitor.w
            && w.y + w.h <= monitor.y + monitor.h
            && w.w >= monitor.w / 2.
            && w.h >= monitor.h / 2.
            && w.h < monitor.h
    };
    match work.filter(believable) {
        Some(w) => Rect {
            y: w.y + WORK_MARGIN / 2.,
            h: w.h - WORK_MARGIN,
            ..w
        },
        None => Rect {
            y: monitor.y + SCREEN_ROOM / 2.,
            h: monitor.h - SCREEN_ROOM,
            ..monitor
        },
    }
}

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

/// With no tray to open at (`no_tray`): centred near the top of the first
/// screen, below a top bar.
pub fn near_top(size: (f32, f32), screens: &[Rect]) -> Option<Placement> {
    let s = screens.first()?;
    Some(Placement {
        x: s.x + (s.w - size.0) / 2.,
        y: s.y + NO_TRAY_TOP,
        upward: false,
    })
}

/// How far below the screen's top edge: clear of GNOME's top bar.
const NO_TRAY_TOP: f32 = 64.;

/// Where the bar with the tray icons sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub bottom: bool,
    pub thickness: f32,
}

impl Bar {
    /// A click that is the bar's edge already (macOS: the menu bar's lower
    /// edge under the icon, `sys::tray_click`): the panel hangs a gap
    /// below it.
    pub const EDGE: Bar = Bar {
        bottom: false,
        thickness: 0.,
    };

    /// Stock GNOME: the top bar. With Dash to Panel: its panel, at the
    /// bottom unless it's been moved, at its configured size.
    pub fn of_desktop() -> Bar {
        // Windows: the taskbar, at the bottom unless moved.
        if cfg!(windows) {
            return Bar {
                bottom: true,
                thickness: TASKBAR,
            };
        }
        // macOS: the menu bar, at the top.
        if cfg!(target_os = "macos") {
            return Bar {
                bottom: false,
                thickness: MENU_BAR,
            };
        }
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
                thickness: TOP_BAR,
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
            .filter(|n| BAR_SIZES.contains(n))
            .fold(None, |m: Option<f32>, n| Some(m.map_or(n, |m| m.max(n))))
            .unwrap_or(TASKBAR);
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
        Some((s.x + s.w - TRAY_FROM_RIGHT, y))
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

/// A panel that opens upward, its height changed: where its top goes so
/// its bottom stays at the bar (in the window system's pixels). Not on
/// macOS, where the panel always hangs from the menu bar.
#[cfg(any(not(target_os = "macos"), test))]
pub fn top_keeping_bottom(top: i32, height: u32, new_height: u32) -> i32 {
    let grown = i64::from(new_height) - i64::from(height);
    i32::try_from(i64::from(top) - grown).unwrap_or(top)
}

#[cfg(test)]
mod tests {
    const HD: super::Rect = super::Rect {
        x: 0.,
        y: 0.,
        w: 1920.,
        h: 1080.,
    };

    /// Windows open inside the bars the system reports: below GNOME's top
    /// bar, above Dash to Panel at the bottom.
    #[test]
    fn windows_open_between_the_bars() {
        let work = super::Rect {
            y: 32.,
            h: 1080. - 32. - 48.,
            ..HD
        };
        let a = super::area(HD, Some(work));
        assert_eq!(a.y, 32. + super::WORK_MARGIN / 2.);
        assert_eq!(a.y + a.h, 1080. - 48. - super::WORK_MARGIN / 2.);
        // A Dock on the left: centred in what's left of the screen.
        let docked = super::Rect {
            x: 80.,
            w: 1840.,
            ..work
        };
        assert_eq!(super::area(HD, Some(docked)).x, 80.);
    }

    /// A work area that can't be this screen's (X11's spans all of them),
    /// or that reports no bar at all: the room kept before.
    #[test]
    fn a_work_area_not_believed_keeps_room_for_bars() {
        let before = super::area(HD, None);
        assert_eq!(before.h, 1080. - super::SCREEN_ROOM);
        let spanning = super::Rect {
            w: 3840.,
            h: 1000.,
            ..HD
        };
        let tiny = super::Rect { h: 400., ..HD };
        for w in [spanning, tiny, HD] {
            assert_eq!(super::area(HD, Some(w)), before, "{w:?}");
        }
    }

    /// Undo or Redo adds a row to "What Crake did": the panel grows, and
    /// opening upward it grows up, its bottom staying at the bar (it was
    /// closed and opened again at the new size, which blinked and jumped).
    #[test]
    fn a_panel_opening_upward_grows_up() {
        assert_eq!(super::top_keeping_bottom(600, 400, 440), 560);
        assert_eq!(
            super::top_keeping_bottom(600, 400, 360),
            640,
            "shrinks down"
        );
        assert_eq!(super::top_keeping_bottom(600, 400, 400), 600);
    }

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
