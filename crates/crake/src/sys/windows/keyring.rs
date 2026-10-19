//! Credential Manager (generic credentials, this user, this computer). A
//! credential holds at most 2560 bytes, so a certificate is kept in parts:
//! `Crake/<tunnel>/<kind>/0`, `/1`, ...

use crake_os::windows::credentials;

use crate::keyring::Kind;

fn target(tunnel: &str, kind: Kind, part: usize) -> String {
    format!("Crake/{tunnel}/{}/{part}", kind.as_str())
}

pub async fn get(tunnel: &str, kind: Kind) -> Option<Vec<u8>> {
    let mut secret = Vec::new();
    for part in 0.. {
        let Some(blob) = credentials::read(&target(tunnel, kind, part)) else {
            break;
        };
        secret.extend_from_slice(&blob);
    }
    (!secret.is_empty()).then_some(secret)
}

pub async fn set(tunnel: &str, display: &str, kind: Kind, secret: &[u8]) -> Result<(), String> {
    forget(tunnel, kind).await;
    let comment = kind.label(display);
    for (part, chunk) in secret.chunks(credentials::MAX_BLOB).enumerate() {
        if let Err(e) = credentials::write(&target(tunnel, kind, part), &comment, "crake", chunk) {
            forget(tunnel, kind).await;
            return Err(e.to_string());
        }
    }
    Ok(())
}

pub async fn forget(tunnel: &str, kind: Kind) {
    for part in 0.. {
        if !credentials::delete(&target(tunnel, kind, part)) {
            break;
        }
    }
}
