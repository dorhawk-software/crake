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
        // openvpn3 doesn't always name the session's device (seen with an
        // Azure gateway's profile): the process running the session, which
        // the app names too, holds it open.
        let named = fields.get("device").map(String::as_str).unwrap_or("");
        let device = if named.is_empty() {
            fields
                .get("backend_pid")
                .and_then(|p| p.parse().ok())
                .and_then(crate::sys::tun_of_process)
                .unwrap_or_default()
        } else {
            named.to_owned()
        };
        let device = device.as_str();
        let display = {
            let mut map = self.tunnels.lock().unwrap();
            let e = map
                .get_mut(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if e.config.protocol != crate::config::Protocol::Openvpn3 {
                bail!("{} isn't an OpenVPN 3 tunnel", e.config.display());
            }
            if !crate::sys::ADOPTS_OPENVPN3 {
                bail!(
                    "OpenVPN 3 isn't available on this system: Crake can't use {} here yet.",
                    e.config.display()
                );
            }
            // Already routed through this device (the app taking the
            // session back after a restart): nothing to do.
            if e.state == TunnelState::Connected
                && e.info.as_ref().map(|i| i.device.as_str()) == Some(device)
            {
                return Ok(());
            }
            e.config.display().to_owned()
        };
        // Only a real tun device of this machine. Refused, it says why in
        // the log too: the app's notification of it goes away.
        let refused = if device.is_empty() {
            Some(format!(
                "OpenVPN 3 didn\u{2019}t say which device {display}\u{2019}s session uses."
            ))
        } else if !device
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            || crate::net::tun_index(device).is_none() && self.cfg.apply_network
        {
            Some(format!(
                "{device:?} isn\u{2019}t a VPN device on this machine."
            ))
        } else {
            None
        };
        if let Some(why) = refused {
            // A tunnel already up on another device keeps going.
            let up = self
                .tunnels
                .lock()
                .unwrap()
                .get(name)
                .is_some_and(|e| e.info.is_some());
            if !up {
                self.set_trouble(name, Some(why.clone()));
                self.log.system(
                    Some(name),
                    format!("{display} couldn\u{2019}t connect."),
                    Some(why.clone()),
                );
            }
            bail!(why);
        }
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.state = TunnelState::Connecting;
            e.why = None;
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
        // (Not this machine's devices when told to touch nothing: tests.)
        let own = if self.cfg.apply_network {
            crate::probe::ipv4_of(device)
        } else {
            None
        };
        // What it settled on, kept with the handover's fields: a craked
        // taking over after an upgrade finds the link reset by the one
        // before it, and a tunnel registered with no domains can't keep
        // openvpn3 from making it every name's DNS again (2026-10-15).
        let settled_dns = dns
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let settled_domains = domains.join(",");
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
            let mut fields = fields.clone();
            fields.insert("device".to_owned(), device.to_owned());
            if let Some(index) = crate::net::tun_index(device) {
                fields.insert("index".to_owned(), index.to_string());
            }
            if !settled_dns.is_empty() {
                fields.insert("dns".to_owned(), settled_dns);
            }
            if !settled_domains.is_empty() {
                fields.insert("domains".to_owned(), settled_domains);
            }
            e.attached = Some(fields);
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
        // Refused at attach (it clashed, or its device wasn't one): it
        // never came up, nothing to undo, and the log has said why.
        let never_up = self.tunnels.lock().unwrap().get(name).is_some_and(|e| {
            matches!(e.state, TunnelState::Off | TunnelState::Trouble) && e.info.is_none()
        });
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
        if state == TunnelState::Trouble {
            self.set_trouble(name, failed.clone());
        } else {
            self.set_state(name, state, true);
        }
        self.update_hush().await;
        let msg = match state {
            TunnelState::Trouble => crake_proto::stopped_unexpectedly(&label),
            _ => format!("Switched {label} off."),
        };
        self.log.system(Some(name), msg, failed);
        Ok(())
    }

    /// OpenVPN 3: where each attached session's device is now. openvpn3
    /// makes a session's device again when it reconnects it (a network
    /// that went away for a while, a wake from sleep), often under another
    /// name (tun1 came back as tun0), or under the same name as a new
    /// device, whose routes the kernel took with the old one. While the
    /// process running the session lives, so does the session: craked says
    /// it's reconnecting, and routes through its device once it's back. A
    /// session whose device is gone with no process left (the app quit,
    /// and openvpn3 took the session down with it) is down, not "its DNS
    /// doesn't answer" for good.
    pub async fn check_devices(&self) {
        if !self.cfg.apply_network {
            return;
        }
        let attached: Vec<(String, HashMap<String, String>, bool)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(n, e)| Some((n.clone(), e.attached.clone()?, e.info.is_some())))
            .collect();
        for (name, fields, up) in attached {
            let device = fields.get("device").cloned().unwrap_or_default();
            let had = fields.get("index").and_then(|i| i.parse::<u32>().ok());
            let index = crate::net::tun_index(&device);
            // The device it was handed is still there: up, whatever craked
            // can see of the session's process. (As the package runs it,
            // craked can't look into openvpn3's process, which runs as
            // another user: 0.8.0 took that for a session reconnecting and
            // pulled every OpenVPN 3 tunnel's routes, 2026-10-15.)
            if up && index.is_some() && (had.is_none() || had == index) {
                continue;
            }
            // The same name as a new device (the kernel took the old one's
            // routes with it), or back after a reconnect: routed again.
            if index.is_some() {
                let mut fields = fields;
                fields.remove("index");
                self.attach_again(&name, &fields).await;
                continue;
            }
            // Gone. While the session's process runs, so does the session:
            // its new device, where craked can see it, or wait (the app
            // hands the session over again once openvpn3 has it back).
            let pid = fields
                .get("backend_pid")
                .and_then(|p| p.parse().ok())
                .filter(|p| crate::sys::process_alive(*p));
            if let Some(pid) = pid {
                match crate::sys::tun_of_process(pid) {
                    Some(now) if crate::net::tun_index(&now).is_some() => {
                        let mut fields = fields;
                        fields.insert("device".to_owned(), now);
                        fields.remove("index");
                        self.attach_again(&name, &fields).await;
                    }
                    _ if up => self.reconnecting(&name).await,
                    _ => {}
                }
                continue;
            }
            let why = format!(
                "{}: {device} is gone. Switch it on to connect again.",
                crake_proto::trouble::OPENVPN3_ENDED
            );
            if let Err(e) = self.detach(&name, Some(why)).await {
                tracing::warn!("{name}: {e:#}");
            }
        }
    }

    /// OpenVPN 3 is setting a session up again: its device is gone, and
    /// what went through it waits for the new one.
    async fn reconnecting(&self, name: &str) {
        let Ok((_, label, _)) = self.entry(name) else {
            return;
        };
        self.remove_routing(name).await;
        self.set_state(name, TunnelState::Connecting, true);
        self.update_hush().await;
        self.log.system(
            Some(name),
            format!("{label} is reconnecting."),
            Some("OpenVPN 3 is setting its session up again.".to_owned()),
        );
    }

    /// The session's device is back (or new): route through it as at the
    /// handover. craked's own settings for the old one go first.
    async fn attach_again(&self, name: &str, fields: &HashMap<String, String>) {
        self.remove_routing(name).await;
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.attached = None;
        }
        self.set_state(name, TunnelState::Connecting, true);
        if let Err(e) = self.attach(name, fields).await {
            tracing::warn!("{name}: {e:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where OpenVPN 3 isn't (Windows, macOS), a session handed over is
    /// refused saying so, not taken for a device that isn't there.
    #[tokio::test]
    async fn openvpn3_sessions_are_refused_where_there_is_no_openvpn3() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = Manager::new(crate::config::Config {
            log_path: dir.path().join("log"),
            state_path: dir.path().join("hosts.json"),
            managed_dir: dir.path().join("managed.d"),
            probing: false,
            use_resolved: false,
            apply_network: false,
            ..Default::default()
        })
        .unwrap();
        let name = mgr
            .add_tunnel(&HashMap::from([
                ("display_name".to_owned(), "Initech".to_owned()),
                ("gateway".to_owned(), "initech".to_owned()),
                ("protocol".to_owned(), "openvpn3".to_owned()),
            ]))
            .unwrap();
        let fields = HashMap::from([("device".to_owned(), "tun0".to_owned())]);
        let attached = mgr.attach(&name, &fields).await;
        if crate::sys::ADOPTS_OPENVPN3 {
            attached.unwrap();
        } else {
            let e = format!("{:#}", attached.unwrap_err());
            assert!(e.contains("OpenVPN 3 isn't available"), "{e}");
            assert_eq!(mgr.get(&name).unwrap().state, TunnelState::Off);
        }
    }
}
