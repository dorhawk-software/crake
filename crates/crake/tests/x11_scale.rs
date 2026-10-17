//! A real GPUI window on a real X server (Xvfb, run by scripts/x11-test.sh;
//! skipped unless CRAKE_X11_TEST=1): when the desktop's scale changes while
//! Crake runs, its windows follow.
//!
//! GNOME on Wayland runs Crake through XWayland, whose scale is the highest
//! of the monitors' (xwayland-native-scaling) and is published as Xft.dpi in
//! the root window's RESOURCE_MANAGER. Plugging a HiDPI monitor into a laptop
//! that ran at 100% doubles it; GPUI read Xft.dpi once at start, so every
//! window opened afterwards came out half size. The cursor follows too: it
//! was loaded once at the scale Crake started with, and after a change the
//! windows had the new scale and the cursor the old (twice the size).

/// The app's own code for XWayland's root cursor, tried here on Xvfb.
#[cfg(target_os = "linux")]
#[path = "../src/sys/linux/root_cursor.rs"]
mod root_cursor;

#[cfg(target_os = "linux")]
mod x11 {
    use std::time::{Duration, Instant};

    use gpui_kit::{
        App, AppContext as _, AsyncApp, Bounds, Context, IntoElement, Render, Window, WindowBounds,
        WindowHandle, WindowOptions, div, point, prelude::*, px, size,
    };
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xfixes::ConnectionExt as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    const FIRST: &str = "crake x11 scale test";
    const SECOND: &str = "crake x11 scale test, opened later";
    const PLAIN: &str = "crake x11 scale test, no cursor of its own";
    /// The window's size in logical pixels.
    const WIDTH: f32 = 400.;
    const HEIGHT: f32 = 300.;
    /// How long the window and the server get for each step.
    const PATIENCE: Duration = Duration::from_secs(10);
    const POLL: Duration = Duration::from_millis(50);

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            // A pointing hand: GPUI sets this window's cursor itself.
            div()
                .size_full()
                .bg(gpui_kit::rgb(0x2DD4BF))
                .cursor_pointer()
        }
    }

    /// A window that never asks for a cursor: GPUI's arrow, from the start.
    struct Plain;

    impl Render for Plain {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(gpui_kit::rgb(0x2DD4BF))
        }
    }

    /// What the desktop does: the X resources with this Xft.dpi.
    fn set_dpi(conn: &RustConnection, root: u32, dpi: u32) {
        set_resources(conn, root, &format!("Xft.dpi:\t{dpi}\n"));
    }

    /// GNOME's xsettings: Xft.dpi and the cursor's size, both at the scale.
    fn set_scale(conn: &RustConnection, root: u32, dpi: u32, cursor: u32) {
        set_resources(
            conn,
            root,
            &format!("Xft.dpi:\t{dpi}\nXcursor.theme:\tAdwaita\nXcursor.size:\t{cursor}\n"),
        );
    }

    fn set_resources(conn: &RustConnection, root: u32, resources: &str) {
        conn.change_property8(
            PropMode::REPLACE,
            root,
            AtomEnum::RESOURCE_MANAGER,
            AtomEnum::STRING,
            resources.as_bytes(),
        )
        .expect("set RESOURCE_MANAGER");
        conn.flush().expect("flush");
    }

    /// A window's size in X pixels, found by its title, once the server has it.
    fn x_size(conn: &RustConnection, root: u32, title: &str) -> Option<(u16, u16)> {
        let tree = conn.query_tree(root).ok()?.reply().ok()?;
        tree.children.into_iter().find_map(|w| {
            let name = conn
                .get_property(false, w, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 64)
                .ok()?
                .reply()
                .ok()?;
            if name.value != title.as_bytes() {
                return None;
            }
            let g = conn.get_geometry(w).ok()?.reply().ok()?;
            Some((g.width, g.height))
        })
    }

    /// The width of the cursor the server shows now (XFixes).
    fn cursor_width(conn: &RustConnection) -> Option<u16> {
        Some(conn.xfixes_get_cursor_image().ok()?.reply().ok()?.width)
    }

    /// Puts the pointer in the middle of the window called `title`.
    fn point_at(conn: &RustConnection, root: u32, title: &str) {
        let tree = conn.query_tree(root).expect("tree").reply().expect("tree");
        let window = tree
            .children
            .into_iter()
            .find(|w| {
                conn.get_property(false, *w, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 64)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .is_some_and(|n| n.value == title.as_bytes())
            })
            .expect("the window");
        let g = conn
            .get_geometry(window)
            .expect("geometry")
            .reply()
            .expect("geometry");
        conn.warp_pointer(
            x11rb::NONE,
            window,
            0,
            0,
            0,
            0,
            (g.width / 2) as i16,
            (g.height / 2) as i16,
        )
        .expect("warp");
        conn.flush().expect("flush");
    }

    /// Puts the pointer where no window is: the root window's own cursor.
    fn point_at_root(conn: &RustConnection, root: u32) {
        conn.warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 1800, 1000)
            .expect("warp");
        conn.flush().expect("flush");
    }

    /// Waits until the cursor's width passes `ok` (or PATIENCE runs out);
    /// says what it saw, and fails the test if it doesn't.
    async fn check_cursor(
        cx: &mut AsyncApp,
        conn: &RustConnection,
        ok: impl Fn(u16) -> bool,
        step: &str,
    ) -> u16 {
        let start = Instant::now();
        let width = loop {
            let width = cursor_width(conn).unwrap_or(0);
            if ok(width) || start.elapsed() > PATIENCE {
                break width;
            }
            cx.background_executor().timer(POLL).await;
        };
        println!("{step}: cursor {width} px wide");
        if !ok(width) {
            println!("x11_scale: FAILED: {step}");
            std::process::exit(1);
        }
        width
    }

    #[derive(Debug, PartialEq)]
    struct Seen {
        x_pixels: Option<(u16, u16)>,
        scale: f32,
        logical: (f32, f32),
    }

    fn expect(scale: f32) -> Seen {
        Seen {
            x_pixels: Some(((WIDTH * scale) as u16, (HEIGHT * scale) as u16)),
            scale,
            logical: (WIDTH, HEIGHT),
        }
    }

    fn open(cx: &mut App, title: &'static str) -> WindowHandle<Blank> {
        open_with(cx, title, || Blank)
    }

    fn open_with<V: Render>(
        cx: &mut App,
        title: &'static str,
        view: impl FnOnce() -> V + 'static,
    ) -> WindowHandle<V> {
        let bounds = Bounds::new(point(px(100.), px(100.)), size(px(WIDTH), px(HEIGHT)));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui_kit::TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                is_resizable: false,
                ..Default::default()
            },
            |_, cx| cx.new(|_| view()),
        )
        .expect("open a window")
    }

    /// Waits until the window is as wanted (or PATIENCE runs out); says
    /// what it saw, and fails the test if it isn't.
    async fn check(
        cx: &mut AsyncApp,
        conn: &RustConnection,
        root: u32,
        (handle, title): (WindowHandle<Blank>, &str),
        want: Seen,
        step: &str,
    ) {
        let start = Instant::now();
        let now = loop {
            let now = handle
                .update(cx, |_, window, _| {
                    let viewport = window.viewport_size();
                    Seen {
                        x_pixels: x_size(conn, root, title),
                        scale: window.scale_factor(),
                        logical: (f32::from(viewport.width), f32::from(viewport.height)),
                    }
                })
                .expect("the window");
            if now == want || start.elapsed() > PATIENCE {
                break now;
            }
            cx.background_executor().timer(POLL).await;
        };
        println!("{step}: {now:?}");
        if now != want {
            println!("x11_scale: FAILED: {step}: wanted {want:?}");
            std::process::exit(1);
        }
    }

    pub fn main() {
        if std::env::var_os("CRAKE_X11_TEST").is_none() {
            println!("x11_scale: skipped (run scripts/x11-test.sh)");
            return;
        }
        let (conn, screen) = x11rb::connect(None).expect("an X server (DISPLAY)");
        let root = conn.setup().roots[screen].root;
        // XFixes answers only once its version is asked.
        conn.xfixes_query_version(5, 0)
            .expect("XFixes")
            .reply()
            .expect("XFixes");
        // The laptop alone, at 100%.
        set_dpi(&conn, root, 96);

        gpui_kit::application().run(move |cx: &mut App| {
            let first = (open(cx, FIRST), FIRST);
            cx.spawn(async move |cx| {
                check(cx, &conn, root, first, expect(1.), "at 100%").await;

                // A HiDPI monitor plugged in: XWayland's scale becomes 2.
                set_dpi(&conn, root, 192);
                let step = "a window open when the scale became 2";
                check(cx, &conn, root, first, expect(2.), step).await;
                // What the user saw: windows opened afterwards at half size.
                let second = (cx.update(|cx| open(cx, SECOND)), SECOND);
                let step = "a window opened after it became 2";
                check(cx, &conn, root, second, expect(2.), step).await;

                // Unplugged again.
                set_dpi(&conn, root, 96);
                check(cx, &conn, root, first, expect(1.), "back at 100%").await;
                check(cx, &conn, root, second, expect(1.), "back at 100%").await;

                // The cursor, as GNOME sets it: 24 px at 100%, 48 at 200%.
                set_scale(&conn, root, 96, 24);
                check(cx, &conn, root, first, expect(1.), "at 100%, cursor 24").await;
                point_at(&conn, root, FIRST);
                let small = check_cursor(cx, &conn, |w| w > 0, "pointing at 100%").await;
                set_scale(&conn, root, 192, 48);
                check(cx, &conn, root, first, expect(2.), "at 200%, cursor 48").await;
                check_cursor(cx, &conn, |w| w > small, "the cursor grew with the scale").await;
                set_scale(&conn, root, 96, 24);
                check(
                    cx,
                    &conn,
                    root,
                    first,
                    expect(1.),
                    "back at 100%, cursor 24",
                )
                .await;
                let step = "the cursor is its old size again";
                check_cursor(cx, &conn, |w| w == small, step).await;
                // The cursor's size alone (Accessibility's "Large cursor").
                set_scale(&conn, root, 96, 48);
                let large =
                    check_cursor(cx, &conn, |w| w > small, "a larger cursor, same scale").await;
                // A window that sets no cursor (the panel's plain parts): the
                // theme's arrow at its size, not the server's own cursor (under
                // XWayland at another size, until a hand replaced it).
                cx.update(|cx| open_with(cx, PLAIN, || Plain));
                cx.background_executor().timer(PATIENCE / 10).await;
                point_at(&conn, root, PLAIN);
                let step = "a window with no cursor of its own shows the theme's arrow";
                check_cursor(cx, &conn, |w| w == large, step).await;
                // XWayland's root cursor (sys::keep_root_cursor): the theme's
                // arrow at the desktop's size, following it. Xvfb isn't
                // XWayland, where the app does it; here the test runs it.
                if crate::root_cursor::is_xwayland(&conn) {
                    println!("x11_scale: FAILED: Xvfb taken for XWayland");
                    std::process::exit(1);
                }
                std::thread::spawn(|| {
                    let (conn, screen) = x11rb::connect(None).expect("X");
                    crate::root_cursor::follow(&conn, screen);
                });
                point_at_root(&conn, root);
                let step = "the root window's cursor is the theme's arrow";
                check_cursor(cx, &conn, |w| w == large, step).await;
                set_scale(&conn, root, 96, 24);
                let step = "the root window's cursor follows the size";
                check_cursor(cx, &conn, |w| w == small, step).await;
                println!("x11_scale: ok");
                std::process::exit(0);
            })
            .detach();
        });
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    x11::main();
}
