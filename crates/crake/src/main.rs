//! `crake`: Crake in the user session. Tray icon, the panel, notifications.
//! Everything Crake decides happens in craked; this shows it and passes on
//! what the user chooses.
// Unsafe code lives in crake-os alone (Windows and macOS follow).
#![cfg_attr(target_os = "linux", forbid(unsafe_code))]
// Windows: a windowed app, so no console window opens beside it (release
// builds; development builds keep the console for their debug lines).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod requests;
mod updates;
mod windows;

mod autostart;
// Browsers that look names up themselves (DNS over HTTPS).
mod browser_dns;

mod chart;
// Who draws the window buttons: macOS, or Crake.
mod chrome;

mod client;

mod controls;

mod detect;

mod engines;
mod focus;

mod gateway_cert;

mod head;

mod keyring;

mod log_view;

mod model;

mod motion;

mod new_tunnel;

mod notify;
// When nothing shows tray icons: what would bring the bird back.
mod no_tray;
mod no_tray_view;
mod tray_host;

mod openvpn;
mod ovpn3;

mod ovpn_file;

mod panel;

mod place;

mod prefs;
mod report;

mod service;

mod settings_view;

mod signin;

mod signin_view;

mod sys;
use sys::single;

mod theme;

mod full_name;

mod profile_editor;

mod timeline;
mod tray;

mod units;

mod versions;

/// CRAKE_DEMO=arrive: a decision this often, for hosts 1 to LAST_HOST.
const ARRIVE_EVERY: Duration = Duration::from_secs(8);
const LAST_HOST: u8 = 250;

// With craked over its socket, openconnect and nftables: Linux.
#[cfg(all(test, target_os = "linux"))]
mod e2e_openvpn;
#[cfg(all(test, target_os = "linux"))]
mod e2e_protocols;
// Real gateways through Cloudflare, on CI's Mac and Windows (CLAUDE.md, 7j).
#[cfg(all(test, target_os = "linux"))]
mod e2e_tests;
#[cfg(all(test, any(target_os = "macos", windows)))]
mod gateway_tests;
#[cfg(test)]
mod journey_kit;
// What the macOS package installs, read from packaging/macos: any system.
#[cfg(all(test, target_os = "linux"))]
mod journey_tests;
#[cfg(test)]
mod macos_packaging_tests;
// The app's side against a real craked: the Linux, Windows and macOS tests'.
#[cfg(test)]
mod test_app;
mod traffic_view;
mod tunnel_detail;
#[cfg(all(test, target_os = "linux"))]
mod wire_tests;
// With the installed craked service and openconnect: Windows.
#[cfg(all(test, windows))]
mod windows_journey;
// With craked as a process of its own, Homebrew's openconnect, and the CI
// gateway through Cloudflare: macOS.
#[cfg(all(test, target_os = "macos"))]
mod macos_journey;
#[cfg(all(test, windows))]
mod windows_tests;

// What the parts of the app call on each other.
use requests::{keep_certificate, on_request, start_left_on};
use updates::{on_update, refresh_tray, watch_openvpn3, watch_service, watch_versions};

/// How long a restarted Crake waits for the old one to quit.
const RESTART_WAIT: std::time::Duration = std::time::Duration::from_secs(10);
const RESTART_POLL: std::time::Duration = std::time::Duration::from_millis(100);
use windows::{
    open_detail, open_log, open_new_tunnel, open_profile_editor, open_settings, open_sign_in,
    open_traffic, panel_blurred, refit_panel, toggle_panel,
};

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
    /// Open the Traffic window (or bring it to the front).
    Traffic,
    /// The Traffic window: ask craked for the counters right now.
    Live,
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
    /// Show or hide the bird in the tray.
    ShowBird(bool),
    /// The tray came or went, or what would bring it back (`tray_host`).
    NoTray(tray_host::Event),
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
    /// OpenVPN 3: the session (its openvpn3 path) connected (its device
    /// and DNS, and its control: `Reply::Cancel` disconnects it).
    Attach(
        String,
        String,
        std::collections::HashMap<String, String>,
        async_channel::Sender<signin::Reply>,
    ),
    /// OpenVPN 3: the session (its openvpn3 path) ended (why, if it
    /// dropped).
    Detached(String, String, Option<String>),
    /// Open tunnel detail for this tunnel (or bring it to the front).
    OpenDetail(String),
    /// Tunnel detail: ask craked for the tunnel's hosts and traffic again.
    Detail(String),
    Unpin(String, String),
    Forget(String, String),
    /// Forget everything learned for a tunnel (tunnel detail's "Forget all…").
    ForgetLearned(String),
    /// A tunnel that stopped on its own, seen: Off (the panel's Dismiss).
    DismissTrouble(String),
    /// Tunnel detail's "Edit…": an OpenVPN 3 tunnel's profile, as text.
    EditProfile(String),
    /// Off and on again if it's on (its profile changed).
    Reconnect(String),
    /// An OpenVPN 3 session followed here is connected again after
    /// reconnecting: hand it to craked again (its device, as now).
    Reattach(String, std::collections::HashMap<String, String>),
    /// macOS: System Settings' Login Items & Extensions, where Crake's
    /// service is switched on (`service`).
    OpenServiceSettings,
    /// Start the updated Crake in this one's place (`versions`).
    Restart,
    /// Open a page in the browser (a new version's download page).
    OpenUrl(String),
    /// OpenVPN: the user's answer to question `id` of a tunnel's sign-in.
    Answer(String, u32, String),
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
    tray: Option<tray::Handle>,
    /// The log window, while it's open.
    log: Option<AnyWindowHandle>,
    /// The Traffic window, while it's open.
    traffic: Option<AnyWindowHandle>,
    /// The profile editor, with the tunnel it edits.
    profile_editor: Option<(String, AnyWindowHandle)>,
    /// The Settings window, while it's open.
    settings: Option<AnyWindowHandle>,
    /// Tunnel detail, while it's open (and for which tunnel).
    detail: Option<(String, AnyWindowHandle)>,
    /// Started at login: switch on the tunnels left on, once craked is reached.
    autostart_pending: bool,
    /// Started at login (--autostart): no tray is said in a notification.
    started_at_login: bool,
    /// A tunnel being added (New tunnel): its certificate for the keyring,
    /// kept until craked says its name.
    adding: Option<Option<Vec<u8>>>,
    /// The New tunnel window, while it's open.
    new_tunnel: Option<AnyWindowHandle>,
    /// OpenVPN 3 sessions this app runs, by tunnel: each one's openvpn3
    /// path and its control. One per tunnel: a newer one replaces it.
    sessions: std::collections::HashMap<String, (String, async_channel::Sender<signin::Reply>)>,
    /// OpenVPN 3 sessions started outside Crake, being taken over.
    adopting: std::collections::HashSet<String>,
    /// The tray state last shown, and the animation running for it.
    tray_shape: tray::Shape,
    tray_motion: Option<gpui_kit::Task<()>>,
    /// The open panel's motion; a refit keeps it, so nothing replays.
    panel_motion: Option<std::rc::Rc<panel::Motion>>,
    /// How many times a window asked for Crake to be the active app
    /// (`windows::to_front`); tests read it, as GPUI's test platform
    /// records no activation.
    activations: u32,
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
            "bird-grass.svg",
            "bird-alone.svg",
            "call.svg",
            "head-warning.svg",
            "warning-triangle.svg",
            "warning-mark.svg",
            "bird-badge-warning.svg",
            "bird-badge-trouble.svg",
            "bird-badge-mark.svg",
            "bird-badge-warning-mark.svg",
            "clipboard.svg",
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
            "chevron-up.svg",
            "chevron-left.svg",
            "chevron-right.svg",
            "mark-gateway.svg",
            "sliders.svg",
            "grip.svg",
            "section-general.svg",
            "section-learning.svg",
            "section-names.svg",
            "section-privacy.svg",
            "section-managed.svg",
            "section-about.svg",
            "help.svg",
            "trash.svg",
            "plus.svg",
            "section-tunnels.svg",
            "disclose-right.svg",
            "disclose-down.svg",
            "broom.svg",
            "section-diagnostics.svg",
            "file.svg",
        );
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<gpui_kit::SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}

fn main() {
    // `crake report`: the file, no window.
    if let Some(code) = report::cli(&std::env::args().skip(1).collect::<Vec<_>>()) {
        std::process::exit(code);
    }
    // GNOME on Wayland doesn't let an app place its windows; through XWayland
    // the panel can open right at the tray icon. CRAKE_WAYLAND=1 keeps Wayland
    // (the panel then goes where the desktop puts it).
    let x11 = std::env::var_os("CRAKE_WAYLAND").is_none() && std::env::var_os("DISPLAY").is_some();
    if x11 {
        // First thing in main, while there's one thread (crake-os checks).
        #[cfg(target_os = "linux")]
        {
            crake_os::linux::unset_env_while_alone("WAYLAND_DISPLAY");
            sys::keep_root_cursor();
        }
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
            let model = if std::env::var("CRAKE_DEMO").as_deref() == Ok("log") {
                model::demo_log()
            } else if demo && std::env::var("CRAKE_SHOW").is_ok_and(|v| v.starts_with("detail:")) {
                model::demo_detail()
            } else if std::env::var("CRAKE_DEMO").as_deref() == Ok("empty") {
                // No tunnel yet: the first-run panel.
                let mut m = model::demo();
                m.tunnels.clear();
                m
            } else if demo {
                model::demo()
            } else {
                Model::default()
            };
            // CRAKE_DEMO=expired: Nordbank's session ran out (the panel's
            // and tunnel detail's words for why a tunnel stopped).
            // CRAKE_DEMO=openvpn3: Nordbank is an OpenVPN 3 tunnel (its profile
            // editor's "Edit…" in tunnel detail).
            let model = match std::env::var("CRAKE_DEMO").as_deref() {
                Ok("expired") => model::demo_expired(model),
                Ok("openvpn3") => model::demo_openvpn3(model),
                Ok("leftover") => model::demo_leftover(model),
                // CRAKE_DEMO=staying: an OpenVPN 3 tunnel stays (locked).
                Ok("staying") => model::demo_staying(model),
                // CRAKE_DEMO=long: names that take two lines in the panel.
                Ok("long") => model::demo_long_names(model),
                _ => model,
            };
            let mut model = model;
            model.no_tray = std::env::var("CRAKE_DEMO")
                .ok()
                .as_deref()
                .and_then(no_tray::demo);
            let setup = Setup {
                model,
                x11,
                demo,
                connect: (!demo).then(|| Box::new(client::start) as Connect),
                // Settings: "Show the bird in the tray".
                tray: cx.global::<prefs::Prefs>().show_bird,
                single_instance: !demo,
                openvpn3: !demo,
                autostart: std::env::args().any(|a| a == autostart::FLAG),
            };
            if !start(cx, setup) {
                std::process::exit(0);
            }
            // An entry an older Crake wrote, as this one writes it.
            if !demo {
                autostart::refresh();
            }

            debug("ready; the panel opens from the tray icon");
            // CRAKE_SHOW=detail:<tunnel> (development): open tunnel detail.
            if let Some(t) = std::env::var("CRAKE_SHOW")
                .ok()
                .and_then(|v| v.strip_prefix("detail:").map(str::to_owned))
            {
                open_detail(t, cx);
            }
            // CRAKE_SHOW=profile:<tunnel> (development): its profile editor.
            if let Some(t) = std::env::var("CRAKE_SHOW")
                .ok()
                .and_then(|v| v.strip_prefix("profile:").map(str::to_owned))
            {
                open_profile_editor(t, cx);
            }
            // CRAKE_SHOW=settings[:<section>] (development): open Settings at start.
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v.starts_with("settings")) {
                open_settings(cx);
            }
            // CRAKE_SHOW=log[:saved] (development): open the log; ":saved",
            // with a report just saved (its row under the title).
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v == "log") {
                open_log(cx);
            }
            // CRAKE_SHOW=traffic (development): open the Traffic window.
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v == "traffic") {
                open_traffic(cx);
            }
            // CRAKE_SHOW=newtunnel[:<step>] (development): open New tunnel.
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v.starts_with("newtunnel")) {
                open_new_tunnel(None, cx);
            }
            // CRAKE_SHOW=panel (development): open the panel at start;
            // hover:<minutes ago> opens it pointing at its chart.
            if std::env::var("CRAKE_SHOW").is_ok_and(|v| v == "panel" || v.starts_with("hover:")) {
                toggle_panel(None, cx);
            }
            // CRAKE_DEMO=arrive (development): a decision every 8 s, to watch
            // rows arrive, ticks grow and the tray call.
            if std::env::var("CRAKE_DEMO").as_deref() == Ok("arrive") {
                cx.spawn(async move |cx| {
                    // 1 to 250, over and over: the hosts stay valid addresses.
                    for n in (1..=LAST_HOST).cycle() {
                        cx.background_executor().timer(ARRIVE_EVERY).await;
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

/// craked as the app reaches it: given where to send its updates, the
/// channel for commands (client::start; a test's own socket).
pub(crate) type Connect =
    Box<dyn FnOnce(async_channel::Sender<client::Update>) -> async_channel::Sender<Command>>;

/// What the app starts with: the real things (main), or a test's.
pub(crate) struct Setup {
    pub model: Model,
    /// Running through XWayland, where the panel can be placed at the icon.
    pub x11: bool,
    /// CRAKE_DEMO: the design's sample data, no craked.
    pub demo: bool,
    /// None: no craked (demo mode).
    pub connect: Option<Connect>,
    /// The tray icon (StatusNotifierItem on the session bus).
    pub tray: bool,
    /// One Crake per session: a second start opens the first one's panel.
    pub single_instance: bool,
    /// Follow and take over OpenVPN 3 sessions.
    pub openvpn3: bool,
    /// Started at login (--autostart).
    pub autostart: bool,
}

/// The app: its model, craked's client, the tray, and the loops that apply
/// craked's updates and the windows' requests. False when another Crake
/// is already running (it was told to open its panel).
pub(crate) fn start(cx: &mut App, setup: Setup) -> bool {
    let Setup {
        model,
        x11,
        demo,
        connect,
        tray,
        single_instance,
        openvpn3,
        autostart,
    } = setup;
    let model = cx.new(|_| model);
    let (requests, incoming_requests) = async_channel::unbounded();
    // One Crake per session (not in demo mode, so a demo can run
    // beside the real one): a second start opens this one's panel.
    if single_instance {
        // Started by Restart: the old Crake is quitting; wait for it.
        if std::env::var_os(versions::RESTARTING).is_some() {
            let since = std::time::Instant::now();
            while single::taken() && since.elapsed() < RESTART_WAIT {
                std::thread::sleep(RESTART_POLL);
            }
        }
        let again = requests.clone();
        if single::claim(move || {
            again.send_blocking(Request::Panel(None)).ok();
        }) {
            return false;
        }
    }
    let (updates, incoming_updates) = async_channel::unbounded();
    let commands = match connect {
        Some(connect) => connect(updates),
        None => async_channel::unbounded().0,
    };
    let tray = if tray {
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
        traffic: None,
        profile_editor: None,
        settings: None,
        detail: None,
        autostart_pending: autostart,
        started_at_login: autostart,
        adding: None,
        new_tunnel: None,
        sessions: Default::default(),
        adopting: Default::default(),
        tray_shape: tray::Shape::Off,
        tray_motion: None,
        panel_motion: None,
        activations: 0,
    });
    // The tray follows the model, whatever changed it: craked, or a
    // sign-in in this app.
    let model = cx.global::<Shell>().model.clone();
    cx.observe(&model, |_, cx| refresh_tray(cx)).detach();
    if openvpn3 {
        watch_openvpn3(cx);
        openvpn::move_from_openvpn3(cx);
        watch_service(cx);
        watch_versions(cx);
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
    true
}

/// With CRAKE_DEBUG set: a line on stderr.
pub fn debug(msg: &str) {
    if std::env::var_os("CRAKE_DEBUG").is_some() {
        eprintln!("crake: {msg}");
    }
}
