//! Opening Crake's windows: the panel at the tray icon, Settings, tunnel
//! detail, New tunnel, sign-in and the log.

use super::*;

pub(crate) mod dock;

/// A window refits to its content only when it's off by more than this
/// (half a pixel: rounding is not a change).
pub const FIT_SLACK: f32 = 0.5;
/// The panel opening upward is fitted again this long after it grew (the
/// window system may not have placed it yet: `sys::resize_upward`, which
/// keeps its bottom where it was meanwhile).
pub const REFIT_AGAIN: [Duration; 2] = [Duration::from_millis(150), Duration::from_millis(500)];
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

/// The panel's height, kept for the next time it opens (it fitted its
/// window to it in place).
pub fn panel_height(height: f32, cx: &mut App) {
    cx.global_mut::<Shell>().panel_height = Some(height);
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
    refresh_browsers(cx);
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
    // Where Crake may place it: through XWayland, on Windows and macOS.
    let placeable = x11 || cfg!(windows) || cfg!(target_os = "macos");
    // The real monitors (XRandR, Windows' or macOS'); GPUI's own list if
    // that fails.
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
        // macOS: the click is the menu bar's edge under the icon.
        (true, Some((x, y))) if cfg!(target_os = "macos") => {
            place::place_at_bar((x as f32, y as f32), panel_size, &screens, place::Bar::EDGE)
        }
        (true, Some((x, y))) => place::place((x as f32, y as f32), panel_size, &screens),
        // No tray at all (`no_tray`): near the top, not at an icon.
        (true, None) if model.read(cx).no_tray.is_some() => {
            place::near_top(panel_size, &screens).or_else(|| place::centered(panel_size, &screens))
        }
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
            window_background: crate::chrome::window_background(),
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

/// Which browsers look names up themselves, read again from their
/// settings files (not in tests: nothing of this computer's).
fn refresh_browsers(cx: &mut App) {
    if cx.try_global::<prefs::Prefs>().is_none_or(|p| p.scratch) {
        return;
    }
    let found = crate::browser_dns::found();
    let model = cx.global::<Shell>().model.clone();
    model.update(cx, |m, cx| {
        if m.own_dns_browsers != found {
            m.own_dns_browsers = found;
            cx.notify();
        }
    });
}

/// macOS: Crake is a menu bar app (LSUIElement) and its panel doesn't
/// activate it, so a window it opens would land behind the app in front.
/// Make Crake the active app first, as menu bar apps do, in the Dock while
/// the window is open (`dock`). (GPUI's `activate_window` only orders the
/// window front.)
pub(super) fn to_front(cx: &mut App) {
    cx.global_mut::<Shell>().activations += 1;
    dock::window_opens(cx);
    if cfg!(target_os = "macos") {
        cx.activate(true);
    }
}

/// Settings (design/screens/settings.html), one window.
pub(super) fn open_settings(cx: &mut App) {
    to_front(cx);
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
        Ok(v) if v.starts_with("settings:diagnostics") => settings_view::Section::Diagnostics,
        _ => settings_view::Section::Tunnels,
    };
    // Never taller than its screen (a 1366x768 laptop cut 52 px off, 2026-10-19):
    // the main column scrolls.
    let height = settings_view::HEIGHT.min(place::max_window_height());
    let bounds = centered(settings_view::WIDTH, height, cx);
    // Like the log: a transparent window, the card draws its 12 px frame.
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Crake settings".into()),
                appears_transparent: true,
                traffic_light_position: crate::chrome::traffic_lights(),
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: crate::chrome::window_background(),
            kind: WindowKind::Normal,
            app_id: Some("crake".into()),
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
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
pub(super) fn open_detail(tunnel: String, host: Option<String>, cx: &mut App) {
    to_front(cx);
    if let Some((shown, handle)) = cx.global_mut::<Shell>().detail.take() {
        if shown == tunnel
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            show_host(handle, host, cx);
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
                traffic_light_position: crate::chrome::traffic_lights(),
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: crate::chrome::window_background(),
            kind: WindowKind::Normal,
            app_id: Some("crake".into()),
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
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
        show_host(handle, host, cx);
        cx.global_mut::<Shell>().detail = Some((tunnel, handle));
    }
}

/// Tunnel detail on a host's row (the log's "Show in ACME").
fn show_host(handle: AnyWindowHandle, host: Option<String>, cx: &mut App) {
    if let (Some(host), Some(detail)) = (host, handle.downcast::<tunnel_detail::TunnelDetail>()) {
        let _ = detail.update(cx, |view, _, cx| view.show_host(host, cx));
    }
}

/// New tunnel (design/screens/new-tunnel.html), or the same window editing
/// a tunnel added in the app. One at a time.
pub(super) fn open_new_tunnel(editing: Option<String>, cx: &mut App) {
    to_front(cx);
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
                traffic_light_position: crate::chrome::traffic_lights(),
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: crate::chrome::window_background(),
            kind: WindowKind::Normal,
            app_id: Some("crake".into()),
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
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
    to_front(cx);
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
        trusted: cx
            .global::<prefs::Prefs>()
            .trusted
            .get(&tunnel.gateway)
            .cloned(),
    };
    let bounds = centered(signin_view::WIDTH, SIGNIN_HEIGHT, cx);
    let title = format!("Sign in to {}", tunnel.display);
    let opened = cx.open_window(signin_options(bounds), move |window, cx| {
        window.set_window_title(&title);
        crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
        window.set_client_inset(px(0.));
        let view = cx.new(|cx| signin_view::SignIn::new(spec, model, requests, window, cx));
        let handle = window.window_handle();
        cx.subscribe(&view, move |_, _: &signin_view::Close, cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        })
        .detach();
        view
    });
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
    match place::window_area() {
        Some(a) => Bounds::new(
            gpui_kit::point(px(a.x + (a.w - w) / 2.), px(a.y + ((a.h - h) / 2.).max(0.))),
            size(px(w), px(h)),
        ),
        None => Bounds::centered(None, size(px(w), px(h)), cx),
    }
}

/// The full log: one window, brought back if it's already open.
pub(super) fn open_log(cx: &mut App) {
    refresh_browsers(cx);
    to_front(cx);
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
                traffic_light_position: crate::chrome::traffic_lights(),
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: crate::chrome::window_background(),
            kind: WindowKind::Normal,
            app_id: Some("crake".into()),
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
            window.set_client_inset(px(0.));
            let view = cx.new(|cx| log_view::Log::new(model, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &log_view::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Shell>().log = None;
            })
            .detach();
            // A routed row's "Show in ACME": tunnel detail on that host.
            cx.subscribe(&view, |_, e: &log_view::ShowIn, cx| {
                let (tunnel, host) = (e.0.clone(), e.1.clone());
                cx.defer(move |cx| open_detail(tunnel, Some(host), cx));
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

/// The Traffic window (design/proposals/traffic-window.html), from the
/// panel's footer. One at a time; a normal window, like the log.
pub(super) fn open_traffic(cx: &mut App) {
    to_front(cx);
    if let Some(handle) = cx.global::<Shell>().traffic
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let shell = cx.global::<Shell>();
    let (model, requests) = (shell.model.clone(), shell.requests.clone());
    let height = place::window_height(traffic_view::HEIGHT);
    let bounds = centered(traffic_view::WIDTH, height, cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Crake traffic".into()),
                appears_transparent: true,
                traffic_light_position: crate::chrome::traffic_lights(),
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: crate::chrome::window_background(),
            kind: WindowKind::Normal,
            app_id: Some("crake".into()),
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
            window.set_client_inset(px(0.));
            let view = cx.new(|cx| traffic_view::Traffic::new(model, requests, window, cx));
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &traffic_view::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Shell>().traffic = None;
            })
            .detach();
            view
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.global_mut::<Shell>().traffic = Some(handle);
    }
}

/// The profile editor for `tunnel`'s OpenVPN 3 profile
/// (design/proposals/edit-ovpn-profile.html): one at a time; asked again
/// for the same tunnel, the open one comes to the front.
pub(super) fn open_profile_editor(tunnel: String, cx: &mut App) {
    to_front(cx);
    if let Some((open, handle)) = cx.global::<Shell>().profile_editor.clone() {
        if open == tunnel
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            return;
        }
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
    let shell = cx.global::<Shell>();
    let requests = shell.requests.clone();
    let store: std::sync::Arc<dyn profile_editor::Store> = if shell.demo {
        std::sync::Arc::new(profile_editor::Demo)
    } else {
        std::sync::Arc::new(profile_editor::Openvpn3)
    };
    let Some(t) = shell
        .model
        .read(cx)
        .tunnels
        .iter()
        .find(|x| x.name == tunnel)
        .cloned()
    else {
        return;
    };
    let height = place::window_height(profile_editor::HEIGHT);
    let bounds = centered(profile_editor::WIDTH, height, cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some(format!("{}\u{2019}s profile", t.display).into()),
                appears_transparent: true,
                traffic_light_position: crate::chrome::traffic_lights(),
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: crate::chrome::window_background(),
            kind: WindowKind::Normal,
            app_id: Some("crake".into()),
            is_resizable: true,
            ..Default::default()
        },
        move |window, cx| {
            crate::sys::rounded_frame(window, crate::theme::Theme::current(cx).line);
            window.set_client_inset(px(0.));
            let view = cx.new(|cx| {
                profile_editor::ProfileEditor::new(
                    t.name.clone(),
                    t.display.clone(),
                    t.gateway.clone(),
                    store,
                    requests,
                    window,
                    cx,
                )
            });
            let handle = window.window_handle();
            cx.subscribe(&view, move |_, _: &profile_editor::Close, cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Shell>().profile_editor = None;
            })
            .detach();
            view
        },
    );
    if let Ok(handle) = opened {
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.global_mut::<Shell>().profile_editor = Some((tunnel, handle));
    }
}

/// The sign-in window: a plain transparent window (the card draws its own
/// rounded frame, as in sign-in.html; gpui-kit's frame is square). A normal
/// window, opened in front so it's noticed, that goes behind others like
/// any window when the user clicks elsewhere: a web sign-in happens in the
/// browser, which a window kept above everything covered.
fn signin_options(bounds: Bounds<gpui_kit::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_decorations: Some(WindowDecorations::Client),
        window_background: crate::chrome::window_background(),
        kind: WindowKind::Normal,
        app_id: Some("crake".into()),
        is_resizable: false,
        is_minimizable: true,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;

    use super::*;

    /// Seen during a web sign-in: the window stayed above every other one,
    /// over the very browser page the user had to sign in on, and couldn't
    /// be sent behind or minimized.
    #[test]
    fn sign_in_goes_behind_other_windows_like_any_window() {
        let o = signin_options(Bounds::default());
        assert!(matches!(o.kind, WindowKind::Normal), "a pop-up stays above");
        assert!(o.is_minimizable);
        assert!(o.focus && o.show, "it opens in front, to be noticed");
    }

    /// The app shell as main() starts it, in demo mode (no craked, no tray).
    fn start_app(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| {
            crate::start(
                cx,
                crate::Setup {
                    model: Model::default(),
                    x11: false,
                    demo: true,
                    connect: None,
                    tray: false,
                    single_instance: false,
                    openvpn3: false,
                    autostart: false,
                },
            )
        });
    }

    fn activations(cx: &mut TestAppContext) -> u32 {
        cx.update(|cx| cx.global::<Shell>().activations)
    }

    /// Every window Crake opens makes Crake the active app first, also
    /// when it's open already: on macOS (a menu bar app, whose panel
    /// doesn't activate it) they opened behind the app in front.
    #[gpui_kit::test]
    fn every_window_brings_crake_to_the_front(cx: &mut TestAppContext) {
        start_app(cx);
        type Open = fn(&mut App);
        let opens: [(&str, Open); 4] = [
            ("Settings", open_settings),
            ("the log", open_log),
            ("New tunnel", |cx| open_new_tunnel(None, cx)),
            ("tunnel detail", |cx| open_detail("acme".into(), None, cx)),
        ];
        for (name, open) in opens {
            for time in ["opening", "opening again"] {
                let before = activations(cx);
                cx.update(open);
                cx.run_until_parked();
                assert!(
                    activations(cx) > before,
                    "{time} {name} left Crake behind the app in front"
                );
            }
        }
    }

    /// On a Mac, Crake is in the Dock while a window of its own is open,
    /// and only then: not for the panel, and out again once the last
    /// window closes, whichever closes last.
    #[gpui_kit::test]
    fn in_the_dock_only_while_a_window_is_open(cx: &mut TestAppContext) {
        start_app(cx);
        let in_dock = |cx: &mut TestAppContext| cx.update(|cx| cx.global::<dock::InDock>().0);
        assert!(!in_dock(cx), "a menu bar app starts out of the Dock");
        cx.update(|cx| {
            cx.set_global(crate::prefs::Prefs {
                scratch: true,
                ..Default::default()
            });
            open_panel(None, cx);
        });
        cx.run_until_parked();
        assert!(!in_dock(cx), "the panel alone keeps it out");
        cx.update(open_settings);
        cx.update(open_log);
        cx.run_until_parked();
        assert!(in_dock(cx), "Settings brings it into the Dock");
        let close = |cx: &mut TestAppContext, handle: Option<AnyWindowHandle>| {
            handle
                .expect("the window is open")
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
            cx.run_until_parked();
        };
        let settings = cx.update(|cx| cx.global::<Shell>().settings);
        close(cx, settings);
        assert!(in_dock(cx), "the log is still open");
        // The panel opened again over the log (Settings took the focus it
        // closes without): the log closing leaves only the panel.
        cx.update(|cx| open_panel(None, cx));
        cx.run_until_parked();
        let log = cx.update(|cx| cx.global::<Shell>().log);
        close(cx, log);
        assert!(
            cx.update(|cx| cx.global::<Shell>().panel.is_some()),
            "the panel is open"
        );
        assert!(!in_dock(cx), "only the panel is left");
    }

    /// A sign-in kept from before had run out (2026-10-28: switched off
    /// overnight, the gateway forgot it): switched on a moment ago, the
    /// tunnel's sign-in opens at once, rather than "Ended by the gateway"
    /// and a second switch-on. Not just switched on (a reconnect by
    /// itself), nothing opens unasked. (Which sign-in opens is checked, not
    /// the window: it would start a real sign-in, on the network.)
    #[gpui_kit::test]
    fn a_sign_in_that_ran_out_opens_again_when_just_switched_on(cx: &mut TestAppContext) {
        start_app(cx);
        let (name, display) = cx.update(|cx| {
            cx.set_global(crate::prefs::Prefs {
                scratch: true,
                notify: "none".into(),
                ..Default::default()
            });
            let model = cx.global::<Shell>().model.clone();
            model.update(cx, |m, _| {
                // Off, signed in from before.
                m.tunnels = vec![client::Tunnel {
                    name: "acme".into(),
                    display: "ACME".into(),
                    gateway: "vpn.acme.example".into(),
                    state: "off".into(),
                    protocol: "anyconnect".into(),
                    signed_in: true,
                    ..Default::default()
                }];
                ("acme".to_owned(), "ACME".to_owned())
            })
        });
        let ran_out = crake_proto::LogEntry {
            ts: String::new(),
            kind: crake_proto::LogKind::System,
            tunnel: Some(name.clone()),
            message: crake_proto::needs_sign_in(&display),
            reason: None,
            hosts: Vec::new(),
            others: Vec::new(),
        };
        // As craked says it, not switched on just now: no window.
        let windows = |cx: &mut TestAppContext| cx.update(|cx| cx.windows().len());
        let before = windows(cx);
        let said = client::Update::Appended(ran_out.clone());
        cx.update(|cx| crate::updates::on_update(said, cx));
        cx.run_until_parked();
        assert_eq!(windows(cx), before, "nothing opens unasked");

        // Switched on by the user a moment ago: its sign-in, once.
        cx.update(|cx| crate::requests::on_request(Request::Switch(name.clone(), true), cx));
        let opens = |cx: &mut TestAppContext| {
            cx.update(|cx| crate::requests::sign_in_to_open(&ran_out, cx))
                .map(|t| t.name)
        };
        assert_eq!(opens(cx), Some(name), "the sign-in opens");
        assert_eq!(opens(cx), None, "once");
    }
}
