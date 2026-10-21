//! Opening What's new: one window at a time (opened again, it shows what
//! it was asked for, in place of the one open).

use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, WindowBounds, WindowDecorations, WindowKind,
    WindowOptions, px,
};

use super::view::{Close, HEIGHT, WIDTH, WhatsNew};

/// Where the open window is, if one is.
#[derive(Default)]
pub struct Open(pub Option<AnyWindowHandle>);

impl gpui_kit::Global for Open {}

/// The window: since `since` (from the panel's notice) or this version.
pub fn open(since: Option<String>, cx: &mut App) {
    crate::windows::to_front(cx);
    if let Some(handle) = cx.try_global::<Open>().and_then(|o| o.0)
        && handle
            .update(cx, |_, window, _| window.remove_window())
            .is_ok()
    {
        cx.set_global(Open(None));
    }
    let height = crate::place::max_window_height_or(HEIGHT).min(HEIGHT);
    let bounds = crate::windows::centered(WIDTH, height, cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("What's new in Crake".into()),
                appears_transparent: true,
                traffic_light_position: crate::chrome::traffic_lights(),
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: gpui_kit::WindowBackgroundAppearance::Transparent,
            kind: WindowKind::Normal,
            app_id: Some("crake".into()),
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::plain_frame(window);
            window.set_client_inset(px(0.));
            let view = cx.new(|cx| WhatsNew::new(since, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.set_global(Open(None));
            })
            .detach();
            view
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.set_global(Open(Some(handle)));
    }
}
