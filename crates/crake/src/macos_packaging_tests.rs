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
        &plist("software.dorhawk.craked.plist"),
        "AssociatedBundleIdentifiers",
    );
    assert!(
        named.contains(&app[0]),
        "craked's launchd file names {named:?}, not Crake.app ({})",
        app[0]
    );
}
