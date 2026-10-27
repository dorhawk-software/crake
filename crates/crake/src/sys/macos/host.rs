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

/// The menu bar's image from the Mac's drawings (RGBA, `tray::Icon::mac`):
/// one image of `MAC_POINTS` points holding both, so macOS shows the one
/// drawn for the display's pixels. tray-icon takes a single image, and a
/// 1x display halved the 2x one: its eye a grey smudge, its badge soft
/// (until 2026-10-27).
pub fn set_tray_image(tray: &tray_icon::TrayIcon, drawings: &[(u32, Vec<u8>)]) -> bool {
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return false;
    };
    let Some(button) = tray.ns_status_item().and_then(|item| item.button(mtm)) else {
        return false;
    };
    let Some(image) = menu_bar_image(drawings) else {
        return false;
    };
    button.setImage(Some(&image));
    true
}

/// One image of `MAC_POINTS` points with a representation per drawing.
fn menu_bar_image(
    drawings: &[(u32, Vec<u8>)],
) -> Option<objc2::rc::Retained<objc2_app_kit::NSImage>> {
    use objc2::AnyThread;
    use objc2_app_kit::{NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSData, NSSize};
    if drawings.is_empty() {
        return None;
    }
    let points = f64::from(crate::tray::art::MAC_POINTS);
    let size = NSSize::new(points, points);
    let image = NSImage::initWithSize(NSImage::alloc(), size);
    for (n, rgba) in drawings {
        let rep = NSBitmapImageRep::imageRepWithData(&NSData::from_vec(png(*n, rgba)?))?;
        rep.setSize(size);
        image.addRepresentation(&rep);
    }
    Some(image)
}

/// A square RGBA image as PNG, which AppKit reads into an image.
fn png(size: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(rgba).ok()?;
    writer.finish().ok()?;
    Some(out)
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

#[cfg(test)]
mod tests {
    use crate::tray::art::{MAC_POINTS, MAC_SIZE};

    #[test]
    fn the_menu_bar_image_holds_the_1x_and_2x_drawings() {
        // macOS picks the representation for the display's pixels: both
        // are there, each the menu bar's 22 points, at 1 and 2 px a point.
        let drawings = crate::tray::Art::default().icon().mac;
        let image = super::menu_bar_image(&drawings).expect("an image");
        assert_eq!(image.size().width, f64::from(MAC_POINTS));
        let reps = image.representations();
        let pixels: Vec<isize> = reps.iter().map(|r| r.pixelsWide()).collect();
        assert_eq!(pixels, [MAC_POINTS as isize, MAC_SIZE as isize]);
        assert!(reps.iter().all(|r| r.size().width == f64::from(MAC_POINTS)));
        assert!(super::menu_bar_image(&[]).is_none(), "nothing to show");
    }
}
