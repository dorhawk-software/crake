//! A quiet notification per host Crake routed on its own, with Undo
//! (design/DESIGN.md, Platform notes). Decisions you made yourself aren't
//! announced back to you. Settings choose: nothing, a quiet notification, or
//! one with the bird's call ("crex crex").

use crake_proto::{LogEntry, LogKind};
use notify_rust::{Notification, Timeout, Urgency};

use crate::Request;

/// Crake decided it, not you.
pub fn worth_telling(entry: &LogEntry) -> bool {
    entry.kind == LogKind::Routed
        && !entry.hosts.is_empty()
        && !entry.reason.as_deref().unwrap_or("").starts_with("You ")
}

/// What every notification starts from: the entry's message and reason.
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

pub fn routed(entry: &LogEntry, requests: async_channel::Sender<Request>) {
    let (Some(tunnel), hosts) = (entry.tunnel.clone(), entry.hosts.clone()) else {
        return;
    };
    let mut n = note(entry, Urgency::Low, 8000);
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
pub fn also_answered(
    entry: &LogEntry,
    others: Vec<(String, String)>,
    requests: async_channel::Sender<Request>,
) {
    let (Some(from), hosts) = (entry.tunnel.clone(), entry.hosts.clone()) else {
        return;
    };
    let mut n = note(entry, Urgency::Normal, 15000);
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
pub fn asked(entry: &LogEntry, display: String, requests: async_channel::Sender<Request>) {
    let (Some(tunnel), hosts) = (entry.tunnel.clone(), entry.hosts.clone()) else {
        return;
    };
    let mut n = note(entry, Urgency::Normal, 20000);
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

/// Two rasping syllables of rapid clicks, as a 16-bit mono WAV.
fn call_wav() -> Vec<u8> {
    const RATE: u32 = 22_050;
    let mut samples: Vec<i16> = Vec::new();
    let mut seed: u32 = 0x00c0_ffee;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2. - 1.
    };
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
                let v = (tone + noise() * 0.6) * env * 0.33;
                samples.push((v * i16::MAX as f32) as i16);
            }
            let gap = ms(7.5) + (click % 3) * ms(0.6);
            samples.extend(std::iter::repeat_n(0, gap));
        }
        if syllable == 0 {
            samples.extend(std::iter::repeat_n(0, ms(160.)));
        }
    }
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
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
