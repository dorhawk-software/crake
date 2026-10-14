//! When there's a newer Crake (design/proposals/new-version.html):
//! - a package manager updated it: craked, restarted by the package, is
//!   newer than this app, which says "Crake was updated to X. Restart it to
//!   finish." (nothing leaves the computer);
//! - a new version is out, on an install nothing else updates (a Mac
//!   without Homebrew, Windows, a .deb or .rpm installed from a file): once
//!   a day Crake asks dorhawk.software for the newest version's number
//!   (`versions.json`, the file the website reads) and says "Crake X is
//!   out." Settings → General switches that off (on by default, decided by
//!   the user).

use std::path::Path;

/// What's asked: the newest version of each dorhawk.software app.
pub const URL: &str = "https://packages.dorhawk.software/versions.json";
/// A version's download page.
pub const RELEASES: &str = "https://github.com/dorhawk-software/crake/releases/tag/v";
/// How long `curl` gets.
const WAIT_SECS: &str = "15";
/// Set for the Crake that Restart starts: it waits for the old one to go.
pub const RESTARTING: &str = "CRAKE_RESTARTING";
/// Asked once a day (seconds)...
pub const EVERY: u64 = 24 * 60 * 60;
/// ...looked at once an hour while Crake runs.
pub const LOOK: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// This Crake's version.
pub fn this() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// `a` is a newer version than `b` (numbers compared part by part; a part
/// that isn't a number counts as 0).
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (parts(a), parts(b));
    (0..a.len().max(b.len()))
        .map(|i| {
            (
                a.get(i).copied().unwrap_or(0),
                b.get(i).copied().unwrap_or(0),
            )
        })
        .find(|(x, y)| x != y)
        .is_some_and(|(x, y)| x > y)
}

/// Crake's newest version from `versions.json`.
pub fn newest_in(json: &str) -> Option<String> {
    let all: serde_json::Value = serde_json::from_str(json).ok()?;
    Some(all.get("crake")?.get("version")?.as_str()?.to_owned())
}

/// A package manager updates this Crake, so it needn't ask: Linux with
/// dorhawk.software's apt or dnf repository, Arch (pacman), NixOS (the Nix
/// store); a Mac with the Homebrew cask. `exists` stands for the file
/// system (tests).
pub fn updated_elsewhere(exe: &Path, exists: impl Fn(&str) -> bool) -> bool {
    if exe.starts_with("/nix/store") {
        return true;
    }
    let managers: &[&str] = if cfg!(target_os = "macos") {
        &["/opt/homebrew/Caskroom/crake", "/usr/local/Caskroom/crake"]
    } else if cfg!(windows) {
        &[]
    } else {
        &[
            "/etc/apt/sources.list.d/dorhawk.sources",
            "/etc/apt/sources.list.d/dorhawk.list",
            "/etc/yum.repos.d/dorhawk.repo",
            "/usr/bin/pacman",
        ]
    };
    managers.iter().any(|p| exists(p))
}

/// The newest version, asked of dorhawk.software with the system's `curl`
/// (macOS and Windows 10 and later have it; without it, nothing is asked).
pub fn fetch() -> Option<String> {
    let mut cmd = std::process::Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    cmd.args([
        "--silent",
        "--fail",
        "--location",
        "--max-time",
        WAIT_SECS,
        URL,
    ]);
    crate::sys::no_console(&mut cmd);
    let out = cmd.output().ok()?;
    out.status.success().then_some(())?;
    newest_in(&String::from_utf8_lossy(&out.stdout))
}

/// The binary to start again: on Linux a package replaced it, so the
/// running one's path says "/usr/bin/crake (deleted)"; the new one is at
/// the path without that.
pub fn running_exe(exe: std::path::PathBuf) -> std::path::PathBuf {
    match exe.to_str().and_then(|p| p.strip_suffix(" (deleted)")) {
        Some(p) => p.into(),
        None => exe,
    }
}

/// What the panel says about versions, if anything: "updated, restart" (a
/// newer craked) first, else "a new version is out" (when asked, and not
/// "Not now"'d).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Restart(String),
    Out(String),
}

pub fn notice(
    service: Option<&str>,
    newest: Option<&str>,
    dismissed: &std::collections::BTreeSet<String>,
) -> Option<Notice> {
    if let Some(s) = service.filter(|s| newer(s, this())) {
        return Some(Notice::Restart(s.to_owned()));
    }
    let n = newest.filter(|n| newer(n, this()) && !dismissed.contains(&dismiss_key(n)))?;
    Some(Notice::Out(n.to_owned()))
}

/// What "Not now" remembers: that version (the next one is said again).
pub fn dismiss_key(version: &str) -> String {
    format!("version:{version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_their_numbers() {
        assert!(newer("0.7.0", "0.6.1"));
        assert!(newer("0.10.0", "0.9.9"), "not as text");
        assert!(newer("1.0.0", "0.99.0"));
        assert!(!newer("0.6.1", "0.6.1"));
        assert!(!newer("0.6.0", "0.6.1"));
    }

    #[test]
    fn the_newest_is_read_from_versions_json() {
        let json = r#"{ "crake": { "released": "2026-10-14", "version": "0.6.1" } }"#;
        assert_eq!(newest_in(json).as_deref(), Some("0.6.1"));
        assert_eq!(newest_in("not json"), None);
        assert_eq!(newest_in(r#"{"other": {"version": "1.0.0"}}"#), None);
    }

    #[test]
    fn a_package_manager_updates_some_installs() {
        let none = |_: &str| false;
        let exe = Path::new("/usr/bin/crake");
        assert!(!updated_elsewhere(exe, none), "a .deb from a file");
        // dorhawk.software's apt repository updates Linux installs only.
        let apt = |p: &str| p == "/etc/apt/sources.list.d/dorhawk.sources";
        assert_eq!(updated_elsewhere(exe, apt), cfg!(target_os = "linux"));
        let nix = Path::new("/nix/store/abc-crake-0.6.1/bin/crake");
        assert!(updated_elsewhere(nix, none));
        let mac = Path::new("/Applications/Crake.app/Contents/MacOS/crake");
        let brew = |p: &str| p == "/opt/homebrew/Caskroom/crake";
        assert_eq!(updated_elsewhere(mac, brew), cfg!(target_os = "macos"));
    }

    #[test]
    fn the_new_binary_is_started_not_the_deleted_one() {
        assert_eq!(
            running_exe("/usr/bin/crake (deleted)".into()),
            Path::new("/usr/bin/crake")
        );
        assert_eq!(
            running_exe("/usr/bin/crake".into()),
            Path::new("/usr/bin/crake")
        );
    }

    #[test]
    fn restart_comes_first_and_not_now_is_per_version() {
        let none = std::collections::BTreeSet::new();
        assert_eq!(
            notice(Some("99.0.0"), Some("99.1.0"), &none),
            Some(Notice::Restart("99.0.0".into()))
        );
        assert_eq!(
            notice(Some(this()), Some("99.1.0"), &none),
            Some(Notice::Out("99.1.0".into()))
        );
        let said = [dismiss_key("99.1.0")].into();
        assert_eq!(notice(None, Some("99.1.0"), &said), None);
        assert_eq!(
            notice(None, Some("99.2.0"), &said),
            Some(Notice::Out("99.2.0".into())),
            "the next version is said again"
        );
        assert_eq!(notice(None, Some(this()), &none), None);
    }
}
