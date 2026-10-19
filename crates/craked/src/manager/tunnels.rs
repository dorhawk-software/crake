//! A tunnel's life: switched on and off, openconnect run and supervised, its
//! hook, coming up and going down (OpenVPN 3's sessions: `attached`).

use super::*;

/// Leaving: the tunnels get 12 s to log off, looked at every POLL.
const STOP_POLLS: u32 = 120;
const POLL: Duration = Duration::from_millis(100);

impl Manager {
    pub fn connect(self: &Arc<Self>, name: &str) -> Result<()> {
        self.connect_with(name, None)
    }

    /// Connect with a session from a separate sign-in ([`crate::auth`]).
    pub fn connect_with(self: &Arc<Self>, name: &str, session: Option<Session>) -> Result<()> {
        let (config, stop_rx, session, quiet) = {
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
            e.why = None;
            e.info = None;
            // A new sign-in replaces the kept one; none given, resume the kept one.
            if session.is_some() {
                e.session = session.clone();
            }
            let session = session.or_else(|| e.session.clone());
            (
                e.config.clone(),
                rx,
                session,
                std::mem::take(&mut e.handover),
            )
        };
        let _ = self.changes.send(name.to_owned());
        if !quiet {
            let line = format!("Switched {} on.", config.display());
            self.log.system(Some(name), line, None);
        }
        let this = Arc::clone(self);
        if config.protocol == crate::config::Protocol::Openvpn {
            tokio::spawn(async move { this.supervise_openvpn(config, stop_rx).await });
            return Ok(());
        }
        if config.protocol == crate::config::Protocol::Ipsec {
            #[cfg(target_os = "linux")]
            {
                tokio::spawn(async move { this.supervise_ipsec(config, stop_rx).await });
                return Ok(());
            }
            #[cfg(not(target_os = "linux"))]
            {
                drop(stop_rx);
                self.set_state(name, TunnelState::Off, false);
                if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
                    e.running = false;
                    e.stop = None;
                }
                bail!("IPsec tunnels aren\u{2019}t built for this system yet.");
            }
        }
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
        let how = if (e.session.is_some() || e.token.is_some()) && keep {
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
                e.why = None;
                drop(map);
                let _ = self.changes.send(name.to_owned());
                Ok(())
            }
        }
    }

    /// The user has seen that the tunnel stopped (the panel's Dismiss,
    /// design/proposals/dismiss-trouble.html): Off, with nothing logged.
    /// Why it stopped stays, for tunnel detail, until it's switched on.
    pub fn dismiss_trouble(&self, name: &str) -> Result<()> {
        let mut map = self.tunnels.lock().unwrap();
        let e = map
            .get_mut(name)
            .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
        if e.state != TunnelState::Trouble {
            return Ok(());
        }
        e.state = TunnelState::Off;
        drop(map);
        let _ = self.changes.send(name.to_owned());
        Ok(())
    }

    /// An OpenVPN 3 session's process outlived its session (the app finds
    /// it: it sees the user's sessions, craked doesn't): said in the log.
    pub fn left_running(&self, tunnel: &str, profile: &str, pid: u32) {
        let known = self.tunnels.lock().unwrap().contains_key(tunnel);
        self.log.system(
            known.then_some(tunnel),
            crake_proto::LEFT_RUNNING,
            Some(crake_proto::left_running_reason(profile, pid)),
        );
    }

    /// Stop every running tunnel and wait for them to finish (daemon shutdown).
    pub async fn shutdown(&self) {
        self.traffic
            .save(&self.cfg.state_path.with_file_name("traffic.json"));
        crate::conntrack::connections_idle(true);
        // Leaving: log every session off, unless a package upgrade asked
        // for them to be handed to the new craked.
        self.stop_all();
        for _ in 0..STOP_POLLS {
            let busy = self.tunnels.lock().unwrap().values().any(|e| e.running);
            if !busy {
                break;
            }
            tokio::time::sleep(POLL).await;
        }
        #[cfg(target_os = "linux")]
        self.stop_charon().await;
        // Leave nothing behind: Crake off is Crake gone. Its own nftables
        // table, policy rules and routing tables (OpenVPN 3 sessions, which
        // outlive craked, included) and its dummy links.
        if let Err(e) = self.routing.reset_all().await {
            tracing::error!("removing Crake's routing: {e:#}");
        }
        if self.cfg.apply_network && self.cfg.use_resolved {
            for dev in [crate::hush::DEVICE, crate::keep::DEVICE] {
                self.resolved.revert(dev).await;
                self.net.remove(dev).await;
            }
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
        if let Err(e) = self.net.loose_rp_filter(&info.device) {
            tracing::error!("{e:#}");
        }
        for dns in &info.dns {
            self.net.route_dns(&info.device, *dns).await?;
        }
        for dns in &info.dns6 {
            self.net.route_dns6(&info.device, *dns).await?;
        }
        let routed = self.apply_routing(name, &display, &info).await?;
        let mut first = first;
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.info = Some(info.clone());
            first &= !std::mem::take(&mut e.resumed);
            if first || e.up_since.is_none() {
                e.up_since = Some(hosts::now());
            }
            e.ups += 1;
            self.traffic.track(&info.device, name);
        }
        // A tunnel up (again, or a new one): what found no answer before may
        // answer through it now; hosts probed meanwhile may be tried again
        // at once (seen 2026-10-18 on CI's Windows: a host probed through
        // the first tunnel was never tried through the next one for
        // PROBED_FOR; the same when a reconnect followed a probe at once).
        self.probed.lock().unwrap().clear();
        // Again, now that its pushed networks and gateway are known.
        self.sync_syn_race().await;
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

    /// Apply one call from the hook. An error makes the hook fail, which makes
    /// openconnect give up on that step.
    pub async fn handle_hook(&self, msg: HookMessage) -> Result<()> {
        let (display, state, protocol) = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(&msg.tunnel)
                .ok_or_else(|| anyhow!("unknown tunnel {}", msg.tunnel))?;
            (e.config.display().to_owned(), e.state, e.config.protocol)
        };
        // Only a tunnel Crake started may report in.
        if state == TunnelState::Off || state == TunnelState::Trouble {
            bail!("{display} is not running");
        }
        let name = msg.tunnel.as_str();

        match msg.reason.as_str() {
            "pre-init" => Ok(()),
            "connect" | "reconnect" => {
                let info = match protocol {
                    crate::config::Protocol::Openvpn => {
                        // Windows' openvpn keeps the pushed DNS to itself:
                        // what its output said fills in what the hook lacks.
                        let mut env = msg.env.clone();
                        if let Some(p) = self.pushed.lock().unwrap().get(name) {
                            p.fill(&mut env);
                        }
                        crate::openvpn::env::info(&env)?
                    }
                    crate::config::Protocol::Ipsec => {
                        // charon calls updown once per pair of traffic
                        // selectors, and again when it signs in anew: one
                        // tunnel coming up once.
                        if state == TunnelState::Connected {
                            return Ok(());
                        }
                        crate::ipsec::env::info(&msg.env, &interface_name(name))?
                    }
                    _ => TunnelInfo::from_env(&msg.env)?,
                };
                // What the engine handed over: its variables' names (values
                // may be personal), and what Crake made of them.
                tracing::debug!(
                    tunnel = name,
                    keys = ?msg.env.keys().collect::<Vec<_>>(),
                    ?info,
                    "the hook's {}",
                    msg.reason
                );
                if !crate::sys::tunnel_device(name, &info.device) {
                    bail!("device {} does not belong to {display}", info.device);
                }
                if let Some(addr) = info.address {
                    self.net
                        .assign(
                            &info.device,
                            addr,
                            info.prefix.unwrap_or(crate::hosts::V4_BITS),
                            info.mtu,
                        )
                        .await?;
                }
                if let Some((addr, prefix)) = info.address6 {
                    self.net.assign6(&info.device, addr, prefix).await?;
                }
                self.tunnel_up(name, &display, info, msg.reason == "connect")
                    .await
            }
            "attempt-reconnect" => {
                self.set_state(name, TunnelState::Connecting, false);
                if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
                    e.ups += 1;
                }
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
                        let why = crake_proto::trouble::from_openconnect(&tail)
                            .unwrap_or_else(|| format!("openconnect exited with {s}. {tail}").trim().to_owned());
                        (TunnelState::Trouble, Some(why))
                    }
                    Err(e) => (TunnelState::Trouble, Some(e.to_string())),
                },
                how = &mut stop_rx => {
                    let keep = matches!(how, Ok(Stop::KeepSession));
                    crate::sys::stop(&mut child, keep).await;
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
                                let keep = matches!(how, Ok(Stop::KeepSession));
                                crate::sys::stop(&mut child, keep).await;
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
                        e.ups += 1;
                    }
                    crate::sys::stop(&mut child, true).await;
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
                    std::mem::take(&mut e.refused) | std::mem::take(&mut e.handover)
                }
                None => false,
            }
        };
        if refused {
            // Already said why ("Turn ACME off to use Nordbank."), or handed
            // over to the new craked.
            self.set_state(name, TunnelState::Off, true);
            return;
        }
        if state == TunnelState::Trouble {
            self.set_trouble(name, reason.clone());
        } else {
            self.set_state(name, state, true);
        }
        let msg = match state {
            TunnelState::Trouble => crake_proto::stopped_unexpectedly(label),
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
        cmd.arg(format!("--protocol={}", t.protocol.as_str()));
        if crate::sys::NAMES_DEVICES {
            cmd.arg(format!("--interface={}", interface_name(&t.name)));
        }
        cmd.arg(format!("--script={}", self.cfg.hook_path.display()))
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
            .kill_on_drop(true);
        crate::sys::own_group(&mut cmd);
        let mut child = spawn(&mut cmd)
            .await
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

/// Windows: cscript.exe, which runs the hook's script, greets each time.
fn script_host_banner(line: &str) -> bool {
    line.starts_with("Microsoft (R) Windows Script Host")
        || line.starts_with("Copyright (C) Microsoft Corporation")
}

pub(super) async fn pump<R: tokio::io::AsyncRead + Unpin>(r: BufReader<R>, o: Output) {
    let mut lines = r.lines();
    // Once UDP has worked here, a later hiccup isn't a blocked network.
    let mut udp_worked = false;
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() || script_host_banner(&line) {
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

/// Start openconnect. "Text file busy" (another thread had just forked
/// while the file was being written, as tests write fake ones) passes in a
/// moment: tried again a few times.
async fn spawn(cmd: &mut Command) -> std::io::Result<tokio::process::Child> {
    const TRIES: u32 = 10;
    const PAUSE: Duration = Duration::from_millis(20);
    let mut tried = 1;
    loop {
        match cmd.spawn() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tried < TRIES => {
                tried += 1;
                tokio::time::sleep(PAUSE).await;
            }
            done => return done,
        }
    }
}
