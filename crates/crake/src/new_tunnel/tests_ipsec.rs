//! New tunnel's IPsec window tests (design/proposals/ipsec-tunnel.html
//! and ipsec-certificates.html), apart from tests.rs for its size. IPsec
//! is Linux's for now: elsewhere its card says "Not available".

use gpui_kit::TestAppContext;

use super::tests::{ANYCONNECT, open};
use super::*;

/// Where the IPsec card sits in the protocol list (`PROTOCOLS`).
const IPSEC: usize = 9;

/// IPsec (design/proposals/ipsec-tunnel.html, chosen 2026-10-19): chosen,
/// the list folds and Import .ovpn… goes; the gateway's key is asked
/// before Continue, the username before step 3; the key, the gateway's
/// identity and the domains go to craked with the rest.
// IPsec is Linux's for now: elsewhere its card says "Not available".
#[cfg(target_os = "linux")]
#[gpui_kit::test]
fn an_ipsec_gateway_with_its_key(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Nordbank", cx);
    assert!(
        w.shown("import-ovpn", cx),
        "the import while Detect is chosen"
    );
    w.click(("protocol", IPSEC), cx);
    assert!(w.shown("protocols-more", cx), "folded after the choice");
    assert!(
        !w.shown(("protocol", ANYCONNECT), cx),
        "the rest folded away"
    );
    assert!(!w.shown("import-ovpn", cx), "no import with IPsec chosen");
    w.type_into(|t| t.gateway.clone(), "gw.nordbank.example", cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 1, "no key yet");
    assert!(
        w.read(cx, |t| t.problem.clone())
            .unwrap_or_default()
            .contains("key"),
        "the key is asked for"
    );
    w.type_into(|t| t.ipsec.key.clone(), "hunter2hunter2", cx);
    w.type_into(
        |t| t.ipsec.domains.clone(),
        "nordbank.internal nordbank.example",
        cx,
    );
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2);

    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2, "no username yet");
    w.type_into(|t| t.user.clone(), "alice", cx);
    w.click("advanced", cx);
    w.type_into(|t| t.ipsec.identity.clone(), "vpn.nordbank.example", cx);
    w.click("next", cx);
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["protocol"], "ipsec");
    assert_eq!(fields["gateway"], "gw.nordbank.example");
    assert_eq!(fields["gateway_key"], "hunter2hunter2");
    assert_eq!(fields["gateway_id"], "vpn.nordbank.example");
    assert_eq!(fields["domains"], "nordbank.internal\nnordbank.example");
    assert_eq!(fields["sign_in"], "password");
    assert_eq!(fields["user"], "alice");
}

/// An IPsec gateway with a certificate (design/proposals/ipsec-certificates.html,
/// 1, A): "Its certificate" asks for no key, says what's checked, takes IT's
/// authority in the key's place (a file that isn't one is said and holds
/// Continue), and sends no key.
#[cfg(target_os = "linux")]
#[gpui_kit::test]
fn an_ipsec_gateway_with_its_certificate(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Nordbank", cx);
    w.click(("protocol", IPSEC), cx);
    w.type_into(|t| t.gateway.clone(), "gw.nordbank.example", cx);
    assert!(!w.read(cx, |t| t.ipsec.certificate), "a key at first");
    w.click("ipsec-certificate", cx);
    assert!(w.read(cx, |t| t.ipsec.certificate));
    assert!(
        w.shown("ipsec-choose-ca", cx),
        "IT's authority in the key's place"
    );
    let line = w.view.read_with(cx, |v, cx| v.proof_line(cx));
    assert!(line.contains("is for gw.nordbank.example"), "{line}");

    // A file that isn't an authority: said, and Continue waits.
    w.view.update(cx, |v, cx| {
        v.set_authority(
            "nordbank.ovpn",
            Ok(b"client\nremote gw 1194\n".to_vec()),
            cx,
        )
    });
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 1);
    assert!(
        w.read(cx, |t| t.problem.clone())
            .unwrap_or_default()
            .contains("isn\u{2019}t a certificate"),
    );
    let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----";
    w.view.update(cx, |v, cx| {
        v.set_authority("nordbank-ca.pem", Ok(pem.as_bytes().to_vec()), cx)
    });
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2, "no key needed");
    w.type_into(|t| t.user.clone(), "alice", cx);
    w.click("next", cx);
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["gateway_proof"], "certificate");
    assert_eq!(fields["gateway_ca"], pem);
    assert!(!fields.contains_key("gateway_key"), "{fields:?}");
}

/// A gateway typed as an address: few certificates name one, so the line
/// says the identity can be given instead (ipsec-certificates.html, 4).
#[cfg(target_os = "linux")]
#[gpui_kit::test]
fn an_ipsec_gateway_by_its_address_points_to_its_identity(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Nordbank", cx);
    w.click(("protocol", IPSEC), cx);
    w.type_into(|t| t.gateway.clone(), "203.0.113.5", cx);
    w.click("ipsec-certificate", cx);
    let line = w.view.read_with(cx, |v, cx| v.proof_line(cx));
    assert!(
        line.starts_with("Its certificate must be for 203.0.113.5."),
        "{line}"
    );
    assert!(line.contains("Gateway\u{2019}s identity"), "{line}");
    // Back to a key: the key is asked for again, naming the other way.
    w.click("ipsec-key", cx);
    w.click("next", cx);
    assert!(
        w.read(cx, |t| t.problem.clone())
            .unwrap_or_default()
            .contains("or choose Its certificate"),
    );
}

/// IPsec's step 2 (design/proposals/ipsec-certificates.html, 2, A): three
/// cards; "Certificate" needs the file and no username; "Certificate and
/// password" needs both; the file goes to the keyring with Add.
#[cfg(target_os = "linux")]
#[gpui_kit::test]
fn an_ipsec_tunnel_signs_in_with_a_certificate(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Nordbank", cx);
    w.click(("protocol", IPSEC), cx);
    w.type_into(|t| t.gateway.clone(), "gw.nordbank.example", cx);
    w.click("ipsec-certificate", cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2);

    // Certificate alone: the file, and no username.
    w.click(("ipsec-method", 1usize), cx);
    assert!(w.shown("ipsec-choose-cert", cx));
    w.click("next", cx);
    assert_eq!(
        w.read(cx, |t| t.problem.clone()).as_deref(),
        Some("Choose the certificate file.")
    );
    w.view.update(cx, |v, cx| {
        v.cert_file = Some(("alice.p12".into(), vec![1, 2, 3]));
        cx.notify();
    });
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 3, "no username asked");

    // With a password as well: the username too.
    w.click("back", cx);
    w.click(("ipsec-method", 2usize), cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2, "no username yet");
    w.type_into(|t| t.user.clone(), "alice", cx);
    w.click("next", cx);
    w.click("next", cx);
    match w.requests.try_recv() {
        Ok(Request::AddTunnel(fields, Some(file))) => {
            assert_eq!(fields["sign_in"], "certificate-password");
            assert_eq!(fields["certificate"], "keyring");
            assert_eq!(fields["user"], "alice");
            assert_eq!(file, [1, 2, 3], "the file, for the keyring");
        }
        other => panic!("expected AddTunnel with the file, got {other:?}"),
    }
}
