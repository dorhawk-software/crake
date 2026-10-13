//! "Start when you log in" on Linux: an autostart entry for this very
//! binary in `~/.config/autostart` (freedesktop's autostart spec).

use std::path::{Path, PathBuf};

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("autostart/crake.desktop"))
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
    let _ = std::fs::write(&p, entry(&exe.display().to_string(), flag));
}

fn entry(exe: &str, flag: &str) -> String {
    // A path with spaces must be quoted in Exec.
    let exe = if exe.contains(' ') {
        format!("\"{exe}\"")
    } else {
        exe.to_owned()
    };
    format!(
        "[Desktop Entry]\nType=Application\nName=Crake\nComment=Routes only what needs a tunnel\n\
         Exec={exe} {flag}\nIcon=network-vpn\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n"
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_entry_starts_this_binary() {
        let e = super::entry("/home/me/My Apps/crake", "--autostart");
        assert!(
            e.contains("Exec=\"/home/me/My Apps/crake\" --autostart\n"),
            "{e}"
        );
        assert!(e.starts_with("[Desktop Entry]\n"));
    }
}
