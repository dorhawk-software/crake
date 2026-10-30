//! macOS: the journey (`journey_tests.rs` is Linux's, `windows_journey.rs`
//! Windows'), the regression suite where most real bugs came from (a
//! colleague's Mac). The real app shell (`start`: its model, request
//! router, windows and client) on GPUI's test platform, craked as a
//! process of its own serving the app where the installed one does
//! (`gateway_tests::own_craked`, as root), Homebrew's openconnect, and the
//! CI gateway through Cloudflare (scripts/ci-through-cloudflare.sh). One
//! day in the app: the panel's switch and sign-in, a host probed, undone
//! and redone from "What Crake did", a short name chosen there, Settings, tunnel detail's "Add a
//! host", the log's Copy for helpdesk, a report, New tunnel, switching
//! off, and craked starting again under the app.
//!
//! Skipped unless CRAKE_E2E=1.

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::TestAppContext;

use crate::client::{self, Command};
use crate::journey_kit::{
    click, index, model, panel, request, sign_in, state, type_into, until, wait_window, window,
};
use crate::model::Model;
use crate::test_app::{App, enabled, gateway, get};
use crate::{Request, Setup, Shell, settings_view, signin_view, start, tunnel_detail};

/// `addr` goes through a tunnel: macOS sends it out of a utun.
fn routed(addr: &str) -> bool {
    let out = std::process::Command::new("route")
        .args(["-n", "get", addr])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|l| l.trim().starts_with("interface: utun"))
}

/// craked without the journey's tunnels, then (`acme`) with ACME as New
/// tunnel adds it: a client of its own, before the app starts and after
/// it's done.
fn fresh(acme: bool) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (tx, updates) = async_channel::unbounded();
        let mut app = App::new(client::start(tx), updates);
        app.within(Duration::from_secs(30), "craked", |m| {
            m.reachable == Some(true) && !m.settings.is_empty()
        })
        .await;
        for name in ["acme", "globex"] {
            if app.state(name).is_some() {
                app.send(Command::RemoveTunnel(name.into()));
                app.until("an old one gone", |m| {
                    !m.tunnels.iter().any(|t| t.name == name)
                })
                .await;
            }
        }
        if !acme {
            return;
        }
        let gateway = gateway();
        let fields: HashMap<String, String> = [
            ("display_name", "ACME"),
            ("gateway", gateway.as_str()),
            ("protocol", "anyconnect"),
            ("user", "alice"),
            ("sign_in", "password"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        app.send(Command::AddTunnel(fields));
        app.until("ACME added", |m| m.tunnels.iter().any(|t| t.name == "acme"))
            .await;
        // The probe wait as the Linux journey leaves it: 3 s, its default.
        app.send(Command::SetSetting("probe_wait_secs".into(), "3".into()));
        app.settle(Duration::from_secs(1)).await;
    });
}

/// craked stopped as launchd stops it.
fn stop(craked: &mut std::process::Child) {
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &craked.id().to_string()])
        .status();
    let _ = craked.wait();
}

#[gpui_kit::test]
fn a_day_with_crake_on_macos(cx: &mut TestAppContext) {
    if !enabled() {
        return;
    }
    // Real I/O from other threads: craked, openconnect, the client.
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let mut craked = crate::gateway_tests::own_craked(dir.path());
    fresh(true);
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::prefs::Prefs {
            scratch: true,
            ..Default::default()
        });
        assert!(start(
            cx,
            Setup {
                model: Model::default(),
                x11: false,
                demo: false,
                connect: Some(Box::new(client::start)),
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
        Duration::from_secs(20),
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
    assert!(
        cx.read(|cx| cx.global::<Shell>().tray_shape) != crate::tray::Shape::Off,
        "the tray says connected"
    );

    // 2. A host learned, shown in "What Crake did", undone and redone there.
    let page =
        std::thread::spawn(|| (0..5).find_map(|_| get("10.40.1.10", Duration::from_secs(5)).ok()));
    assert!(
        page.join().unwrap().is_some_and(|b| b.contains("crex")),
        "probed and routed"
    );
    let decided = |cx: &mut TestAppContext, action: crate::model::Action| {
        model(cx)
            .decisions(crate::panel::DECISIONS)
            .iter()
            .position(|d| {
                d.entry.hosts.iter().any(|h| h == "10.40.1.10")
                    && d.action.as_ref() == Some(&action)
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
    until(
        cx,
        "undone: 10.40.1.10 goes direct",
        Duration::from_secs(10),
        |_| !routed("10.40.1.10"),
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
        |_| routed("10.40.1.10"),
    );

    // 2b. A short name, chosen in "What Crake did", forgotten in tunnel
    //     detail.
    crate::journey_kit::short_name(cx, &crate::journey_kit::look_up);

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
    request(cx, Request::OpenDetail("acme".into(), None));
    let (d, view) = wait_window::<tunnel_detail::TunnelDetail>(cx, "tunnel detail opens");
    let field = view.read_with(cx, |v, _| v.add_field());
    type_into(cx, d, field, "10.40.1.11");
    click(cx, d, "send");
    until(
        cx,
        "10.40.1.11 sent through ACME",
        Duration::from_secs(10),
        |_| routed("10.40.1.11"),
    );
    assert!(
        get("10.40.1.11", Duration::from_secs(2))
            .unwrap()
            .contains("crex")
    );

    // 5. The log, and Copy for helpdesk; a report from Settings.
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
    let report = crate::journey_kit::save_a_report(cx);
    assert!(report.contains("Sent 10.40.1.10 through ACME."), "{report}");
    assert!(
        report.contains("\nThis computer\n  The internet: "),
        "{report}"
    );
    assert!(report.contains("\n  ACME's names: "), "{report}");

    // 6. New tunnel: added in craked, listed in the app; Cancel at sign-in.
    request(cx, Request::NewTunnel(None));
    let (n, view) = wait_window::<crate::new_tunnel::NewTunnel>(cx, "New tunnel opens");
    // As the Linux journey: the height a full HD screen grants.
    cx.simulate_window_resize(n, gpui_kit::size(gpui_kit::px(720.), gpui_kit::px(1000.)));
    let (name, gateway_field, user) = view.read_with(cx, |v, _| v.fields_for_tests());
    type_into(cx, n, name, "Globex");
    click(cx, n, ("protocol", 1usize));
    type_into(cx, n, gateway_field, &gateway());
    click(cx, n, "next");
    click(cx, n, ("method", 1usize));
    type_into(cx, n, user, "alice");
    click(cx, n, "next");
    click(cx, n, "next");
    until(
        cx,
        "Globex added in craked and listed",
        Duration::from_secs(10),
        |cx| model(cx).tunnels.iter().any(|t| t.name == "globex"),
    );
    let (w, _) = wait_window::<signin_view::SignIn>(cx, "Globex's sign-in opens");
    click(cx, w, "cancel");
    until(cx, "sign-in closed", Duration::from_secs(10), |cx| {
        window::<signin_view::SignIn>(cx).is_none()
    });

    // 7. Switched off from Settings → Tunnels (switched on from the panel
    //    in step 1): its routes go.
    request(cx, Request::Settings);
    let (s, _) = wait_window::<settings_view::Settings>(cx, "Settings opens");
    click(cx, s, ("section", 0usize));
    click(cx, s, gpui_kit::ElementId::Name("switch-acme".into()));
    until(cx, "ACME off", Duration::from_secs(15), |cx| {
        state(cx, "acme") == "off"
    });
    assert!(!routed("10.40.1.10"));

    // 8. craked stops and starts again (an upgrade, a crash): the app
    //    follows.
    stop(&mut craked);
    craked = crate::gateway_tests::own_craked(dir.path());
    until(
        cx,
        "and back, with both tunnels",
        Duration::from_secs(30),
        |cx| {
            let m = model(cx);
            m.reachable == Some(true) && m.tunnels.iter().any(|t| t.name == "globex")
        },
    );
    // Nothing left for the next test.
    fresh(false);
    stop(&mut craked);
}
