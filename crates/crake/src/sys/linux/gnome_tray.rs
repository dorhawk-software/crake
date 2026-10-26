//! GNOME and its AppIndicator extension, for when nothing shows tray icons
//! (`crate::no_tray`): what GNOME Shell says about the extension
//! (org.gnome.Shell.Extensions on the session bus), switching it on, and
//! marking a newly installed one to be on at the next login (GNOME's
//! enabled-extensions, through `gsettings`).

use std::collections::HashMap;
use std::process::Command;

use zbus::Connection;
use zbus::OwnedVariant;

use crate::no_tray::{EXTENSION, Gnome, Known, Remedy, UBUNTU_EXTENSION};

const SHELL: &str = "org.gnome.Shell";
const SHELL_PATH: &str = "/org/gnome/Shell";
const EXTENSIONS: &str = "org.gnome.Shell.Extensions";
const SCHEMA: &str = "org.gnome.shell";
const ENABLED: &str = "enabled-extensions";

/// What would bring the bird back here. On GNOME, a newly installed
/// extension is marked to be on at the next login as a side effect, so the
/// row's "Log out and back in, and the bird appears" is true.
pub fn look() -> Remedy {
    let gnome = run(gnome());
    let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let remedy = crate::no_tray::decide(gnome.as_ref(), &os);
    if remedy == (Remedy::NextLogin { marked: true }) && !mark_for_login() {
        return Remedy::NextLogin { marked: false };
    }
    remedy
}

/// Asks GNOME Shell to switch the extension on; then what would bring the
/// bird back if no tray came of it (`no_tray::refused`).
pub fn switch_on(id: &str) -> Remedy {
    run(async {
        let conn = Connection::session().await.ok()?;
        let on: bool = call(&conn, "EnableExtension", &(id,)).await.ok()?;
        on.then_some(())
    });
    // The tray, when it comes, says so itself (`watcher_online`); this is
    // what the row says if it doesn't.
    run(gnome())
        .map(|g| crate::no_tray::refused(&g))
        .unwrap_or(Remedy::NoTray)
}

/// GNOME's Extensions app, where a refusal is explained.
pub fn open_extensions() {
    let opened = Command::new("gnome-extensions-app").spawn().is_ok()
        || Command::new("gtk-launch")
            .arg("org.gnome.Extensions")
            .spawn()
            .is_ok();
    if !opened {
        crate::debug("couldn't open GNOME's Extensions app");
    }
}

fn run<T>(f: impl Future<Output = Option<T>>) -> Option<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?
        .block_on(f)
}

async fn call<B, R>(conn: &Connection, method: &str, body: &B) -> zbus::Result<R>
where
    B: serde::Serialize + zbus::DynamicType,
    R: for<'d> zbus::wire::DynamicDeserialize<'d>,
{
    conn.call_method(Some(SHELL), SHELL_PATH, Some(EXTENSIONS), method, body)
        .await?
        .body()
        .deserialize()
}

/// What GNOME says; None when this isn't GNOME (or its Shell doesn't answer).
async fn gnome() -> Option<Gnome> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if !desktop.split(':').any(|d| d.eq_ignore_ascii_case("gnome")) {
        return None;
    }
    let conn = Connection::session().await.ok()?;
    let user_extensions: OwnedVariant = conn
        .call_method(
            Some(SHELL),
            SHELL_PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(EXTENSIONS, "UserExtensionsEnabled"),
        )
        .await
        .ok()?
        .body()
        .deserialize()
        .ok()?;
    let mut known = None;
    for id in [UBUNTU_EXTENSION, EXTENSION] {
        let info: HashMap<String, OwnedVariant> =
            call(&conn, "GetExtensionInfo", &(id,)).await.ok()?;
        if let Some(k) = known_of(&info) {
            known = Some((id.to_owned(), k));
            break;
        }
    }
    Some(Gnome {
        known,
        installed: installed(EXTENSION),
        user_extensions: bool::try_from(user_extensions).unwrap_or(true),
        writable: gsettings(&["writable", SCHEMA, ENABLED]).as_deref() == Some("true"),
    })
}

/// GNOME's answer for an extension it knows (an empty one for others).
fn known_of(info: &HashMap<String, OwnedVariant>) -> Option<Known> {
    let state = f64::try_from(info.get("state")?.try_clone().ok()?).ok()?;
    let enabled = info
        .get("enabled")
        .and_then(|v| bool::try_from(v.try_clone().ok()?).ok())
        .unwrap_or(false);
    Some(Known {
        // GNOME sends its small state numbers as doubles.
        state: state as u32,
        enabled,
    })
}

/// Its files are where GNOME Shell looks for extensions.
fn installed(id: &str) -> bool {
    let home = std::env::var("XDG_DATA_HOME").ok().or_else(|| {
        std::env::var("HOME")
            .ok()
            .map(|h| format!("{h}/.local/share"))
    });
    let system =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(system.split(':').map(str::to_owned))
        .any(|dir| {
            std::path::Path::new(&dir)
                .join("gnome-shell/extensions")
                .join(id)
                .join("metadata.json")
                .exists()
        })
}

/// Puts the extension in GNOME's enabled-extensions, for the next login.
fn mark_for_login() -> bool {
    let Some(list) = gsettings(&["get", SCHEMA, ENABLED]) else {
        return false;
    };
    let mut ids = parse_list(&list);
    if ids.iter().any(|id| id == EXTENSION) {
        return true;
    }
    ids.push(EXTENSION.to_owned());
    let value = format!(
        "[{}]",
        ids.iter()
            .map(|id| format!("'{id}'"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    gsettings(&["set", SCHEMA, ENABLED, &value]).is_some()
}

/// GVariant's text for a list of strings: "['a', 'b']" or "@as []".
fn parse_list(text: &str) -> Vec<String> {
    text.trim()
        .trim_start_matches("@as")
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('\'').to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn gsettings(args: &[&str]) -> Option<String> {
    let out = Command::new("gsettings").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsettings_lists_are_read() {
        assert_eq!(parse_list("@as []"), Vec::<String>::new());
        assert_eq!(
            parse_list("['a@b.c', 'ding@rastersoft.com']"),
            vec!["a@b.c".to_owned(), "ding@rastersoft.com".to_owned()]
        );
    }

    #[test]
    fn what_gnome_says_of_an_extension() {
        let mut info = HashMap::new();
        assert_eq!(known_of(&info), None, "unknown: an empty answer");
        info.insert("state".to_owned(), OwnedVariant::from(2.0_f64));
        info.insert("enabled".to_owned(), OwnedVariant::from(false));
        assert_eq!(
            known_of(&info),
            Some(Known {
                state: 2,
                enabled: false
            })
        );
    }
}
