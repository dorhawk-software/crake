//! XWayland's root window gets the cursor theme's arrow at the desktop's
//! size, and keeps it as the scale or the cursor size changes.
//!
//! Coming onto an X window (Crake, through XWayland) from a Wayland one,
//! XWayland first shows the cursor it last showed, the root window's, and a
//! moment later the window's own. GNOME sets the root's cursor once, when
//! XWayland starts, at the scale of the moment: seen 2026-10-17 at 48 px
//! on a session back at 100% (24 px), so the pointer blinked large each
//! time it came onto a Crake window, and `xsetroot -cursor_name left_ptr`
//! stopped it. Only on XWayland: on an X11 session the root's cursor is the
//! session's to choose.
//!
//! Kept free of the rest of the app: `tests/x11_scale.rs` includes this
//! file and checks it on a real X server.

use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, Window,
};

/// What the theme calls its arrow (the freedesktop name, then X's).
const ARROW: [&str; 2] = ["default", "left_ptr"];
/// After the resources change, the root's cursor is set again this much
/// later, in case GNOME set its own in between.
const AGAIN: Duration = Duration::from_secs(1);

/// Whether the X server is XWayland.
pub fn is_xwayland(conn: &impl Connection) -> bool {
    conn.extension_information("XWAYLAND")
        .ok()
        .flatten()
        .is_some()
}

/// Gives the root window the theme's arrow, at the size the X resources
/// say (Xcursor.size, else Xft.dpi), as GPUI loads its own cursors.
pub fn set(conn: &impl Connection, screen: usize) -> Result<(), String> {
    let root = root(conn, screen);
    let database = x11rb::resource_manager::new_from_default(conn).map_err(|e| e.to_string())?;
    let handle = x11rb::cursor::Handle::new(conn, screen, &database)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    let cursor = ARROW
        .iter()
        .find_map(|name| {
            handle
                .load_cursor(conn, name)
                .ok()
                .filter(|c| *c != x11rb::NONE)
        })
        .ok_or("the cursor theme has no arrow")?;
    conn.change_window_attributes(root, &ChangeWindowAttributesAux::new().cursor(cursor))
        .map_err(|e| e.to_string())?;
    // The root holds on to it; the id is no longer needed.
    conn.free_cursor(cursor).map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())
}

/// Sets the root's cursor now and again whenever the X resources change
/// (the scale, the cursor's size or theme); returns only if the
/// connection does.
pub fn follow(conn: &impl Connection, screen: usize) {
    let root = root(conn, screen);
    let watch = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    if conn.change_window_attributes(root, &watch).is_err() {
        return;
    }
    let _ = set(conn, screen);
    while let Ok(event) = conn.wait_for_event() {
        let Event::PropertyNotify(e) = event else {
            continue;
        };
        if e.window != root || e.atom != u32::from(AtomEnum::RESOURCE_MANAGER) {
            continue;
        }
        let _ = set(conn, screen);
        std::thread::sleep(AGAIN);
        let _ = set(conn, screen);
    }
}

fn root(conn: &impl Connection, screen: usize) -> Window {
    conn.setup().roots[screen].root
}
