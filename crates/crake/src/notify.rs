//! A quiet notification per host Crake routed on its own, with Undo
//! (design/DESIGN.md, Platform notes). Decisions you made yourself aren't
//! announced back to you. Settings choose: nothing, a quiet notification, or
//! one with the bird's call ("crex crex").

use crake_proto::{LogEntry, LogKind};
use notify_rust::Notification;
#[cfg(not(target_os = "macos"))]
use notify_rust::{Timeout, Urgency};

use crate::Request;

// macOS: notify-rust shows plain notifications there (no actions, no
// urgency); the panel offers what the buttons would.
#[cfg(target_os = "macos")]
pub use plain::{also_answered, asked, left_running, no_tray, routed, stopped};

/// Crake decided it, not you.
pub fn worth_telling(entry: &LogEntry) -> bool {
    entry.kind == LogKind::Routed
        && !entry.hosts.is_empty()
        && !entry.reason.as_deref().unwrap_or("").starts_with("You ")
}

/// How long each notification stays, in ms: a routed host (quiet, brief),
/// another tunnel answering too, a host waiting for your decision.
#[cfg(not(target_os = "macos"))]
const ROUTED_SHOWN: u32 = 8000;
#[cfg(not(target_os = "macos"))]
const ALSO_ANSWERED_SHOWN: u32 = 15_000;
#[cfg(not(target_os = "macos"))]
const ASKED_SHOWN: u32 = 20_000;
/// A tunnel that stopped by itself: until dismissed (it may need you).
#[cfg(not(target_os = "macos"))]
const STOPPED_SHOWN: u32 = 0;

/// What every notification starts from: the entry's message and reason.
#[cfg(not(target_os = "macos"))]
fn note(entry: &LogEntry, urgency: Urgency, timeout_ms: u32) -> Notification {
    let mut n = Notification::new();
    n.appname("Crake")
        .summary(&entry.message)
        .body(entry.reason.as_deref().unwrap_or(""))
        .urgency(urgency)
        .timeout(Timeout::Milliseconds(timeout_ms));
    // The desktop's notification category (freedesktop only).
    #[cfg(target_os = "linux")]
    n.hint(notify_rust::Hint::Category("network".into()));
    n
}

#[cfg(not(target_os = "macos"))]
pub fn routed(entry: &LogEntry, requests: async_channel::Sender<Request>) {
    let (Some(tunnel), hosts) = (entry.tunnel.clone(), entry.hosts.clone()) else {
        return;
    };
    let mut n = note(entry, Urgency::Low, ROUTED_SHOWN);
    n.action("undo", "Undo").action("default", "Open Crake");
    std::thread::spawn(move || {
        let Ok(handle) = n.show() else { return };
        handle.wait_for_action(|action| match action {
            "undo" => {
                requests.send_blocking(Request::Undo(tunnel, hosts)).ok();
            }
            "default" => {
                requests.send_blocking(Request::Panel(None)).ok();
            }
            _ => {}
        });
    });
}

/// Another tunnel answered for a host too ("When two tunnels answer: Ask
/// me"): offer to move it. `others` are (name, display) of those tunnels.
#[cfg(not(target_os = "macos"))]
pub fn also_answered(
    entry: &LogEntry,
    others: Vec<(String, String)>,
    requests: async_channel::Sender<Request>,
) {
    let (Some(from), hosts) = (entry.tunnel.clone(), entry.hosts.clone()) else {
        return;
    };
    let mut n = note(entry, Urgency::Normal, ALSO_ANSWERED_SHOWN);
    for (name, display) in &others {
        n.action(name, &format!("Use {display}"));
    }
    n.action("keep", "Keep it");
    std::thread::spawn(move || {
        let Ok(handle) = n.show() else { return };
        handle.wait_for_action(|action| {
            if others.iter().any(|(name, _)| name == action) {
                requests
                    .send_blocking(Request::Move(
                        from.clone(),
                        action.to_owned(),
                        hosts.clone(),
                    ))
                    .ok();
            }
        });
    });
}

/// "Ask me first": a host needs the tunnel; send it through, or leave it.
#[cfg(not(target_os = "macos"))]
pub fn asked(entry: &LogEntry, display: String, requests: async_channel::Sender<Request>) {
    let (Some(tunnel), hosts) = (entry.tunnel.clone(), entry.hosts.clone()) else {
        return;
    };
    let mut n = note(entry, Urgency::Normal, ASKED_SHOWN);
    n.action("send", &format!("Send through {display}"))
        .action("leave", "Leave it");
    std::thread::spawn(move || {
        let Ok(handle) = n.show() else { return };
        handle.wait_for_action(|action| {
            if action == "send" {
                requests.send_blocking(Request::Send(tunnel, hosts)).ok();
            }
        });
    });
}

/// A tunnel stopped without being asked to (`LogEntry::is_unexpected_stop`):
/// say why, and offer to switch it back on (sign-in opens if it needs one).
#[cfg(not(target_os = "macos"))]
pub fn stopped(entry: &LogEntry, requests: async_channel::Sender<Request>) {
    let Some(tunnel) = entry.tunnel.clone() else {
        return;
    };
    let mut n = note(entry, Urgency::Normal, STOPPED_SHOWN);
    n.action("on", "Switch on").action("default", "Open Crake");
    std::thread::spawn(move || {
        let Ok(handle) = n.show() else { return };
        handle.wait_for_action(|action| match action {
            "on" => {
                requests.send_blocking(Request::Switch(tunnel, true)).ok();
            }
            "default" => {
                requests.send_blocking(Request::Panel(None)).ok();
            }
            _ => {}
        });
    });
}

/// An OpenVPN 3 process left running (`LogEntry::is_left_running`): stays
/// until dismissed, since only the user can stop it (the reason has how).
#[cfg(not(target_os = "macos"))]
pub fn left_running(entry: &LogEntry) {
    let n = note(entry, Urgency::Normal, STOPPED_SHOWN);
    std::thread::spawn(move || {
        let _ = n.show();
    });
}

/// Nothing shows tray icons (`no_tray`), told once at login: what would
/// bring the bird back, with the button that does it when Crake can.
#[cfg(not(target_os = "macos"))]
pub fn no_tray(remedy: &crate::no_tray::Remedy, requests: async_channel::Sender<Request>) {
    let mut n = Notification::new();
    n.appname("Crake")
        .summary(crate::no_tray::NOTIFICATION_TITLE)
        .body(remedy.notification())
        .urgency(Urgency::Normal)
        .timeout(Timeout::Milliseconds(STOPPED_SHOWN));
    let switch = match remedy {
        crate::no_tray::Remedy::SwitchOn(id) => Some(id.clone()),
        _ => None,
    };
    if switch.is_some() {
        n.action("switch", crate::no_tray::SWITCH_ON);
    }
    n.action("default", "Open Crake");
    std::thread::spawn(move || {
        let Ok(handle) = n.show() else { return };
        handle.wait_for_action(|action| {
            let request = match (action, &switch) {
                ("switch", Some(id)) => {
                    Request::NoTray(crate::tray_host::Event::SwitchOn(id.clone()))
                }
                ("default", _) => Request::Panel(None),
                _ => return,
            };
            requests.send_blocking(request).ok();
        });
    });
}

#[cfg(target_os = "macos")]
mod plain {
    use super::{LogEntry, Notification, Request};

    fn show(entry: &LogEntry) {
        let mut n = Notification::new();
        n.appname("Crake")
            .summary(&entry.message)
            .body(entry.reason.as_deref().unwrap_or(""));
        std::thread::spawn(move || {
            let _ = n.show();
        });
    }

    pub fn routed(entry: &LogEntry, _requests: async_channel::Sender<Request>) {
        show(entry);
    }

    pub fn also_answered(
        entry: &LogEntry,
        _others: Vec<(String, String)>,
        _requests: async_channel::Sender<Request>,
    ) {
        show(entry);
    }

    pub fn asked(entry: &LogEntry, _display: String, _requests: async_channel::Sender<Request>) {
        show(entry);
    }

    pub fn stopped(entry: &LogEntry, _requests: async_channel::Sender<Request>) {
        show(entry);
    }

    pub fn left_running(entry: &LogEntry) {
        show(entry);
    }

    /// macOS always has its menu bar: never called there.
    pub fn no_tray(_remedy: &crate::no_tray::Remedy, _requests: async_channel::Sender<Request>) {}
}

/// The corncrake's call, "crex crex": two short rasps, made here (no sound
/// file to ship) and played with whatever the desktop has.
pub fn crex() {
    std::thread::spawn(|| {
        let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
            return;
        };
        let path = std::path::PathBuf::from(dir).join("crake/crex.wav");
        if !path.exists() {
            if let Some(d) = path.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            if std::fs::write(&path, call_wav()).is_err() {
                return;
            }
        }
        for player in ["pw-play", "paplay", "aplay"] {
            let played = std::process::Command::new(player)
                .arg(&path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if played {
                return;
            }
        }
    });
}

/// The call as a WAV file: 16-bit mono.
fn call_wav() -> Vec<u8> {
    wav(&call_samples(), RATE)
}

/// The call's sample rate: plenty for a rasp.
const RATE: u32 = 22_050;

/// Two rasping syllables of rapid clicks.
// literal: the call's sound, tuned by ear to a corncrake's rasp
fn call_samples() -> Vec<i16> {
    let mut samples: Vec<i16> = Vec::new();
    let mut noise = Noise(0x00c0_ffee);
    let ms = |m: f32| (RATE as f32 * m / 1000.) as usize;
    for syllable in 0..2 {
        // About 14 clicks in a fifth of a second: the rasp.
        for click in 0..14 {
            let len = ms(7.);
            for i in 0..len {
                let t = i as f32 / RATE as f32;
                let env = (1. - i as f32 / len as f32).powi(2);
                let tone = (t * 2200. * std::f32::consts::TAU).sin() * 0.5
                    + (t * 3300. * std::f32::consts::TAU).sin() * 0.25;
                let v = (tone + noise.next() * 0.6) * env * 0.33;
                samples.push((v * i16::MAX as f32) as i16);
            }
            let gap = ms(7.5) + (click % 3) * ms(0.6);
            samples.extend(std::iter::repeat_n(0, gap));
        }
        if syllable == 0 {
            samples.extend(std::iter::repeat_n(0, ms(160.)));
        }
    }
    samples
}

/// White noise from Marsaglia's xorshift (shifts 13, 17, 5): the same every
/// time, from -1 to 1.
struct Noise(u32);

impl Noise {
    const A: u32 = 13;
    const B: u32 = 17;
    const C: u32 = 5;

    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << Self::A;
        self.0 ^= self.0 >> Self::B;
        self.0 ^= self.0 << Self::C;
        (self.0 as f32 / u32::MAX as f32) * 2. - 1.
    }
}

/// 16-bit mono PCM samples at `rate` as a WAV (RIFF) file.
fn wav(samples: &[i16], rate: u32) -> Vec<u8> {
    /// "RIFF" and the file's size, then "WAVE".
    const PREAMBLE: u32 = 8;
    /// "WAVE", the fmt chunk (its header and 16 bytes) and the data chunk's
    /// header: what the RIFF size counts besides the samples.
    const HEADERS: u32 = 36;
    const FMT_LENGTH: u32 = 16;
    const PCM: u16 = 1;
    const MONO: u16 = 1;
    const SAMPLE: u16 = size_of::<i16>() as u16;
    const BITS: u16 = SAMPLE * u8::BITS as u16;
    let data_len = size_of_val(samples) as u32;
    let mut wav = Vec::with_capacity((PREAMBLE + HEADERS + data_len) as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(HEADERS + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&FMT_LENGTH.to_le_bytes());
    wav.extend_from_slice(&PCM.to_le_bytes());
    wav.extend_from_slice(&MONO.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * u32::from(SAMPLE)).to_le_bytes()); // bytes a second
    wav.extend_from_slice(&SAMPLE.to_le_bytes()); // bytes a frame
    wav.extend_from_slice(&BITS.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_call_is_a_short_wav() {
        let wav = super::call_wav();
        assert_eq!(&wav[..4], b"RIFF");
        let secs = (wav.len() - 44) as f32 / 2. / 22_050.;
        assert!((0.4..0.8).contains(&secs), "{secs} s");
    }
}
