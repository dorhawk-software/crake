//! A tunnel that lost its gateway, and its return: what the log says about
//! its hosts meanwhile (design/screens/tray-panel.html?moment=back).

use super::*;
use crate::units::{SECS_PER_HOUR as HOUR, SECS_PER_MINUTE as MINUTE};

impl Manager {
    /// Why "lost the gateway" matters: what can't be reached until it's back.
    pub(super) fn while_lost(&self, name: &str) -> String {
        if self.learning(name) == crate::policy::Learning::FullTunnel {
            return "Everything went through it, so nothing can be reached until it\u{2019}s back."
                .into();
        }
        match self.host_count(name) {
            0 => "Nothing went through it. Everything goes direct as usual.".into(),
            n => format!(
                "{} can\u{2019}t be reached until it\u{2019}s back. Everything else goes direct as usual.",
                its(n)
            ),
        }
    }

    /// Why "is back" matters: how long it was gone, and its hosts again.
    pub(super) fn back_after(&self, name: &str, lost_at: Option<u64>) -> String {
        let gone =
            lost_at.map(|at| format!("Gone for {}.", gone_for(hosts::now().saturating_sub(at))));
        let hosts = if self.learning(name) == crate::policy::Learning::FullTunnel {
            Some("Everything goes through it again.".to_owned())
        } else {
            match self.host_count(name) {
                0 => None,
                1 => Some(format!("{} goes through it again.", its(1))),
                n => Some(format!("{} go through it again.", its(n))),
            }
        };
        match (gone, hosts) {
            (Some(gone), Some(hosts)) => format!("{gone} {hosts}"),
            (Some(one), None) | (None, Some(one)) => one,
            (None, None) => "Nothing went through it yet.".into(),
        }
    }

    /// Its pinned and learned hosts.
    fn host_count(&self, name: &str) -> usize {
        self.hosts
            .lock()
            .unwrap()
            .tunnels
            .get(name)
            .map_or(0, |t| t.pinned.len() + t.learned.len())
    }
}

/// "Its 1 host", "Its 5 hosts".
fn its(n: usize) -> String {
    if n == 1 {
        "Its 1 host".into()
    } else {
        format!("Its {n} hosts")
    }
}

/// "42 s", "3 min", "1 h 5 min".
fn gone_for(secs: u64) -> String {
    match secs {
        s if s < MINUTE => format!("{s} s"),
        s if s < HOUR => format!("{} min", s / MINUTE),
        s if s % HOUR < MINUTE => format!("{} h", s / HOUR),
        s => format!("{} h {} min", s / HOUR, s % HOUR / MINUTE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gone_for_reads_as_the_panel_says_it() {
        assert_eq!(gone_for(42), "42 s");
        assert_eq!(gone_for(3 * 60 + 10), "3 min");
        assert_eq!(gone_for(3600 + 5 * 60), "1 h 5 min");
        assert_eq!(gone_for(2 * 3600), "2 h");
        assert_eq!(its(1), "Its 1 host");
        assert_eq!(its(5), "Its 5 hosts");
    }
}
