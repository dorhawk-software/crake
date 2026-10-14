//! macOS: Crake's service (craked), registered by Crake.app itself from its
//! launchd file inside the app (Contents/Library/LaunchDaemons;
//! packaging/macos/software.dorhawk.craked.plist), with SMAppService:
//! System Settings lists it under Crake, and macOS has the user switch it
//! on once there (`crate::service`, the panel's "needs your OK").

use objc2::rc::Retained;
use objc2_foundation::NSString;
use objc2_service_management::{SMAppService, SMAppServiceStatus};

const DAEMON: &str = "software.dorhawk.craked.plist";

fn daemon() -> Retained<SMAppService> {
    // SAFETY: a constructor taking a valid string.
    unsafe { SMAppService::daemonServiceWithPlistName(&NSString::from_str(DAEMON)) }
}

/// Whether macOS lets craked run, registering it first if it isn't yet
/// (macOS then asks the user: a notification, and the switch in System
/// Settings). None: this Crake isn't one that can register it (a
/// development build outside Crake.app).
pub fn on() -> Option<bool> {
    let service = daemon();
    // SAFETY: plain calls on a valid service object.
    let mut status = unsafe { service.status() };
    if status == SMAppServiceStatus::NotRegistered {
        // "Operation not permitted" until the user switches it on; the
        // status says what's needed either way.
        // SAFETY: as above.
        let registered = unsafe { service.registerAndReturnError() };
        // SAFETY: as above.
        status = unsafe { service.status() };
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
    // SAFETY: a class method without arguments.
    unsafe { SMAppService::openSystemSettingsLoginItems() };
}
