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

/// Why Windows says Crake's service isn't running, from its service
/// events (`stopped`): an antivirus kept the unsigned
/// craked.exe from starting on a colleague's PC, and the panel said only
/// "isn't running" (2026-10-26).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stopped {
    /// Windows was refused starting it: an antivirus, most often.
    Denied,
    /// Its program isn't there (an antivirus may have taken it).
    Missing,
    /// It didn't start: Windows' error code, or None when it didn't start
    /// in time.
    NotStarted(Option<u32>),
    /// It stopped unexpectedly; Windows starts it again.
    Crashed,
}

/// The panel's title and sentence for `stopped`; `program`: the folder
/// Crake is installed in.
pub fn said(stopped: &Stopped, program: &str) -> (String, String) {
    const AGAIN: &str = "Restarting the computer may help; if not, install Crake again.";
    match stopped {
        Stopped::Denied => (
            "Windows wouldn\u{2019}t start Crake\u{2019}s service.".into(),
            format!(
                "It was refused, most often by an antivirus. Allow Crake there ({program} and {}), then restart the computer.",
                crake_proto::STATE_DIR
            ),
        ),
        Stopped::Missing => (
            "Crake\u{2019}s service is missing.".into(),
            "Its program isn\u{2019}t there any more; an antivirus may have taken it. Restore it there, or install Crake again.".into(),
        ),
        Stopped::NotStarted(Some(code)) => (
            "Windows couldn\u{2019}t start Crake\u{2019}s service.".into(),
            format!("Windows said: error {code}. {AGAIN}"),
        ),
        Stopped::NotStarted(None) => (
            "Crake\u{2019}s service didn\u{2019}t start in time.".into(),
            AGAIN.into(),
        ),
        Stopped::Crashed => (
            "Crake\u{2019}s service stopped unexpectedly.".into(),
            "Nothing is routed. Windows starts it again, and this panel fills in by itself once it\u{2019}s back.".into(),
        ),
    }
}

/// Why craked isn't running: its program gone (`missing`), or the newest
/// of its service events (`events`, Windows' System log; none elsewhere).
pub fn stopped(missing: bool, events: Option<&str>) -> Option<Stopped> {
    if missing {
        return Some(Stopped::Missing);
    }
    stopped_from(events?)
}

/// The Service Control Manager's events about a service, by number.
mod event {
    /// "The service failed to start due to the following error".
    pub const FAILED_TO_START: u32 = 7000;
    /// "A timeout was reached while waiting for the service to connect".
    pub const TIMED_OUT: u32 = 7009;
    /// "The service terminated unexpectedly" (with and without a restart).
    pub const TERMINATED: [u32; 2] = [7031, 7034];
    /// "The service entered the running/stopped state": nothing wrong.
    pub const STATE: u32 = 7036;
    /// ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_ACCESS_DENIED.
    pub const NOT_FOUND: [u32; 2] = [2, 3];
    pub const ACCESS_DENIED: u32 = 5;
}

/// The events `stopped_from` asks Windows' System log for: the newest of
/// these about Crake (`wevtutil qe System /q:...`). Data='Crake': the
/// service's display name, the first or second of an event's values.
pub fn events_query() -> String {
    use event::*;
    let ids = [
        FAILED_TO_START,
        TIMED_OUT,
        TERMINATED[0],
        TERMINATED[1],
        STATE,
    ]
    .map(|id| format!("EventID={id}"))
    .join(" or ");
    format!(
        "*[System[Provider[@Name='Service Control Manager'] and ({ids})] and EventData[Data='Crake']]"
    )
}

/// Why it stopped, from the newest of those events (`/f:xml`): read by
/// number and error code, since their words are the system's language.
fn stopped_from(xml: &str) -> Option<Stopped> {
    use event::*;
    let after = |tag: &str| xml.split_once(tag).map(|(_, rest)| rest);
    let id: u32 = after("<EventID")?
        .split_once('>')?
        .1
        .split('<')
        .next()?
        .trim()
        .parse()
        .ok()?;
    // The error as "%%5" (an insertion of Windows' message number 5).
    let code: Option<u32> = after("%%").and_then(|rest| {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    });
    match id {
        FAILED_TO_START => Some(match code {
            Some(ACCESS_DENIED) => Stopped::Denied,
            Some(c) if NOT_FOUND.contains(&c) => Stopped::Missing,
            c => Stopped::NotStarted(c),
        }),
        TIMED_OUT => Some(Stopped::NotStarted(None)),
        id if TERMINATED.contains(&id) => Some(Stopped::Crashed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The newest of Crake's service events says why, by number: Norton's
    /// refusal on a colleague's PC was 7000 with %%5 (2026-10-26).
    #[test]
    fn windows_service_events_say_why_it_isnt_running() {
        let event = |id: &str, data: &str| {
            format!(
                "<Event><System><Provider Name='Service Control Manager'/><EventID Qualifiers='49152'>{id}</EventID></System><EventData>{data}</EventData></Event>"
            )
        };
        let failed = |code: &str| {
            event(
                "7000",
                &format!("<Data Name='param1'>Crake</Data><Data Name='param2'>%%{code}</Data>"),
            )
        };
        assert_eq!(stopped_from(&failed("5")), Some(Stopped::Denied));
        assert_eq!(stopped_from(&failed("2")), Some(Stopped::Missing));
        assert_eq!(
            stopped_from(&failed("1053")),
            Some(Stopped::NotStarted(Some(1053)))
        );
        let timed_out = event(
            "7009",
            "<Data Name='param1'>30000</Data><Data Name='param2'>Crake</Data>",
        );
        assert_eq!(stopped_from(&timed_out), Some(Stopped::NotStarted(None)));
        let crashed = event(
            "7034",
            "<Data Name='param1'>Crake</Data><Data Name='param2'>1</Data>",
        );
        assert_eq!(stopped_from(&crashed), Some(Stopped::Crashed));
        // Started or stopped as usual last: nothing to tell; nor with none.
        let state = event(
            "7036",
            "<Data Name='param1'>Crake</Data><Data Name='param2'>spu\u{161}t\u{11b}no</Data>",
        );
        assert_eq!(stopped_from(&state), None);
        assert_eq!(stopped(false, Some("")), None);
        assert_eq!(stopped(false, None), None, "nothing said");
        assert_eq!(stopped(true, Some(&state)), Some(Stopped::Missing));
        let q = events_query();
        assert!(q.contains("EventID=7000 or EventID=7009"), "{q}");
        assert!(q.ends_with("EventData[Data='Crake']]"), "{q}");
    }

    /// Each reason says what to do; a refusal names the folders to allow.
    #[test]
    fn each_reason_says_what_to_do() {
        let (title, sentence) = said(&Stopped::Denied, r"C:\Program Files\Crake");
        assert_eq!(
            title,
            "Windows wouldn\u{2019}t start Crake\u{2019}s service."
        );
        assert!(
            sentence.contains(r"C:\Program Files\Crake and "),
            "{sentence}"
        );
        assert!(sentence.contains(crake_proto::STATE_DIR), "{sentence}");
        for s in [
            Stopped::Missing,
            Stopped::NotStarted(Some(1053)),
            Stopped::NotStarted(None),
            Stopped::Crashed,
        ] {
            let (title, sentence) = said(&s, "");
            assert!(title.ends_with('.') && sentence.ends_with('.'), "{s:?}");
        }
    }

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
