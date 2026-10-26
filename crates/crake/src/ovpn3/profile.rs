//! A profile's text, read and replaced, for the profile editor
//! (design/proposals/edit-ovpn-profile.html). openvpn3 can't change a
//! profile in place: the new text is imported under the same name, the
//! old profile's tags and overrides are carried over, and only then is the
//! old one removed. Blocking: call these off the UI thread.

use std::collections::HashMap;

use zbus::Connection;
use zbus::{OwnedObjectPath, OwnedVariant};

use super::{CONFIGURATION, call, patiently, property};

/// A profile, as openvpn3 shows it to its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetched {
    Text(String),
    /// Locked by whoever imported it (`locked_down`): its text can't be read.
    Locked,
    /// No profile by that name (removed meanwhile).
    Missing,
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())
}

/// openvpn3's own words for a refusal.
fn said(e: zbus::Error) -> String {
    crate::client::error_text(&e).unwrap_or_else(|| e.to_string())
}

/// The profiles named `name`.
async fn named(conn: &Connection, name: &str) -> Result<Vec<OwnedObjectPath>, String> {
    let body = (name,);
    patiently(|| {
        call::<_, Vec<OwnedObjectPath>>(
            conn,
            CONFIGURATION,
            "/net/openvpn/v3/configuration",
            CONFIGURATION,
            "LookupConfigName",
            &body,
        )
    })
    .await
    .map_err(said)
}

/// The text of the profile `name`.
pub fn fetch(name: &str) -> Result<Fetched, String> {
    runtime()?.block_on(async {
        let conn = Connection::system().await.map_err(|e| e.to_string())?;
        let Some(path) = named(&conn, name).await?.into_iter().next() else {
            return Ok(Fetched::Missing);
        };
        let locked: bool = property(
            &conn,
            CONFIGURATION,
            path.as_str(),
            CONFIGURATION,
            "locked_down",
        )
        .await
        .unwrap_or(false);
        if locked {
            return Ok(Fetched::Locked);
        }
        match call::<_, String>(
            &conn,
            CONFIGURATION,
            path.as_str(),
            CONFIGURATION,
            "Fetch",
            &(),
        )
        .await
        {
            Ok(text) => Ok(Fetched::Text(text)),
            // Not the owner's to read (shared with the user, say).
            Err(e) if crate::client::error_name(&e).is_some() => Ok(Fetched::Locked),
            Err(e) => Err(e.to_string()),
        }
    })
}

/// Whether the profile `name` is locked (its text can't be read), or None
/// when openvpn3 can't say (not running, no such profile).
pub fn locked(name: &str) -> Option<bool> {
    runtime().ok()?.block_on(async {
        let conn = Connection::system().await.ok()?;
        let path = named(&conn, name).await.ok()?.into_iter().next()?;
        property(
            &conn,
            CONFIGURATION,
            path.as_str(),
            CONFIGURATION,
            "locked_down",
        )
        .await
        .ok()
    })
}

/// Give openvpn3 `text` as the profile `name`: imported (kept as the old one
/// was), the old one's tags and overrides set on it, then the old one
/// removed. openvpn3 refusing the text changes nothing.
pub fn replace(name: &str, text: &str) -> Result<(), String> {
    runtime()?.block_on(async {
        let conn = Connection::system().await.map_err(|e| e.to_string())?;
        let old = named(&conn, name).await?;
        let first = old.first().map(|p| p.as_str().to_owned());
        let read = async |what: &str| -> Option<OwnedVariant> {
            property::<OwnedVariant>(&conn, CONFIGURATION, first.as_deref()?, CONFIGURATION, what)
                .await
                .ok()
        };
        let persistent = read("persistent")
            .await
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(true);
        let tags: Vec<String> = read("tags")
            .await
            .and_then(|v| Vec::<String>::try_from(v).ok())
            .unwrap_or_default();
        let overrides: HashMap<String, OwnedVariant> = read("overrides")
            .await
            .and_then(|v| HashMap::<String, OwnedVariant>::try_from(v).ok())
            .unwrap_or_default();
        let new: OwnedObjectPath = call(
            &conn,
            CONFIGURATION,
            "/net/openvpn/v3/configuration",
            CONFIGURATION,
            "Import",
            // Not single use; kept on disk as the old one was.
            &(name, text, false, persistent),
        )
        .await
        .map_err(|e| format!("OpenVPN 3 refused it: \u{201c}{}\u{201d}. Nothing was changed.", said(e)))?;
        for tag in &tags {
            let _ = call::<_, ()>(&conn, CONFIGURATION, new.as_str(), CONFIGURATION, "AddTag", &(tag,))
                .await;
        }
        for (key, value) in &overrides {
            let value: &zbus::Variant = value;
            let _ = call::<_, ()>(
                &conn,
                CONFIGURATION,
                new.as_str(),
                CONFIGURATION,
                "SetOverride",
                &(key, value),
            )
            .await;
        }
        for path in &old {
            call::<_, ()>(&conn, CONFIGURATION, path.as_str(), CONFIGURATION, "Remove", &())
                .await
                .map_err(|e| {
                    format!(
                        "The new profile is in OpenVPN 3, but the old one couldn\u{2019}t be removed: {}",
                        said(e)
                    )
                })?;
        }
        Ok(())
    })
}

/// Against this computer's openvpn3, with a throwaway profile of its own
/// (never the user's): imported, read, replaced with its tag and override
/// kept, then removed. Off unless CRAKE_OPENVPN3_TEST=1.
#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "client\ndev tun\nremote vpn.initech.example 1194\n<ca>\n-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n</ca>\n";

    #[test]
    fn a_profile_is_read_and_replaced_keeping_its_tags_and_overrides() {
        if std::env::var("CRAKE_OPENVPN3_TEST").as_deref() != Ok("1") {
            eprintln!(
                "skipped: set CRAKE_OPENVPN3_TEST=1 (imports and removes a throwaway profile)"
            );
            return;
        }
        let name = format!("crake-test-{}", std::process::id());
        crate::ovpn3::import(&name, PROFILE).unwrap();
        let rt = runtime().unwrap();
        let path = rt.block_on(async {
            let conn = Connection::system().await.unwrap();
            let path = named(&conn, &name).await.unwrap().remove(0);
            call::<_, ()>(
                &conn,
                CONFIGURATION,
                path.as_str(),
                CONFIGURATION,
                "AddTag",
                &("crake-test",),
            )
            .await
            .unwrap();
            let value = zbus::Variant::from(true);
            call::<_, ()>(
                &conn,
                CONFIGURATION,
                path.as_str(),
                CONFIGURATION,
                "SetOverride",
                &("allow-compression", &value),
            )
            .await
            .ok();
            path
        });
        let Fetched::Text(text) = fetch(&name).unwrap() else {
            panic!("not readable")
        };
        assert!(text.contains("vpn.initech.example"), "{text}");
        replace(&name, &text.replace("vpn.initech", "vpn2.initech")).unwrap();
        let Fetched::Text(after) = fetch(&name).unwrap() else {
            panic!("not readable")
        };
        assert!(after.contains("vpn2.initech.example"), "{after}");
        let left = rt.block_on(async {
            let conn = Connection::system().await.unwrap();
            let paths = named(&conn, &name).await.unwrap();
            let tags: Vec<String> = property(
                &conn,
                CONFIGURATION,
                paths[0].as_str(),
                CONFIGURATION,
                "tags",
            )
            .await
            .unwrap();
            for p in &paths {
                let _ = call::<_, ()>(
                    &conn,
                    CONFIGURATION,
                    p.as_str(),
                    CONFIGURATION,
                    "Remove",
                    &(),
                )
                .await;
            }
            (paths.len(), paths[0] != path, tags)
        });
        assert_eq!(left.0, 1, "one profile under the name, the old one removed");
        assert!(left.1, "a new profile");
        assert!(
            left.2.contains(&"crake-test".to_owned()),
            "its tag kept: {:?}",
            left.2
        );
    }
}
