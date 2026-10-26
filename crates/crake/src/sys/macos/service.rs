//! macOS: Crake's service (craked), registered by Crake.app itself from its
//! launchd file inside the app (Contents/Library/LaunchDaemons;
//! packaging/macos/software.dorhawk.crake.daemon.plist), with SMAppService:
//! System Settings lists it under Crake, and macOS has the user switch it
//! on once there (`crate::service`, the panel's "needs your OK").

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use crake_os::macos::service::{Service, Status as SMAppServiceStatus};

use crate::service::Revive;

/// Not 0.6's label (software.dorhawk.craked): macOS keeps that daemon's
/// record, and SMAppService answered for it ("enabled"), registering
/// nothing.
const DAEMON: &str = "software.dorhawk.crake.daemon.plist";

/// Registered once this run already (or tried).
static REGISTERED: AtomicBool = AtomicBool::new(false);

/// Since when craked has been out of reach while macOS says it's on.
static DOWN_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

/// Attempts this run to bring it back (`crate::service::revive`).
static REVIVED: AtomicU32 = AtomicU32::new(0);

fn daemon() -> Service {
    Service::daemon(DAEMON)
}

/// Whether macOS lets craked run, registering it first if it isn't yet
/// (macOS then asks the user: a notification, and the switch in System
/// Settings), and again if it's on but launchd doesn't run it
/// (`reachable`: whether the app reaches craked, None while it looks).
/// None: this Crake isn't one that can register it (a development build
/// outside Crake.app).
pub fn on(reachable: Option<bool>) -> Option<bool> {
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
    if status == SMAppServiceStatus::Enabled && revive(&service, reachable) {
        status = service.status();
    }
    if status == SMAppServiceStatus::Enabled {
        Some(true)
    } else if status == SMAppServiceStatus::RequiresApproval {
        Some(false)
    } else {
        None
    }
}

/// A craked macOS says is on but the app can't reach: registered again
/// once it has been out of reach a while (`crate::service::revive`).
/// Whether anything was tried.
fn revive(service: &Service, reachable: Option<bool>) -> bool {
    let mut since = DOWN_SINCE.lock().unwrap_or_else(PoisonError::into_inner);
    if reachable != Some(false) {
        *since = None;
        return false;
    }
    let down_for = since.get_or_insert_with(Instant::now).elapsed();
    let tried = REVIVED.load(Ordering::Relaxed);
    let Some(step) = crate::service::revive(down_for, tried) else {
        return false;
    };
    REVIVED.store(tried + 1, Ordering::Relaxed);
    if step == Revive::Reregister {
        let gone = service.unregister();
        crate::debug(&format!("craked unregistered: {gone:?}"));
    }
    let registered = service.register();
    crate::debug(&format!(
        "craked out of reach for {down_for:?} while on: {step:?}: {registered:?}"
    ));
    true
}

/// System Settings, at Login Items & Extensions.
pub fn open_settings() {
    crake_os::macos::service::open_login_items();
}
