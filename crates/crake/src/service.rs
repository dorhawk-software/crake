//! Crake's service (craked) as the system runs it. On a Mac the app
//! registers it itself (SMAppService), so System Settings lists it as
//! Crake, and macOS has the user switch it on once there, under Login
//! Items & Extensions; until then nothing is routed, and the panel says so
//! with a button to that page (design/proposals/macos-service-approval.html,
//! option A). Elsewhere the package runs it: nothing to ask.

#[cfg(any(target_os = "macos", test))]
use std::time::Duration;

/// How long craked may be out of reach while macOS says it's switched on
/// before the app registers it again: launchd starts a craked that stopped
/// again within seconds (KeepAlive), so past this it isn't loaded at all.
/// Homebrew's cask upgrade does that: it takes the daemon out of launchd
/// (`launchctl remove`) and macOS still says "enabled" (found 2026-10-26).
#[cfg(any(target_os = "macos", test))]
pub const REVIVE_AFTER: Duration = Duration::from_secs(15);

/// How the app brings back a craked that macOS says is on but launchd
/// doesn't run. `launchctl bootstrap` can't: only SMAppService loads a
/// launchd file that names its program in the app (`BundleProgram`).
#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Revive {
    /// Register it again.
    Register,
    /// Unregister and register it: macOS may then ask for the user's OK
    /// again, which the panel shows with its button.
    Reregister,
}

/// What to try for a craked out of reach for `down_for` while macOS says
/// it's on, `tried` attempts having been made this run: each a
/// REVIVE_AFTER apart, two at most.
#[cfg(any(target_os = "macos", test))]
pub fn revive(down_for: Duration, tried: u32) -> Option<Revive> {
    if down_for < REVIVE_AFTER * (tried + 1) {
        return None;
    }
    match tried {
        0 => Some(Revive::Register),
        1 => Some(Revive::Reregister),
        _ => None,
    }
}

/// What the user has to do for craked to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    /// Switch it on the first time.
    Approval,
    /// It was on, and was switched off in System Settings.
    SwitchedOff,
}

/// What the panel asks of the user. `on`: whether the system lets craked
/// run (`sys::service::on`: None where that isn't the system's way);
/// `was_on`: craked has run on this Mac before (`Prefs::service_was_on`).
pub fn wanted(on: Option<bool>, was_on: bool) -> Option<Wanted> {
    match on {
        Some(false) if was_on => Some(Wanted::SwitchedOff),
        Some(false) => Some(Wanted::Approval),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_user_is_asked_follows_macos() {
        assert_eq!(wanted(Some(true), false), None);
        assert_eq!(wanted(Some(true), true), None);
        assert_eq!(wanted(Some(false), false), Some(Wanted::Approval));
        assert_eq!(wanted(Some(false), true), Some(Wanted::SwitchedOff));
        assert_eq!(wanted(None, false), None);
    }

    #[test]
    fn a_craked_macos_says_is_on_but_isnt_loaded_is_registered_again() {
        let secs = Duration::from_secs;
        // A craked launchd starts again within seconds is left to launchd.
        assert_eq!(revive(secs(3), 0), None);
        assert_eq!(revive(REVIVE_AFTER, 0), Some(Revive::Register));
        // Registering didn't bring it back: once more, from scratch.
        assert_eq!(revive(REVIVE_AFTER, 1), None);
        assert_eq!(revive(REVIVE_AFTER * 2, 1), Some(Revive::Reregister));
        // Then it's the user's: never a loop of registering.
        assert_eq!(revive(secs(3600), 2), None);
    }
}
