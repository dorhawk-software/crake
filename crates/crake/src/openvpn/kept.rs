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
}

/// The two questions kept, as craked's `kind`s name them.
pub fn kind(asked: &str) -> Option<Kind> {
    match asked {
        "user" => Some(Kind::Username),
        "password" => Some(Kind::Password),
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

/// Forget both (refused, or the tunnel removed). Blocking.
pub fn forget(tunnel: &str) {
    for kind in [Kind::Username, Kind::Password] {
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
