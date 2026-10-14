//! Settings changed in the app, keeping internal names private (the hush
//! link), and clearing the log.

use super::*;

impl Manager {
    /// Change one setting; it takes effect at once.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let (expiry_changed, log_days) = {
            let mut s = self.settings.lock().unwrap();
            let before = s.expiry_secs();
            s.set(key, value)?;
            s.save(&crate::settings::path_for(&self.cfg.state_path));
            (s.expiry_secs() != before, s.log_days)
        };
        match key {
            "log_days" => self.log.keep(log_days),
            // Learned hosts' timeouts live in the kernel sets: reload them.
            "forget_after_days" if expiry_changed => {
                let routed: Vec<(String, String, TunnelInfo)> = {
                    let map = self.tunnels.lock().unwrap();
                    map.values()
                        .filter(|e| e.routed)
                        .filter_map(|e| {
                            Some((
                                e.config.name.clone(),
                                e.config.display().to_owned(),
                                e.info.clone()?,
                            ))
                        })
                        .collect()
                };
                for (name, label, info) in routed {
                    if let Err(e) = self.apply_routing(&name, &label, &info).await {
                        tracing::error!("reloading {name} for the new expiry: {e:#}");
                    }
                }
            }
            "ask_unknown" => self.unknown.lock().unwrap().clear(),
            "keep_private" => self.update_hush().await,
            _ => {}
        }
        let _ = self.changes.send(String::new());
        Ok(())
    }

    /// "Keep internal names private": the domains of tunnels that aren't up
    /// go to the hush link (see `hush`); with none, or the setting off, it goes.
    pub async fn update_hush(&self) {
        if !(self.cfg.apply_network && self.cfg.use_resolved) {
            return;
        }
        let domains = self.hush_domains().await;
        self.apply_hush(domains).await;
    }

    /// Another VPN client may start or stop serving a company's domain at
    /// any moment: looked at again (with the device check), and changed
    /// only when that changes what's kept private.
    pub(super) async fn refresh_hush(&self) {
        if !(self.cfg.apply_network && self.cfg.use_resolved) {
            return;
        }
        let domains = self.hush_domains().await;
        if self.hush_last.lock().unwrap().as_ref() != Some(&domains) {
            self.apply_hush(domains).await;
        }
    }

    /// The domains kept private now: those of the tunnels that are off,
    /// less what another program serves.
    async fn hush_domains(&self) -> Vec<String> {
        let private = self.settings.lock().unwrap().keep_private;
        let domains: Vec<String> = if private {
            let map = self.tunnels.lock().unwrap();
            let store = self.hosts.lock().unwrap();
            let mut v: Vec<String> = Vec::new();
            for e in map.values().filter(|e| !e.routed) {
                let saved = store.tunnels.get(&e.config.name).map(|t| t.domains.clone());
                for d in saved
                    .unwrap_or_default()
                    .iter()
                    .chain(&e.config.dns_domains)
                {
                    let d = dns::normalize(d);
                    if dns::valid_domain(&d) && !v.contains(&d) {
                        v.push(d);
                    }
                }
            }
            v
        } else {
            Vec::new()
        };
        if domains.is_empty() {
            return domains;
        }
        let ours: Vec<String> = self
            .tunnels
            .lock()
            .unwrap()
            .values()
            .filter_map(|e| Some(e.info.as_ref()?.device.clone()))
            .collect();
        not_served(domains, &self.resolved.served_elsewhere(&ours).await)
    }

    async fn apply_hush(&self, domains: Vec<String>) {
        *self.hush_last.lock().unwrap() = Some(domains.clone());
        let _hush = self.hush.lock().await;
        if domains.is_empty() {
            self.resolved.revert(crate::hush::DEVICE).await;
            self.net.remove(crate::hush::DEVICE).await;
            return;
        }
        if let Err(e) = self.net.dummy(crate::hush::DEVICE, crate::hush::ADDR).await {
            tracing::error!("keeping internal names private: {e:#}");
            return;
        }
        if !self
            .hush_serving
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            tokio::spawn(async {
                if let Err(e) = crate::hush::serve().await {
                    tracing::error!("answering for private names: {e:#}");
                }
            });
        }
        if let Err(e) = self
            .resolved
            .set_link(
                crate::hush::DEVICE,
                &[crate::hush::ADDR.into()],
                &domains,
                false,
            )
            .await
        {
            tracing::error!("keeping internal names private: {e:#}");
        }
        self.resolved.flush().await;
    }

    /// Empty the log. Learned hosts stay.
    pub fn clear_log(&self) {
        self.log.clear();
    }
}

/// The domains to keep private, less those another program's DNS settings
/// serve now (another VPN client signed in to the same company: AnyConnect
/// beside Crake on a colleague's Mac kept failing every internal name,
/// refused by Crake): a domain served elsewhere, or one under it, is left to
/// that program.
fn not_served(domains: Vec<String>, served: &[String]) -> Vec<String> {
    let served: Vec<String> = served
        .iter()
        .map(|d| d.trim_end_matches('.').to_ascii_lowercase())
        .filter(|d| !d.is_empty())
        .collect();
    domains
        .into_iter()
        .filter(|d| {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            !served
                .iter()
                .any(|s| d == *s || d.ends_with(&format!(".{s}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_another_vpn_serves_arent_kept_private() {
        let hush = ["acme.internal", "ux.acme.internal", "nordbank.example"]
            .map(String::from)
            .to_vec();
        let served = ["ACME.internal.".to_owned(), "home".to_owned()];
        assert_eq!(not_served(hush.clone(), &served), ["nordbank.example"]);
        // Served only further down: the rest stays private.
        let deeper = ["ux.acme.internal".to_owned()];
        assert_eq!(
            not_served(hush.clone(), &deeper),
            ["acme.internal", "nordbank.example"]
        );
        // A name merely ending the same way isn't under it.
        let other = ["me.internal".to_owned()];
        assert_eq!(not_served(hush.clone(), &other), hush);
        assert_eq!(not_served(hush.clone(), &[]), hush);
    }
}
