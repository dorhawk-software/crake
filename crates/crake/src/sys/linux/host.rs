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
    fn at_100_percent_x_pixels_are_logical_pixels() {
        let xrandr = vec![(true, rect(0., 0., 1920., 1080.))];
        assert_eq!(
            in_logical_pixels(xrandr, 1.),
            vec![rect(0., 0., 1920., 1080.)]
        );
    }
}
