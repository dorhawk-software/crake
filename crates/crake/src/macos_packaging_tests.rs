//! The macOS package's launchd files, as macOS reads them: checked on any
//! system, from packaging/macos.

use std::path::Path;

/// The plist file at `packaging/macos/<name>`.
fn plist(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/macos")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The strings under `<key>key</key>` in a plist: one `<string>`, or the
/// `<string>`s of an `<array>`.
fn strings(plist: &str, key: &str) -> Vec<String> {
    let Some(at) = plist.find(&format!("<key>{key}</key>")) else {
        return Vec::new();
    };
    let rest = &plist[at..];
    let value = rest[key.len() + "<key></key>".len()..].trim_start();
    let scope = if value.starts_with("<array>") {
        &value[..value.find("</array>").expect("an array's end")]
    } else {
        &value[..value.find("</string>").map_or(0, |e| e + "</string>".len())]
    };
    scope
        .split("<string>")
        .skip(1)
        .map(|s| s[..s.find("</string>").expect("a string's end")].to_owned())
        .collect()
}

/// macOS lists craked under System Settings' background items. Without
/// naming Crake.app it shows the signing certificate's owner and a blank
/// icon ("ONDŘEJ ŠIMON"); named, it's Crake with Crake's icon.
#[test]
fn craked_appears_as_crake_in_background_items() {
    let app = strings(&plist("Info.plist"), "CFBundleIdentifier");
    assert_eq!(app.len(), 1, "Crake.app's bundle identifier: {app:?}");
    let named = strings(
        &plist("software.dorhawk.crake.daemon.plist"),
        "AssociatedBundleIdentifiers",
    );
    assert!(
        named.contains(&app[0]),
        "craked's launchd file names {named:?}, not Crake.app ({})",
        app[0]
    );
}

/// Registered by Crake.app itself (SMAppService), from inside the app:
/// craked's launchd file names its program by its place in the bundle, so
/// macOS lists it under Crake, not under the signing certificate's name.
#[test]
fn craked_runs_from_inside_crake_app() {
    let daemon = plist("software.dorhawk.crake.daemon.plist");
    // Not 0.6's label: SMAppService answered for that daemon's record.
    assert_eq!(strings(&daemon, "Label"), ["software.dorhawk.crake.daemon"]);
    assert_eq!(strings(&daemon, "BundleProgram"), ["Contents/MacOS/craked"]);
    assert!(
        strings(&daemon, "ProgramArguments").is_empty() && strings(&daemon, "Program").is_empty(),
        "no path outside the app: {daemon}"
    );
}

/// "Start when you log in", registered by Crake.app (SMAppService) too: it
/// starts Crake from inside the app, with the flag that switches on the
/// tunnels left on, in the user's graphical session.
#[test]
fn the_login_agent_starts_crake_from_inside_the_app() {
    let agent = plist("software.dorhawk.crake.login.plist");
    // Not the older agent file's label, whose record macOS keeps.
    assert_eq!(strings(&agent, "Label"), ["software.dorhawk.crake.login"]);
    assert_eq!(strings(&agent, "BundleProgram"), ["Contents/MacOS/crake"]);
    assert_eq!(
        strings(&agent, "ProgramArguments"),
        ["crake", crate::autostart::FLAG]
    );
    assert_eq!(strings(&agent, "LimitLoadToSessionType"), ["Aqua"]);
}
