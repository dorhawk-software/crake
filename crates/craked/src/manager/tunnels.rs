//! A tunnel's life: switched on and off, openconnect run and supervised, its
//! hook, OpenVPN 3 sessions attached and detached, coming up and going down.

use super::*;

impl Manager {
    pub fn connect(self: &Arc<Self>, name: &str) -> Result<()> {
        self.connect_with(name, None)
    }

    /// Connect with a session from a separate sign-in ([`crate::auth`]).
    pub fn connect_with(self: &Arc<Self>, name: &str, session: Option<Session>) -> Result<()> {
        let (config, stop_rx, session) = {
            let mut map = self.tunnels.lock().unwrap();
            let e = map
                .get_mut(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if matches!(e.state, TunnelState::Connecting | TunnelState::Connected) {
                bail!("{} is already on", e.config.display());
            }
            // OpenVPN 3 sessions are the user's: the app starts them.
            if e.config.protocol == crate::config::Protocol::Openvpn3 {
                bail!("{} is switched on from the app.", e.config.display());
            }
            let (tx, rx) = oneshot::channel();
            if e.running {
                bail!("{} is still switching off", e.config.display());
            }
            e.stop = Some(tx);
            e.running = true;
            e.state = TunnelState::Connecting;
            e.info = None;
            // A new sign-in replaces the kept one; none given, resume the kept one.
            if session.is_some() {
                e.session = session.clone();
            }
            let session = session.or_else(|| e.session.clone());
            (e.config.clone(), rx, session)
        };
        let _ = self.changes.send(name.to_owned());
        self.log.system(
            Some(name),
            format!("Switched {} on.", config.display()),
            None,
        );
        let this = Arc::clone(self);
        tokio::spawn(async move { this.supervise(config, session, stop_rx).await });
        Ok(())
    }

    pub fn disconnect(&self, name: &str) -> Result<()> {
        let mut map = self.tunnels.lock().unwrap();
        let e = map
            .get_mut(name)
            .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
        // With a kept session, switch off without logging off, so switching
        // on again needs no new sign-in (unless the tunnel says otherwise).
        let keep = e.config.own.as_ref().is_none_or(|o| o.keep_signed_in);
        let how = if e.session.is_some() && keep {
            Stop::KeepSession
        } else {
            Stop::LogOff
        };
        match e.stop.take() {
            Some(tx) => {
                let _ = tx.send(how);
                Ok(())
            }
            // Trouble or Off: nothing is running; just settle to Off.
            None => {
                e.state = TunnelState::Off;
                drop(map);
                let _ = self.changes.send(name.to_owned());
                Ok(())
            }
        }
    }

    /// Stop every running tunnel and wait for them to finish (daemon shutdown).
    pub async fn shutdown(&self) {
        self.traffic
            .save(&self.cfg.state_path.with_file_name("traffic.json"));
        {
            // Leaving: log every session off.
            let mut map = self.tunnels.lock().unwrap();
            for e in map.values_mut() {
                e.session = None;
                if let Some(tx) = e.stop.take() {
                    let _ = tx.send(Stop::LogOff);
                }
            }
        }
        for _ in 0..120 {
            let busy = self.tunnels.lock().unwrap().values().any(|e| e.running);
            if !busy {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        // Leave nothing behind: Crake off is Crake gone.
        if self.cfg.apply_network && self.cfg.use_resolved {
            self.resolved.revert(crate::hush::DEVICE).await;
            self.net.remove(crate::hush::DEVICE).await;
        }
    }

    /// A tunnel's device is up with this info (openconnect's hook, or an
    /// OpenVPN 3 session attached by the app): DNS routes, routing, DNS,
    /// state, and a line in the log.
    pub(super) async fn tunnel_up(
        &self,
        name: &str,
        display: &str,
        info: TunnelInfo,
        first: bool,
    ) -> Result<()> {
        let display = display.to_owned();
        // What it got this time, for the next check before switching on.
        {
            let mut store = self.hosts.lock().unwrap();
            let t = store.tunnel(name);
            t.last_dns = info.dns.clone();
            t.last_net = own_net(&info);
            store.save();
        }
        // Two tunnels that share a DNS server, a network or a pinned network
        // can't both work: don't half-work, say which to switch off.
        if let Some((other, why)) = self.clash(name, &info.dns, own_net(&info)) {
            let text = format!("Turn {other} off to use {display}.");
            self.log.system(Some(name), text.clone(), Some(why.clone()));
            let stop = {
                let mut map = self.tunnels.lock().unwrap();
                map.get_mut(name).and_then(|e| {
                    e.refused = true;
                    e.stop.take()
                })
            };
            return match stop {
                // openconnect: stop it, keeping the sign-in for later.
                Some(tx) => {
                    let _ = tx.send(Stop::KeepSession);
                    Ok(())
                }
                // An OpenVPN 3 session: the app ends it.
                None => {
                    if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
                        e.refused = false;
                    }
                    self.set_state(name, TunnelState::Off, true);
                    Err(anyhow!("{text} {why}"))
                }
            };
        }
        for dns in &info.dns {
            self.net.route_dns(&info.device, *dns).await?;
        }
        let routed = self.apply_routing(name, &display, &info).await?;
        let mut first = first;
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.info = Some(info.clone());
            first &= !std::mem::take(&mut e.resumed);
            if first || e.up_since.is_none() {
                e.up_since = Some(hosts::now());
            }
            self.traffic.track(&info.device, name);
        }
        let domains = self.domains(name);
        self.register_dns(name, &display, &info, &domains).await;
        // Remember them, to keep them private while it's off.
        {
            let mut store = self.hosts.lock().unwrap();
            store.tunnel(name).domains = domains.clone();
            store.save();
        }
        self.update_hush().await;
        let summary = describe(&info, &domains);
        self.set_state(name, TunnelState::Connected, false);
        let full = self.learning(name) == crate::policy::Learning::FullTunnel;
        let text = match (first, routed) {
            (false, _) => format!("{display} is back."),
            (true, _) if full => {
                format!("{display} is up. Everything goes through it (full tunnel).")
            }
            (true, 0) => format!("{display} is up. Routing nothing yet."),
            (true, 1) => {
                format!("{display} is up. Sending 1 known destination through it.")
            }
            (true, n) => {
                format!("{display} is up. Sending {n} known destinations through it.")
            }
        };
        self.log.system(Some(name), text, Some(summary));
        Ok(())
    }

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
            || !std::path::Path::new(&format!("/sys/class/net/{device}/tun_flags")).exists()
                && self.cfg.apply_network
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
            for _ in 0..10 {
                if let Ok((servers, names)) = self.resolved.link(device).await
                    && !servers.is_empty()
                {
                    dns = servers;
                    if domains.is_empty() {
                        domains = names;
                    }
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
        if let Some(how) = fields.get("transport").filter(|t| !t.is_empty()) {
            self.transport
                .lock()
                .unwrap()
                .insert(name.to_owned(), how.to_ascii_uppercase());
        }
        let own = crate::probe::ipv4_of(device);
        let info = TunnelInfo {
            device: device.to_owned(),
            gateway: fields.get("gateway").and_then(|g| g.parse().ok()),
            address: own.map(|(a, _)| a),
            prefix: own.map(|(_, p)| p),
            mtu: None,
            dns,
            domains: domains
                .into_iter()
                .map(|d| dns::normalize(&d))
                .filter(|d| dns::valid_domain(d))
                .collect(),
            split_include: list("routes"),
            split_exclude: Vec::new(),
        };
        self.tunnel_up(name, &display, info, true).await
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
        }
        let state = if failed.is_some() {
            TunnelState::Trouble
        } else {
            TunnelState::Off
        };
        self.set_state(name, state, true);
        self.update_hush().await;
        let msg = match state {
            TunnelState::Trouble => format!("{label} stopped unexpectedly."),
            _ => format!("Switched {label} off."),
        };
        self.log.system(Some(name), msg, failed);
        Ok(())
    }

    /// Apply one call from the hook. An error makes the hook fail, which makes
    /// openconnect give up on that step.
    pub async fn handle_hook(&self, msg: HookMessage) -> Result<()> {
        let (display, state) = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(&msg.tunnel)
                .ok_or_else(|| anyhow!("unknown tunnel {}", msg.tunnel))?;
            (e.config.display().to_owned(), e.state)
        };
        // Only a tunnel Crake started may report in.
        if state == TunnelState::Off || state == TunnelState::Trouble {
            bail!("{display} is not running");
        }
        let name = msg.tunnel.as_str();

        match msg.reason.as_str() {
            "pre-init" => Ok(()),
            "connect" | "reconnect" => {
                let info = TunnelInfo::from_env(&msg.env)?;
                let expected = interface_name(name);
                if info.device != expected {
                    bail!("device {} does not belong to {display}", info.device);
                }
                if let Some(addr) = info.address {
                    self.net
                        .assign(&info.device, addr, info.prefix.unwrap_or(32), info.mtu)
                        .await?;
                }
                self.tunnel_up(name, &display, info, msg.reason == "connect")
                    .await
            }
            "attempt-reconnect" => {
                self.set_state(name, TunnelState::Connecting, false);
                self.log.system(
                    Some(name),
                    format!("{display} lost the gateway. Trying again."),
                    None,
                );
                Ok(())
            }
            "disconnect" => Ok(()), // the supervisor logs the end once the process exits
            other => {
                tracing::debug!(reason = other, "ignoring hook reason");
                Ok(())
            }
        }
    }

    pub(super) async fn supervise(
        self: Arc<Self>,
        config: TunnelConfig,
        session: Option<Session>,
        mut stop_rx: oneshot::Receiver<Stop>,
    ) {
        let name = config.name.clone();
        let label = config.display().to_owned();
        // Where UDP is known not to get through, start without DTLS (7b).
        let network = crate::networks::current();
        let configured = config.extra_args.iter().any(|a| a == "--no-dtls");
        let mut no_dtls = configured
            || network
                .as_deref()
                .is_some_and(|n| self.networks.lock().unwrap().udp_blocked(n));
        let (state, reason) = loop {
            let mut child = match self
                .spawn_openconnect(&config, session.clone(), no_dtls)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("{label}: {e:#}");
                    break (TunnelState::Trouble, Some(format!("{e:#}")));
                }
            };
            let last_line = Arc::new(Mutex::new(String::new()));
            let (udp_tx, mut udp_rx) = tokio::sync::mpsc::unbounded_channel();
            {
                // Over TCP from the start; otherwise openconnect says.
                let mut transport = self.transport.lock().unwrap();
                if no_dtls {
                    transport.insert(name.clone(), "TLS".to_owned());
                } else {
                    transport.remove(&name);
                }
            }
            let output = Output {
                tunnel: name.clone(),
                last: Arc::clone(&last_line),
                udp_failed: udp_tx,
                transport: Arc::clone(&self.transport),
            };
            forward_output(&mut child, output);

            tokio::select! {
                status = child.wait() => break match status {
                    Ok(s) if s.success() => (TunnelState::Off, None),
                    Ok(s) => {
                        let tail = last_line.lock().unwrap().clone();
                        (TunnelState::Trouble, Some(format!("openconnect exited with {s}. {tail}").trim().to_owned()))
                    }
                    Err(e) => (TunnelState::Trouble, Some(e.to_string())),
                },
                how = &mut stop_rx => {
                    let signal = match how {
                        Ok(Stop::KeepSession) => libc::SIGHUP,
                        _ => libc::SIGTERM,
                    };
                    terminate(&mut child, signal).await;
                    break (TunnelState::Off, None);
                }
                Some(()) = udp_rx.recv(), if !no_dtls => {
                    // UDP doesn't get through here: remember the network and
                    // carry on over TCP at once, keeping the sign-in.
                    if let Some(n) = &network {
                        self.networks.lock().unwrap().block_udp(n);
                    }
                    let can_resume = session.is_some() || config.passwd_file.is_some();
                    let why = "Its UDP channel (DTLS) didn\u{2019}t get through on this network.";
                    if !can_resume {
                        self.log.system(
                            Some(&name),
                            "UDP is blocked here.".to_owned(),
                            Some(format!("{why} Next time {label} starts over TCP here.")),
                        );
                        // openconnect carries on over HTTPS by itself.
                        let (state, reason) = tokio::select! {
                            status = child.wait() => match status {
                                Ok(s) if s.success() => (TunnelState::Off, None),
                                Ok(s) => (TunnelState::Trouble, Some(format!("openconnect exited with {s}."))),
                                Err(e) => (TunnelState::Trouble, Some(e.to_string())),
                            },
                            how = &mut stop_rx => {
                                let signal = match how {
                                    Ok(Stop::KeepSession) => libc::SIGHUP,
                                    _ => libc::SIGTERM,
                                };
                                terminate(&mut child, signal).await;
                                (TunnelState::Off, None)
                            }
                        };
                        break (state, reason);
                    }
                    self.log.system(
                        Some(&name),
                        format!("UDP is blocked here. {label} runs over TCP on this network."),
                        Some(format!("{why} Crake reconnected without it, keeping the sign-in.")),
                    );
                    if let Some(e) = self.tunnels.lock().unwrap().get_mut(&name) {
                        e.resumed = true;
                    }
                    terminate(&mut child, libc::SIGHUP).await;
                    no_dtls = true;
                }
            }
        };
        self.finish(&name, &label, state, reason).await;
    }

    pub(super) async fn finish(
        &self,
        name: &str,
        label: &str,
        state: TunnelState,
        reason: Option<String>,
    ) {
        self.remove_routing(name).await;
        self.update_hush().await;
        let refused = {
            let mut map = self.tunnels.lock().unwrap();
            match map.get_mut(name) {
                Some(e) => {
                    e.stop = None;
                    e.running = false;
                    // A failed run may mean the session is no good any more.
                    if state == TunnelState::Trouble && !e.refused {
                        e.session = None;
                    }
                    std::mem::take(&mut e.refused)
                }
                None => false,
            }
        };
        if refused {
            // Already said why ("Turn ACME off to use Nordbank.").
            self.set_state(name, TunnelState::Off, true);
            return;
        }
        self.set_state(name, state, true);
        let msg = match state {
            TunnelState::Trouble => format!("{label} stopped unexpectedly."),
            _ => format!("Switched {label} off."),
        };
        self.log.system(Some(name), msg, reason);
    }

    pub(super) async fn spawn_openconnect(
        &self,
        t: &TunnelConfig,
        session: Option<Session>,
        no_dtls: bool,
    ) -> Result<Child> {
        let mut cmd = Command::new(&self.cfg.openconnect);
        cmd.arg(format!("--protocol={}", t.protocol.as_str()))
            .arg(format!("--interface={}", interface_name(&t.name)))
            .arg(format!("--script={}", self.cfg.hook_path.display()))
            .arg("--non-inter");
        if let Some(user) = &t.user {
            cmd.arg(format!("--user={user}"));
        }
        let mut target = t.gateway.clone();
        let stdin_line = match (&session, &t.passwd_file) {
            (Some(s), _) => {
                cmd.arg("--cookie-on-stdin");
                if let Some(fp) = &s.fingerprint {
                    cmd.arg(format!("--servercert={fp}"));
                }
                if let Some(r) = &s.resolve {
                    cmd.arg(format!("--resolve={r}"));
                }
                // Introduce itself as at sign-in, unless the config says how.
                if let Some(ua) = &s.user_agent
                    && !t.extra_args.iter().any(|a| a.starts_with("--useragent"))
                {
                    cmd.arg(format!("--useragent={ua}"));
                }
                target = s.connect_url.clone();
                Some(s.cookie.clone())
            }
            (None, Some(p)) => {
                cmd.arg("--passwd-on-stdin");
                let text = tokio::fs::read_to_string(p)
                    .await
                    .with_context(|| format!("reading {}", p.display()))?;
                Some(text.lines().next().unwrap_or("").to_owned())
            }
            (None, None) => None,
        };
        if no_dtls && !t.extra_args.iter().any(|a| a == "--no-dtls") {
            cmd.arg("--no-dtls");
        }
        cmd.args(&t.extra_args)
            .arg("--")
            .arg(&target)
            // English output, whatever the system language, so it can be read.
            .env("LC_ALL", "C")
            .env(ENV_TUNNEL, &t.name)
            .env(ENV_SOCKET, &self.cfg.hook_socket)
            .stdin(if stdin_line.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Its own process group: Ctrl+C on a terminal reaches craked only,
            // which then stops openconnect in order.
            .process_group(0)
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .with_context(|| format!("starting {}", self.cfg.openconnect.display()))?;
        if let (Some(pw), Some(mut stdin)) = (stdin_line, child.stdin.take()) {
            stdin.write_all(format!("{pw}\n").as_bytes()).await.ok();
        }
        Ok(child)
    }
}

pub(super) fn describe(info: &TunnelInfo, domains: &[String]) -> String {
    let addr = info
        .address
        .map_or("no address".to_owned(), |a| a.to_string());
    let names = if domains.is_empty() || info.dns.is_empty() {
        "No names go to its DNS.".to_owned()
    } else {
        format!("Names under {} go to its DNS.", domains.join(", "))
    };
    format!(
        "Address {addr} on {}. {names} {} pushed network(s) kept as hints.",
        info.device,
        info.split_include.len()
    )
}

/// openconnect's output goes to the journal; its last line explains a
/// failure, and a sign that UDP doesn't get through is passed on.
/// Where one openconnect's output goes.
#[derive(Clone)]
pub(super) struct Output {
    tunnel: String,
    last: Arc<Mutex<String>>,
    udp_failed: tokio::sync::mpsc::UnboundedSender<()>,
    transport: Arc<Mutex<HashMap<String, String>>>,
}

pub(super) fn forward_output(child: &mut Child, output: Output) {
    if let Some(out) = child.stdout.take() {
        tokio::spawn(pump(BufReader::new(out), output.clone()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(pump(BufReader::new(err), output));
    }
}

pub(super) async fn pump<R: tokio::io::AsyncRead + Unpin>(r: BufReader<R>, o: Output) {
    let mut lines = r.lines();
    // Once UDP has worked here, a later hiccup isn't a blocked network.
    let mut udp_worked = false;
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        tracing::info!(target: "openconnect", tunnel = %o.tunnel, "{line}");
        let worked = crate::networks::udp_worked(&line);
        udp_worked |= worked;
        let failed = crate::networks::udp_failed(&line);
        if !udp_worked && failed {
            let _ = o.udp_failed.send(());
        }
        // How packets travel now, for tunnel detail.
        let now = if worked {
            Some(if line.starts_with("ESP") {
                "ESP"
            } else {
                "DTLS"
            })
        } else if failed || line.contains("using HTTPS instead") {
            Some("TLS")
        } else {
            None
        };
        if let Some(how) = now {
            o.transport
                .lock()
                .unwrap()
                .insert(o.tunnel.clone(), how.to_owned());
        }
        *o.last.lock().unwrap() = line;
    }
}

/// SIGTERM lets openconnect call the hook with `disconnect`; kill after a grace period.
pub(super) async fn terminate(child: &mut Child, signal: i32) {
    if let Some(pid) = child.id() {
        // SAFETY: plain signal to a child we own.
        unsafe { libc::kill(pid as i32, signal) };
    }
    if tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}
