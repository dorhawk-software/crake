//! OpenVPN 3's sessions: the app starts one (as the user, who owns the
//! profile) and hands craked its device and DNS (`attach`); craked routes
//! through it like any tunnel, until it ends (`detach`).

use super::*;

/// openvpn3 may still be telling resolved a device's DNS: 2 s of grace.
const RESOLVED_POLLS: u32 = 10;
const RESOLVED_POLL: Duration = Duration::from_millis(200);

impl Manager {
    /// OpenVPN 3: the app started the session (as the user, who owns the
    /// profile) and hands over its device and DNS. Routing works as for
    /// openconnect tunnels from here.
    pub async fn attach(&self, name: &str, fields: &HashMap<String, String>) -> Result<()> {
        let display = {
            let mut map = self.tunnels.lock().unwrap();
            let e = map
                .get_mut(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if e.config.protocol != crate::config::Protocol::Openvpn3 {
                bail!("{} isn't an OpenVPN 3 tunnel", e.config.display());
            }
            // Already routed through this device (the app taking the
            // session back after a restart): nothing to do.
            if e.state == TunnelState::Connected
                && e.info.as_ref().map(|i| i.device.as_str())
                    == fields.get("device").map(String::as_str)
            {
                return Ok(());
            }
            e.state = TunnelState::Connecting;
            e.config.display().to_owned()
        };
        let device = fields.get("device").map(String::as_str).unwrap_or("");
        // Only a real tun device of this machine.
        if device.is_empty()
            || !device
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            || !crate::net::is_tun(device) && self.cfg.apply_network
        {
            bail!("{device:?} isn't a VPN device on this machine");
        }
        let list = |k: &str| -> Vec<String> {
            fields
                .get(k)
                .map(|v| {
                    v.split([',', '\n', ' '])
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut dns: Vec<Ipv4Addr> = list("dns").iter().filter_map(|a| a.parse().ok()).collect();
        let mut domains = list("domains");
        // The app can't read openvpn3's network settings (netcfg is closed
        // to users): take what openvpn3 gave resolved for the device, which
        // is what the server pushed. A moment's grace: it may still be
        // setting it.
        if dns.is_empty() {
            for _ in 0..RESOLVED_POLLS {
                if let Ok((servers, names)) = self.resolved.link(device).await
                    && !servers.is_empty()
                {
                    dns = servers;
                    if domains.is_empty() {
                        domains = names;
                    }
                    break;
                }
                tokio::time::sleep(RESOLVED_POLL).await;
            }
        }
        if let Some(how) = fields.get("transport").filter(|t| !t.is_empty()) {
            self.transport
                .lock()
                .unwrap()
                .insert(name.to_owned(), how.to_ascii_uppercase());
        }
        let own = crate::probe::ipv4_of(device);
        let servers = dns.iter().copied().map(IpAddr::V4).collect();
        let info = TunnelInfo {
            servers,
            device: device.to_owned(),
            gateway: fields.get("gateway").and_then(|g| g.parse().ok()),
            gateway6: fields.get("gateway").and_then(|g| g.parse().ok()),
            address: own.map(|(a, _)| a),
            prefix: own.map(|(_, p)| p),
            mtu: None,
            dns,
            address6: None,
            dns6: Vec::new(),
            domains: domains
                .into_iter()
                .map(|d| dns::normalize(&d))
                .filter(|d| dns::valid_domain(d))
                .collect(),
            split_include: list("routes"),
            split_exclude: Vec::new(),
        };
        self.tunnel_up(name, &display, info, true).await?;
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.attached = Some(fields.clone());
        }
        Ok(())
    }

    /// OpenVPN 3: the session ended (switched off in the app, or it dropped).
    pub async fn detach(&self, name: &str, failed: Option<String>) -> Result<()> {
        let label = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .map(|e| e.config.display().to_owned())
            .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
        // Refused at attach (it clashed): it never came up, nothing to undo.
        let never_up = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .is_some_and(|e| e.state == TunnelState::Off && e.info.is_none());
        if never_up {
            return Ok(());
        }
        self.remove_routing(name).await;
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.info = None;
            e.attached = None;
        }
        let state = if failed.is_some() {
            TunnelState::Trouble
        } else {
            TunnelState::Off
        };
        self.set_state(name, state, true);
        self.update_hush().await;
        let msg = match state {
            TunnelState::Trouble => crake_proto::stopped_unexpectedly(&label),
            _ => format!("Switched {label} off."),
        };
        self.log.system(Some(name), msg, failed);
        Ok(())
    }

    /// OpenVPN 3: a session that ended without the app saying so (the app
    /// quit, and openvpn3 took the session down with it) leaves its device
    /// gone. Such a tunnel is down, not "its DNS doesn't answer" for good.
    pub async fn check_devices(&self) {
        if !self.cfg.apply_network {
            return;
        }
        let attached: Vec<(String, String)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.attached.is_some())
            .filter_map(|(n, e)| Some((n.clone(), e.info.as_ref()?.device.clone())))
            .collect();
        for (name, device) in attached {
            if crate::net::is_tun(&device) {
                continue;
            }
            let why = format!("OpenVPN 3 ended its session: {device} is gone.");
            if let Err(e) = self.detach(&name, Some(why)).await {
                tracing::warn!("{name}: {e:#}");
            }
        }
    }
}
