//! "Start when you log in" on macOS: Crake.app's login agent (its launchd
//! file inside the app, Contents/Library/LaunchAgents;
//! packaging/macos/software.dorhawk.crake.login.plist), registered by the app
//! itself with SMAppService, so System Settings lists it under Crake, not
//! under the signing certificate's name. It starts Crake with the flag that
//! switches on the tunnels left on (the plist names it).
//! Crakes before 0.7 wrote ~/Library/LaunchAgents/software.dorhawk.crake.plist
//! themselves: taken away, and the app's agent registered instead
//! (`autostart::refresh` at each start).

use std::path::{Path, PathBuf};

use crake_os::macos::service::{Service, Status as SMAppServiceStatus};

const AGENT: &str = "software.dorhawk.crake.login.plist";
/// The file older Crakes wrote (its label's record macOS keeps, so the
/// app's agent has a label of its own).
const OLDER: &str = "software.dorhawk.crake.plist";

fn agent() -> Service {
    Service::agent(AGENT)
}

/// What an older Crake wrote.
fn older() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/LaunchAgents").join(OLDER))
}

pub fn enabled() -> bool {
    older().is_some_and(|p| p.exists()) || agent().status() == SMAppServiceStatus::Enabled
}

/// On or off. The agent names its program and flag itself (`_exe`, `_flag`:
/// the other systems' way).
pub fn set(on: bool, _exe: &Path, _flag: &str) {
    // An older Crake's file goes; the job it loaded at this login stays
    // (it may be this very Crake) and is gone at the next.
    if let Some(p) = older().filter(|p| p.exists()) {
        let _ = std::fs::remove_file(p);
    }
    let agent = agent();
    let done = if on {
        agent.register()
    } else {
        agent.unregister()
    };
    if let Err(e) = done {
        eprintln!("start at login: {e:?}");
    }
}
