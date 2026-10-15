//! What craked says, applied: the model, notifications, the tray, OpenVPN 3
//! sessions started elsewhere.

use super::*;

/// A tunnel switched on for a new connection waits for the tunnel list to
/// catch up; the tray's "routed" shape shows a few seconds; OpenVPN 3
/// sessions started elsewhere are looked for this often; the tray's frames
/// are FRAME_MS apart (CLAUDE.md, 7c).
const AFTER_TUNNELS: Duration = Duration::from_millis(400);
const ROUTED_SHOWN: Duration = Duration::from_secs(5);
const OPENVPN3_LOOK: Duration = Duration::from_secs(3);
/// How often macOS is asked whether Crake's service may run.
const SERVICE_LOOK: Duration = Duration::from_secs(2);
const FRAME_MS: u64 = 70;
/// After a change, how long until the bar is told about the icon again
/// (`tray::Handle::settle`): well past GNOME's tray extension's 30 ms
/// batching and its fetch of the image.
const SETTLE_MS: u64 = 300;

pub(super) fn on_update(update: client::Update, cx: &mut App) {
    // craked refused an OpenVPN 3 session (it clashes with a tunnel that's
    // up; the log says which): end it, so it doesn't stay up unrouted.
    if let client::Update::AttachRefused(t) = &update
        && let Some((_, control)) = cx.global_mut::<Shell>().sessions.remove(t)
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
            cx.background_executor().timer(AFTER_TUNNELS).await;
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
        // A tunnel stopped by itself: say why, offer to switch it back on.
        if entry.is_unexpected_stop() && how != "none" {
            notify::stopped(&entry, requests.clone());
        }
        // An OpenVPN 3 process left running: it needs the user (sudo).
        if entry.is_left_running() && how != "none" {
            notify::left_running(&entry);
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
            cx.background_executor().timer(ROUTED_SHOWN).await;
            cx.update(refresh_tray);
        })
        .detach();
    }
    start_left_on(cx);
    refresh_tray(cx);
}

/// macOS: whether Crake's service is registered and switched on (the app
/// registers it, SMAppService; macOS asks the user to switch it on once):
/// looked at every SERVICE_LOOK, and the panel told what to ask of the user.
/// Elsewhere the package runs it.
pub(super) fn watch_service(cx: &mut App) {
    if !cfg!(target_os = "macos") || cx.global::<Shell>().demo {
        return;
    }
    cx.spawn(async move |cx| {
        loop {
            let on = cx
                .background_executor()
                .spawn(async { crate::sys::service::on() })
                .await;
            cx.update(|cx| {
                if on == Some(true) && !cx.global::<prefs::Prefs>().service_was_on {
                    cx.global_mut::<prefs::Prefs>().service_was_on = true;
                    cx.global::<prefs::Prefs>().save();
                }
                let was_on = cx.global::<prefs::Prefs>().service_was_on;
                let wanted = crate::service::wanted(on, was_on);
                let model = cx.global::<Shell>().model.clone();
                model.update(cx, |m, cx| {
                    if m.service != wanted {
                        m.service = wanted;
                        cx.notify();
                    }
                });
            });
            cx.background_executor().timer(SERVICE_LOOK).await;
        }
    })
    .detach();
}

/// The newest Crake, asked of dorhawk.software once a day (`versions`),
/// where nothing else updates this install and the user hasn't switched
/// it off; what it said last is kept, so the notice outlives a restart.
pub(super) fn watch_versions(cx: &mut App) {
    if cx.global::<Shell>().demo {
        return;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    if crate::versions::updated_elsewhere(&exe, |p| std::path::Path::new(p).exists()) {
        return;
    }
    cx.spawn(async move |cx| {
        loop {
            let (on, at, known) = cx.update(|cx| {
                let p = cx.global::<prefs::Prefs>();
                (
                    p.check_versions,
                    p.versions_checked_at,
                    p.newest_version.clone(),
                )
            });
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let mut newest = known;
            if on && now.saturating_sub(at) >= crate::versions::EVERY {
                let asked = cx
                    .background_executor()
                    .spawn(async { crate::versions::fetch() })
                    .await;
                // Not answered (offline): asked again in an hour.
                if asked.is_some() {
                    newest = asked;
                    cx.update(|cx| {
                        let p = cx.global_mut::<prefs::Prefs>();
                        p.versions_checked_at = now;
                        p.newest_version.clone_from(&newest);
                        p.save();
                    });
                }
            }
            cx.update(|cx| {
                let model = cx.global::<Shell>().model.clone();
                model.update(cx, |m, cx| {
                    if m.newest != newest {
                        m.newest.clone_from(&newest);
                        cx.notify();
                    }
                });
            });
            cx.background_executor().timer(crate::versions::LOOK).await;
        }
    })
    .detach();
}

/// OpenVPN 3 sessions of Crake's tunnels that were started elsewhere
/// (openvpn3's indicator or command line, or Crake before a restart): once
/// connected, Crake takes them over, routes through them, and switching
/// the tunnel off ends them.
pub(super) fn watch_openvpn3(cx: &mut App) {
    cx.spawn(async move |cx| {
        let mut leftovers = ovpn3::leftover::Leftovers::default();
        loop {
            cx.background_executor().timer(OPENVPN3_LOOK).await;
            // Crake's OpenVPN 3 tunnels (name, profile), and those it isn't
            // already following.
            let (ours, wanted) = cx.update(|cx| {
                let shell = cx.global::<Shell>();
                let m = shell.model.read(cx);
                let ours = m
                    .tunnels
                    .iter()
                    .filter(|t| t.protocol == "openvpn3")
                    .map(|t| (t.name.clone(), t.gateway.clone()))
                    .collect::<Vec<_>>();
                let wanted = ours
                    .iter()
                    .filter(|(name, _)| {
                        !shell.sessions.contains_key(name)
                            && !shell.adopting.contains(name)
                            && !m.signing_in.contains(name)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                (ours, wanted)
            });
            if ours.is_empty() {
                continue;
            }
            // A session's process that outlived its session (leftover.rs).
            let backends = cx
                .background_executor()
                .spawn(async { ovpn3::leftover::backends() })
                .await;
            let said = leftovers.look(
                std::time::Instant::now(),
                &backends,
                ovpn3::leftover::started,
            );
            let running = leftovers.running();
            cx.update(|cx| {
                let model = cx.global::<Shell>().model.clone();
                let dismissed = cx.global::<prefs::Prefs>().dismissed.clone();
                model.update(cx, |m, cx| {
                    let now: Vec<model::Leftover> = running
                        .iter()
                        .map(|(pid, started, profile)| model::Leftover {
                            pid: *pid,
                            started: *started,
                            session: m
                                .tunnels
                                .iter()
                                .find(|t| t.protocol == "openvpn3" && t.gateway == *profile)
                                .map_or_else(|| profile.clone(), |t| t.display.clone()),
                        })
                        // Seen ("Got it"): not said again, in the panel or the tray.
                        .filter(|l| !dismissed.contains(&l.dismiss_key()))
                        .collect();
                    if m.leftovers != now {
                        m.leftovers = now;
                        cx.notify();
                    }
                });
            });
            if !said.is_empty() {
                let commands = cx.update(|cx| cx.global::<Shell>().commands.clone());
                for (pid, profile) in said {
                    let tunnel = ours
                        .iter()
                        .find(|(_, p)| *p == profile)
                        .map(|(name, _)| name.clone())
                        .unwrap_or_default();
                    commands
                        .send(Command::LeftRunning(tunnel, profile, pid))
                        .await
                        .ok();
                }
            }
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

pub(super) fn refresh_tray(cx: &mut App) {
    let shell = cx.global::<Shell>();
    let m = shell.model.read(cx);
    let (shape, summary, tunnels) = (m.shape(), m.summary(), m.menu_tunnels());
    let before = shell.tray_shape;
    // The motion first: the new shape goes up with the motion's first frame
    // on, not with its resting icon (going off, the tray showed the head
    // asleep, then awake again as the motion began, then asleep).
    if before != shape {
        cx.global_mut::<Shell>().tray_shape = shape;
        animate_tray(before, shape, cx);
    }
    if let Some(tray) = &cx.global::<Shell>().tray {
        tray.update(move |t| {
            t.shape = shape;
            t.summary = summary;
            t.tunnels = tunnels;
        });
    }
}

/// The tray icon's motion from one state to the next (DESIGN.md, Tray icon):
/// a short run of frames, or, while connecting, the outline drawing itself
/// over and over. A newer change replaces a running animation.
/// The bar told about the icon again once it has had time to fetch the
/// last one (`tray::Handle::settle`).
async fn settle(handle: &crate::tray::Handle, cx: &mut gpui_kit::AsyncApp) {
    cx.background_executor()
        .timer(std::time::Duration::from_millis(SETTLE_MS))
        .await;
    handle.settle();
}

pub(super) fn animate_tray(from: tray::Shape, to: tray::Shape, cx: &mut App) {
    let Some(handle) = cx.global::<Shell>().tray.clone() else {
        return;
    };
    if !motion::enabled() {
        handle.update(|t| t.frame = None);
        let task = cx.spawn(async move |cx| {
            settle(&handle, cx).await;
        });
        cx.global_mut::<Shell>().tray_motion = Some(task);
        return;
    }
    let (base, badge) = to.badged();
    let looping = base == tray::Shape::Connecting;
    let frames: Vec<tray::Icon> = if looping {
        tray::connecting_loop(badge)
            .iter()
            .map(tray::Art::icon)
            .collect()
    } else {
        tray::transition(from, to)
            .iter()
            .map(tray::Art::icon)
            .collect()
    };
    // On at once, before the shape changes under it.
    let first = frames.first().cloned();
    handle.update(move |t| t.frame = first);
    let task = cx.spawn(async move |cx| {
        loop {
            for frame in &frames {
                let frame = frame.clone();
                handle.update(move |t| t.frame = Some(frame));
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(
                        FRAME_MS * u64::from(motion::slowed()),
                    ))
                    .await;
            }
            if !looping || frames.is_empty() {
                break;
            }
        }
        handle.update(|t| t.frame = None);
        settle(&handle, cx).await;
    });
    // Dropping the previous task stops its animation.
    cx.global_mut::<Shell>().tray_motion = Some(task);
}
