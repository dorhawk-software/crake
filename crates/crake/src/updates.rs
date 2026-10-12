//! What craked says, applied: the model, notifications, the tray, OpenVPN 3
//! sessions started elsewhere.

use super::*;

pub(super) fn on_update(update: client::Update, cx: &mut App) {
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

/// OpenVPN 3 sessions of Crake's tunnels that were started elsewhere
/// (openvpn3's indicator or command line, or Crake before a restart): once
/// connected, Crake takes them over, routes through them, and switching
/// the tunnel off ends them.
pub(super) fn watch_openvpn3(cx: &mut App) {
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
pub(super) fn animate_tray(from: tray::Shape, to: tray::Shape, cx: &mut App) {
    let Some(handle) = cx.global::<Shell>().tray.clone() else {
        return;
    };
    if !motion::enabled() {
        handle.update(|t| t.frame = None);
        cx.global_mut::<Shell>().tray_motion = None;
        return;
    }
    let looping = to == tray::Shape::Connecting;
    let frames: Vec<tray::Icon> = if looping {
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
