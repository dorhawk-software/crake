//! The GNOME keyring (Secret Service, through oo7).

use crate::keyring::Kind;

fn attributes(tunnel: &str, kind: Kind) -> Vec<(&'static str, String)> {
    vec![
        ("application", "crake".to_owned()),
        ("tunnel", tunnel.to_owned()),
        ("kind", kind.as_str().to_owned()),
    ]
}

pub async fn get(tunnel: &str, kind: Kind) -> Option<Vec<u8>> {
    let keyring = oo7::Keyring::new().await.ok()?;
    keyring.unlock().await.ok()?;
    let items = keyring.search_items(&attributes(tunnel, kind)).await.ok()?;
    let item = items.first()?;
    item.unlock().await.ok()?;
    Some(item.secret().await.ok()?.as_bytes().to_vec())
}

pub async fn set(tunnel: &str, display: &str, kind: Kind, secret: &[u8]) -> Result<(), String> {
    let keyring = oo7::Keyring::new().await.map_err(|e| e.to_string())?;
    keyring.unlock().await.map_err(|e| e.to_string())?;
    let secret = match kind {
        Kind::CertificatePassword | Kind::Password | Kind::Username => {
            oo7::Secret::text(String::from_utf8_lossy(secret))
        }
        Kind::Certificate => oo7::Secret::blob(secret),
    };
    keyring
        .create_item(
            &kind.label(display),
            &attributes(tunnel, kind),
            secret,
            true,
        )
        .await
        .map_err(|e| e.to_string())
}

pub async fn forget(tunnel: &str, kind: Kind) {
    if let Ok(keyring) = oo7::Keyring::new().await {
        let _ = keyring.delete(&attributes(tunnel, kind)).await;
    }
}
