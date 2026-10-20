//! Signing in again: a wrong password asked again, and switching on
//! again with what each kind of gateway keeps.

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
        let kept = up
            .app
            .model
            .tunnels
            .iter()
            .any(|t| t.name == name && t.signed_in);
        assert!(kept, "{} keeps its sign-in while off", up.label);
        up.app.send(Command::Connect(name.into()));
        up.app
            .within(WAIT, "connected with the kept sign-in", |m| {
                state(m, name) == "connected"
            })
            .await;
    }
    up.device = up.crake.device(name);
}
