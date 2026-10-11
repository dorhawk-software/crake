//! The login Keychain (generic passwords: service "Crake", account
//! `<tunnel>/<kind>`).

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use crate::keyring::Kind;

const SERVICE: &str = "Crake";

fn account(tunnel: &str, kind: Kind) -> String {
    format!("{tunnel}/{}", kind.as_str())
}

pub async fn get(tunnel: &str, kind: Kind) -> Option<Vec<u8>> {
    get_generic_password(SERVICE, &account(tunnel, kind)).ok()
}

pub async fn set(tunnel: &str, _display: &str, kind: Kind, secret: &[u8]) -> Result<(), String> {
    set_generic_password(SERVICE, &account(tunnel, kind), secret).map_err(|e| e.to_string())
}

pub async fn forget(tunnel: &str, kind: Kind) {
    let _ = delete_generic_password(SERVICE, &account(tunnel, kind));
}
