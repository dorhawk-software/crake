//! "Start when you log in" on macOS: a LaunchAgent of the user's,
//! `~/Library/LaunchAgents/software.dorhawk.crake.plist`, which launchd
//! loads at every login and which starts this very binary (Crake.app's).
//! (Linux writes an autostart entry; macOS ignores those.)

use std::path::{Path, PathBuf};

const LABEL: &str = "software.dorhawk.crake";
/// Crake.app's bundle identifier (packaging/macos/Info.plist): System
/// Settings shows the agent as Crake, with its icon, not under the name on
/// the signing certificate.
const APP: &str = "software.dorhawk.Crake";

fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(format!("Library/LaunchAgents/{LABEL}.plist")))
}

pub fn enabled() -> bool {
    path().is_some_and(|p| p.exists())
}

pub fn set(on: bool, exe: &Path, flag: &str) {
    let Some(p) = path() else { return };
    if !on {
        let _ = std::fs::remove_file(p);
        return;
    }
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&p, agent(&exe.display().to_string(), flag));
}

/// Text in a plist's XML.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn agent(exe: &str, flag: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \t<key>Label</key>\n\t<string>{LABEL}</string>\n\
         \t<key>AssociatedBundleIdentifiers</key>\n\t<array>\n\t\t<string>{APP}</string>\n\t</array>\n\
         \t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t\t<string>{}</string>\n\t</array>\n\
         \t<key>RunAtLoad</key>\n\t<true/>\n\
         \t<key>LimitLoadToSessionType</key>\n\t<string>Aqua</string>\n\
         \t<key>ProcessType</key>\n\t<string>Interactive</string>\n\
         </dict>\n</plist>\n",
        escape(exe),
        escape(flag)
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_agent_starts_this_binary_at_login() {
        let a = super::agent(
            "/Applications/Crake & Co.app/Contents/MacOS/crake",
            "--autostart",
        );
        assert!(
            a.contains("<string>/Applications/Crake &amp; Co.app/Contents/MacOS/crake</string>"),
            "{a}"
        );
        assert!(a.contains("<string>--autostart</string>"));
        assert!(a.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(a.contains("<string>software.dorhawk.crake</string>"));
        assert!(a.contains(
            "<key>AssociatedBundleIdentifiers</key>\n\t<array>\n\t\t<string>software.dorhawk.Crake</string>"
        ));
    }
}
