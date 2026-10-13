//! Browsers that look names up themselves, over encrypted DNS (DNS over
//! HTTPS), past the system's DNS: Crake never sees a company's name fail
//! there, so it can't route it before the browser connects (its probe
//! still catches the host, later). Read from each browser's own settings
//! files, in the user's home, by the app.

/// Firefox: a profile's prefs.js sets `network.trr.mode` 2 (encrypted
/// DNS first) or 3 (encrypted DNS only).
pub fn firefox_uses_its_own(prefs_js: &str) -> bool {
    prefs_js.lines().any(|line| {
        let line = line.trim();
        line.starts_with("user_pref(\"network.trr.mode\"")
            && line
                .rsplit(',')
                .next()
                .map(|v| v.trim_end_matches(");").trim())
                .is_some_and(|v| v == "2" || v == "3")
    })
}

/// Chrome, Chromium, Edge, Brave: `Local State` sets Secure DNS to
/// "secure" (a provider chosen by hand). "automatic" only upgrades when
/// the system's DNS server is itself an encrypted provider, which with
/// Crake answering (resolved, or craked's own server) it isn't.
pub fn chromium_uses_its_own(local_state: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(local_state)
        .ok()
        .and_then(|v| {
            v.pointer("/dns_over_https/mode")?
                .as_str()
                .map(|m| m == "secure")
        })
        .unwrap_or(false)
}

use std::path::{Path, PathBuf};

/// The browsers on this computer that look names up themselves, by name.
pub fn found() -> Vec<String> {
    found_in(&places())
}

/// Where the browsers keep their settings: Firefox's profile folders, and
/// each Chromium browser's `Local State`.
struct Places {
    firefox: Vec<PathBuf>,
    chromium: Vec<(&'static str, PathBuf)>,
}

fn places() -> Places {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let (Some(home), roaming, local) = (
        env("HOME").or_else(|| env("USERPROFILE")),
        env("APPDATA"),
        env("LOCALAPPDATA"),
    ) else {
        return Places {
            firefox: Vec::new(),
            chromium: Vec::new(),
        };
    };
    const LOCAL_STATE: &str = "Local State";
    if cfg!(windows) {
        let (roaming, local) = (roaming.unwrap_or_default(), local.unwrap_or_default());
        let user_data = |p: &str| local.join(p).join("User Data").join(LOCAL_STATE);
        Places {
            firefox: vec![roaming.join("Mozilla/Firefox/Profiles")],
            chromium: vec![
                ("Chrome", user_data("Google/Chrome")),
                ("Chromium", user_data("Chromium")),
                ("Edge", user_data("Microsoft/Edge")),
                ("Brave", user_data("BraveSoftware/Brave-Browser")),
            ],
        }
    } else if cfg!(target_os = "macos") {
        let support = home.join("Library/Application Support");
        let state = |p: &str| support.join(p).join(LOCAL_STATE);
        Places {
            firefox: vec![support.join("Firefox/Profiles")],
            chromium: vec![
                ("Chrome", state("Google/Chrome")),
                ("Chromium", state("Chromium")),
                ("Edge", state("Microsoft Edge")),
                ("Brave", state("BraveSoftware/Brave-Browser")),
            ],
        }
    } else {
        let config = home.join(".config");
        let state = |p: &Path| p.join(LOCAL_STATE);
        Places {
            // Ubuntu's Firefox is a snap; Flathub's a flatpak.
            firefox: vec![
                home.join(".mozilla/firefox"),
                home.join("snap/firefox/common/.mozilla/firefox"),
                home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
            ],
            chromium: vec![
                ("Chrome", state(&config.join("google-chrome"))),
                ("Chromium", state(&config.join("chromium"))),
                (
                    "Chromium",
                    state(&home.join("snap/chromium/common/chromium")),
                ),
                ("Edge", state(&config.join("microsoft-edge"))),
                ("Brave", state(&config.join("BraveSoftware/Brave-Browser"))),
            ],
        }
    }
}

fn found_in(places: &Places) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let firefox = places
        .firefox
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .any(|profile| {
            std::fs::read_to_string(profile.path().join("prefs.js"))
                .is_ok_and(|t| firefox_uses_its_own(&t))
        });
    if firefox {
        out.push("Firefox".into());
    }
    for (name, file) in &places.chromium {
        if !out.iter().any(|n| n == name)
            && std::fs::read_to_string(file).is_ok_and(|t| chromium_uses_its_own(&t))
        {
            out.push((*name).into());
        }
    }
    out
}

/// "Firefox", "Firefox and Chrome", "Firefox, Chrome and Edge".
pub fn and_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A snap Firefox profile with encrypted DNS only, Chrome with Secure
    /// DNS set by hand, Brave left as it came: Firefox and Chrome.
    #[test]
    fn the_browsers_found_in_their_folders() {
        let home = tempfile::tempdir().unwrap();
        let firefox = home
            .path()
            .join("snap/firefox/common/.mozilla/firefox/ab12.default");
        std::fs::create_dir_all(&firefox).unwrap();
        std::fs::write(
            firefox.join("prefs.js"),
            "user_pref(\"network.trr.mode\", 3);\n",
        )
        .unwrap();
        let chrome = home.path().join(".config/google-chrome");
        std::fs::create_dir_all(&chrome).unwrap();
        std::fs::write(
            chrome.join("Local State"),
            r#"{"dns_over_https":{"mode":"secure"}}"#,
        )
        .unwrap();
        let brave = home.path().join("brave");
        std::fs::create_dir_all(&brave).unwrap();
        std::fs::write(
            brave.join("Local State"),
            r#"{"dns_over_https":{"mode":"automatic"}}"#,
        )
        .unwrap();
        let places = Places {
            firefox: vec![
                home.path().join(".mozilla/firefox"),
                home.path().join("snap/firefox/common/.mozilla/firefox"),
            ],
            chromium: vec![
                ("Chrome", chrome.join("Local State")),
                ("Brave", brave.join("Local State")),
            ],
        };
        assert_eq!(
            found_in(&places),
            vec!["Firefox".to_owned(), "Chrome".to_owned()]
        );
        assert_eq!(and_list(&found_in(&places)), "Firefox and Chrome");
    }

    #[test]
    fn firefox_with_dns_over_https_on() {
        let prefs = |mode: &str| {
            format!(
                "// Mozilla User Preferences\nuser_pref(\"browser.startup.page\", 3);\nuser_pref(\"network.trr.mode\", {mode});\nuser_pref(\"network.trr.uri\", \"https://dns.example/dns-query\");\n"
            )
        };
        assert!(firefox_uses_its_own(&prefs("2")), "encrypted DNS first");
        assert!(firefox_uses_its_own(&prefs("3")), "encrypted DNS only");
        assert!(!firefox_uses_its_own(&prefs("0")), "default");
        assert!(!firefox_uses_its_own(&prefs("5")), "switched off");
        assert!(!firefox_uses_its_own(
            "user_pref(\"browser.startup.page\", 3);\n"
        ));
    }

    #[test]
    fn chromium_with_secure_dns_set_by_hand() {
        let state = |mode: &str| {
            format!(
                r#"{{"browser":{{"enabled_labs_experiments":[]}},"dns_over_https":{{"mode":"{mode}","templates":"https://dns.example/dns-query{{?dns}}"}}}}"#
            )
        };
        assert!(chromium_uses_its_own(&state("secure")));
        assert!(!chromium_uses_its_own(&state("automatic")));
        assert!(!chromium_uses_its_own(&state("off")));
        assert!(!chromium_uses_its_own(r#"{"browser":{}}"#));
        assert!(!chromium_uses_its_own("not json"));
    }
}
