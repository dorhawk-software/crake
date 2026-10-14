//! Crake's service (craked) as the system runs it. On a Mac the app
//! registers it itself (SMAppService), so System Settings lists it as
//! Crake, and macOS has the user switch it on once there, under Login
//! Items & Extensions; until then nothing is routed, and the panel says so
//! with a button to that page (design/proposals/macos-service-approval.html,
//! option A). Elsewhere the package runs it: nothing to ask.

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
}
