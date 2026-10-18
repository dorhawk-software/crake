//! Tunnels added in the app: add, change, remove, and their fields for
//! editing.

use super::*;

/// A tunnel being removed gets 15 s to log off, looked at this often.
const LOGOFF_POLLS: u32 = 150;
const LOGOFF_POLL: Duration = Duration::from_millis(100);

impl Manager {
    /// The tunnels added in the app, as they stand.
    pub(super) fn own_tunnels(&self) -> Vec<crate::own::OwnTunnel> {
        let map = self.tunnels.lock().unwrap();
        let mut v: Vec<(usize, crate::own::OwnTunnel)> = map
            .values()
            .filter_map(|e| Some((e.index, e.config.own.clone()?)))
            .collect();
        v.sort_by_key(|(i, _)| *i);
        v.into_iter().map(|(_, o)| o).collect()
    }

    pub(super) fn save_own(&self) {
        crate::own::save(
            &crate::own::path_for(&self.cfg.state_path),
            &self.own_tunnels(),
        );
    }

    /// New tunnel: add one of the user's own. Returns its name.
    pub fn add_tunnel(&self, fields: &HashMap<String, String>) -> Result<String> {
        let name = {
            let map = self.tunnels.lock().unwrap();
            let taken: Vec<String> = map.keys().cloned().collect();
            crate::own::name_for(
                fields.get("display_name").map_or("", |s| s.as_str()),
                &taken,
            )
        };
        let mut fields = fields.clone();
        let profile = take_profile(&mut fields, true)?;
        let own = crate::own::OwnTunnel::from_map(&name, &fields)?;
        if let Some(text) = &profile {
            self.keep_profile(&name, text)?;
        }
        {
            let mut map = self.tunnels.lock().unwrap();
            if map.len() >= crate::routing::MAX_TUNNELS {
                bail!("Crake can hold {} tunnels.", crate::routing::MAX_TUNNELS);
            }
            // Its own mark, rule and table: the first index nobody uses.
            let used: Vec<usize> = map.values().map(|e| e.index).collect();
            let index = (0..).find(|i| !used.contains(i)).unwrap_or(0);
            map.insert(name.clone(), Entry::new(own.to_config(), index));
        }
        self.save_own();
        let _ = self.changes.send(name.clone());
        self.log.system(
            Some(&name),
            format!("Added {}.", own.display_name),
            Some(format!("{} at {}.", own.protocol.as_str(), own.gateway)),
        );
        Ok(name)
    }

    /// Edit one of the user's own tunnels. Sign-in changes apply the next
    /// time it connects; what may go through it, at once.
    pub async fn update_tunnel(&self, name: &str, fields: &HashMap<String, String>) -> Result<()> {
        let mut fields = fields.clone();
        let profile = take_profile(&mut fields, false)?;
        let own = crate::own::OwnTunnel::from_map(name, &fields)?;
        let (was, routed) = {
            let mut map = self.tunnels.lock().unwrap();
            let e = map
                .get_mut(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if e.config.own.is_none() {
                bail!(
                    "{} isn't one you added here, so it can't be changed here.",
                    e.config.display()
                );
            }
            let was = e.config.learning();
            e.config = own.to_config();
            (was, e.routed)
        };
        if let Some(text) = &profile {
            self.keep_profile(name, text)?;
        }
        self.save_own();
        // A new scope or a full tunnel: rebuild its routing now.
        if routed && was != own.learning() {
            let info = self
                .tunnels
                .lock()
                .unwrap()
                .get(name)
                .and_then(|e| e.info.clone());
            if let Some(info) = info {
                self.apply_routing(name, &own.display_name, &info).await?;
            }
        }
        let _ = self.changes.send(name.to_owned());
        self.log
            .system(Some(name), format!("Changed {}.", own.display_name), None);
        Ok(())
    }

    /// Remove one of the user's own tunnels: switched off first, its hosts
    /// forgotten. The log keeps its history.
    pub async fn remove_tunnel(&self, name: &str) -> Result<()> {
        let label = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if e.config.own.is_none() {
                bail!(
                    "{} isn't one you added here, so it can't be removed here.",
                    e.config.display()
                );
            }
            e.config.display().to_owned()
        };
        // Off first: log off, so no session is left behind on the gateway.
        let stop = self.tunnels.lock().unwrap().get_mut(name).and_then(|e| {
            e.session = None;
            e.stop.take()
        });
        if let Some(tx) = stop {
            let _ = tx.send(Stop::LogOff);
        }
        for _ in 0..LOGOFF_POLLS {
            let running = self
                .tunnels
                .lock()
                .unwrap()
                .get(name)
                .is_some_and(|e| e.running);
            if !running {
                break;
            }
            tokio::time::sleep(LOGOFF_POLL).await;
        }
        if self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .is_some_and(|e| e.running)
        {
            bail!("{label} didn't switch off. Try again in a moment.");
        }
        self.tunnels.lock().unwrap().remove(name);
        let _ = std::fs::remove_file(crate::openvpn::profile_path(&self.cfg.state_path, name));
        {
            let mut store = self.hosts.lock().unwrap();
            store.tunnels.remove(name);
            store.save();
        }
        {
            let mut settings = self.settings.lock().unwrap();
            settings.order.retain(|n| n != name);
            settings.save(&crate::settings::path_for(&self.cfg.state_path));
        }
        self.save_own();
        self.update_hush().await;
        let _ = self.changes.send(name.to_owned());
        self.log.system(
            Some(name),
            format!("Removed {label}."),
            Some("What Crake learned for it went too.".into()),
        );
        Ok(())
    }

    /// An OpenVPN tunnel's profile, as the user gave it (read again, against
    /// what may run, each time it starts): root's only.
    fn keep_profile(&self, name: &str, text: &str) -> Result<()> {
        use std::io::Write;
        let path = crate::openvpn::profile_path(&self.cfg.state_path, name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
        }
        let mut f = crate::sys::create_private(&path)
            .with_context(|| format!("writing {}", path.display()))?;
        f.write_all(text.as_bytes())?;
        Ok(())
    }

    /// One of the user's own tunnels in text form, for editing.
    pub fn own_fields(&self, name: &str) -> Option<BTreeMap<String, String>> {
        let map = self.tunnels.lock().unwrap();
        Some(map.get(name)?.config.own.as_ref()?.to_map())
    }
}

/// An OpenVPN tunnel's profile from AddTunnel's fields (needed when adding,
/// optional when changing one), checked; its gateway is the one it names.
fn take_profile(fields: &mut HashMap<String, String>, adding: bool) -> Result<Option<String>> {
    let openvpn = fields.get("protocol").map(String::as_str) == Some("openvpn");
    let text = fields.remove("profile").filter(|t| !t.trim().is_empty());
    if !openvpn {
        return Ok(None);
    }
    let Some(text) = text else {
        if adding {
            bail!("An OpenVPN tunnel needs its profile (a .ovpn file).");
        }
        return Ok(None);
    };
    let profile = crate::openvpn::profile::check(&text)?;
    fields.insert("gateway".into(), profile.remote);
    Ok(Some(text))
}
