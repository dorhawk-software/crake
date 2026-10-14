//! A real GPUI window on a real X server (Xvfb, run by scripts/x11-test.sh;
//! skipped unless CRAKE_X11_TEST=1): when the desktop's scale changes while
//! Crake runs, its windows follow.
//!
//! GNOME on Wayland runs Crake through XWayland, whose scale is the highest
//! of the monitors' (xwayland-native-scaling) and is published as Xft.dpi in
//! the root window's RESOURCE_MANAGER. Plugging a HiDPI monitor into a laptop
//! that ran at 100% doubles it; GPUI read Xft.dpi once at start, so every
//! window opened afterwards came out half size.

#[cfg(target_os = "linux")]
mod x11 {
    use std::time::{Duration, Instant};

    use gpui_kit::{
        App, AppContext as _, AsyncApp, Bounds, Context, IntoElement, Render, Window, WindowBounds,
        WindowHandle, WindowOptions, div, point, prelude::*, px, size,
    };
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    const FIRST: &str = "crake x11 scale test";
    const SECOND: &str = "crake x11 scale test, opened later";
    /// The window's size in logical pixels.
    const WIDTH: f32 = 400.;
    const HEIGHT: f32 = 300.;
    /// How long the window and the server get for each step.
    const PATIENCE: Duration = Duration::from_secs(10);
    const POLL: Duration = Duration::from_millis(50);

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(gpui_kit::rgb(0x2DD4BF))
        }
    }

    /// What the desktop does: the X resources with this Xft.dpi.
    fn set_dpi(conn: &RustConnection, root: u32, dpi: u32) {
        let resources = format!("Xft.dpi:\t{dpi}\n");
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
            |_, cx| cx.new(|_| Blank),
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
