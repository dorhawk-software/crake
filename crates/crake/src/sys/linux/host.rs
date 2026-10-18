//! Small things that differ by system: the monitors, the private folder,
//! and the connection to craked (the rest is `sys::unix`'s, shared with
//! macOS).

use std::path::PathBuf;

use crate::place::Rect;

/// The monitors as X11 (XRandR) knows them, primary first, in GPUI's
/// logical pixels. GPUI's X11 backend reports one screen spanning them all,
/// which can't tell where one ends.
pub fn monitors() -> Vec<Rect> {
    // Tests see a machine without a display, whatever this one has.
    if cfg!(test) {
        return Vec::new();
    }
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt;
    let Ok((conn, screen)) = x11rb::connect(None) else {
        return Vec::new();
    };
    let Some(root) = conn.setup().roots.get(screen).map(|s| s.root) else {
        return Vec::new();
    };
    let Ok(Ok(reply)) = conn.randr_get_monitors(root, true).map(|c| c.reply()) else {
        return Vec::new();
    };
    let list = reply
        .monitors
        .iter()
        .map(|m| {
            let r = Rect {
                x: f32::from(m.x),
                y: f32::from(m.y),
                w: f32::from(m.width),
                h: f32::from(m.height),
            };
            (m.primary, r)
        })
        .collect();
    in_logical_pixels(list, scale(&conn))
}

/// The scale GPUI's X11 backend draws at, read as it reads it:
/// GPUI_X11_SCALE_FACTOR, else Xft.dpi over 96 (GNOME publishes XWayland's
/// scale there: 192 at 200%), else 1.
fn scale(conn: &impl x11rb::connection::Connection) -> f32 {
    /// Xft.dpi at 100%.
    const BASE_DPI: f32 = 96.;
    if let Some(scale) = std::env::var("GPUI_X11_SCALE_FACTOR")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|s| s.is_normal() && *s > 0.)
    {
        return scale;
    }
    x11rb::resource_manager::new_from_default(conn)
        .ok()
        .and_then(|db| db.get_value::<f32>("Xft.dpi", "Xft.dpi").ok().flatten())
        .filter(|dpi| dpi.is_normal() && *dpi > 0.)
        .map_or(1., |dpi| dpi / BASE_DPI)
}

/// XRandR's monitors (X pixels, the primary flagged) as GPUI places
/// windows: in logical pixels, the primary first.
fn in_logical_pixels(mut list: Vec<(bool, Rect)>, scale: f32) -> Vec<Rect> {
    list.sort_by_key(|(primary, _)| !primary);
    list.into_iter()
        .map(|(_, r)| Rect {
            x: r.x / scale,
            y: r.y / scale,
            w: r.w / scale,
            h: r.h / scale,
        })
        .collect()
}

/// The panel opening upward, at `height` (logical pixels, the whole
/// window): resized and moved in one step, its bottom staying at the bar,
/// on X11 (where Crake's panel runs, through XWayland). False when it
/// can't (Wayland): the panel is opened again at the new size instead.
pub fn resize_upward(window: &gpui_kit::Window, height: f32) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt};
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };
    let id = match handle.as_raw() {
        RawWindowHandle::Xcb(h) => h.window.get(),
        RawWindowHandle::Xlib(h) => match u32::try_from(h.window) {
            Ok(id) => id,
            Err(_) => return false,
        },
        _ => return false,
    };
    let Ok((conn, screen)) = x11rb::connect(None) else {
        return false;
    };
    let Some(root) = conn.setup().roots.get(screen).map(|s| s.root) else {
        return false;
    };
    let size = conn.get_geometry(id).ok().and_then(|c| c.reply().ok());
    let at = conn
        .translate_coordinates(id, root, 0, 0)
        .ok()
        .and_then(|c| c.reply().ok());
    let (Some(size), Some(at)) = (size, at) else {
        return false;
    };
    let new_height = (height * window.scale_factor()).round().max(1.) as u32;
    let bottom = anchored_bottom(
        id,
        crate::place::top_keeping_bottom(i32::from(at.dst_y), u32::from(size.height), 0),
    );
    let top = crate::place::top_keeping_bottom(bottom, 0, new_height);
    let moved = conn.configure_window(
        id,
        &ConfigureWindowAux::new()
            .x(i32::from(at.dst_x))
            .y(top)
            .height(new_height),
    );
    moved.is_ok() && conn.flush().is_ok()
}

/// Where the panel's bottom belongs while it's being fitted: GNOME Shell
/// places a new window only once its first frame is drawn, and a resize
/// that came before that kept the window's first top with the new height,
/// so the panel grew down under the bar (seen 2026-10-17 with two long
/// names in "What Crake did"). Asked again a moment later (`fit_window`),
/// the bottom comes from here, not from where the window ended up.
static ANCHOR: std::sync::Mutex<Option<(u32, i32, std::time::Instant)>> =
    std::sync::Mutex::new(None);
/// How long the bottom is kept: past the last of `windows::REFIT_AGAIN`.
const ANCHORED_FOR: std::time::Duration = std::time::Duration::from_secs(1);

/// The bottom kept for window `id`, or `bottom` (where it is now) once the
/// last fit is past `ANCHORED_FOR` (the panel may have been dragged
/// since).
fn anchored_bottom(id: u32, bottom: i32) -> i32 {
    let now = std::time::Instant::now();
    let Ok(mut anchor) = ANCHOR.lock() else {
        return bottom;
    };
    let kept = match *anchor {
        Some((was, kept, at)) if was == id && now - at < ANCHORED_FOR => kept,
        _ => bottom,
    };
    *anchor = Some((id, kept, now));
    kept
}

/// XWayland's root window keeps the theme's arrow at the desktop's size,
/// on a thread of its own (`root_cursor`: the pointer blinked large coming
/// onto Crake's windows). Nothing on other X servers.
pub fn keep_root_cursor() {
    if cfg!(test) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("root cursor".into())
        .spawn(|| {
            let Ok((conn, screen)) = x11rb::connect(None) else {
                return;
            };
            if super::root_cursor::is_xwayland(&conn) {
                super::root_cursor::follow(&conn, screen);
            }
        });
}

/// The default browser's name ("Firefox"), as its desktop entry says it,
/// for the sign-in window ("Waiting for you in Firefox"); "your browser"
/// when the desktop doesn't say.
pub fn browser_name() -> String {
    let fallback = || "your browser".to_owned();
    if cfg!(test) {
        return fallback();
    }
    let Ok(out) = std::process::Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
    else {
        return fallback();
    };
    let id = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !id.ends_with(".desktop") || id.contains('/') {
        return fallback();
    }
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    let shared =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(shared.split(':').map(PathBuf::from))
        .find_map(|dir| std::fs::read_to_string(dir.join("applications").join(&id)).ok())
        .and_then(|entry| desktop_name(&entry))
        .unwrap_or_else(fallback)
}

/// A desktop entry's name, without "Web Browser" after it ("Firefox Web
/// Browser" is Firefox).
fn desktop_name(entry: &str) -> Option<String> {
    let mut main = false;
    for line in entry.lines().map(str::trim) {
        if line.starts_with('[') {
            main = line == "[Desktop Entry]";
        } else if main && let Some(name) = line.strip_prefix("Name=") {
            let name = name.trim().trim_end_matches(" Web Browser").trim();
            return (!name.is_empty()).then(|| name.to_owned());
        }
    }
    None
}

/// A folder only this user can read, for files that live as long as a
/// sign-in ($XDG_RUNTIME_DIR/crake, on tmpfs).
pub fn private_dir() -> Result<PathBuf, String> {
    use std::os::unix::fs::DirBuilderExt;
    let base = std::env::var_os("XDG_RUNTIME_DIR").ok_or("no XDG_RUNTIME_DIR")?;
    let dir = PathBuf::from(base).join("crake");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(crate::sys::OWNER_ONLY_DIR)
        .create(&dir)
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Linux windows need nothing more: Crake's own frame is the frame.
pub fn plain_frame(_window: &gpui_kit::Window) {}

/// Windows rounds the panel itself; here it rounds its own card.
pub fn rounded_frame(_window: &gpui_kit::Window, _border: gpui_kit::Rgba) {}

/// A command-line program started from the app: on Linux it has no window.
pub fn no_console(_cmd: &mut std::process::Command) {}

/// The installed craked, on the system bus (not peer to peer).
pub async fn connect_craked() -> zbus::Result<(zbus::Connection, bool)> {
    Ok((zbus::Connection::system().await?, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    /// A laptop at 200% (primary) under a 4K monitor at 150%: XWayland draws
    /// at 2, so X pixels are twice GNOME's (and GPUI's) logical pixels, and
    /// the tray's click comes in logical pixels. The monitors must too, or
    /// the panel opens at twice the click's coordinates.
    #[test]
    fn monitors_are_in_gpuis_logical_pixels() {
        let xrandr = vec![
            (false, rect(0., 0., 5120., 2880.)),
            (true, rect(608., 2880., 3840., 2400.)),
        ];
        assert_eq!(
            in_logical_pixels(xrandr, 2.),
            vec![rect(304., 1440., 1920., 1200.), rect(0., 0., 2560., 1440.)]
        );
    }

    #[test]
    fn the_browsers_name_from_its_desktop_entry() {
        let firefox = "[Desktop Entry]\nVersion=1.0\nName=Firefox Web Browser\nName[cs]=Webový prohlížeč\n\
                       [Desktop Action new-window]\nName=New Window\n";
        assert_eq!(desktop_name(firefox).as_deref(), Some("Firefox"));
        let chrome = "[Desktop Action x]\nName=Other\n[Desktop Entry]\nName=Google Chrome\n";
        assert_eq!(desktop_name(chrome).as_deref(), Some("Google Chrome"));
        assert_eq!(desktop_name("[Desktop Entry]\nExec=x\n"), None);
    }

    #[test]
    fn at_100_percent_x_pixels_are_logical_pixels() {
        let xrandr = vec![(true, rect(0., 0., 1920., 1080.))];
        assert_eq!(
            in_logical_pixels(xrandr, 1.),
            vec![rect(0., 0., 1920., 1080.)]
        );
    }

    /// GNOME Shell may place the panel after it has grown, keeping its
    /// first top: fitting again a moment later puts its bottom back where
    /// it was before it grew, not where the window ended up.
    #[test]
    fn a_panel_fitted_again_keeps_its_first_bottom() {
        const PANEL: u32 = 0x60_0001;
        assert_eq!(super::anchored_bottom(PANEL, 1270), 1270);
        // Grown down by 112 px instead of up.
        assert_eq!(super::anchored_bottom(PANEL, 1382), 1270);
        // Another window starts afresh.
        assert_eq!(super::anchored_bottom(PANEL + 1, 900), 900);
    }
}
