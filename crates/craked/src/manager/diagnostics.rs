//! craked's view of the computer, for the app's helpdesk copies: each
//! tunnel's device, address and DNS, and who answers the system's DNS.
//! What a helpdesk would otherwise ask the user to look up by hand.

use super::*;

impl Manager {
    /// One line per tunnel (in config order), then the system's DNS.
    pub fn diagnostics(&self) -> Vec<String> {
        let mut tunnels: Vec<(usize, String, String, TunnelState, Option<TunnelInfo>)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .map(|(name, e)| {
                (
                    e.index,
                    name.clone(),
                    e.config.display().to_owned(),
                    e.state,
                    e.info.clone(),
                )
            })
            .collect();
        tunnels.sort_by_key(|t| t.0);
        let mut lines = Vec::new();
        for (_, name, display, state, info) in tunnels {
            let Some(info) = info.filter(|_| state == TunnelState::Connected) else {
                lines.push(format!("{display}: {}", state.as_str()));
                continue;
            };
            let (learned, pinned) = {
                let mut store = self.hosts.lock().unwrap();
                let th = store.tunnel(&name);
                (th.learned.len(), th.pinned.len())
            };
            let address = info
                .address
                .map_or_else(|| "no address".to_owned(), |a| format!("address {a}"));
            let dns = if info.servers.is_empty() {
                "no DNS".to_owned()
            } else {
                format!("DNS {}", join(&info.servers))
            };
            let domains = if info.domains.is_empty() {
                String::new()
            } else {
                format!(" for {}", info.domains.join(", "))
            };
            lines.push(format!(
                "{display}: {}, {address}, {dns}{domains}; {learned} hosts learned, {pinned} pinned",
                info.device
            ));
        }
        lines.extend(crate::resolved::view());
        lines
    }
}
