# gpui-pre-linux 0.3.8, patched for Crake

Vendored from crates.io (Apache-2.0, see LICENSE-APACHE) and used through
`[patch.crates-io]` in the workspace `Cargo.toml`.

Why: on GNOME Wayland an app can't place its own window, so Crake's tray
panel runs through XWayland, where it can. Upstream's X11 backend only offers
an unmanaged (override-redirect) pop-up for exact placement, which never gets
focus, so the panel couldn't close when the user clicks elsewhere.

Changes, all in `src/linux/x11/window.rs`, marked `CRAKE PATCH`:

1. `WM_NORMAL_HINTS` carries the window's position as user-specified, so the
   window manager honours the position the app asked for.
2. `WindowKind::PopUp` windows are managed (not override-redirect), get
   `_NET_WM_STATE_SKIP_TASKBAR`, `_SKIP_PAGER` and `_ABOVE` instead of the
   notification window type.

To update GPUI: re-vendor the matching gpui-pre-linux version and re-apply.
