//! "Start when you log in" on Windows: the user's Run key,
//! `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`, value "Crake",
//! which starts this very binary at sign-in (and shows in Task Manager's
//! Startup apps).

use std::path::Path;

use crake_os::windows::registry::{self, Root, Value};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Crake";

pub fn enabled() -> bool {
    registry::has_string(Root::CurrentUser, RUN, VALUE)
}

pub fn set(on: bool, exe: &Path, flag: &str) {
    if on {
        let command = command(&exe.display().to_string(), flag);
        let _ = registry::set_value(Root::CurrentUser, RUN, VALUE, &Value::Sz(&command));
    } else {
        registry::delete_value(Root::CurrentUser, RUN, VALUE);
    }
}

/// The command line: the binary quoted (Program Files has a space).
fn command(exe: &str, flag: &str) -> String {
    format!("\"{exe}\" {flag}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_run_value_starts_this_binary() {
        assert_eq!(
            super::command(r"C:\Program Files\Crake\crake.exe", "--autostart"),
            r#""C:\Program Files\Crake\crake.exe" --autostart"#
        );
    }
}
