//! Small things that differ by system: the monitors, the private folder,
//! the menu bar's look, and the connection to craked (the rest is
//! `sys::unix`'s, shared with Linux).

use std::path::PathBuf;

use crate::place::Rect;

/// The displays, the main one first, in points from the main display's
/// top-left corner: where GPUI places windows (relative to the main
/// display, whose own bounds it gives every display).
pub fn monitors() -> Vec<Rect> {
    // Tests see a machine without a display, whatever this one has.
    if cfg!(test) {
        return Vec::new();
    }
    displays().into_iter().map(|(r, _)| r).collect()
}

/// The main display's work area (less the menu bar and the Dock), in the
/// same points as `monitors`: AppKit's visible frame, which counts up from
/// the bottom of the menu bar's screen, turned to count down from its top.
pub fn work_area() -> Option<Rect> {
    if cfg!(test) {
        return None;
    }
    let mtm = objc2::MainThreadMarker::new()?;
    let screens = objc2_app_kit::NSScreen::screens(mtm);
    let main = screens.iter().next()?;
    let (frame, visible) = (main.frame(), main.visibleFrame());
    Some(Rect {
        x: visible.origin.x as f32,
        y: (frame.origin.y + frame.size.height - (visible.origin.y + visible.size.height)) as f32,
        w: visible.size.width as f32,
        h: visible.size.height as f32,
    })
}

/// Each active display, the main one first, with its scale (pixels per
/// point).
fn displays() -> Vec<(Rect, f64)> {
    use objc2_core_graphics::{
        CGDisplayBounds, CGDisplayCopyDisplayMode, CGDisplayMode, CGMainDisplayID,
    };
    const MAX_DISPLAYS: usize = 16;
    let ids = crake_os::macos::active_displays(MAX_DISPLAYS);
    let main = CGMainDisplayID();
    let mut found: Vec<(bool, Rect, f64)> = ids
        .iter()
        .map(|&id| {
            let b = CGDisplayBounds(id);
            let mode = CGDisplayCopyDisplayMode(id);
            let points = CGDisplayMode::width(mode.as_deref());
            let pixels = CGDisplayMode::pixel_width(mode.as_deref());
            let scale = if points == 0 {
                1.
            } else {
                pixels as f64 / points as f64
            };
            let rect = Rect {
                x: b.origin.x as f32,
                y: b.origin.y as f32,
                w: b.size.width as f32,
                h: b.size.height as f32,
            };
            (id == main, rect, scale)
        })
        .collect();
    found.sort_by_key(|(main, ..)| !main);
    found.into_iter().map(|(_, r, s)| (r, s)).collect()
}

/// A folder only this user can read, for files that live as long as a
/// sign-in ($TMPDIR, which macOS gives each user, /crake).
/// The default browser, for the sign-in window ("Waiting for you in your
/// browser"): not looked up here yet (Linux reads its desktop entry).
pub fn browser_name() -> String {
    "your browser".to_owned()
}

pub fn private_dir() -> Result<PathBuf, String> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::env::temp_dir().join("crake");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(crate::sys::OWNER_ONLY_DIR)
        .create(&dir)
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Windows rounds Crake's windows itself; here each card rounds its own
/// (`chrome::card_edge`).
pub fn rounded_frame(_window: &gpui_kit::Window, _border: gpui_kit::Rgba) {}

/// A command-line program started from the app: on macOS it has no window.
pub fn no_console(_cmd: &mut std::process::Command) {}

/// The installed craked, on its socket (peer to peer: no system bus on
/// macOS).
pub async fn connect_craked() -> zbus::Result<(zbus::Connection, bool)> {
    Ok((
        super::super::connect_dev(std::path::Path::new(crake_proto::APP_SOCKET)).await?,
        true,
    ))
}

/// Where the panel hangs from: the menu bar's lower edge under the icon
/// (`place::Bar::EDGE`), in the points windows are placed in.
pub fn tray_click(_at: tray_icon::dpi::PhysicalPosition<f64>, icon: tray_icon::Rect) -> (i32, i32) {
    let x = icon.position.x + f64::from(icon.size.width) / 2.;
    let y = icon.position.y + f64::from(icon.size.height);
    logical(x, y)
}

/// tray-icon's pixels (points times the scale of the icon's display) in
/// points: the display whose bounds hold the point at its own scale.
fn logical(x: f64, y: f64) -> (i32, i32) {
    let displays = displays();
    let scale = displays
        .iter()
        .find(|(r, s)| {
            let (px, py) = ((x / s) as f32, (y / s) as f32);
            px >= r.x && px < r.x + r.w && py >= r.y && py <= r.y + r.h
        })
        .or(displays.first())
        .map_or(1., |(_, s)| *s);
    ((x / scale) as i32, (y / scale) as i32)
}

/// The menu bar under Crake's icon is light: the bar's own appearance, as
/// macOS draws it over the wallpaper (until 2026-10-27 the system's dark
/// mode, which on Tahoe can disagree with the bar); else `light_bar`.
pub fn tray_on_light_bar(tray: &tray_icon::TrayIcon) -> bool {
    tray.ns_status_item()
        .and_then(|item| crake_os::macos::status_item_on_light_bar(&item))
        .unwrap_or_else(light_bar)
}

/// The menu bar is light unless the system's appearance is dark.
pub fn light_bar() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map_or(true, |o| {
            !String::from_utf8_lossy(&o.stdout).contains("Dark")
        })
}

/// The menu bar's image in pixels: the Mac's own drawing of the bird,
/// which tray-icon shows at 22 pt (`tray::art::MAC_SIZE`; until 0.15.0 a
/// 36 px image, which it showed at 22 pt too, stretched, the bird filling
/// it: larger and softer than the bar's other icons).
pub fn tray_icon_size() -> u32 {
    crate::tray::art::MAC_SIZE
}

/// The panel opening upward: never here (it hangs from the menu bar), so
/// a plain resize keeps its top.
pub fn resize_upward(_window: &gpui_kit::Window, _height: f32) -> bool {
    false
}
