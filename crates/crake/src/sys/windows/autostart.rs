//! "Start when you log in" on Windows: the user's Run key,
//! `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`, value "Crake",
//! which starts this very binary at sign-in (and shows in Task Manager's
//! Startup apps).

use std::path::Path;

use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Crake";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

pub fn enabled() -> bool {
    let (key, value) = (wide(RUN), wide(VALUE));
    let mut size = 0u32;
    // SAFETY: NUL-terminated names; only the size is asked for.
    let found = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    };
    found == 0
}

pub fn set(on: bool, exe: &Path, flag: &str) {
    let (key, value) = (wide(RUN), wide(VALUE));
    if !on {
        // SAFETY: NUL-terminated names.
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) };
        return;
    }
    let command = wide(&command(&exe.display().to_string(), flag));
    let bytes = (command.len() * size_of::<u16>()) as u32;
    // SAFETY: NUL-terminated names; `command` is the REG_SZ's data, `bytes`
    // long, its NUL included.
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            REG_SZ,
            command.as_ptr().cast(),
            bytes,
        )
    };
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
