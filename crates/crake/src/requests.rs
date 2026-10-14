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

pub(super) fn on_request(request: Request, cx: &mut App) {
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
                Some((_, control)) => {
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
        Request::ForgetLearned(t) => send(Command::ForgetLearned(t)),
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
                });
            }
            remember_on(&name, false, cx);
            send(Command::RemoveTunnel(name));
        }
        Request::OpenServiceSettings => crate::sys::service::open_settings(),
        Request::Quit => cx.quit(),
        Request::Restart => restart(cx),
        Request::OpenUrl(url) => cx.open_url(&url),
    }
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
