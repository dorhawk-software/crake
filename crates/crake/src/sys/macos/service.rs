//! macOS: Crake's service (craked), registered by Crake.app itself from its
//! launchd file inside the app (Contents/Library/LaunchDaemons;
//! packaging/macos/software.dorhawk.crake.daemon.plist), with SMAppService:
//! System Settings lists it under Crake, and macOS has the user switch it
//! on once there (`crate::service`, the panel's "needs your OK").

use std::sync::atomic::{AtomicBool, Ordering};

use crake_os::macos::service::{Service, Status as SMAppServiceStatus};

/// Not 0.6's label (software.dorhawk.craked): macOS keeps that daemon's
/// record, and SMAppService answered for it ("enabled"), registering
/// nothing.
const DAEMON: &str = "software.dorhawk.crake.daemon.plist";

/// Registered once this run already (or tried).
static REGISTERED: AtomicBool = AtomicBool::new(false);

fn daemon() -> Service {
    Service::daemon(DAEMON)
}

/// Whether macOS lets craked run, registering it first if it isn't yet
/// (macOS then asks the user: a notification, and the switch in System
/// Settings). None: this Crake isn't one that can register it (a
/// development build outside Crake.app).
pub fn on() -> Option<bool> {
    let service = daemon();
    let mut status = service.status();
    // Never registered reads "not found" as well as "not registered" (CI's
    // Mac said NotFound while the launchd file was in the app): either way
    // it's registered, once a run; still not found then, it isn't there (a
    // development build outside Crake.app).
    let settled =
        status == SMAppServiceStatus::Enabled || status == SMAppServiceStatus::RequiresApproval;
    if !settled && !REGISTERED.swap(true, Ordering::Relaxed) {
        // "Operation not permitted" until the user switches it on; the
        // status says what's needed either way.
        let registered = service.register();
        status = service.status();
        crate::debug(&format!(
            "craked registered with SMAppService: {registered:?}, now {status:?}"
        ));
    } else {
        crate::debug(&format!("craked's SMAppService status: {status:?}"));
    }
    if status == SMAppServiceStatus::Enabled {
        Some(true)
    } else if status == SMAppServiceStatus::RequiresApproval {
        Some(false)
    } else {
        None
    }
}

/// System Settings, at Login Items & Extensions.
pub fn open_settings() {
    crake_os::macos::service::open_login_items();
}
