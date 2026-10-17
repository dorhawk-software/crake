//! What the user can change from the Settings window (design/screens/
//! settings.html), kept next to the host store in `settings.json`. The config
//! file gives the defaults; a value set here wins.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::hosts::Target;

/// A read-only entry of the Settings map (not stored): why "Keep internal
/// names private" can't keep them now (`hush::why_not`).
pub const KEEP_PRIVATE_TROUBLE: &str = "keep_private_trouble";

/// When two tunnels answer for the same host.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Both {
    /// Use the first in order, and ask whether the other is wanted instead.
    #[default]
    Ask,
    Faster,
    First,
}

impl Both {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Faster => "faster",
            Self::First => "first",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Seconds to wait for a direct answer before trying the tunnels: 1, 2, 3 or 5.
    pub probe_wait_secs: u64,
    /// Forget learned hosts unused for this many days; None: never.
    pub forget_after_days: Option<u64>,
    /// Try tunnels in this order (names); the rest follow in config order.
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub both: Both,
    /// Never sent through a tunnel by learning: hosts, networks, or names
    /// ("*.mybank.example", "router.home").
    #[serde(default)]
    pub never: Vec<String>,
    /// Ask the tunnels about names public DNS doesn't know, and about the
    /// organisation's names that may have private addresses.
    pub ask_unknown: bool,
    /// Names under a tunnel's domains never go to public DNS, even with the
    /// tunnel off.
    pub keep_private: bool,
    /// Keep the log this many days; None: don't keep one on disk.
    pub log_days: Option<u64>,
}

impl Settings {
    pub fn defaults(cfg: &crate::config::Config) -> Self {
        Self {
            probe_wait_secs: cfg.probe_wait_secs,
            forget_after_days: Some(cfg.learned_expiry_days),
            order: Vec::new(),
            both: Both::Ask,
            never: cfg.never.iter().map(Target::to_string).collect(),
            ask_unknown: true,
            keep_private: true,
            log_days: Some(crate::config::LOG_DAYS),
        }
    }

    /// The saved settings, or the defaults.
    pub fn load(path: &Path, cfg: &crate::config::Config) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| Self::defaults(cfg))
    }

    pub fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(tmp, path);
            }
        }
    }

    /// Learned hosts' lifetime, in seconds. "Never" is ten years.
    pub fn expiry_secs(&self) -> u64 {
        self.forget_after_days.unwrap_or(crate::config::NEVER_DAYS) * crate::units::SECS_PER_DAY
    }

    /// Change one setting from its text form (the D-Bus API). Lists are one
    /// item per line.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let days = |v: &str| -> Result<Option<u64>> {
            match v {
                "never" | "off" => Ok(None),
                n => match n.parse::<u64>() {
                    Ok(d) if (1..=crate::config::MAX_DAYS).contains(&d) => Ok(Some(d)),
                    _ => bail!("{v:?} isn't a number of days"),
                },
            }
        };
        let flag = |v: &str| -> Result<bool> {
            match v {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => bail!("{v:?} isn't true or false"),
            }
        };
        let lines = |v: &str| -> Vec<String> {
            v.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect()
        };
        match key {
            "probe_wait_secs" => {
                let n: u64 = value.parse().unwrap_or(0);
                if !crate::config::PROBE_WAITS.contains(&n) {
                    bail!("Wait 1, 2, 3 or 5 seconds.");
                }
                self.probe_wait_secs = n;
            }
            "forget_after_days" => self.forget_after_days = days(value)?,
            "order" => self.order = lines(value),
            "both" => {
                self.both = match value {
                    "ask" => Both::Ask,
                    "faster" => Both::Faster,
                    "first" => Both::First,
                    _ => bail!("{value:?} isn't ask, faster or first"),
                }
            }
            "never" => {
                let items = lines(value);
                for i in &items {
                    let _ = Never::parse_one(i)?;
                }
                self.never = items;
            }
            "ask_unknown" => self.ask_unknown = flag(value)?,
            "keep_private" => self.keep_private = flag(value)?,
            "log_days" => self.log_days = days(value)?,
            _ => bail!("no setting called {key:?}"),
        }
        Ok(())
    }

    /// Every setting in its text form.
    pub fn to_map(&self) -> std::collections::HashMap<String, String> {
        let days = |d: Option<u64>| d.map_or("never".to_owned(), |d| d.to_string());
        [
            ("probe_wait_secs", self.probe_wait_secs.to_string()),
            ("forget_after_days", days(self.forget_after_days)),
            ("order", self.order.join("\n")),
            ("both", self.both.as_str().to_owned()),
            ("never", self.never.join("\n")),
            ("ask_unknown", self.ask_unknown.to_string()),
            ("keep_private", self.keep_private.to_string()),
            ("log_days", days(self.log_days)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
    }

    pub fn never(&self) -> Never {
        let mut n = Never::default();
        for item in &self.never {
            match Never::parse_one(item) {
                Ok(Ok(t)) => n.targets.push(t),
                Ok(Err(name)) => n.names.push(name),
                Err(_) => {}
            }
        }
        n
    }
}

/// Where `settings.json` lives: next to the host store.
pub fn path_for(state_path: &Path) -> PathBuf {
    state_path.with_file_name("settings.json")
}

/// The "never send through a tunnel" list, ready to check.
#[derive(Debug, Default, Clone)]
pub struct Never {
    pub targets: Vec<Target>,
    /// "*.mybank.example" (names under it) or "router.home" (that name).
    pub names: Vec<String>,
}

impl Never {
    /// A host or network (Ok(Ok)), or a name pattern (Ok(Err)).
    fn parse_one(item: &str) -> Result<std::result::Result<Target, String>> {
        if let Ok(t) = item.parse::<Target>() {
            return Ok(Ok(t));
        }
        let bare = item.strip_prefix("*.").unwrap_or(item);
        if crate::dns::valid_domain(bare) && bare.contains('.') {
            return Ok(Err(item.to_ascii_lowercase()));
        }
        bail!("{item:?} is neither a host, a network nor a name.")
    }

    /// Is this host (known as `name`, if a name is known) on the list?
    pub fn covers(&self, name: Option<&str>, addr: impl Into<std::net::IpAddr>) -> bool {
        let addr = addr.into();
        self.targets.iter().any(|t| t.contains(addr))
            || name.is_some_and(|n| {
                let n = crate::dns::normalize(n);
                self.names.iter().any(|p| match p.strip_prefix("*.") {
                    Some(d) => n.ends_with(&format!(".{d}")),
                    None => n == *p,
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_change_from_their_text_form() {
        let mut s = Settings::defaults(&crate::config::Config::default());
        s.set("probe_wait_secs", "5").unwrap();
        assert!(s.set("probe_wait_secs", "4").is_err());
        s.set("forget_after_days", "never").unwrap();
        assert_eq!(s.expiry_secs(), 3650 * 86_400);
        s.set("both", "faster").unwrap();
        s.set("never", "*.mybank.example\n192.168.1.0/24\n")
            .unwrap();
        assert!(s.set("never", "not a host!").is_err());
        let n = s.never();
        let ip = |a: &str| a.parse::<std::net::Ipv4Addr>().unwrap();
        assert!(n.covers(Some("www.MyBank.example."), ip("20.0.0.1")));
        assert!(!n.covers(Some("mybank.example.evil"), ip("20.0.0.1")));
        assert!(n.covers(None, ip("192.168.1.9")));
        s.set("log_days", "never").unwrap();
        let back = s.to_map();
        assert_eq!(back["log_days"], "never");
        assert_eq!(back["never"], "*.mybank.example\n192.168.1.0/24");
        let mut again = Settings::defaults(&crate::config::Config::default());
        for (k, v) in &back {
            again.set(k, v).unwrap();
        }
        assert_eq!(again, s);
    }
}
