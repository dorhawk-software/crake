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
