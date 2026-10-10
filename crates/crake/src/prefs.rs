//! What the app remembers for the user, apart from craked: how to introduce
//! itself to gateways that need it, and the notices the user has dismissed.
//! In `$XDG_STATE_HOME/crake/app.json` (`~/.local/state/crake/app.json`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    /// Gateway → user agent, for gateways that only let Cisco's client in.
    #[serde(default)]
    pub user_agents: BTreeMap<String, String>,
    /// Notices the user said "Got it" to, e.g. "anyconnect:acme".
    #[serde(default)]
    pub dismissed: BTreeSet<String>,
    /// "Copy for helpdesk" leaves host names and addresses out.
    #[serde(default)]
    pub hide_hosts: bool,
    /// "system", "light" or "dark".
    #[serde(default = "system")]
    pub appearance: String,
    /// When Crake routes a host: "none", "quiet" or "crex" (with the call).
    #[serde(default = "quiet")]
    pub notify: String,
    /// Show the bird in the top bar.
    #[serde(default = "yes")]
    pub show_bird: bool,
    /// Tunnels the user left on: switched on again when Crake starts at login.
    #[serde(default)]
    pub left_on: BTreeSet<String>,
    /// At login, switch on the tunnels left on (off: Crake only starts).
    #[serde(default)]
    pub reconnect: bool,
    /// Not written (demo mode): nothing real is remembered.
    #[serde(skip)]
    pub scratch: bool,
}

impl gpui_kit::Global for Prefs {}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            user_agents: Default::default(),
            dismissed: Default::default(),
            hide_hosts: false,
            appearance: system(),
            notify: quiet(),
            show_bird: true,
            left_on: Default::default(),
            reconnect: false,
            scratch: false,
        }
    }
}

fn system() -> String {
    "system".into()
}
fn quiet() -> String {
    "quiet".into()
}
fn yes() -> bool {
    true
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("crake/app.json"))
}

impl Prefs {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if self.scratch {
            return;
        }
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            // Whole or not at all.
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }

    /// The tunnel's "acts as AnyConnect" notice, while it should show.
    pub fn acting_as(&self, tunnel: &str, gateway: &str) -> Option<&str> {
        let ua = self.user_agents.get(gateway)?;
        (!self.dismissed.contains(&format!("anyconnect:{tunnel}"))).then_some(ua.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_shows_until_dismissed() {
        let mut p = Prefs::default();
        assert_eq!(p.acting_as("acme", "vpn.acme.example"), None);
        p.user_agents
            .insert("vpn.acme.example".into(), "AnyConnect".into());
        assert_eq!(p.acting_as("acme", "vpn.acme.example"), Some("AnyConnect"));
        p.dismissed.insert("anyconnect:acme".into());
        assert_eq!(p.acting_as("acme", "vpn.acme.example"), None);
        let back: Prefs = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }
}
