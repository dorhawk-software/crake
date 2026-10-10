//! Motion (design/proposals/animations.html): every animation marks something
//! that happened; short, eased, no bounce. Off when GNOME's animations are
//! off (or CRAKE_MOTION=off): everything then appears at once.

use std::sync::LazyLock;

static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    if std::env::var("CRAKE_MOTION").as_deref() == Ok("off") {
        return false;
    }
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() != "false")
        .unwrap_or(true)
});

pub fn enabled() -> bool {
    *ENABLED
}

/// CRAKE_MOTION=slow (development): everything ten times slower, to look at.
pub fn slowed() -> u32 {
    if std::env::var("CRAKE_MOTION").as_deref() == Ok("slow") {
        10
    } else {
        1
    }
}

/// cubic-bezier(.2, 0, 0, 1)-like: quick start, gentle landing.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t).powi(3)
}
