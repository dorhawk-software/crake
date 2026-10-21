//! An OpenVPN tunnel's username and password, kept once they've worked, as
//! AnyConnect's password is (the keyring; `keyring::Kind::Username` and
//! `Password`), and given to craked when it asks again: a password
//! sign-in asks once. Refused (craked asks again, `retry`), they're
//! forgotten and the window asks. Demo mode keeps nothing; tests keep them
//! in memory.

use crate::keyring::Kind;

/// What the user typed for a tunnel, until it's up (then it's kept).
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    pub user: Option<String>,
    pub password: Option<String>,
    /// The password of the user's certificate file (IPsec's "Unlock your
    /// certificate"; OpenVPN's profile key), kept as AnyConnect keeps it.
    pub key_password: Option<String>,
}

/// The two questions kept, as craked's `kind`s name them.
pub fn kind(asked: &str) -> Option<Kind> {
    match asked {
        "user" => Some(Kind::Username),
        "password" => Some(Kind::Password),
        "key-password" => Some(Kind::CertificatePassword),
        _ => None,
    }
}

/// How long the keyring gets to answer (it may ask to be unlocked).
#[cfg(not(test))]
const KEYRING_WAIT: std::time::Duration = std::time::Duration::from_secs(90);

#[cfg(test)]
static MEMORY: std::sync::Mutex<Option<std::collections::HashMap<(String, &'static str), String>>> =
    std::sync::Mutex::new(None);

/// What's kept for `tunnel`. Blocking: off the UI thread.
pub fn read(tunnel: &str, kind: Kind) -> Option<String> {
    #[cfg(test)]
    {
        MEMORY
            .lock()
            .unwrap()
            .get_or_insert_default()
            .get(&(tunnel.to_owned(), kind.as_str()))
            .cloned()
    }
    #[cfg(not(test))]
    {
        let bytes = block(crate::keyring::get(tunnel, kind))??;
        String::from_utf8(bytes).ok()
    }
}

/// Keep what worked. Blocking.
pub fn write(tunnel: &str, display: &str, typed: &Typed) {
    for (kind, value) in [
        (Kind::Username, &typed.user),
        (Kind::Password, &typed.password),
        (Kind::CertificatePassword, &typed.key_password),
    ] {
        let Some(value) = value else { continue };
        #[cfg(test)]
        {
            let _ = display;
            MEMORY
                .lock()
                .unwrap()
                .get_or_insert_default()
                .insert((tunnel.to_owned(), kind.as_str()), value.clone());
        }
        #[cfg(not(test))]
        {
            let _ = block(crate::keyring::set(tunnel, display, kind, value.as_bytes()));
        }
    }
}

/// The tunnel's certificate file, as New tunnel kept it in the keyring
/// (7e), in base64 for craked (IPsec asks for it). Blocking.
pub fn certificate(tunnel: &str) -> Option<String> {
    #[cfg(test)]
    {
        // Tests keep it in base64 already.
        MEMORY
            .lock()
            .unwrap()
            .get_or_insert_default()
            .get(&(tunnel.to_owned(), Kind::Certificate.as_str()))
            .cloned()
    }
    #[cfg(not(test))]
    {
        use base64::Engine as _;
        let bytes = block(crate::keyring::get(tunnel, Kind::Certificate))??;
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    }
}

/// A certificate kept for `tunnel` (base64), for tests.
#[cfg(test)]
pub fn keep_certificate_for_tests(tunnel: &str, b64: &str) {
    MEMORY.lock().unwrap().get_or_insert_default().insert(
        (tunnel.to_owned(), Kind::Certificate.as_str()),
        b64.to_owned(),
    );
}

/// Forget the username and password (refused, or the tunnel removed), or
/// only the certificate's password when that's what was refused. Blocking.
pub fn forget(tunnel: &str) {
    forget_kinds(tunnel, &[Kind::Username, Kind::Password]);
}

pub fn forget_key_password(tunnel: &str) {
    forget_kinds(tunnel, &[Kind::CertificatePassword]);
}

fn forget_kinds(tunnel: &str, kinds: &[Kind]) {
    for &kind in kinds {
        #[cfg(test)]
        {
            MEMORY
                .lock()
                .unwrap()
                .get_or_insert_default()
                .remove(&(tunnel.to_owned(), kind.as_str()));
        }
        #[cfg(not(test))]
        {
            let _ = block(crate::keyring::forget(tunnel, kind));
        }
    }
}

/// A keyring call on a runtime of its own, or None after `KEYRING_WAIT`.
#[cfg(not(test))]
fn block<T>(f: impl std::future::Future<Output = T>) -> Option<T> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    rt.block_on(async { tokio::time::timeout(KEYRING_WAIT, f).await.ok() })
}
