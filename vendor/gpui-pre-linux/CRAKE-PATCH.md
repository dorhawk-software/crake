# gpui-pre-linux 0.3.8, patched for Crake

Vendored from crates.io (Apache-2.0, see LICENSE-APACHE) and used through
`[patch.crates-io]` in the workspace `Cargo.toml`.

Why: on GNOME Wayland an app can't place its own window, so Crake's tray
panel runs through XWayland, where it can. Upstream's X11 backend only offers
an unmanaged (override-redirect) pop-up for exact placement, which never gets
focus, so the panel couldn't close when the user clicks elsewhere.

Changes in `src/linux/x11/window.rs` (and, for 3, `client.rs`), marked
`CRAKE PATCH`:

1. `WM_NORMAL_HINTS` carries the window's position as user-specified, so the
   window manager honours the position the app asked for.
2. A `WindowKind::PopUp` that asks for focus (`focus: true`, GPUI's
   default, as Crake's panel and sign-in do) is managed (not
   override-redirect) and gets `_NET_WM_STATE_SKIP_TASKBAR`, `_SKIP_PAGER`
   and `_ABOVE` instead of the notification window type. Pop-ups with
   `focus: false` stay as upstream has them.
3. The scale follows Xft.dpi while the app runs. Upstream reads it once at
   start; GNOME publishes XWayland's scale there (the highest monitor's, with
   xwayland-native-scaling), so a laptop at 100% that gets a HiDPI monitor
   goes to 192, and every window opened afterwards came out half size. The
   client watches the root window's RESOURCE_MANAGER, reads the scale again
   and gives each window the new one, keeping its logical size (the window
   asks for the new number of pixels; its ConfigureNotify tells GPUI).
   Tested on a real X server: `crates/crake/tests/x11_scale.rs`
   (`scripts/x11-test.sh`, Xvfb with a software GPU).

Keeping it current: the change itself is `vendor/gpui-pre-linux.patch` (a
diff against crates.io's pristine source). `scripts/update-gpui.sh` takes the
newest gpui-kit, the gpui-pre-linux it pins, applies the patch and vendors
the result; `.github/workflows/gpui.yml` runs it weekly and opens a pull
request (or an issue when the patch no longer applies). When upstream has
taken the change (the patch applies backwards), the script drops this copy
and the `[patch]`. `scripts/check-gpui-patch.sh` (in the lint gate) fails
the build if Cargo quietly stopped using this copy.

Upstream (GPUI is Zed's; gpui-pre-* are snapshots of Zed's crates, packaged
by gpui-kit's author): change 1 is zed-industries/zed#64447 (reported by
someone else; its fix also sets static gravity, which a window with a title
bar needs), change 2 is zed-industries/zed#65493 (ours; this patch is the
fix proposed there). Once upstream has both, the weekly update notices and
drops this copy; if upstream's fix differs from this patch, the patch stops
applying and the update opens an issue instead.
