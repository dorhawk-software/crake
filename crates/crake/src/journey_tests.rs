//! The app as a user drives it, end to end: the real app shell (`start`:
//! its model, request router, windows and D-Bus client) in front, a real
//! craked behind a real D-Bus socket, real openconnect, nftables and
//! policy routing, and a real AnyConnect gateway (ocserv, scripts/e2e).
//! GPUI's test platform stands in for the screen: windows open, render and
//! take clicks and typing, nothing is drawn.
//!
//! One day in the app, step by step: the panel and its switch, sign-in,
//! a host learned and undone, Settings, tunnel detail, the log, New tunnel,
//! switching off, and craked restarting under the app. Each step says
//! which it is when it fails. A regression suite: it stays green.
//!
//! Skipped unless CRAKE_E2E=1; run with `scripts/e2e-test.sh`.

use std::os::unix::fs::MetadataExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, Render, TestAppContext};

use craked::manager::Manager;

use crate::e2e_tests::{config, sh};
use crate::model::Model;
use crate::test_app::{enabled, get};
use crate::{Request, Setup, Shell, panel, settings_view, signin_view, start, tunnel_detail};

/// craked, in a runtime of its own, serving the app on `socket`.
struct Daemon {
    rt: tokio::runtime::Runtime,
    mgr: Arc<Manager>,
}

fn daemon(dir: &std::path::Path) -> Daemon {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let mgr = rt.block_on(async {
        let cfg = config(dir);
        let hook = cfg.hook_socket.clone();
        let mgr = Manager::new(cfg).unwrap();
        mgr.init().await.unwrap();
        craked::hook_server::serve(mgr.clone(), &hook).unwrap();
        let socket = dir.join("crake.sock");
        let _ = std::fs::remove_file(&socket);
        let owner = std::fs::metadata(dir).unwrap().uid();
        craked::dbus::serve_p2p(mgr.clone(), &socket, owner).unwrap();
        mgr
    });
    Daemon { rt, mgr }
}

/// Let the app and craked work until `done` holds; `what` names the step.
fn until(
    cx: &mut TestAppContext,
    what: &str,
    wait: Duration,
    done: impl Fn(&mut TestAppContext) -> bool,
) {
    let end = Instant::now() + wait;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn model(cx: &mut TestAppContext) -> Model {
    cx.read(|cx| cx.global::<Shell>().model.read(cx).clone())
}

fn request(cx: &mut TestAppContext, r: Request) {
    cx.update(|cx| cx.global::<Shell>().requests.send_blocking(r).unwrap());
    cx.run_until_parked();
}

/// The open window whose view is a `V`, and that view.
fn window<V: 'static + Render>(cx: &mut TestAppContext) -> Option<(AnyWindowHandle, Entity<V>)> {
    cx.update(|cx| {
        cx.windows().into_iter().find_map(|w| {
            let typed = w.downcast::<V>()?;
            Some((w, typed.root(cx).ok()?))
        })
    })
}

fn wait_window<V: 'static + Render>(
    cx: &mut TestAppContext,
    what: &str,
) -> (AnyWindowHandle, Entity<V>) {
    until(cx, what, Duration::from_secs(10), |cx| {
        window::<V>(cx).is_some()
    });
    window::<V>(cx).unwrap()
}

fn click(cx: &mut TestAppContext, w: AnyWindowHandle, id: impl Into<gpui_kit::ElementId>) {
    let id = id.into();
    settle(cx, w);
    if std::env::var_os("CRAKE_JOURNEY_DEBUG").is_some() {
        cx.update_window(w, |_, window, cx| {
            window.render_frame(cx);
            let found = window
                .try_find(id.clone())
                .map(|e| (e.bounds(), e.visible()));
            eprintln!(
                "click {id:?}: viewport {:?}, element {found:?}",
                window.viewport_size()
            );
        })
        .unwrap();
    }
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// A few frames, as a screen would draw them: a window that fits itself
/// to its content (New tunnel) resizes on the frame after it measures.
/// GPUI's test platform takes a resize without telling the window, as a
/// window system's configure event would: it's delivered here.
fn settle(cx: &mut TestAppContext, w: AnyWindowHandle) {
    for _ in 0..3 {
        let asked = cx
            .update_window(w, |_, window, cx| {
                window.render_frame(cx);
                let asked = window.bounds().size;
                (asked != window.viewport_size()).then_some(asked)
            })
            .unwrap();
        if let Some(size) = asked {
            cx.simulate_window_resize(w, size);
        }
        cx.run_until_parked();
    }
}

fn type_into(
    cx: &mut TestAppContext,
    w: AnyWindowHandle,
    field: Entity<gpui_kit::component::input::InputState>,
    text: &str,
) {
    cx.update_window(w, |_, window, cx| {
        field.update(cx, |f, cx| f.focus(window, cx));
        window.render_frame(cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// The panel, opened the way the tray icon opens it.
fn panel(cx: &mut TestAppContext) -> AnyWindowHandle {
    if window::<panel::Panel>(cx).is_none() {
        // A tray click within half a second of the panel closing on blur
        // means "close" (the click takes focus first; windows.rs): a person
        // clicks later than that.
        let blurred = cx.read(|cx| cx.global::<Shell>().panel_blurred.map(|t| t.elapsed()));
        if let Some(ago) = blurred {
            std::thread::sleep(Duration::from_millis(600).saturating_sub(ago));
        }
        request(cx, Request::Panel(None));
    }
    wait_window::<panel::Panel>(cx, "the panel opens").0
}

fn state(cx: &mut TestAppContext, tunnel: &str) -> String {
    model(cx)
        .tunnels
        .iter()
        .find(|t| t.name == tunnel)
        .map(|t| t.state.clone())
        .unwrap_or_default()
}

fn index(cx: &mut TestAppContext, tunnel: &str) -> usize {
    model(cx)
        .tunnels
        .iter()
        .position(|t| t.name == tunnel)
        .expect("listed")
}

/// Sign in as the window asks: trust the gateway, give the password.
fn sign_in(cx: &mut TestAppContext) {
    use crate::signin::Ask;
    let (w, view) = wait_window::<signin_view::SignIn>(cx, "the sign-in window opens");
    let asked = |cx: &mut TestAppContext, want: fn(&Ask) -> bool| {
        view.read_with(
            cx,
            |v, _| matches!(v.stage(), signin_view::Stage::Asking(p) if want(&p.ask)),
        )
    };
    until(
        cx,
        "asked to trust the gateway",
        Duration::from_secs(20),
        |cx| asked(cx, |a| matches!(a, Ask::TrustServer { .. })),
    );
    click(cx, w, "yes");
    until(
        cx,
        "asked for the password",
        Duration::from_secs(20),
        |cx| asked(cx, |a| *a == Ask::Secret),
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.input("hunter2", cx);
    })
    .unwrap();
    click(cx, w, "continue");
}

#[gpui_kit::test]
fn a_day_with_crake(cx: &mut TestAppContext) {
    if !enabled() {
        return;
    }
    // Real I/O from other threads: craked, openconnect, the client.
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let craked = daemon(dir.path());
    let socket = dir.path().join("crake.sock");
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::prefs::Prefs {
            scratch: true,
            ..Default::default()
        });
        let connect = Box::new(move |updates| crate::client::start_on(socket, updates));
        assert!(start(
            cx,
            Setup {
                model: Model::default(),
                x11: false,
                demo: false,
                connect: Some(connect),
                tray: false,
                single_instance: false,
                openvpn3: false,
                autostart: false,
            }
        ));
    });
    until(
        cx,
        "craked reached, ACME listed",
        Duration::from_secs(10),
        |cx| {
            let m = model(cx);
            m.reachable == Some(true) && m.tunnels.iter().any(|t| t.name == "acme")
        },
    );

    // 1. The panel's switch: sign-in, then connected.
    let p = panel(cx);
    let acme = index(cx, "acme");
    click(cx, p, ("switch", acme));
    sign_in(cx);
    until(cx, "ACME connected", Duration::from_secs(30), |cx| {
        state(cx, "acme") == "connected"
    });
    assert!(sh("ip -4 -o addr show crake-acme").contains("inet 10.40.255."));
    assert!(
        cx.read(|cx| cx.global::<Shell>().tray_shape) != crate::tray::Shape::Off,
        "the tray says connected"
    );

    // 2. A host learned, shown in "What Crake did", undone and redone there.
    let page = std::thread::spawn(|| get("10.40.1.10", Duration::from_secs(12)));
    assert!(
        page.join().unwrap().unwrap().contains("crex"),
        "probed and routed"
    );
    let decided = |cx: &mut TestAppContext, action: crate::model::Action| {
        model(cx).decisions(panel::DECISIONS).iter().position(|d| {
            d.entry.hosts.iter().any(|h| h == "10.40.1.10") && d.action.as_ref() == Some(&action)
        })
    };
    until(
        cx,
        "the decision in the panel",
        Duration::from_secs(10),
        |cx| decided(cx, crate::model::Action::Undo).is_some(),
    );
    let p = panel(cx);
    let i = decided(cx, crate::model::Action::Undo).unwrap();
    click(cx, p, ("act", i));
    let routed = |d: &Daemon| {
        sh("nft list set inet crake learned_acme").contains("10.40.1.10")
            && d.mgr.get("acme").is_some()
    };
    until(
        cx,
        "undone: 10.40.1.10 goes direct",
        Duration::from_secs(10),
        |_| !routed(&craked),
    );
    until(cx, "Redo offered", Duration::from_secs(10), |cx| {
        decided(cx, crate::model::Action::Redo).is_some()
    });
    let p = panel(cx);
    let i = decided(cx, crate::model::Action::Redo).unwrap();
    click(cx, p, ("act", i));
    until(
        cx,
        "redone: 10.40.1.10 through ACME again",
        Duration::from_secs(10),
        |_| routed(&craked),
    );

    // 3. Settings: Learning's wait, changed in craked and shown back.
    request(cx, Request::Settings);
    let (s, _) = wait_window::<settings_view::Settings>(cx, "Settings opens");
    click(cx, s, ("section", 2usize));
    click(cx, s, ("wait", 0usize));
    until(
        cx,
        "the wait is 1 s in craked",
        Duration::from_secs(10),
        |cx| model(cx).setting("probe_wait_secs") == "1",
    );

    // 4. Tunnel detail: "Add a host" sends one through ACME.
    request(cx, Request::OpenDetail("acme".into()));
    let (d, view) = wait_window::<tunnel_detail::TunnelDetail>(cx, "tunnel detail opens");
    let field = view.read_with(cx, |v, _| v.add_field());
    type_into(cx, d, field, "10.40.1.11");
    click(cx, d, "send");
    until(
        cx,
        "10.40.1.11 sent through ACME",
        Duration::from_secs(10),
        |_| sh("nft list set inet crake learned_acme").contains("10.40.1.11"),
    );
    assert!(
        get("10.40.1.11", Duration::from_secs(2))
            .unwrap()
            .contains("crex")
    );

    // 5. The log, and Copy for helpdesk.
    request(cx, Request::Log);
    let (l, _) = wait_window::<crate::log_view::Log>(cx, "the log opens");
    click(cx, l, "helpdesk");
    let copied = cx
        .read_from_clipboard()
        .and_then(|c| c.text())
        .unwrap_or_default();
    assert!(copied.contains("Sent 10.40.1.10 through ACME."), "{copied}");

    // 6. New tunnel: added in craked, listed in the app; Cancel at sign-in.
    request(cx, Request::NewTunnel(None));
    let (n, view) = wait_window::<crate::new_tunnel::NewTunnel>(cx, "New tunnel opens");
    // New tunnel asks to grow to each step on the next frame, which the
    // test platform never delivers: give it the height a full HD screen
    // grants (its cap without a known screen).
    cx.simulate_window_resize(n, gpui_kit::size(gpui_kit::px(720.), gpui_kit::px(1000.)));
    let (name, gateway, user) = view.read_with(cx, |v, _| v.fields_for_tests());
    type_into(cx, n, name, "Globex");
    click(cx, n, ("protocol", 1usize));
    type_into(cx, n, gateway, &crate::test_app::gateway());
    click(cx, n, "next");
    click(cx, n, ("method", 1usize));
    type_into(cx, n, user, "alice");
    click(cx, n, "next");
    click(cx, n, "next");
    until(
        cx,
        "Globex added in craked and listed",
        Duration::from_secs(10),
        |cx| {
            craked.mgr.get("globex").is_some()
                && model(cx).tunnels.iter().any(|t| t.name == "globex")
        },
    );
    let (w, _) = wait_window::<signin_view::SignIn>(cx, "Globex's sign-in opens");
    click(cx, w, "cancel");
    until(cx, "sign-in closed", Duration::from_secs(10), |cx| {
        window::<signin_view::SignIn>(cx).is_none()
    });

    // 7. Switched off from the panel: its interface and routes go.
    let p = panel(cx);
    let acme = index(cx, "acme");
    click(cx, p, ("switch", acme));
    until(cx, "ACME off", Duration::from_secs(15), |cx| {
        state(cx, "acme") == "off"
    });
    assert!(!sh("ip link").contains("crake-acme"));

    // 8. craked stops and comes back (an upgrade, a crash): the app follows.
    let Daemon { rt, mgr } = craked;
    rt.block_on(mgr.shutdown());
    drop(rt);
    until(
        cx,
        "the app sees craked gone",
        Duration::from_secs(15),
        |cx| model(cx).reachable == Some(false),
    );
    let craked = daemon(dir.path());
    until(
        cx,
        "and back, with both tunnels",
        Duration::from_secs(20),
        |cx| {
            let m = model(cx);
            m.reachable == Some(true) && m.tunnels.iter().any(|t| t.name == "globex")
        },
    );
    craked.rt.block_on(craked.mgr.shutdown());
}
