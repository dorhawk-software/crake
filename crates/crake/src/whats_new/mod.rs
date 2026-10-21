//! What's new (design/proposals/whats-new.html, chosen by the user
//! 2026-10-21): after an update the panel says so once ("Crake is now
//! 0.15.0: 9 changes since 0.13.1."), and a window shows what changed since
//! the version you had, in the changelog's own words, built in (nothing is
//! fetched). Settings → About opens it any time.

pub mod changelog;
mod open;
mod view;

#[cfg(test)]
mod tests;

pub use open::open;
/// The window, for the journeys (Linux's).
#[cfg(all(test, target_os = "linux"))]
pub use view::WhatsNew;

use crate::prefs::Prefs;
use crate::versions;
use changelog::{Release, UNRELEASED};

/// Versions shown in full before "Show earlier versions".
pub const IN_FULL: usize = 3;

/// `a` is newer than `b`; a development build's Unreleased is newer than
/// any version.
fn newer(a: &str, b: &str) -> bool {
    match (a == UNRELEASED, b == UNRELEASED) {
        (true, false) => true,
        (_, true) => false,
        _ => versions::newer(a, b),
    }
}

/// The releases with something in them, newest first, up to `this` (and a
/// development build's Unreleased).
fn upto<'a>(releases: &'a [Release], this: &str) -> Vec<&'a Release> {
    releases
        .iter()
        .filter(|r| !r.is_empty())
        .filter(|r| r.version == UNRELEASED || !versions::newer(&r.version, this))
        .collect()
}

/// Those after `seen`.
fn after<'a>(releases: &'a [Release], this: &str, seen: &str) -> Vec<&'a Release> {
    upto(releases, this)
        .into_iter()
        .filter(|r| newer(&r.version, seen))
        .collect()
}

/// What the panel says after an update, if anything (`seen`: the version
/// last seen, as kept; None: none kept, an update from before Crake kept
/// it). Same version or older (a downgrade): nothing. Only what's
/// underneath changed: nothing.
pub fn notice_in(releases: &[Release], this: &str, seen: Option<&str>) -> Option<String> {
    let told: usize = match seen {
        Some(s) if !versions::newer(this, s) => return None,
        Some(s) => after(releases, this, s).iter().map(|r| r.told()).sum(),
        None => upto(releases, this)
            .iter()
            .find(|r| r.version == this)
            .map_or(0, |r| r.told()),
    };
    if told == 0 {
        return None;
    }
    Some(match seen {
        Some(s) => {
            let changes = if told == 1 { "change" } else { "changes" };
            // The version stays with "since" when the line wraps.
            format!("Crake is now {this}: {told} {changes} since\u{a0}{s}.")
        }
        None => format!("Crake is now {this}."),
    })
}

/// The panel's notice for this Crake, while it should show. No version
/// seen and no app.json before: a fresh install, nothing to tell.
pub fn notice(prefs: &Prefs) -> Option<String> {
    if prefs.seen_version.is_none() && !prefs.existed {
        return None;
    }
    notice_in(
        changelog::releases(),
        versions::this(),
        prefs.seen_version.as_deref(),
    )
}

/// What the window shows: since which version (its subtitle), the
/// versions in full, and the rest behind "Show earlier versions".
pub struct Shown<'a> {
    pub since: Option<String>,
    pub full: Vec<&'a Release>,
    pub earlier: Vec<&'a Release>,
}

/// From the panel's notice, the versions since the one seen (up to
/// `IN_FULL` of them in full); otherwise (About) this version.
pub fn shown_in<'a>(releases: &'a [Release], this: &str, since: Option<&str>) -> Shown<'a> {
    let all = upto(releases, this);
    let new = since.map_or(0, |s| after(releases, this, s).len());
    let n = new.clamp(1, IN_FULL).min(all.len());
    let (full, earlier) = all.split_at(n);
    Shown {
        since: since.filter(|_| new > 0).map(str::to_owned),
        full: full.to_vec(),
        earlier: earlier.to_vec(),
    }
}

/// This version seen: kept, unless a newer one was (a downgrade keeps the
/// highest).
pub fn mark_seen(prefs: &mut Prefs) {
    let this = versions::this();
    if prefs
        .seen_version
        .as_deref()
        .is_none_or(|s| versions::newer(this, s))
    {
        prefs.seen_version = Some(this.to_owned());
    }
}

/// The version CRAKE_DEMO=whatsnew pretends was seen before.
const DEMO_SEEN: &str = "0.13.1";

/// When the app starts: a fresh install (no app.json yet) has nothing to
/// tell, so this version counts as seen. An update from before Crake kept
/// the version has app.json without it, and is told. CRAKE_DEMO=whatsnew:
/// updated from 0.13.1.
pub fn first_start(prefs: &mut Prefs) {
    if std::env::var("CRAKE_DEMO").as_deref() == Ok("whatsnew") {
        prefs.seen_version = Some(DEMO_SEEN.to_owned());
    } else if !prefs.existed && prefs.seen_version.is_none() {
        mark_seen(prefs);
        prefs.save();
    }
}
