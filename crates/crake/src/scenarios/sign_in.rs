//! Signing in: New tunnel finding the gateway, a wrong password asked
//! again, and switching on again with what each kind of gateway keeps.

use super::{Up, WAIT, sign_in_and_connect, state};
use crate::client::Command;

/// A wrong password is refused and asked again, saying so; the right one
/// connects.
pub(crate) async fn a_wrong_password_is_asked_again(up: &mut Up) {
    up.off().await;
    up.add_again().await;
    sign_in_and_connect(&mut up.app, up.gateway, &["hunter3", "hunter2"]).await;
    up.device = up.crake.device(up.name);
}

/// Switched off and on again: AnyConnect and OpenVPN connect with the
/// sign-in craked kept (the gateway's cookie, OpenVPN's token), asking
/// nothing; IPsec asks the password again (the app answers it from the
/// keyring, which these tests don't use).
pub(crate) async fn switching_on_again_uses_what_was_kept(up: &mut Up) {
    up.off().await;
    let name = up.name;
    if !up.gateway.keeps_sign_in() {
        sign_in_and_connect(&mut up.app, up.gateway, &["hunter2"]).await;
    } else {
        // The flag may follow "off" by a moment (CI's Mac, 2026-10-20).
        up.app
            .until("its sign-in kept while off", |m| {
                m.tunnels.iter().any(|t| t.name == name && t.signed_in)
            })
            .await;
        up.app.send(Command::Connect(name.into()));
        up.app
            .within(WAIT, "connected with the kept sign-in", |m| {
                state(m, name) == "connected"
            })
            .await;
    }
    up.device = up.crake.device(name);
}

/// New tunnel's Detect against the AnyConnect gateway: an AnyConnect
/// gateway, and when its certificate runs out, read in a handshake (the
/// test gateway's is valid for ten years).
pub(crate) async fn new_tunnel_finds_the_gateway_and_its_certificate(_up: &mut Up) {
    let gateway = super::platform::anyconnect_gateway();
    let found = tokio::task::spawn_blocking(move || {
        crate::detect::gateway(&gateway, std::time::Duration::from_secs(15))
    })
    .await
    .unwrap();
    let crate::detect::Found::AnyConnect {
        expires: Some(expires),
        ..
    } = found
    else {
        panic!("not an AnyConnect gateway with a certificate: {found:?}");
    };
    let year = jiff::SignedDuration::from_hours(365 * 24);
    assert!(
        expires > jiff::Timestamp::now() + year,
        "valid for years: {expires}"
    );
}
