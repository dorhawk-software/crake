//! The journey's IPsec step (design/proposals/ipsec-tunnel.html): New
//! tunnel adds an IPsec tunnel with its gateway's key, the sign-in window
//! asks its password (the username is the tunnel's) with "Remember the
//! password in your keyring", and it connects through the e2e image's
//! strongSwan gateway; the real app shell on GPUI's headless platform, a
//! real craked with its own charon.
//!
//! Its own test, apart from `journey_tests`, which every distribution's
//! package test runs: those containers have no IPsec gateway. Run where
//! one is, by scripts/e2e-test.sh (CRAKE_E2E_IPSEC=1).

use std::time::Duration;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext};

use crate::e2e_tests::sh;
use crate::journey_kit::{
    click, model, request, sign_in_of, state, type_into, until, wait_window, window,
};
use crate::journey_tests::daemon;
use crate::model::Model;
use crate::test_app::enabled;
use crate::{Request, Setup, signin_view, start};

/// The e2e IPsec gateway (scripts/e2e/ipsec.conf).
const GATEWAY: &str = "198.51.100.10";
/// Where the IPsec card sits in New tunnel's list (`PROTOCOLS`).
const IPSEC: usize = 9;

/// The app's shell with a real craked, reached.
fn app_with_craked(cx: &mut TestAppContext, dir: &std::path::Path) -> crate::journey_tests::Daemon {
    cx.executor().allow_parking();
    let craked = daemon(dir);
    let socket = dir.join("crake.sock");
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
    until(cx, "craked reached", Duration::from_secs(10), |cx| {
        model(cx).reachable == Some(true)
    });
    craked
}

#[gpui_kit::test]
fn an_ipsec_tunnel_added_and_signed_in(cx: &mut TestAppContext) {
    if !enabled() || std::env::var_os("CRAKE_E2E_IPSEC").is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let craked = app_with_craked(cx, dir.path());

    // New tunnel: IPsec chosen, the list folds; the key and the domain.
    request(cx, Request::NewTunnel(None));
    let (n, view) = wait_window::<crate::new_tunnel::NewTunnel>(cx, "New tunnel opens");
    cx.simulate_window_resize(n, gpui_kit::size(gpui_kit::px(720.), gpui_kit::px(1000.)));
    let (name, gateway, user) = view.read_with(cx, |v, _| v.fields_for_tests());
    let (key, domains, identity) = view.read_with(cx, |v, _| v.ipsec_fields_for_tests());
    type_into(cx, n, name, "Nordbank");
    click(cx, n, ("protocol", IPSEC));
    type_into(cx, n, gateway, GATEWAY);
    type_into(cx, n, key, "hunter2hunter2");
    type_into(cx, n, domains, "acme.internal");
    click(cx, n, "next");
    type_into(cx, n, user, "alice");
    click(cx, n, "advanced");
    type_into(cx, n, identity, "vpn.acme.example");
    click(cx, n, "next");
    click(cx, n, "next");
    until(
        cx,
        "Nordbank added in craked and listed",
        Duration::from_secs(10),
        |cx| {
            craked.mgr.get("nordbank").is_some()
                && model(cx).tunnels.iter().any(|t| t.name == "nordbank")
        },
    );
    // The key is craked's alone: never in what the app lists.
    assert!(
        !format!("{:?}", model(cx).tunnels).contains("hunter2hunter2"),
        "the key isn't listed"
    );

    // Switched on (by Add and connect): only the password is asked, with
    // "Remember the password in your keyring".
    // The app switches it on once the tunnel list has caught up (a timer
    // in updates.rs): the test platform's clock moves only when told.
    cx.executor().advance_clock(Duration::from_secs(1));
    let (w, sign) = sign_in_of(cx, "nordbank");
    until(
        cx,
        "asked for the password",
        Duration::from_secs(20),
        |cx| {
            sign.read_with(cx, |v, _| {
            matches!(v.stage(), signin_view::Stage::Asking(p) if p.ask == crate::signin::Ask::Secret)
        })
        },
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.input("hunter2", cx);
    })
    .unwrap();
    click(cx, w, "continue");
    until(cx, "Nordbank connected", Duration::from_secs(30), |cx| {
        state(cx, "nordbank") == "connected"
    });
    let info = craked.mgr.get("nordbank").unwrap().info.unwrap();
    assert_eq!(info.device, "crake-nordbank");
    assert!(
        craked
            .mgr
            .get("nordbank")
            .unwrap()
            .config
            .dns_domains
            .contains(&"acme.internal".to_owned()),
        "the domain typed in New tunnel"
    );

    // Remembered ("Remember the password in your keyring" is on): off and
    // on again, craked asks and the app answers from the keyring, with no
    // window.
    assert!(sign.read_with(cx, |v, _| *v.stage() == signin_view::Stage::Up));
    cx.executor().advance_clock(Duration::from_secs(5));
    until(cx, "sign-in closed", Duration::from_secs(10), |cx| {
        window::<signin_view::SignIn>(cx).is_none()
    });
    request(cx, Request::Switch("nordbank".into(), false));
    until(cx, "Nordbank off", Duration::from_secs(15), |cx| {
        state(cx, "nordbank") == "off"
    });
    request(cx, Request::Switch("nordbank".into(), true));
    // The kept password is read from the keyring off the main thread.
    cx.executor().advance_clock(Duration::from_secs(1));
    until(
        cx,
        "Nordbank connected with the kept password",
        Duration::from_secs(30),
        |cx| state(cx, "nordbank") == "connected",
    );
    assert!(
        window::<signin_view::SignIn>(cx).is_none(),
        "nothing asked the second time"
    );

    request(cx, Request::Switch("nordbank".into(), false));
    until(cx, "Nordbank off again", Duration::from_secs(15), |cx| {
        state(cx, "nordbank") == "off"
    });
    assert!(!sh("ip link").contains("crake-nordbank"));
    let crate::journey_tests::Daemon { rt, mgr } = craked;
    rt.block_on(mgr.shutdown());
}

/// The e2e gateway's address for its connection with a certificate
/// (scripts/e2e/ipsec-cert.conf).
const CERT_GATEWAY: &str = "198.51.100.11";

/// A gateway with a certificate no authority here vouches for
/// (design/proposals/ipsec-certificates.html): New tunnel's "Its
/// certificate", no key; switched on, the password, then the trust
/// question in the sign-in window, saying whose certificate it is and who
/// signed it; trusted, it connects.
#[gpui_kit::test]
fn an_ipsec_gateway_with_a_certificate_trusted_in_the_sign_in_window(cx: &mut TestAppContext) {
    if !enabled() || std::env::var_os("CRAKE_E2E_IPSEC").is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let craked = app_with_craked(cx, dir.path());

    request(cx, Request::NewTunnel(None));
    let (n, view) = wait_window::<crate::new_tunnel::NewTunnel>(cx, "New tunnel opens");
    cx.simulate_window_resize(n, gpui_kit::size(gpui_kit::px(720.), gpui_kit::px(1000.)));
    let (name, gateway, user) = view.read_with(cx, |v, _| v.fields_for_tests());
    let (_, domains, identity) = view.read_with(cx, |v, _| v.ipsec_fields_for_tests());
    type_into(cx, n, name, "Globex");
    click(cx, n, ("protocol", IPSEC));
    type_into(cx, n, gateway, CERT_GATEWAY);
    click(cx, n, "ipsec-certificate");
    type_into(cx, n, domains, "acme.internal");
    click(cx, n, "next");
    type_into(cx, n, user, "alice");
    click(cx, n, "advanced");
    type_into(cx, n, identity, "vpn-cert.acme.example");
    click(cx, n, "next");
    click(cx, n, "next");
    until(
        cx,
        "Globex added in craked",
        Duration::from_secs(10),
        |cx| {
            craked.mgr.get("globex").is_some()
                && model(cx).tunnels.iter().any(|t| t.name == "globex")
        },
    );
    assert_eq!(
        craked.mgr.own_fields("globex").unwrap()["gateway_proof"],
        "certificate"
    );

    cx.executor().advance_clock(Duration::from_secs(1));
    let (w, sign) = sign_in_of(cx, "globex");
    until(
        cx,
        "asked for the password",
        Duration::from_secs(20),
        |cx| {
            sign.read_with(cx, |v, _| {
            matches!(v.stage(), signin_view::Stage::Asking(p) if p.ask == crate::signin::Ask::Secret)
        })
        },
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.input("hunter2", cx);
    })
    .unwrap();
    click(cx, w, "continue");
    until(
        cx,
        "asked to trust the gateway",
        Duration::from_secs(30),
        |cx| {
            sign.read_with(cx, |v, _| {
            matches!(v.stage(), signin_view::Stage::Asking(p) if matches!(&p.ask,
                crate::signin::Ask::TrustServer { about: Some(a), fingerprint: Some(_), trusted_on: None }
                    if a.contains("vpn-cert.acme.example") && a.contains("ACME IPsec Root")))
        })
        },
    );
    click(cx, w, "yes");
    until(cx, "Globex connected", Duration::from_secs(30), |cx| {
        state(cx, "globex") == "connected"
    });

    request(cx, Request::Switch("globex".into(), false));
    until(cx, "Globex off", Duration::from_secs(15), |cx| {
        state(cx, "globex") == "off"
    });
    let crate::journey_tests::Daemon { rt, mgr } = craked;
    rt.block_on(mgr.shutdown());
}

/// The user's own certificate (design/proposals/ipsec-certificates.html,
/// 2): chosen in New tunnel's step 2 ("Certificate", a .p12 file), kept in
/// the keyring; switched on, the app hands it to craked without a window,
/// the sign-in window asks its password ("Unlock your certificate"), then
/// whether to trust the gateway; it connects with no password asked of
/// the gateway.
#[gpui_kit::test]
fn an_ipsec_tunnel_with_the_users_own_certificate(cx: &mut TestAppContext) {
    if !enabled() || std::env::var_os("CRAKE_E2E_IPSEC").is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let craked = app_with_craked(cx, dir.path());
    let p12 = std::fs::read("/mnt/link/ipsec-alice.p12").expect("alice's .p12 from the gateway");

    request(cx, Request::NewTunnel(None));
    let (n, view) = wait_window::<crate::new_tunnel::NewTunnel>(cx, "New tunnel opens");
    cx.simulate_window_resize(n, gpui_kit::size(gpui_kit::px(720.), gpui_kit::px(1000.)));
    let (name, gateway, _) = view.read_with(cx, |v, _| v.fields_for_tests());
    let (_, domains, identity) = view.read_with(cx, |v, _| v.ipsec_fields_for_tests());
    type_into(cx, n, name, "Initech");
    click(cx, n, ("protocol", IPSEC));
    type_into(cx, n, gateway, CERT_GATEWAY);
    click(cx, n, "ipsec-certificate");
    type_into(cx, n, domains, "acme.internal");
    click(cx, n, "next");
    click(cx, n, ("ipsec-method", 1usize));
    // The file chooser is the system's: the file as it would pick it.
    view.update(cx, |v, cx| {
        v.choose_certificate_for_tests("alice.p12", p12, cx)
    });
    click(cx, n, "advanced");
    type_into(cx, n, identity, "vpn-certs.acme.example");
    click(cx, n, "next");
    click(cx, n, "next");
    until(cx, "Initech added", Duration::from_secs(10), |cx| {
        craked.mgr.get("initech").is_some() && model(cx).tunnels.iter().any(|t| t.name == "initech")
    });

    cx.executor().advance_clock(Duration::from_secs(1));
    let (w, sign) = sign_in_of(cx, "initech");
    until(
        cx,
        "asked to unlock the certificate",
        Duration::from_secs(20),
        |cx| {
            sign.read_with(cx, |v, _| {
                matches!(v.stage(), signin_view::Stage::Asking(p)
                if matches!(p.ask, crate::signin::Ask::CertificatePassword { .. }))
            })
        },
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.input("hunter2", cx);
    })
    .unwrap();
    click(cx, w, "continue");
    until(
        cx,
        "asked to trust the gateway",
        Duration::from_secs(30),
        |cx| {
            sign.read_with(cx, |v, _| {
                matches!(v.stage(), signin_view::Stage::Asking(p)
                if matches!(p.ask, crate::signin::Ask::TrustServer { .. }))
            })
        },
    );
    click(cx, w, "yes");
    until(cx, "Initech connected", Duration::from_secs(30), |cx| {
        state(cx, "initech") == "connected"
    });

    request(cx, Request::Switch("initech".into(), false));
    until(cx, "Initech off", Duration::from_secs(15), |cx| {
        state(cx, "initech") == "off"
    });
    let crate::journey_tests::Daemon { rt, mgr } = craked;
    rt.block_on(mgr.shutdown());
}
