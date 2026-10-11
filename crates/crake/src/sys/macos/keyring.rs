//! The login Keychain (generic passwords: service "Crake", account
//! `<tunnel>/<kind>`, labelled as on the other systems).

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password_options,
};
use security_framework::passwords_options::PasswordOptions;

use crate::keyring::Kind;

const SERVICE: &str = "Crake";

fn account(tunnel: &str, kind: Kind) -> String {
    format!("{tunnel}/{}", kind.as_str())
}

pub async fn get(tunnel: &str, kind: Kind) -> Option<Vec<u8>> {
    get_generic_password(SERVICE, &account(tunnel, kind)).ok()
}

pub async fn set(tunnel: &str, display: &str, kind: Kind, secret: &[u8]) -> Result<(), String> {
    forget(tunnel, kind).await;
    let mut options = PasswordOptions::new_generic_password(SERVICE, &account(tunnel, kind));
    options.set_label(&kind.label(display));
    set_generic_password_options(secret, options).map_err(|e| e.to_string())
}

pub async fn forget(tunnel: &str, kind: Kind) {
    let _ = delete_generic_password(SERVICE, &account(tunnel, kind));
}
