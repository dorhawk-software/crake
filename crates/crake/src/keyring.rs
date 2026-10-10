//! What Crake keeps in the GNOME keyring (Secret Service): a tunnel's
//! certificate password and, if the user chose so, the certificate itself.
//! Encrypted by the keyring, unlocked at login; never in Crake's own files.

/// What a secret is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    CertificatePassword,
    Certificate,
    /// The gateway password (sign-in "Username and password").
    Password,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::CertificatePassword => "certificate-password",
            Self::Certificate => "certificate",
            Self::Password => "password",
        }
    }

    fn label(self, display: &str) -> String {
        match self {
            Self::CertificatePassword => format!("Crake: certificate password for {display}"),
            Self::Certificate => format!("Crake: certificate for {display}"),
            Self::Password => format!("Crake: password for {display}"),
        }
    }
}

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
        Kind::CertificatePassword | Kind::Password => {
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
