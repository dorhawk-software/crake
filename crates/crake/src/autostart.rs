//! "Start when you log in" (Settings, General), each system its own way
//! (`sys::autostart`: Linux's autostart entry, macOS' LaunchAgent,
//! Windows' Run key), for this very binary. Written when the user
//! switches it on, and again at each start while it's on (`refresh`).

/// The flag Crake is started with at login: it then switches on the
/// tunnels that were on when the user last used it.
pub const FLAG: &str = "--autostart";

pub fn enabled() -> bool {
    crate::sys::autostart::enabled()
}

pub fn set(on: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    crate::sys::autostart::set(on, &exe, FLAG);
}

/// At start, while "Start when you log in" is on: the entry written again,
/// as this Crake writes it. One an older Crake wrote may lack what's new
/// (macOS' agent gained its link to Crake.app, without which System
/// Settings lists it under the signer's name), or name a binary that has
/// moved since.
pub fn refresh() {
    refresh_with(enabled, || set(true));
}

fn refresh_with(enabled: impl Fn() -> bool, mut write: impl FnMut()) {
    if enabled() {
        write();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    /// On: written again. Off: nothing written (the user switched it
    /// off; a start mustn't switch it back on).
    #[test]
    fn a_start_writes_the_entry_again_only_while_its_on() {
        for on in [true, false] {
            let written = Cell::new(false);
            super::refresh_with(|| on, || written.set(true));
            assert_eq!(
                written.get(),
                on,
                "with it {}",
                if on { "on" } else { "off" }
            );
        }
    }
}
