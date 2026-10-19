//! OpenVPN's browser sign-in through Cloudflare, on CI's Mac and Windows.

use super::*;

/// OpenVPN's browser sign-in (the user's own company gateway's kind), on
/// CI's Mac and Windows: the gateway asks for a page in the browser, the
/// page is opened (here: fetched through the sign-in page's Cloudflare
/// route, CRAKE_GATEWAY_SSO), and the tunnel comes up. Skipped where
/// Cloudflare doesn't reach the page.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openvpn_browser_sign_in_through_cloudflare() {
    let Some(page) = std::env::var("CRAKE_GATEWAY_SSO")
        .ok()
        .filter(|p| !p.is_empty())
    else {
        return;
    };
    if !enabled() {
        return;
    }
    let remote = std::env::var("CRAKE_GATEWAY_OPENVPN").expect("CRAKE_GATEWAY_OPENVPN");
    let ca_file = std::env::var("CRAKE_GATEWAY_CA").expect("CRAKE_GATEWAY_CA");
    let ca = std::fs::read_to_string(&ca_file).unwrap();
    let profile = format!(
        "client\ndev tun\nproto tcp-client\nremote {remote}\nnobind\n\
         remote-cert-tls server\n<ca>\n{ca}</ca>\n\
         <auth-user-pass>\nsso-placeholder\nsso-placeholder\n</auth-user-pass>\n"
    );
    let mut crake = Crake::start().await;
    crake
        .add(&[
            ("display_name", "Initech"),
            ("protocol", "openvpn"),
            ("profile", profile.as_str()),
        ])
        .await;
    let app = &mut crake.app;
    app.send(Command::Connect("initech".into()));
    let ask = app.asked("the sign-in page").await;
    let url = ask.get("url").cloned().unwrap_or_default();
    // The page's own address names the gateway's side; it's reached here
    // through Cloudflare, at `page`.
    let path = url
        .strip_prefix("http://")
        .and_then(|rest| rest.find('/').map(|i| &rest[i..]))
        .unwrap_or_else(|| panic!("a sign-in page: {ask:?}"));
    let said = fetch(&page, path);
    assert!(said.contains("Signed in"), "{said}");
    app.within(WAIT, "Initech connected", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "connected")
    })
    .await;
    assert!(
        get_within("10.40.1.10")
            .await
            .is_some_and(|b| b.contains("crex")),
        "git through Initech"
    );
    let app = &mut crake.app;
    app.send(Command::Disconnect("initech".into()));
    app.within(WAIT, "Initech off", |m| {
        m.tunnels
            .iter()
            .any(|t| t.name == "initech" && t.state == "off")
    })
    .await;
    crake.stop().await;
}

/// An HTTP GET of `path` at `host` (host:port), its whole answer.
fn fetch(host: &str, path: &str) -> String {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(host).unwrap();
    s.set_read_timeout(Some(WAIT)).unwrap();
    write!(s, "GET {path} HTTP/1.0\r\nHost: {host}\r\n\r\n").unwrap();
    let mut body = String::new();
    let _ = s.read_to_string(&mut body);
    body
}
