//! Opening Crake's windows: the panel at the tray icon, Settings, tunnel
//! detail, New tunnel, sign-in and the log.

use super::*;

/// A window refits to its content only when it's off by more than this
/// (half a pixel: rounding is not a change).
pub const FIT_SLACK: f32 = 0.5;
/// A click on the tray right after the panel lost focus is the click that
/// closed it.
const BLURRED_JUST_NOW: Duration = Duration::from_millis(500);
/// Windows' first heights (they then fit their content): tunnel detail,
/// sign-in, the log (and the tallest it opens).
const DETAIL_HEIGHT: f32 = 900.;
const SIGNIN_HEIGHT: f32 = 360.;
const LOG_HEIGHT: f32 = 860.;
const LOG_TALLEST: f32 = 900.;

pub(super) fn blurred_just_now(cx: &mut App) -> bool {
    let shell = cx.global_mut::<Shell>();
    let recent = shell
        .panel_blurred
        .is_some_and(|at| at.elapsed() < BLURRED_JUST_NOW);
    shell.panel_blurred = None;
    recent
}

/// The panel closed itself because something else took focus.
pub fn panel_blurred(window: AnyWindowHandle, cx: &mut App) {
    debug("panel closed itself: it lost focus");
    let shell = cx.global_mut::<Shell>();
    // Only if it's still the open one (not one being replaced).
    if shell
        .panel
        .is_some_and(|p| p.window_id() == window.window_id())
    {
        shell.panel = None;
        shell.panel_blurred = Some(std::time::Instant::now());
    }
}

/// The panel measured itself at a different height than it was opened with.
/// Opening upward its bottom edge must stay put, so it reopens at the right
/// size (once; the height is kept for next time).
pub fn refit_panel(height: f32, cx: &mut App) {
    debug(&format!("panel refit to {height:.0} px"));
    let shell = cx.global_mut::<Shell>();
    shell.panel_height = Some(height);
    if let Some(old) = shell.panel.take() {
        let _ = old.update(cx, |_, window, _| window.remove_window());
    }
    let at = cx.global::<Shell>().last_click;
    open_panel(at, cx);
}

/// The tray icon toggles the panel.
pub(super) fn toggle_panel(at: Option<(i32, i32)>, cx: &mut App) {
    let open = cx.global::<Shell>().panel;
    debug(&format!(
        "toggle: panel open={} click={at:?}",
        open.is_some()
    ));
    if let Some(handle) = open {
        let closed = handle
            .update(cx, |_, window, _| window.remove_window())
            .is_ok();
        cx.global_mut::<Shell>().panel = None;
        if closed {
            return;
        }
    }
    if blurred_just_now(cx) {
        return;
    }
    if at.is_some() {
        cx.global_mut::<Shell>().last_click = at;
    }
    let at = cx.global::<Shell>().last_click;
    let motion = panel::Motion::new(cx.global::<Shell>().model.read(cx));
    cx.global_mut::<Shell>().panel_motion = Some(motion);
    open_panel(at, cx);
}

/// A plain window (not gpui-kit's framed root): the panel draws its own
/// rounded frame and shadow on a transparent background. Through XWayland it
/// opens at the tray icon; otherwise wherever the desktop puts it.
pub(super) fn open_panel(at: Option<(i32, i32)>, cx: &mut App) {
    let shell = cx.global::<Shell>();
    let (model, requests, x11) = (shell.model.clone(), shell.requests.clone(), shell.x11);
    let motion = match &shell.panel_motion {
        Some(m) => m.clone(),
        None => panel::Motion::new(model.read(cx)),
    };
    let height = shell
        .panel_height
        .unwrap_or_else(|| Panel::height(model.read(cx)));
    let m = panel::MARGIN;
    let (w, h) = (panel::WIDTH + 2. * m, height + 2. * m);
    // Where Crake may place it: through XWayland, and on Windows.
    let placeable = x11 || cfg!(windows);
    // The real monitors (XRandR, or Windows'); GPUI's own list if that fails.
    let mut screens = if placeable {
        place::monitors()
    } else {
        Vec::new()
    };
    if screens.is_empty() {
        screens = cx
            .displays()
            .iter()
            .map(|d| {
                let b = d.bounds();
                place::Rect {
                    x: f32::from(b.origin.x),
                    y: f32::from(b.origin.y),
                    w: f32::from(b.size.width),
                    h: f32::from(b.size.height),
                }
            })
            .collect();
    }
    let panel_size = (panel::WIDTH, height);
    let placed = match (placeable, at) {
        (true, Some((x, y))) => place::place((x as f32, y as f32), panel_size, &screens),
        // No position from the tray (Dash to Panel sends none): work out
        // where the tray is from the desktop's bar.
        (true, None) => {
            let bar = place::Bar::of_desktop();
            bar.tray_guess(&screens)
                .and_then(|click| place::place_at_bar(click, panel_size, &screens, bar))
                .or_else(|| place::centered(panel_size, &screens))
        }
        _ => None,
    };
    debug(&format!("screens {screens:?}; panel at {placed:?}"));
    let bounds = match placed {
        // The window includes the shadow margin around the visible panel.
        Some(p) => Bounds::new(
            gpui_kit::point(px(p.x - m), px(p.y - m)),
            size(px(w), px(h)),
        ),
        None => centered(w, h, cx),
    };
    let upward = placed.is_some_and(|p| p.upward);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            window_decorations: Some(WindowDecorations::Client),
            // Windows rounds the panel itself (`sys::rounded_frame`): opaque.
            window_background: if cfg!(windows) {
                gpui_kit::WindowBackgroundAppearance::Opaque
            } else {
                gpui_kit::WindowBackgroundAppearance::Transparent
            },
            // With the patched X11 backend: above other windows, out of the
            // taskbar, focusable (see vendor/gpui-pre-linux/CRAKE-PATCH.md).
            kind: if placed.is_some() {
                WindowKind::PopUp
            } else {
                WindowKind::Normal
            },
            is_resizable: false,
            is_minimizable: false,
            app_id: Some("crake".into()),
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::plain_frame(window);
            crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
            // The visible panel, without the shadow margin, is the window for the desktop.
            window.set_client_inset(px(panel::MARGIN));
            cx.new(|cx| Panel::new(model, requests, motion, upward, window, cx))
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.global_mut::<Shell>().panel = Some(handle);
    }
}

/// Settings (design/screens/settings.html), one window.
pub(super) fn open_settings(cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().settings
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let shell = cx.global::<Shell>();
    let (model, requests) = (shell.model.clone(), shell.requests.clone());
    // CRAKE_SHOW=settings:<section> (development): open at that section.
    let section = match std::env::var("CRAKE_SHOW").as_deref() {
        Ok("settings:general") => settings_view::Section::General,
        Ok("settings:learning") => settings_view::Section::Learning,
        Ok("settings:names") => settings_view::Section::Names,
        Ok("settings:privacy") => settings_view::Section::Privacy,
        Ok("settings:managed") => settings_view::Section::Managed,
        Ok("settings:about") => settings_view::Section::About,
        _ => settings_view::Section::Tunnels,
    };
    let bounds = centered(settings_view::WIDTH, settings_view::HEIGHT, cx);
    // Like the log: a transparent window, the card draws its 12 px frame.
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Crake settings".into()),
                appears_transparent: true,
                traffic_light_position: None,
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
            let view =
                cx.new(|cx| settings_view::Settings::new(model, requests, section, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &settings_view::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Shell>().settings = None;
            })
            .detach();
            view
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.global_mut::<Shell>().settings = Some(handle);
    }
}

/// Tunnel detail (design/screens/tunnel-detail.html). One at a time: asked
/// for another tunnel, it shows that one instead.
pub(super) fn open_detail(tunnel: String, cx: &mut App) {
    if let Some((shown, handle)) = cx.global_mut::<Shell>().detail.take() {
        if shown == tunnel
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            cx.global_mut::<Shell>().detail = Some((shown, handle));
            return;
        }
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
    let shell = cx.global::<Shell>();
    let (model, requests) = (shell.model.clone(), shell.requests.clone());
    // Opened tall, then it fits its content (a window can shrink at once;
    // growing past the screen is refused).
    let height = place::max_window_height_or(DETAIL_HEIGHT);
    let bounds = centered(tunnel_detail::WIDTH, height, cx);
    let name = tunnel.clone();
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Crake tunnel".into()),
                appears_transparent: true,
                traffic_light_position: None,
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
            let view =
                cx.new(|cx| tunnel_detail::TunnelDetail::new(name, model, requests, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &tunnel_detail::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Shell>().detail = None;
            })
            .detach();
            view
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.global_mut::<Shell>().detail = Some((tunnel, handle));
    }
}

/// New tunnel (design/screens/new-tunnel.html), or the same window editing
/// a tunnel added in the app. One at a time.
pub(super) fn open_new_tunnel(editing: Option<String>, cx: &mut App) {
    if let Some(handle) = cx.global_mut::<Shell>().new_tunnel.take() {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
    let shell = cx.global::<Shell>();
    let (model, requests) = (shell.model.clone(), shell.requests.clone());
    let tunnel = editing.and_then(|name| {
        model
            .read(cx)
            .tunnels
            .iter()
            .find(|t| t.name == name)
            .cloned()
    });
    // Opened tall, then it shrinks to each step (the desktop may refuse to
    // grow a window past the screen's edge; shrinking always works).
    let height = place::max_window_height_or(new_tunnel::HEIGHT);
    let bounds = centered(new_tunnel::WIDTH, height, cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Crake new tunnel".into()),
                appears_transparent: true,
                traffic_light_position: None,
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
            let view = cx.new(|cx| new_tunnel::NewTunnel::new(model, requests, tunnel, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &new_tunnel::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Shell>().new_tunnel = None;
            })
            .detach();
            view
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.global_mut::<Shell>().new_tunnel = Some(handle);
    }
}

/// The sign-in window for a tunnel (one at a time per tunnel).
pub(super) fn open_sign_in(tunnel: client::Tunnel, cx: &mut App) {
    let shell = cx.global::<Shell>();
    let (model, requests) = (shell.model.clone(), shell.requests.clone());
    let spec = signin::Spec {
        tunnel: tunnel.name.clone(),
        display: tunnel.display.clone(),
        gateway: tunnel.gateway.clone(),
        protocol: tunnel.protocol.clone(),
        user: tunnel.user.clone(),
        auth_args: tunnel.auth_args.clone(),
        extra_args: tunnel.extra_args.clone(),
        user_agent: cx
            .global::<prefs::Prefs>()
            .user_agents
            .get(&tunnel.gateway)
            .cloned(),
        keyring: !shell.demo,
        openconnect: None,
    };
    let bounds = centered(signin_view::WIDTH, SIGNIN_HEIGHT, cx);
    // A plain transparent window: the card draws its own rounded frame, as
    // in sign-in.html (gpui-kit's frame is square). A pop-up, because it
    // answers the user's own click: in front and above.
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            window_decorations: Some(WindowDecorations::Client),
            window_background: gpui_kit::WindowBackgroundAppearance::Transparent,
            kind: WindowKind::PopUp,
            app_id: Some("crake".into()),
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::plain_frame(window);
            window.set_client_inset(px(0.));
            let view = cx.new(|cx| signin_view::SignIn::new(spec, model, requests, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &signin_view::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            })
            .detach();
            view
        },
    );
    match opened {
        Ok(handle) => {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
        Err(e) => debug(&format!("sign-in window: {e}")),
    }
}

/// A window of this size in the middle of the primary monitor. (GPUI's own
/// centering uses one screen spanning all monitors under X11.)
pub(super) fn centered(w: f32, h: f32, cx: &App) -> Bounds<gpui_kit::Pixels> {
    match place::monitors().first() {
        Some(m) => Bounds::new(
            gpui_kit::point(px(m.x + (m.w - w) / 2.), px(m.y + ((m.h - h) / 2.).max(0.))),
            size(px(w), px(h)),
        ),
        None => Bounds::centered(None, size(px(w), px(h)), cx),
    }
}

/// The full log: one window, brought back if it's already open.
pub(super) fn open_log(cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().log
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let model = cx.global::<Shell>().model.clone();
    let height = place::max_window_height_or(LOG_HEIGHT).min(LOG_TALLEST);
    let bounds = centered(log_view::WIDTH, height, cx);
    // A plain transparent window: the card draws its own 12 px frame, as in
    // log.html. A normal window (it stays, in the taskbar), not a pop-up.
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Crake log".into()),
                appears_transparent: true,
                traffic_light_position: None,
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
            let view = cx.new(|cx| log_view::Log::new(model, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &log_view::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Shell>().log = None;
            })
            .detach();
            view
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.global_mut::<Shell>().log = Some(handle);
    }
}
