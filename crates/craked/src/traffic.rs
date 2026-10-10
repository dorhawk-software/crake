//! How much traffic went direct, per minute, for the last hour: the panel's
//! grey bars. Physical interfaces' byte counters (Wi-Fi, Ethernet) minus what
//! went through Crake's tunnels (whose encrypted carrier also crosses them).
//! Counters only; nothing about destinations is kept.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MINUTES: usize = 61;
/// The log's day chart: all traffic per minute, for two days (it zooms
/// down to 1-minute bars).
const SLOT_MINUTES: u64 = 1;
const SLOTS: u64 = 2 * 24 * 60 / SLOT_MINUTES;
const EVERY: Duration = Duration::from_secs(10);

#[derive(Default)]
pub struct Traffic {
    /// (minute since the epoch, direct bytes), oldest first.
    minutes: Mutex<VecDeque<(u64, u64)>>,
    /// (minute since the epoch, all bytes), oldest first.
    slots: Mutex<VecDeque<(u64, u64)>>,
    last: Mutex<Option<(u64, u64)>>,
    /// Tunnel devices Crake routes through: device -> tunnel. openconnect's
    /// are named crake-<name>; OpenVPN 3's aren't (tun0, ...).
    devices: Mutex<HashMap<String, String>>,
    /// Per tunnel: (minute since the epoch, bytes through it), two days,
    /// oldest first. Tunnel detail's chart.
    tunnels: Mutex<HashMap<String, VecDeque<(u64, u64)>>>,
    /// Per tunnel device: its last counter reading.
    last_tunnels: Mutex<HashMap<String, u64>>,
}

/// What's kept on disk.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Saved {
    #[serde(default)]
    minutes: Vec<(u64, u64)>,
    #[serde(default)]
    slots: Vec<(u64, u64)>,
    #[serde(default)]
    tunnels: HashMap<String, Vec<(u64, u64)>>,
}

impl Traffic {
    /// Sample for the daemon's lifetime. The last hour is kept in `file`, so a
    /// restart doesn't empty the chart.
    pub fn spawn(self: &Arc<Self>, file: PathBuf) {
        self.load(&file, crate::hosts::now());
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(EVERY);
            let mut n = 0u32;
            loop {
                tick.tick().await;
                let devices = this.devices.lock().unwrap().clone();
                let (physical, tunnels, each) = counters(&devices);
                let now = crate::hosts::now();
                this.sample((physical, tunnels), now);
                this.sample_tunnels(each, &devices, now);
                n += 1;
                if n.is_multiple_of(6) {
                    this.save(&file);
                }
            }
        });
    }

    fn load(&self, file: &Path, now: u64) {
        let Ok(text) = std::fs::read_to_string(file) else {
            return;
        };
        // Older files held only the minutes.
        let saved = serde_json::from_str::<Saved>(&text).or_else(|_| {
            serde_json::from_str::<Vec<(u64, u64)>>(&text).map(|minutes| Saved {
                minutes,
                slots: Vec::new(),
                tunnels: HashMap::new(),
            })
        });
        let Ok(saved) = saved else { return };
        let minute = now / 60;
        let slot = minute / SLOT_MINUTES;
        self.minutes.lock().unwrap().extend(
            saved
                .minutes
                .into_iter()
                .filter(|(m, _)| *m <= minute && *m + MINUTES as u64 > minute),
        );
        self.slots.lock().unwrap().extend(
            saved
                .slots
                .into_iter()
                .filter(|(s, _)| *s <= slot && *s + SLOTS > slot),
        );
        let mut tunnels = self.tunnels.lock().unwrap();
        for (tunnel, minutes) in saved.tunnels {
            tunnels.insert(
                tunnel,
                minutes
                    .into_iter()
                    .filter(|(m, _)| *m <= minute && *m + SLOTS * SLOT_MINUTES > minute)
                    .collect(),
            );
        }
    }

    /// `device` carries `tunnel` from now on (until `untrack`).
    pub fn track(&self, device: &str, tunnel: &str) {
        self.devices
            .lock()
            .unwrap()
            .insert(device.to_owned(), tunnel.to_owned());
    }

    pub fn untrack(&self, device: &str) {
        self.devices.lock().unwrap().remove(device);
        self.last_tunnels.lock().unwrap().remove(device);
    }

    /// Bytes through each tunnel device since the last sample, by tunnel.
    fn sample_tunnels(
        &self,
        each: HashMap<String, u64>,
        devices: &HashMap<String, String>,
        now: u64,
    ) {
        let minute = now / 60;
        let mut last = self.last_tunnels.lock().unwrap();
        let mut tunnels = self.tunnels.lock().unwrap();
        for (device, bytes) in each {
            let Some(before) = last.insert(device.clone(), bytes) else {
                continue;
            };
            let Some(tunnel) = devices.get(&device) else {
                continue;
            };
            let added = bytes.saturating_sub(before);
            let series = tunnels.entry(tunnel.clone()).or_default();
            match series.back_mut() {
                Some((m, b)) if *m == minute => *b += added,
                _ => series.push_back((minute, added)),
            }
        }
        for series in tunnels.values_mut() {
            while series
                .front()
                .is_some_and(|(m, _)| *m + SLOTS * SLOT_MINUTES <= minute)
            {
                series.pop_front();
            }
        }
    }

    /// Bytes through `tunnel` per minute, two days: (minute since the
    /// epoch, bytes), oldest first.
    pub fn tunnel(&self, tunnel: &str) -> Vec<(u64, u64)> {
        self.tunnels
            .lock()
            .unwrap()
            .get(tunnel)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn save(&self, file: &Path) {
        // Slots as slot numbers (what `load` expects), not as minutes.
        let slots = self.slots.lock().unwrap().iter().copied().collect();
        let tunnels = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .map(|(t, s)| (t.clone(), s.iter().copied().collect()))
            .collect();
        let saved = Saved {
            minutes: self.last_hour(),
            slots,
            tunnels,
        };
        if let Ok(json) = serde_json::to_vec(&saved) {
            let tmp = file.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, file);
            }
        }
    }

    fn sample(&self, (physical, tunnels): (u64, u64), now: u64) {
        let mut last = self.last.lock().unwrap();
        let Some((p0, t0)) = last.replace((physical, tunnels)) else {
            return;
        };
        // Counters reset when an interface goes away; that interval counts as nothing.
        let direct = physical
            .saturating_sub(p0)
            .saturating_sub(tunnels.saturating_sub(t0));
        let minute = now / 60;
        let mut minutes = self.minutes.lock().unwrap();
        match minutes.back_mut() {
            Some((m, bytes)) if *m == minute => *bytes += direct,
            _ => minutes.push_back((minute, direct)),
        }
        while minutes
            .front()
            .is_some_and(|(m, _)| *m + MINUTES as u64 <= minute)
        {
            minutes.pop_front();
        }
        // Everything, tunnels included, for the day chart.
        let all = physical.saturating_sub(p0);
        let slot = minute / SLOT_MINUTES;
        let mut slots = self.slots.lock().unwrap();
        match slots.back_mut() {
            Some((s, bytes)) if *s == slot => *bytes += all,
            _ => slots.push_back((slot, all)),
        }
        while slots.front().is_some_and(|(s, _)| *s + SLOTS <= slot) {
            slots.pop_front();
        }
    }

    /// All traffic per minute, two days: (minute since the epoch the
    /// slot starts at, bytes), oldest first.
    pub fn days(&self) -> Vec<(u64, u64)> {
        self.slots
            .lock()
            .unwrap()
            .iter()
            .map(|(s, b)| (s * SLOT_MINUTES, *b))
            .collect()
    }

    /// (minute since the epoch, direct bytes), oldest first.
    pub fn last_hour(&self) -> Vec<(u64, u64)> {
        self.minutes.lock().unwrap().iter().copied().collect()
    }
}

/// (bytes on physical interfaces, bytes on Crake's tunnels, bytes per
/// tunnel device), both directions.
fn counters(devices: &HashMap<String, String>) -> (u64, u64, HashMap<String, u64>) {
    let (mut physical, mut tunnels, mut each) = (0, 0, HashMap::new());
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else {
        return (0, 0, each);
    };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let read = |f: &str| {
            std::fs::read_to_string(e.path().join("statistics").join(f))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0)
        };
        let bytes = read("rx_bytes") + read("tx_bytes");
        if name.starts_with("crake-") || devices.contains_key(&name) {
            tunnels += bytes;
            each.insert(name, bytes);
        } else if e.path().join("device").exists() {
            // A real device: not loopback, bridges, Docker or other tunnels.
            physical += bytes;
        }
    }
    (physical, tunnels, each)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_is_physical_minus_tunnels_per_minute() {
        let t = Traffic::default();
        t.sample((1_000, 0), 600);
        t.sample((1_500, 0), 610); // 500 direct
        t.sample((2_500, 400), 650); // 1000 physical, 400 of it tunnel
        t.sample((2_600, 400), 665); // next minute: 100
        assert_eq!(t.last_hour(), vec![(10, 1_100), (11, 100)]);
        t.sample((2_600, 400), 600 + 61 * 60);
        assert_eq!(
            t.last_hour().first().map(|m| m.0),
            Some(11),
            "older than an hour drops off"
        );
    }

    #[test]
    fn each_tunnel_counts_its_own_device() {
        let t = Traffic::default();
        t.track("tun0", "initech");
        let devices = t.devices.lock().unwrap().clone();
        let at = |bytes: u64| HashMap::from([("tun0".to_owned(), bytes)]);
        t.sample_tunnels(at(1_000), &devices, 600);
        t.sample_tunnels(at(1_400), &devices, 610);
        t.sample_tunnels(at(1_500), &devices, 665);
        assert_eq!(t.tunnel("initech"), vec![(10, 400), (11, 100)]);
        // A device nobody tracks counts for no tunnel.
        let other = HashMap::from([("tun1".to_owned(), 9_000)]);
        t.sample_tunnels(other.clone(), &devices, 670);
        t.sample_tunnels(HashMap::from([("tun1".to_owned(), 9_500)]), &devices, 680);
        assert_eq!(t.tunnel("initech"), vec![(10, 400), (11, 100)]);
        // Untracked: its counter starts afresh next time.
        t.untrack("tun0");
        assert!(t.last_tunnels.lock().unwrap().get("tun0").is_none());
    }

    #[test]
    fn a_restart_keeps_the_last_hour() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("traffic.json");
        let t = Traffic::default();
        t.sample((0, 0), 600);
        t.sample((500, 0), 610);
        t.sample((900, 0), 700);
        t.save(&file);
        let again = Traffic::default();
        again.load(&file, 720);
        assert_eq!(again.last_hour(), vec![(10, 500), (11, 400)]);
        assert_eq!(again.days(), vec![(10, 500), (11, 400)]);
        // A file from before the day chart still loads.
        std::fs::write(&file, "[[10,500]]").unwrap();
        let old = Traffic::default();
        old.load(&file, 620);
        assert_eq!(old.last_hour(), vec![(10, 500)]);
        let much_later = Traffic::default();
        much_later.load(&file, 600 + 2 * 3600);
        assert!(
            much_later.last_hour().is_empty(),
            "nothing older than an hour comes back"
        );
    }
}
