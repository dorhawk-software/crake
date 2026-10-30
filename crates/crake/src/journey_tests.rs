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
use std::time::Duration;

use gpui_kit::TestAppContext;

use craked::manager::Manager;

use crate::e2e_tests::{config, sh};
use crate::journey_kit::{
    click, decline, index, model, panel, request, sign_in, sign_in_of, state, type_into, until,
    wait_window, window,
};
use crate::model::Model;
use crate::test_app::{enabled, get};
use crate::{Request, Setup, Shell, panel, settings_view, signin_view, start, tunnel_detail};

/// craked, in a runtime of its own, serving the app on `socket`.
pub(crate) struct Daemon {
    pub(crate) rt: tokio::runtime::Runtime,
    pub(crate) mgr: Arc<Manager>,
}

pub(crate) fn daemon(dir: &std::path::Path) -> Daemon {
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
    crate::journey_kit::first_route_got_it(cx, "10.40.1.10");
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

    // 2b. A short name, chosen in "What Crake did", forgotten in tunnel
    //     detail.
    //     No resolved here: "git" is asked of craked as resolved asks the
    //     hush link (git.crake.internal; the scenarios use the real one).
    let (mgr, rt) = (craked.mgr.clone(), craked.rt.handle().clone());
    crate::journey_kit::short_name(cx, &|name| {
        let q = crate::e2e_tests::query(&format!("{name}.{}", craked::dns::SHORT_SUFFIX));
        rt.block_on(mgr.answer_short(&q))
            .and_then(|reply| craked::dns::parse_message(&reply))
            .map(|a| a.addrs.iter().map(ToString::to_string).collect())
            .unwrap_or_default()
    });

    // 3. Settings: Learning's wait, changed in craked and shown back.
    settings_wait(cx);

    // 3b. What's new: after an update, from the panel's notice and About.
    whats_new(cx);

    // 4. Tunnel detail: "Add a host" sends one through ACME.
    request(cx, Request::OpenDetail("acme".into(), None));
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
    // A host's name copies whole on a click (design/proposals/host-full-name.html).
    click(cx, d, ("host-name", 0usize));
    let copied = cx
        .read_from_clipboard()
        .and_then(|c| c.text())
        .unwrap_or_default();
    assert!(copied.starts_with("10.40.1.1"), "copied {copied:?}");

    // 5. The log: Copy for helpdesk, Save a report….
    the_log(cx);

    // 5b. Traffic, from the panel's footer.
    traffic_window(cx);

    // 5c. The profile editor, from tunnel detail's "Edit…".
    profile_editor(cx);

    // 6. New tunnel: added in craked, listed in the app; Cancel at sign-in.
    request(cx, Request::NewTunnel(None));
    let (n, view) = wait_window::<crate::new_tunnel::NewTunnel>(cx, "New tunnel opens");
    // New tunnel asks to grow to each step on the next frame, which the
    // test platform never delivers: give it the height a full HD screen
    // grants (its cap without a known screen).
    cx.simulate_window_resize(n, gpui_kit::size(gpui_kit::px(720.), gpui_kit::px(1000.)));
    let (name, gateway, user) = view.read_with(cx, |v, _| v.fields_for_tests());
    type_into(cx, n, name, "Globex");
    // A .ovpn file imported from the Gateway label's row (the picker is
    // the platform's; the file it gives): OpenVPN, with the gateway the
    // profile names (design/proposals/openvpn-tunnel.html, 1).
    click(cx, n, "import-ovpn");
    let ovpn = tempfile::tempdir().unwrap();
    std::fs::write(ovpn.path().join("ca.crt"), "-----BEGIN CERTIFICATE-----\n").unwrap();
    std::fs::write(
        ovpn.path().join("globex-lab.ovpn"),
        "client\nremote vpn.globex.example 1194\nca ca.crt\n",
    )
    .unwrap();
    view.update(cx, |v, cx| {
        v.import_for_tests(ovpn.path().join("globex-lab.ovpn"), cx)
    });
    until(
        cx,
        "the .ovpn file is the profile",
        Duration::from_secs(10),
        |cx| {
            view.read_with(cx, |v, _| v.imported_for_tests()).as_deref()
                == Some("vpn.globex.example")
        },
    );
    // The list folded behind OpenVPN: open it.
    click(cx, n, "protocols-more");
    // On with AnyConnect instead.
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
    // Switched on once the tunnel list has caught up (a timer: the test
    // platform's clock moves only when told). Until 2026-10-19 this step
    // found ACME's window, still open, and nothing switched Globex on.
    cx.executor().advance_clock(Duration::from_secs(1));
    let (w, view) = sign_in_of(cx, "globex");
    decline(cx, w, &view);
    until(cx, "sign-in closed", Duration::from_secs(10), |cx| {
        window::<signin_view::SignIn>(cx).is_none()
    });

    // 7. Switched off from Settings → Tunnels (switched on from the panel
    //    in step 1): its interface and routes go.
    request(cx, Request::Settings);
    let (s, _) = wait_window::<settings_view::Settings>(cx, "Settings opens");
    click(cx, s, ("section", 0usize));
    click(cx, s, gpui_kit::ElementId::Name("switch-acme".into()));
    until(cx, "ACME off", Duration::from_secs(15), |cx| {
        state(cx, "acme") == "off"
    });
    assert!(!sh("ip link").contains("crake-acme"));

    // 7b. On again, then stopped on its own; the panel's Dismiss.
    stopped_and_dismissed(cx);

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

/// The log, its Copy for helpdesk and its Save a report….
fn the_log(cx: &mut TestAppContext) {
    request(cx, Request::Log);
    let (l, _) = wait_window::<crate::log_view::Log>(cx, "the log opens");
    click(cx, l, "helpdesk");
    let copied = cx
        .read_from_clipboard()
        .and_then(|c| c.text())
        .unwrap_or_default();
    assert!(copied.contains("Sent 10.40.1.10 through ACME."), "{copied}");
    // A routed row's "Show in ACME": tunnel detail, on that host.
    crate::journey_kit::show_in(cx, l, "10.40.1.10");
    // Settings → Diagnostics → Save a report…: the same decisions, and
    // craked's side of the computer, ACME's name tried through ACME.
    let report = crate::journey_kit::save_a_report(cx);
    assert!(report.contains("Sent 10.40.1.10 through ACME."), "{report}");
    assert!(
        report.contains("\nThis computer\n  The internet: "),
        "{report}"
    );
    assert!(report.contains("\n  ACME's names: "), "{report}");
}

/// What's new (design/proposals/whats-new.html): updated from 0.13.1, the
/// panel says so and its What's new opens the window, this version seen;
/// About's link opens it again, and Close closes it.
fn whats_new(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let prefs = cx.global_mut::<crate::prefs::Prefs>();
        prefs.seen_version = Some("0.13.1".into());
        prefs.existed = true;
    });
    let p = panel(cx);
    click(cx, p, "What's new");
    let (w, _) = wait_window::<crate::whats_new::WhatsNew>(cx, "What's new opens");
    let seen = cx.read(|cx| cx.global::<crate::prefs::Prefs>().seen_version.clone());
    assert_eq!(seen.as_deref(), Some(crate::versions::this()));
    click(cx, w, "close");
    until(cx, "What's new closed", Duration::from_secs(10), |cx| {
        window::<crate::whats_new::WhatsNew>(cx).is_none()
    });
    request(cx, Request::Settings);
    let (s, _) = wait_window::<settings_view::Settings>(cx, "Settings opens");
    click(cx, s, ("section", 7usize));
    click(cx, s, "whats-new");
    let (w, _) = wait_window::<crate::whats_new::WhatsNew>(cx, "About opens it");
    click(cx, w, "close");
    until(cx, "closed again", Duration::from_secs(10), |cx| {
        window::<crate::whats_new::WhatsNew>(cx).is_none()
    });
}

/// Settings: Learning's wait, changed in craked and shown back.
fn settings_wait(cx: &mut TestAppContext) {
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
}

/// The profile editor (tunnel detail's "Edit…" on an OpenVPN 3 tunnel):
/// the window opens and asks openvpn3, which the journey's container
/// doesn't have, so it says it couldn't read the profile; Close closes it.
fn profile_editor(cx: &mut TestAppContext) {
    request(cx, Request::EditProfile("acme".into()));
    let (e, editor) = wait_window::<crate::profile_editor::ProfileEditor>(cx, "the editor opens");
    until(cx, "it says why", Duration::from_secs(10), |cx| {
        editor.read_with(cx, |v, _| {
            matches!(
                v.stage_for_tests(),
                crate::profile_editor::Stage::Unreadable(_)
            )
        })
    });
    click(cx, e, "close");
    until(cx, "the editor closed", Duration::from_secs(10), |cx| {
        window::<crate::profile_editor::ProfileEditor>(cx).is_none()
    });
}

/// ACME on again from the panel, then its openconnect dies (a crash):
/// stopped on its own, the tray's badge on the head asleep (nothing else is
/// up), and the panel's Dismiss makes it plain Off, the badge gone.
fn stopped_and_dismissed(cx: &mut TestAppContext) {
    let p = panel(cx);
    let acme = index(cx, "acme");
    // Switched off keeping its sign-in (step 7): no sign-in this time.
    let kept = model(cx).tunnels[acme].signed_in;
    click(cx, p, ("switch", acme));
    if !kept {
        sign_in(cx);
    }
    until(cx, "ACME connected again", Duration::from_secs(30), |cx| {
        state(cx, "acme") == "connected"
    });
    kill_openconnect("crake-acme");
    until(
        cx,
        "ACME stopped on its own",
        Duration::from_secs(15),
        |cx| state(cx, "acme") == "trouble",
    );
    let tray = |cx: &mut TestAppContext| cx.read(|cx| cx.global::<Shell>().tray_shape);
    assert_eq!(tray(cx), crate::tray::Shape::OffTrouble);
    let p = panel(cx);
    let acme = index(cx, "acme");
    click(cx, p, ("dismiss", acme));
    until(cx, "ACME dismissed: Off", Duration::from_secs(10), |cx| {
        state(cx, "acme") == "off"
    });
    assert_eq!(tray(cx), crate::tray::Shape::Off, "no badge");
}

/// Kill the openconnect running `interface` as a crash would (SIGKILL).
/// Found by its exact arguments in /proc, which in the test's container is
/// the machine's: its number in the container's own numbering is the last
/// of its status' NSpid.
fn kill_openconnect(interface: &str) {
    let want = format!("--interface={interface}");
    let pids: Vec<String> = std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let pid = e.file_name().to_str()?.parse::<u32>().ok()?;
            let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
            let args: Vec<&[u8]> = cmdline.split(|b| *b == 0).collect();
            let openconnect = args.first()?.ends_with(b"openconnect");
            if !(openconnect && args.contains(&want.as_bytes())) {
                return None;
            }
            let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
            status
                .lines()
                .find_map(|l| l.strip_prefix("NSpid:"))
                .and_then(|n| n.split_whitespace().last())
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(pids.len(), 1, "one openconnect for {interface}: {pids:?}");
    let out = sh(&format!("kill -KILL {}; echo $?", pids[0]));
    assert_eq!(out.trim(), "0", "killed: {out}");
}

/// The Traffic window, from the panel's footer: an open connection to
/// 10.40.1.10 is listed going through ACME (craked's conntrack dump).
fn traffic_window(cx: &mut TestAppContext) {
    let open = std::net::TcpStream::connect_timeout(
        &"10.40.1.10:80".parse().unwrap(),
        Duration::from_secs(5),
    )
    .expect("10.40.1.10 through ACME");
    let p = panel(cx);
    click(cx, p, "traffic");
    let (tr, _) = wait_window::<crate::traffic_view::Traffic>(cx, "Traffic opens");
    // The window asks once a second; the test platform's timers don't
    // run on their own, so the journey asks as the window would (the app's
    // request, its client, craked's D-Bus Live), and looks.
    let through_acme = |cx: &mut TestAppContext| {
        model(cx).live.rates.as_ref().is_some_and(|r| {
            r.connections.iter().any(|(c, _)| {
                c.dst.to_string() == "10.40.1.10" && c.tunnel.as_deref() == Some("acme")
            })
        })
    };
    let end = std::time::Instant::now() + Duration::from_secs(15);
    while !through_acme(cx) {
        assert!(
            std::time::Instant::now() < end,
            "never: the connection to 10.40.1.10, through ACME (lists connections: {:?})",
            model(cx).live.lists_connections()
        );
        request(cx, Request::Live);
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(500));
        cx.run_until_parked();
    }
    click(cx, tr, "close");
    drop(open);
}
