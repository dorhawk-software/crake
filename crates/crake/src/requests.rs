//! What the windows ask for, done: switching tunnels, undo, settings, new
//! tunnels, OpenVPN 3 sessions.

use super::*;

/// A certificate chosen in New tunnel goes to the keyring (7e): the file
/// can then be deleted; sign-in writes a private copy for each use.
pub(super) fn keep_certificate(
    tunnel: &str,
    fields: &std::collections::HashMap<String, String>,
    bytes: Vec<u8>,
    cx: &mut App,
) {
    if cx.global::<Shell>().demo {
        return;
    }
    // Tests keep it where their keyring is (memory), as the sign-in reads it.
    #[cfg(test)]
    {
        let _ = fields;
        crate::openvpn::keep_certificate_for_tests(tunnel, &bytes);
    }
    #[cfg(not(test))]
    {
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
}

/// Keep which tunnels the user left on, for "Start when you log in".
pub(super) fn remember_on(tunnel: &str, on: bool, cx: &mut App) {
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
pub(super) fn start_left_on(cx: &mut App) {
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

/// Settings' "Show the bird in the tray".
fn show_bird(on: bool, cx: &mut App) {
    let shell = cx.global_mut::<Shell>();
    if on && shell.tray.is_none() {
        // Switched on in Settings: its row says what's missing.
        crate::tray_host::said_at_start();
        shell.tray = tray::start(shell.requests.clone());
        refresh_tray(cx);
    } else if !on && let Some(tray) = shell.tray.take() {
        tray.shutdown();
        // No bird wanted: nothing is missing.
        crate::tray_host::handle(crate::tray_host::Event::Host(true), cx);
    }
}

/// Switch a tunnel on: said at once if it clashes with one that's up;
/// signed in first without a kept sign-in (OpenVPN: craked asks).
fn switch_on(t: String, cx: &mut App) {
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
    // An OpenVPN or IPsec tunnel just connects: craked asks if it must.
    match tunnel {
        Some(tunnel) if !tunnel.signed_in && !crate::signin::craked_asks(&tunnel.protocol) => {
            open_sign_in(tunnel, cx)
        }
        _ => {
            let commands = cx.global::<Shell>().commands.clone();
            commands.send_blocking(Command::Connect(t)).ok();
        }
    }
}

pub(super) fn on_request(request: Request, cx: &mut App) {
    debug(&format!("request: {request:?}"));
    let commands = cx.global::<Shell>().commands.clone();
    let send = |c: Command| {
        commands.send_blocking(c).ok();
    };
    match request {
        Request::Panel(at) => toggle_panel(at, cx),
        Request::Switch(t, true) => switch_on(t, cx),
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
                Some((_, control)) => {
                    // craked first, then the session: ended first, its
                    // device went before craked knew it was meant
                    // ("stopped unexpectedly", 2026-10-16).
                    if openvpn3 {
                        send(Command::Detach(t.clone(), String::new()));
                    }
                    control.send_blocking(signin::Reply::Cancel).ok();
                }
                // Started before this app did: at least stop routing it.
                None if openvpn3 => send(Command::Detach(t, String::new())),
                None => send(Command::Disconnect(t)),
            }
        }
        Request::Answer(t, id, text) => send(Command::Answer(t, id, text)),
        Request::Undo(t, hosts) => undo(t, hosts, cx),
        Request::Redo(t, hosts) => send(Command::Redo(t, hosts)),
        Request::Log => open_log(cx),
        Request::Traffic => open_traffic(cx),
        Request::EditProfile(t) => open_profile_editor(t, cx),
        Request::Reconnect(t) => reconnect(t, cx),
        Request::Live => {
            if cx.global::<Shell>().demo {
                // No craked: the proposal's sample, moving.
                let second = demo_clock().elapsed().as_secs() + DEMO_HEAD_START;
                let model = cx.global::<Shell>().model.clone();
                model.update(cx, |m, cx| {
                    let live = std::sync::Arc::make_mut(&mut m.live);
                    let now = std::time::Instant::now();
                    // The strip under the graph starts full: the sample's
                    // last five minutes, a look every few seconds.
                    if live.history.is_empty() {
                        for ago in (1..=DEMO_HEAD_START).rev().step_by(DEMO_LOOK_EVERY) {
                            if let Some(at) = now.checked_sub(std::time::Duration::from_secs(ago)) {
                                live.add(crate::model::live::demo(second - ago), at);
                            }
                        }
                    }
                    live.add(crate::model::live::demo(second), now);
                    // Opened at different times, as real connections are.
                    let mut keys: Vec<_> = live.first_seen.keys().cloned().collect();
                    keys.sort();
                    for (i, k) in keys.into_iter().enumerate() {
                        if let Some(at) = now.checked_sub(DEMO_OPEN_FOR[i % DEMO_OPEN_FOR.len()]) {
                            live.first_seen.insert(k, at);
                        }
                    }
                    cx.notify();
                });
            } else {
                send(Command::Live);
            }
        }
        Request::ActsAs(gateway, user_agent) => {
            let prefs = cx.global_mut::<prefs::Prefs>();
            prefs.user_agents.insert(gateway, user_agent);
            prefs.save();
        }
        Request::Dismiss(key) => dismiss(key, cx),
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
        Request::ShowBird(on) => show_bird(on, cx),
        Request::NoTray(event) => crate::tray_host::handle(event, cx),
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
        Request::ForgetLearned(t) => send(Command::ForgetLearned(t)),
        Request::DismissTrouble(t) => dismiss_trouble(t, cx),
        Request::Reattach(t, fields) => send(Command::Attach(t, fields)),
        Request::Attach(t, path, fields, control) => {
            let shell = cx.global_mut::<Shell>();
            // One session per tunnel: a newer one (openvpn3's indicator
            // reconnecting, say) replaces the one followed so far, which
            // ends; when its follower says so, that changes nothing here.
            if let Some(old) = follow_session(&mut shell.sessions, &t, path, control) {
                old.send_blocking(signin::Reply::Cancel).ok();
            }
            shell.adopting.remove(&t);
            // craked's state takes over from the sign-in's "connecting".
            let model = shell.model.clone();
            model.update(cx, |m, cx| {
                m.signing_in(&t, false, true);
                cx.notify();
            });
            send(Command::Attach(t, fields));
        }
        Request::Detached(t, path, why) => {
            let shell = cx.global_mut::<Shell>();
            // Only the session the tunnel has now: one a newer session
            // replaced ended on its own, and must not take that one down.
            if !session_ended(&mut shell.sessions, &t, &path) {
                return;
            }
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
                    crate::openvpn::forget(&n);
                });
            }
            remember_on(&name, false, cx);
            send(Command::RemoveTunnel(name));
        }
        Request::OpenServiceSettings => crate::sys::service::open_settings(),
        Request::Quit => cx.quit(),
        Request::Restart => restart(cx),
        Request::OpenUrl(url) => cx.open_url(&url),
        Request::WhatsNew => {
            // From the panel's notice: since the version seen before.
            let prefs = cx.global::<prefs::Prefs>();
            let since = crate::whats_new::notice(prefs).and(prefs.seen_version.clone());
            seen_version(cx);
            crate::whats_new::open(since, cx);
        }
        Request::SeenVersion => seen_version(cx),
    }
}

/// This version's What's new seen: the panel's notice goes.
fn seen_version(cx: &mut App) {
    let prefs = cx.global_mut::<prefs::Prefs>();
    crate::whats_new::mark_seen(prefs);
    prefs.save();
    let model = cx.global::<Shell>().model.clone();
    model.update(cx, |_, cx| cx.notify());
}

/// Restart: the updated binary is started (told to wait for this one),
/// then this one quits. Tunnels stay up: craked keeps them, and OpenVPN 3
/// sessions are taken over again (`watch_openvpn3`).
fn restart(cx: &mut App) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = std::process::Command::new(crate::versions::running_exe(exe));
    cmd.env(crate::versions::RESTARTING, "1");
    crate::sys::no_console(&mut cmd);
    if cmd.spawn().is_ok() {
        cx.quit();
    }
}

/// CRAKE_DEMO's Traffic window: the sample's clock, started on first use,
/// a few minutes in so the strip under the graph isn't empty.
fn demo_clock() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}
const DEMO_HEAD_START: u64 = 300;
// literal: the sample's connections, open for a dozen seconds to an hour
/// How long each of the demo's connections has been open.
const DEMO_OPEN_FOR: [std::time::Duration; 10] = [
    std::time::Duration::from_secs(12),
    std::time::Duration::from_secs(4 * 60),
    std::time::Duration::from_secs(3600),
    std::time::Duration::from_secs(2 * 60),
    std::time::Duration::from_secs(25 * 60),
    std::time::Duration::from_secs(7),
    std::time::Duration::from_secs(90),
    std::time::Duration::from_secs(15 * 60),
    std::time::Duration::from_secs(40),
    std::time::Duration::from_secs(6 * 60),
];
const DEMO_LOOK_EVERY: usize = 5;

/// Each tunnel's OpenVPN 3 session: its openvpn3 path and its control.
type Sessions = std::collections::HashMap<String, (String, async_channel::Sender<signin::Reply>)>;

/// `tunnel`'s session is now the one at `path`. A different one it had
/// before (openvpn3's indicator reconnecting, say) is to be ended: its
/// control is returned for that. The same session again just changes
/// hands: the earlier follower, its channel gone, stops quietly.
fn follow_session(
    sessions: &mut Sessions,
    tunnel: &str,
    path: String,
    control: async_channel::Sender<signin::Reply>,
) -> Option<async_channel::Sender<signin::Reply>> {
    let new = path.clone();
    let (old, old_control) = sessions.insert(tunnel.to_owned(), (path, control))?;
    (old != new).then_some(old_control)
}

/// The session at `path` ended: whether it was `tunnel`'s current one (and
/// so the tunnel is down). One a newer session replaced changes nothing.
fn session_ended(sessions: &mut Sessions, tunnel: &str, path: &str) -> bool {
    if sessions.get(tunnel).is_some_and(|(p, _)| p != path) {
        return false;
    }
    sessions.remove(tunnel);
    true
}

/// Send hosts direct and keep them so (the panel's Undo).
fn undo(tunnel: String, hosts: Vec<String>, cx: &mut App) {
    if !cx.global::<Shell>().demo {
        let _ = cx
            .global::<Shell>()
            .commands
            .send_blocking(Command::Undo(tunnel, hosts));
        return;
    }
    // No craked in demo mode: the decision it would log, as it says it.
    let model = cx.global::<Shell>().model.clone();
    model.update(cx, |m, cx| {
        let name = m
            .log
            .iter()
            .rev()
            .find(|e| e.hosts == hosts)
            .and_then(|e| {
                e.message
                    .strip_prefix("Sent ")?
                    .split(" through ")
                    .next()
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| hosts.join(", "));
        m.apply(crate::client::Update::Appended(crake_proto::LogEntry {
            ts: jiff::Timestamp::now().to_string(),
            kind: crake_proto::LogKind::Undone,
            tunnel: Some(tunnel),
            message: format!("{name} goes direct again."),
            reason: Some(
                "You undid the route. Crake won\u{2019}t learn this host again on its own.".into(),
            ),
            hosts,
            others: Vec::new(),
        }));
        cx.notify();
    });
}

/// A notice seen ("Got it", "Not now"): remembered, and a leftover OpenVPN 3
/// process's amber badge goes from the tray at once.
fn dismiss(key: String, cx: &mut App) {
    let prefs = cx.global_mut::<prefs::Prefs>();
    prefs.dismissed.insert(key.clone());
    prefs.save();
    let model = cx.global::<Shell>().model.clone();
    model.update(cx, |m, cx| {
        m.leftovers.retain(|l| l.dismiss_key() != key);
        cx.notify();
    });
}

/// A tunnel that stopped on its own, seen: Off, its reason kept.
fn dismiss_trouble(tunnel: String, cx: &mut App) {
    if cx.global::<Shell>().demo {
        // No craked in demo mode: the tunnel is just Off.
        let model = cx.global::<Shell>().model.clone();
        model.update(cx, |m, cx| {
            if let Some(x) = m.tunnels.iter_mut().find(|x| x.name == tunnel) {
                x.state = "off".into();
            }
            cx.notify();
        });
    } else {
        let _ = cx
            .global::<Shell>()
            .commands
            .send_blocking(Command::DismissTrouble(tunnel));
    }
}

/// Off, then on again once craked says it's off: a tunnel whose profile
/// changed (the profile editor's Save). Left alone when it isn't on.
fn reconnect(tunnel: String, cx: &mut App) {
    let model = cx.global::<Shell>().model.clone();
    let state = |m: &Model| {
        m.tunnels
            .iter()
            .find(|x| x.name == tunnel)
            .map(|x| x.state.clone())
            .unwrap_or_default()
    };
    if !matches!(state(model.read(cx)).as_str(), "connected" | "connecting") {
        return;
    }
    let requests = cx.global::<Shell>().requests.clone();
    let _ = requests.send_blocking(Request::Switch(tunnel.clone(), false));
    cx.spawn(async move |cx| {
        let end = std::time::Instant::now() + RECONNECT_WAIT;
        while std::time::Instant::now() < end {
            cx.background_executor().timer(RECONNECT_POLL).await;
            let off = model.read_with(cx, |m, _| {
                matches!(
                    m.tunnels
                        .iter()
                        .find(|x| x.name == tunnel)
                        .map(|x| x.state.as_str()),
                    Some("off" | "trouble")
                )
            });
            if off {
                let _ = requests.send(Request::Switch(tunnel, true)).await;
                return;
            }
        }
    })
    .detach();
}

/// How long a reconnect waits for the tunnel to go off, and how often it looks.
const RECONNECT_WAIT: std::time::Duration = std::time::Duration::from_secs(20);
const RECONNECT_POLL: std::time::Duration = std::time::Duration::from_millis(200);

#[cfg(test)]
mod session_tests {
    use super::*;

    #[test]
    fn a_replaced_session_cannot_take_its_successor_down() {
        let mut sessions = Sessions::new();
        let (first, _a) = async_channel::unbounded();
        let (second, _b) = async_channel::unbounded();
        assert!(follow_session(&mut sessions, "initech", "/s/1".into(), first).is_none());
        // A newer session: the older one is handed back, to be ended.
        assert!(follow_session(&mut sessions, "initech", "/s/2".into(), second).is_some());
        // The older one's end changes nothing; the current one's does.
        assert!(!session_ended(&mut sessions, "initech", "/s/1"));
        assert_eq!(sessions["initech"].0, "/s/2");
        assert!(session_ended(&mut sessions, "initech", "/s/2"));
        assert!(sessions.is_empty());
    }

    #[test]
    fn the_same_session_again_only_changes_hands() {
        let mut sessions = Sessions::new();
        let (first, _a) = async_channel::unbounded();
        let (again, _b) = async_channel::unbounded();
        follow_session(&mut sessions, "initech", "/s/1".into(), first);
        assert!(follow_session(&mut sessions, "initech", "/s/1".into(), again).is_none());
        assert_eq!(sessions["initech"].0, "/s/1");
    }
}
