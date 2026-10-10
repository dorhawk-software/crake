//! `crake`: Crake in the user session. Tray icon, the panel, notifications.
//! Everything Crake decides happens in craked; this shows it and passes on
//! what the user chooses.

mod autostart;
mod chart;
mod client;
mod controls;
mod detect;
mod engines;
mod keyring;
mod log_view;
mod model;
mod motion;
mod new_tunnel;
mod notify;
mod ovpn3;
mod panel;
mod place;
mod prefs;
mod settings_view;
mod signin;
mod signin_view;
mod single;
mod theme;
mod tray;
mod tunnel_detail;

use std::time::Duration;

use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, Global, WindowBounds, WindowDecorations,
    WindowKind, WindowOptions, px, size,
};

use client::Command;
use model::Model;
use panel::Panel;

/// What the tray, notifications and the panel ask of the app.
#[derive(Clone, Debug)]
pub enum Request {
    /// Open the panel, or close it if it's open; at the tray icon that was
    /// clicked (screen position), when known.
    Panel(Option<(i32, i32)>),
    /// Switch a tunnel on (true) or off.
    Switch(String, bool),
    Undo(String, Vec<String>),
    Redo(String, Vec<String>),
    /// Open the full log (or bring it to the front).
    Log,
    /// Signed in for a tunnel: connect with that session.
    ConnectWithSession(String, crake_proto::Session),
    /// This gateway only let Crake in introducing itself so (gateway, user agent).
    ActsAs(String, String),
    /// The user said "Got it" to a notice (its key, e.g. "anyconnect:acme").
    Dismiss(String),
    /// Open Settings (or bring it to the front).
    Settings,
    /// A craked setting changed in the Settings window (key, text form).
    Setting(String, String),
    ClearLog,
    /// Show or hide the bird in the top bar.
    ShowBird(bool),
    /// Send hosts through another tunnel that answered too (from, to, hosts).
    Move(String, String, Vec<String>),
    /// "Ask me first": send these hosts through the tunnel.
    Send(String, Vec<String>),
    /// Open New tunnel, or the same window to edit one (its name).
    NewTunnel(Option<String>),
    /// From New tunnel: add one (fields), maybe with a certificate to keep
    /// in the keyring, and switch it on.
    AddTunnel(std::collections::HashMap<String, String>, Option<Vec<u8>>),
    /// From New tunnel when editing: change one.
    UpdateTunnel(
        String,
        std::collections::HashMap<String, String>,
        Option<Vec<u8>>,
    ),
    /// Settings → Tunnels: remove one (and its keyring items).
    RemoveTunnel(String),
    /// Keep these hosts or networks on a tunnel (the gateway's suggestions).
    Pin(String, Vec<String>),
    /// OpenVPN 3: the session connected (its device and DNS, and its
    /// control: `Reply::Cancel` disconnects it).
    Attach(
        String,
        std::collections::HashMap<String, String>,
        async_channel::Sender<signin::Reply>,
    ),
    /// OpenVPN 3: the session ended (why, if it dropped).
    Detached(String, Option<String>),
    /// Open tunnel detail for this tunnel (or bring it to the front).
    OpenDetail(String),
    /// Tunnel detail: ask craked for the tunnel's hosts and traffic again.
    Detail(String),
    Unpin(String, String),
    Forget(String, String),
    Quit,
}

struct Shell {
    model: Entity<Model>,
    commands: async_channel::Sender<Command>,
    requests: async_channel::Sender<Request>,
    panel: Option<AnyWindowHandle>,
    /// When the panel last closed itself on losing focus. A click on the tray
    /// icon takes focus first, so a click right after that means "close".
    panel_blurred: Option<std::time::Instant>,
    /// Where the tray icon was last clicked: the panel opens there.
    last_click: Option<(i32, i32)>,
    /// The panel's height as last measured, so it opens at the right size.
    panel_height: Option<f32>,
    /// Running through XWayland, where the panel can be placed at the icon.
    x11: bool,
    /// CRAKE_DEMO: the design's sample data, no craked.
    demo: bool,
    tray: Option<ksni::blocking::Handle<tray::Tray>>,
    /// The log window, while it's open.
    log: Option<AnyWindowHandle>,
    /// The Settings window, while it's open.
    settings: Option<AnyWindowHandle>,
    /// Tunnel detail, while it's open (and for which tunnel).
    detail: Option<(String, AnyWindowHandle)>,
    /// Started at login: switch on the tunnels left on, once craked is reached.
    autostart_pending: bool,
    /// A tunnel being added (New tunnel): its certificate for the keyring,
    /// kept until craked says its name.
    adding: Option<Option<Vec<u8>>>,
    /// The New tunnel window, while it's open.
    new_tunnel: Option<AnyWindowHandle>,
    /// OpenVPN 3 sessions this app runs: their controls, by tunnel.
    sessions: std::collections::HashMap<String, async_channel::Sender<signin::Reply>>,
    /// OpenVPN 3 sessions started outside Crake, being taken over.
    adopting: std::collections::HashSet<String>,
    /// The tray state last shown, and the animation running for it.
    tray_shape: tray::Shape,
    tray_motion: Option<gpui_kit::Task<()>>,
    /// The open panel's motion; a refit keeps it, so nothing replays.
    panel_motion: Option<std::rc::Rc<panel::Motion>>,
}

impl Global for Shell {}

/// Crake's own icons (bundled); everything else from gpui-kit.
struct Assets;

impl gpui_kit::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        macro_rules! icons {
            ($($name:literal),* $(,)?) => {
                match path {
                    $(concat!("crake/", $name) => {
                        return Ok(Some(std::borrow::Cow::Borrowed(include_bytes!(concat!("../../../assets/icons/", $name)))));
                    })*
                    _ => {}
                }
            };
        }
        icons!(
            "bird.svg",
            "call.svg",
            "mark-routed.svg",
            "mark-left-alone.svg",
            "mark-undone.svg",
            "mark-pinned.svg",
            "person.svg",
            "shield.svg",
            "block.svg",
            "lock.svg",
            "settings.svg",
            "dots.svg",
            "circle.svg",
            "check.svg",
            "cross.svg",
            "mark-system.svg",
            "search.svg",
            "close.svg",
            "chevron-down.svg",
            "chevron-left.svg",
            "chevron-right.svg",
            "mark-gateway.svg",
            "sliders.svg",
            "grip.svg",
            "bird-thin.svg",
            "call-thin.svg",
            "section-general.svg",
            "section-learning.svg",
            "section-names.svg",
            "section-privacy.svg",
            "section-managed.svg",
            "section-about.svg",
            "help.svg",
            "trash.svg",
            "section-tunnels.svg",
            "disclose-right.svg",
            "disclose-down.svg",
            "broom.svg",
        );
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<gpui_kit::SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}

fn main() {
    // GNOME on Wayland doesn't let an app place its windows; through XWayland
    // the panel can open right at the tray icon. CRAKE_WAYLAND=1 keeps Wayland
    // (the panel then goes where the desktop puts it).
    let x11 = std::env::var_os("CRAKE_WAYLAND").is_none() && std::env::var_os("DISPLAY").is_some();
    if x11 {
        // SAFETY: first thing in main, before any other thread exists.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
    }
    gpui_kit::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            // A tray app: closing the panel (its only window) mustn't end it.
            cx.set_quit_mode(gpui_kit::QuitMode::Explicit);
            // GNOME's "animations" off: springs land at once too.
            cx.set_reduce_motion(!motion::enabled());
            theme::fonts::load(cx);
            theme::Theme::current(cx).sync_components(cx);
            // Demo mode remembers nothing; CRAKE_DEMO=anyconnect shows the
            // "acts as AnyConnect" notice under Globex.
            cx.set_global(match std::env::var("CRAKE_DEMO").as_deref() {
                Ok("anyconnect") => prefs::Prefs {
                    user_agents: [("vpn.globex.example".into(), signin::ANYCONNECT.into())].into(),
                    scratch: true,
                    ..Default::default()
                },
                // CRAKE_APPEARANCE=light|dark (development): pin the theme.
                Ok(_) => prefs::Prefs {
                    scratch: true,
                    appearance: std::env::var("CRAKE_APPEARANCE")
                        .unwrap_or_else(|_| "system".into()),
                    ..Default::default()
                },
                Err(_) => prefs::Prefs::load(),
            });

            // CRAKE_DEMO=1: the design's sample, no craked needed.
            let demo = std::env::var_os("CRAKE_DEMO").is_some();
            let model = cx.new(|_| {
                if std::env::var("CRAKE_DEMO").as_deref() == Ok("log") {
                    model::demo_log()
                } else if demo
                    && std::env::var("CRAKE_SHOW").is_ok_and(|v| v.starts_with("detail:"))
                {
                    model::demo_detail()
                } else if demo {
                    model::demo()
                } else {
                    Model::default()
                }
            });
            let (requests, incoming_requests) = async_channel::unbounded();
            // One Crake per session (not in demo mode, so a demo can run
            // beside the real one): a second start opens this one's panel.
            if !demo {
                let again = requests.clone();
                if single::claim(move || {
                    again.send_blocking(Request::Panel(None)).ok();
                }) {
                    std::process::exit(0);
                }
            }
            let (updates, incoming_updates) = async_channel::unbounded();
            let commands = if demo {
                async_channel::unbounded().0
            } else {
                client::start(updates)
            };
            // Settings: "Show the bird in the top bar".
            let tray = if cx.global::<prefs::Prefs>().show_bird {
                tray::start(requests.clone())
            } else {
                None
            };
            cx.set_global(Shell {
                model,
                commands,
                requests,
                panel: None,
                panel_blurred: None,
                last_click: None,
                panel_height: None,
                x11,
                demo,
                tray,
                log: None,
                settings: None,
                detail: None,
                autostart_pending: std::env::args().any(|a| a == autostart::FLAG),
                adding: None,
                new_tunnel: None,
                sessions: Default::default(),
                adopting: Default::default(),
                tray_shape: tray::Shape::Off,
                tray_motion: None,
                panel_motion: None,
            });
            // The tray follows the model, whatever changed it: craked, or a
            // sign-in in this app.
            let model = cx.global::<Shell>().model.clone();
            cx.observe(&model, |_, cx| refresh_tray(cx)).detach();
            if !demo {
                watch_openvpn3(cx);
            }

            cx.spawn(async move |cx| {
                while let Ok(update) = incoming_updates.recv().await {
                    cx.update(|cx| on_update(update, cx));
                }
            })
            .detach();
            cx.spawn(async move |cx| {
                while let Ok(request) = incoming_requests.recv().await {
                    cx.update(|cx| on_request(request, cx));
                }
            })
            .detach();

            debug("ready; the panel opens from the tray icon");
            // CRAKE_SHOW=detail:<tunnel> (development): open tunnel detail.
            if let Some(t) = std::env::var("CRAKE_SHOW")
                .ok()
                .and_then(|v| v.strip_prefix("detail:").map(str::to_owned))
            {
                open_detail(t, cx);
            }
            // CRAKE_SHOW=settings[:<section>] (development): open Settings at start.
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v.starts_with("settings")) {
                open_settings(cx);
            }
            // CRAKE_SHOW=newtunnel[:<step>] (development): open New tunnel.
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v.starts_with("newtunnel")) {
                open_new_tunnel(None, cx);
            }
            // CRAKE_SHOW=panel (development): open the panel at start.
            if std::env::var("CRAKE_SHOW").as_deref() == Ok("panel") {
                toggle_panel(None, cx);
            }
            // CRAKE_DEMO=arrive (development): a decision every 8 s, to watch
            // rows arrive, ticks grow and the tray call.
            if std::env::var("CRAKE_DEMO").as_deref() == Ok("arrive") {
                cx.spawn(async move |cx| {
                    for n in 1.. {
                        cx.background_executor().timer(Duration::from_secs(8)).await;
                        let entry = crake_proto::LogEntry {
                            ts: jiff::Timestamp::now().to_string(),
                            kind: crake_proto::LogKind::Routed,
                            tunnel: Some("acme".into()),
                            message: format!("Sent build-{n}.acme.internal through ACME."),
                            reason: Some("Only ACME\u{2019}s DNS knows it.".into()),
                            hosts: vec![format!("10.40.9.{n}")],
                            others: Vec::new(),
                        };
                        cx.update(|cx| {
                            on_update(client::Update::Appended(entry), cx);
                            // And the last tunnel's switch flips, to watch it slide.
                            let mut tunnels = cx.global::<Shell>().model.read(cx).tunnels.clone();
                            if let Some(t) = tunnels.last_mut() {
                                t.state = if t.state == "off" { "connected" } else { "off" }.into();
                            }
                            on_update(client::Update::Tunnels(tunnels), cx);
                        });
                    }
                })
                .detach();
            }
            // CRAKE_SHOW=log (development): open the log at start.
            // CRAKE_SHOW=log@540-660: zoomed to those minutes.
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v == "log" || v.starts_with("log@")) {
                open_log(cx);
            }
            // CRAKE_SHOW=signin:<tunnel> (development): open that window at start.
            if let Some(name) = std::env::var("CRAKE_SHOW")
                .ok()
                .and_then(|v| v.strip_prefix("signin:").map(str::to_owned))
            {
                let tunnel = cx
                    .global::<Shell>()
                    .model
                    .read(cx)
                    .tunnels
                    .iter()
                    .find(|t| t.name == name)
                    .cloned();
                if let Some(tunnel) = tunnel {
                    open_sign_in(tunnel, cx);
                }
            }
        });
}

fn on_update(update: client::Update, cx: &mut App) {
    // craked refused an OpenVPN 3 session (it clashes with a tunnel that's
    // up; the log says which): end it, so it doesn't stay up unrouted.
    if let client::Update::AttachRefused(t) = &update
        && let Some(control) = cx.global_mut::<Shell>().sessions.remove(t)
    {
        control.send_blocking(signin::Reply::Cancel).ok();
    }
    // New tunnel's "Add and connect": craked named it; keep its
    // certificate, then switch it on (sign-in opens).
    if let client::Update::Added(name) = &update
        && let Some(certificate) = cx.global_mut::<Shell>().adding.take()
    {
        if let Some(bytes) = certificate {
            keep_certificate(name, &Default::default(), bytes, cx);
        }
        let requests = cx.global::<Shell>().requests.clone();
        let name = name.clone();
        // After the tunnel list has caught up (TunnelChanged follows).
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            requests.send(Request::Switch(name, true)).await.ok();
        })
        .detach();
    }
    let shell = cx.global::<Shell>();
    let (model, requests) = (shell.model.clone(), shell.requests.clone());
    let appended = model.update(cx, |m, cx| {
        let new = m.apply(update);
        cx.notify();
        new
    });
    if let Some(entry) = appended {
        let how = cx.global::<prefs::Prefs>().notify.clone();
        if notify::worth_telling(&entry) && how != "none" {
            notify::routed(&entry, requests.clone());
            if how == "crex" {
                notify::crex();
            }
        }
        // "Ask me first": the user decides (with the call, if they like).
        if entry.kind == crake_proto::LogKind::Asked {
            let display = model.read(cx).display_of(entry.tunnel.as_deref());
            notify::asked(&entry, display, requests.clone());
            if how == "crex" {
                notify::crex();
            }
        }
        // "When two tunnels answer: Ask me".
        if !entry.others.is_empty() {
            let others = model
                .read(cx)
                .tunnels
                .iter()
                .filter(|t| entry.others.contains(&t.name))
                .map(|t| (t.name.clone(), t.display.clone()))
                .collect();
            notify::also_answered(&entry, others, requests);
        }
        // The tray's "routed" shape is shown for a few seconds.
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            cx.update(refresh_tray);
        })
        .detach();
    }
    start_left_on(cx);
    refresh_tray(cx);
}

/// A certificate chosen in New tunnel goes to the keyring (7e): the file
/// can then be deleted; sign-in writes a private copy for each use.
fn keep_certificate(
    tunnel: &str,
    fields: &std::collections::HashMap<String, String>,
    bytes: Vec<u8>,
    cx: &mut App,
) {
    if cx.global::<Shell>().demo {
        return;
    }
    let (tunnel, display) = (
        tunnel.to_owned(),
        fields
            .get("display_name")
            .cloned()
            .unwrap_or_else(|| tunnel.to_owned()),
    );
    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            let _ = rt.block_on(keyring::set(
                &tunnel,
                &display,
                keyring::Kind::Certificate,
                &bytes,
            ));
        }
    });
}

/// Keep which tunnels the user left on, for "Start when you log in".
fn remember_on(tunnel: &str, on: bool, cx: &mut App) {
    if cx.global::<Shell>().demo {
        return;
    }
    let prefs = cx.global_mut::<prefs::Prefs>();
    let changed = if on {
        prefs.left_on.insert(tunnel.to_owned())
    } else {
        prefs.left_on.remove(tunnel)
    };
    if changed {
        prefs.save();
    }
}

/// Started at login: once craked has said which tunnels it has, switch on
/// the ones the user left on (sign-in opens where one is needed).
fn start_left_on(cx: &mut App) {
    let shell = cx.global::<Shell>();
    if !shell.autostart_pending {
        return;
    }
    // Settings: "Switch on the tunnels you left on" (off by default).
    if !cx.global::<prefs::Prefs>().reconnect {
        cx.global_mut::<Shell>().autostart_pending = false;
        return;
    }
    let model = shell.model.read(cx);
    if model.reachable != Some(true) || model.tunnels.is_empty() {
        return;
    }
    let left_on = &cx.global::<prefs::Prefs>().left_on;
    let off: Vec<String> = model
        .tunnels
        .iter()
        .filter(|t| t.state == "off" && left_on.contains(&t.name))
        .map(|t| t.name.clone())
        .collect();
    let requests = shell.requests.clone();
    cx.global_mut::<Shell>().autostart_pending = false;
    for name in off {
        requests.send_blocking(Request::Switch(name, true)).ok();
    }
}

/// OpenVPN 3 sessions of Crake's tunnels that were started elsewhere
/// (openvpn3's indicator or command line, or Crake before a restart): once
/// connected, Crake takes them over, routes through them, and switching
/// the tunnel off ends them.
fn watch_openvpn3(cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            // Crake's OpenVPN 3 tunnels it isn't already following.
            let wanted = cx.update(|cx| {
                let shell = cx.global::<Shell>();
                let m = shell.model.read(cx);
                m.tunnels
                    .iter()
                    .filter(|t| t.protocol == "openvpn3")
                    .filter(|t| {
                        !shell.sessions.contains_key(&t.name)
                            && !shell.adopting.contains(&t.name)
                            && !m.signing_in.contains(&t.name)
                    })
                    .map(|t| (t.name.clone(), t.gateway.clone()))
                    .collect::<Vec<_>>()
            });
            if wanted.is_empty() {
                continue;
            }
            let running = cx
                .background_executor()
                .spawn(async { ovpn3::connected() })
                .await;
            cx.update(|cx| {
                for (tunnel, profile) in wanted {
                    let Some((_, path)) = running.iter().find(|(p, _)| *p == profile) else {
                        continue;
                    };
                    let shell = cx.global::<Shell>();
                    // Checked again: a sign-in may have started meanwhile.
                    if shell.sessions.contains_key(&tunnel)
                        || shell.adopting.contains(&tunnel)
                        || shell.model.read(cx).signing_in.contains(&tunnel)
                    {
                        continue;
                    }
                    let requests = shell.requests.clone();
                    cx.global_mut::<Shell>().adopting.insert(tunnel.clone());
                    ovpn3::adopt(tunnel, path.clone(), requests);
                }
            });
        }
    })
    .detach();
}

fn refresh_tray(cx: &mut App) {
    let shell = cx.global::<Shell>();
    let m = shell.model.read(cx);
    let (shape, summary, tunnels) = (m.shape(), m.summary(), m.menu_tunnels());
    let before = shell.tray_shape;
    if let Some(tray) = &shell.tray {
        tray.update(move |t| {
            t.shape = shape;
            t.summary = summary;
            t.tunnels = tunnels;
        });
    }
    if before != shape {
        cx.global_mut::<Shell>().tray_shape = shape;
        animate_tray(before, shape, cx);
    }
}

/// The tray icon's motion from one state to the next (DESIGN.md, Tray icon):
/// a short run of frames, or, while connecting, the outline drawing itself
/// over and over. A newer change replaces a running animation.
fn animate_tray(from: tray::Shape, to: tray::Shape, cx: &mut App) {
    let Some(handle) = cx.global::<Shell>().tray.clone() else {
        return;
    };
    if !motion::enabled() {
        handle.update(|t| t.frame = None);
        cx.global_mut::<Shell>().tray_motion = None;
        return;
    }
    let looping = to == tray::Shape::Connecting;
    let frames: Vec<ksni::Icon> = if looping {
        tray::connecting_loop()
            .iter()
            .map(tray::Art::icon)
            .collect()
    } else {
        tray::transition(from, to)
            .iter()
            .map(tray::Art::icon)
            .collect()
    };
    let task = cx.spawn(async move |cx| {
        loop {
            for frame in &frames {
                let frame = frame.clone();
                handle.update(move |t| t.frame = Some(frame));
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(
                        70 * u64::from(motion::slowed()),
                    ))
                    .await;
            }
            if !looping || frames.is_empty() {
                break;
            }
        }
        handle.update(|t| t.frame = None);
    });
    // Dropping the previous task stops its animation.
    cx.global_mut::<Shell>().tray_motion = Some(task);
}

/// With CRAKE_DEBUG set: a line on stderr.
pub fn debug(msg: &str) {
    if std::env::var_os("CRAKE_DEBUG").is_some() {
        eprintln!("crake: {msg}");
    }
}

fn on_request(request: Request, cx: &mut App) {
    debug(&format!("request: {request:?}"));
    let commands = cx.global::<Shell>().commands.clone();
    let send = |c: Command| {
        commands.send_blocking(c).ok();
    };
    match request {
        Request::Panel(at) => toggle_panel(at, cx),
        Request::Switch(t, true) => {
            // It can't be up together with a tunnel that's on: say so
            // before signing in, not after.
            let shell = cx.global::<Shell>();
            let model = shell.model.clone();
            let clash = model
                .read(cx)
                .tunnels
                .iter()
                .find(|x| x.name == t)
                .map(|x| x.clash.clone())
                .filter(|c| !c.is_empty());
            if let Some(why) = clash {
                model.update(cx, |m, cx| {
                    m.failed = Some((why, std::time::Instant::now()));
                    cx.notify();
                });
                return;
            }
            remember_on(&t, true, cx);
            // Without a kept sign-in, sign in first (in this session, as the user).
            let tunnel = cx
                .global::<Shell>()
                .model
                .read(cx)
                .tunnels
                .iter()
                .find(|x| x.name == t)
                .cloned();
            match tunnel {
                Some(tunnel) if !tunnel.signed_in => open_sign_in(tunnel, cx),
                _ => send(Command::Connect(t)),
            }
        }
        Request::ConnectWithSession(t, session) => {
            if cx.global::<Shell>().demo {
                // No craked in demo mode: the tunnel just comes up.
                let model = cx.global::<Shell>().model.clone();
                model.update(cx, |m, cx| {
                    if let Some(x) = m.tunnels.iter_mut().find(|x| x.name == t) {
                        x.state = "connected".into();
                        x.signed_in = true;
                    }
                    cx.notify();
                });
            } else {
                send(Command::ConnectWithSession(t, session));
            }
        }
        Request::Switch(t, false) => {
            remember_on(&t, false, cx);
            // An OpenVPN 3 session is the app's: end it (it then detaches).
            let openvpn3 = cx
                .global::<Shell>()
                .model
                .read(cx)
                .tunnels
                .iter()
                .any(|x| x.name == t && x.protocol == "openvpn3");
            match cx.global::<Shell>().sessions.get(&t) {
                Some(control) => {
                    control.send_blocking(signin::Reply::Cancel).ok();
                }
                // Started before this app did: at least stop routing it.
                None if openvpn3 => send(Command::Detach(t, String::new())),
                None => send(Command::Disconnect(t)),
            }
        }
        Request::Undo(t, hosts) => send(Command::Undo(t, hosts)),
        Request::Redo(t, hosts) => send(Command::Redo(t, hosts)),
        Request::Log => open_log(cx),
        Request::ActsAs(gateway, user_agent) => {
            let prefs = cx.global_mut::<prefs::Prefs>();
            prefs.user_agents.insert(gateway, user_agent);
            prefs.save();
        }
        Request::Dismiss(key) => {
            let prefs = cx.global_mut::<prefs::Prefs>();
            prefs.dismissed.insert(key);
            prefs.save();
        }
        Request::Settings => open_settings(cx),
        Request::Setting(key, value) => {
            if cx.global::<Shell>().demo {
                // No craked in demo mode: the window just shows the change.
                let model = cx.global::<Shell>().model.clone();
                model.update(cx, |m, cx| {
                    m.settings.insert(key, value);
                    cx.notify();
                });
            } else {
                send(Command::SetSetting(key, value));
            }
        }
        Request::ClearLog => send(Command::ClearLog),
        Request::ShowBird(on) => {
            let shell = cx.global_mut::<Shell>();
            if on && shell.tray.is_none() {
                shell.tray = tray::start(shell.requests.clone());
                refresh_tray(cx);
            } else if !on && let Some(tray) = shell.tray.take() {
                tray.shutdown();
            }
        }
        Request::Move(from, to, hosts) => send(Command::Move(from, to, hosts)),
        Request::Send(t, hosts) => send(Command::AddHosts(t, hosts)),
        Request::Pin(t, targets) => send(Command::Pin(t, targets)),
        Request::OpenDetail(t) => open_detail(t, cx),
        Request::Detail(t) => {
            if !cx.global::<Shell>().demo {
                send(Command::Hosts(t.clone()));
                send(Command::TunnelTraffic(t));
            }
        }
        Request::Unpin(t, x) => send(Command::Unpin(t, x)),
        Request::Forget(t, x) => send(Command::Forget(t, x)),
        Request::Attach(t, fields, control) => {
            let shell = cx.global_mut::<Shell>();
            shell.sessions.insert(t.clone(), control);
            shell.adopting.remove(&t);
            // craked's state takes over from the sign-in's "connecting".
            let model = shell.model.clone();
            model.update(cx, |m, cx| {
                m.signing_in(&t, false, true);
                cx.notify();
            });
            send(Command::Attach(t, fields));
        }
        Request::Detached(t, why) => {
            let shell = cx.global_mut::<Shell>();
            shell.sessions.remove(&t);
            shell.adopting.remove(&t);
            send(Command::Detach(t, why.unwrap_or_default()));
        }
        Request::NewTunnel(editing) => open_new_tunnel(editing, cx),
        Request::AddTunnel(fields, certificate) => {
            // Once craked has named it: keep the certificate, switch it on.
            cx.global_mut::<Shell>().adding = Some(certificate);
            send(Command::AddTunnel(fields));
        }
        Request::UpdateTunnel(name, fields, certificate) => {
            if let Some(bytes) = certificate {
                keep_certificate(&name, &fields, bytes, cx);
            }
            send(Command::UpdateTunnel(name, fields));
        }
        Request::RemoveTunnel(name) => {
            // Its keyring items go with it.
            if !cx.global::<Shell>().demo {
                let n = name.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    if let Ok(rt) = rt {
                        rt.block_on(async {
                            keyring::forget(&n, keyring::Kind::Certificate).await;
                            keyring::forget(&n, keyring::Kind::CertificatePassword).await;
                            keyring::forget(&n, keyring::Kind::Password).await;
                        });
                    }
                });
            }
            remember_on(&name, false, cx);
            send(Command::RemoveTunnel(name));
        }
        Request::Quit => cx.quit(),
    }
}

fn blurred_just_now(cx: &mut App) -> bool {
    let shell = cx.global_mut::<Shell>();
    let recent = shell
        .panel_blurred
        .is_some_and(|at| at.elapsed() < Duration::from_millis(500));
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
fn toggle_panel(at: Option<(i32, i32)>, cx: &mut App) {
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
fn open_panel(at: Option<(i32, i32)>, cx: &mut App) {
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
    // The real monitors from XRandR; GPUI's own list if that fails.
    let mut screens = if x11 { place::monitors() } else { Vec::new() };
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
    let placed = match (x11, at) {
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
            window_background: gpui_kit::WindowBackgroundAppearance::Transparent,
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
fn open_settings(cx: &mut App) {
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
fn open_detail(tunnel: String, cx: &mut App) {
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
    let height = place::monitors().first().map_or(900., |m| m.h - 120.);
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
fn open_new_tunnel(editing: Option<String>, cx: &mut App) {
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
    let height = place::monitors()
        .first()
        .map_or(new_tunnel::HEIGHT, |m| m.h - 120.);
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
fn open_sign_in(tunnel: client::Tunnel, cx: &mut App) {
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
    };
    let bounds = centered(signin_view::WIDTH, 360., cx);
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
fn centered(w: f32, h: f32, cx: &App) -> Bounds<gpui_kit::Pixels> {
    match place::monitors().first() {
        Some(m) => Bounds::new(
            gpui_kit::point(px(m.x + (m.w - w) / 2.), px(m.y + ((m.h - h) / 2.).max(0.))),
            size(px(w), px(h)),
        ),
        None => Bounds::centered(None, size(px(w), px(h)), cx),
    }
}

/// The full log: one window, brought back if it's already open.
fn open_log(cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().log
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let model = cx.global::<Shell>().model.clone();
    let height = place::monitors()
        .first()
        .map_or(860., |m| (m.h - 120.).min(900.));
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
