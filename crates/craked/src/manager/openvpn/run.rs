//! One OpenVPN tunnel's openvpn: started with craked's copy of the profile,
//! answered over its management interface (a Unix socket beside the
//! hook's), stopped with `signal SIGTERM`, and started again without data
//! channel offload where the gateway can't take it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::io::AsyncWrite;
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use super::super::entry::Answers;
use super::super::*;
use super::sign_in::{Do, SignIn};
use crate::openvpn::{self, AskKind, mgmt};

/// How long openvpn gets to open its management socket.
const MANAGEMENT_WAIT: Duration = Duration::from_secs(10);
const MANAGEMENT_POLL: Duration = Duration::from_millis(50);
/// How long it gets to stop once told to, before it's killed.
const STOP_WAIT: Duration = Duration::from_secs(10);
/// What openvpn 2.6 and later say when the gateway can't take data channel
/// offload (seen with an Azure VPN gateway, 2026-10-18).
const NO_DCO: &str = "Use --disable-dco";

/// Each question gets its own id, so an answer can't go to another.
static NEXT_ASK: AtomicU32 = AtomicU32::new(1);

/// How one run of openvpn ended.
enum Run {
    Ended(TunnelState, Option<String>),
    /// The gateway can't take data channel offload: start again without.
    WithoutDco,
}

/// What openvpn's own output said last, as it ran.
#[derive(Default)]
struct Heard {
    last: String,
}

impl Manager {
    pub(in super::super) async fn supervise_openvpn(
        self: Arc<Self>,
        config: TunnelConfig,
        mut stop_rx: oneshot::Receiver<Stop>,
    ) {
        let name = config.name.clone();
        let label = config.display().to_owned();
        let mut no_dco = self
            .hosts
            .lock()
            .unwrap()
            .tunnels
            .get(&name)
            .is_some_and(|t| t.no_dco);
        let (state, reason) = loop {
            match self.run_openvpn(&config, no_dco, &mut stop_rx).await {
                Ok(Run::Ended(state, reason)) => break (state, reason),
                Ok(Run::WithoutDco) => {
                    {
                        let mut store = self.hosts.lock().unwrap();
                        store.tunnel(&name).no_dco = true;
                        store.save();
                    }
                    self.log.system(
                        Some(&name),
                        format!("{label} starts without data channel offload."),
                        Some(
                            "Its gateway doesn\u{2019}t take it, so openvpn couldn\u{2019}t \
                             connect with it. Crake started it again without, and does \
                             from now on."
                                .into(),
                        ),
                    );
                    no_dco = true;
                }
                Err(e) => {
                    tracing::error!("{label}: {e:#}");
                    break (TunnelState::Trouble, Some(format!("{e:#}")));
                }
            }
        };
        let _ = std::fs::remove_file(self.run_config(&name));
        self.asked(&name);
        self.finish(&name, &label, state, reason).await;
    }

    /// Where a run's config goes: beside the hook's socket (tmpfs), root's only.
    fn run_config(&self, name: &str) -> PathBuf {
        self.cfg
            .hook_socket
            .with_file_name(format!("openvpn-{name}.conf"))
    }

    async fn run_openvpn(
        &self,
        config: &TunnelConfig,
        no_dco: bool,
        stop_rx: &mut oneshot::Receiver<Stop>,
    ) -> Result<Run> {
        let name = &config.name;
        let stored = openvpn::profile_path(&self.cfg.state_path, name);
        let text = tokio::fs::read_to_string(&stored)
            .await
            .with_context(|| format!("reading {}", stored.display()))?;
        // Read again each time: what may run is craked's to decide now.
        let profile = openvpn::profile::check(&text)?;
        let conf = self.run_config(name);
        {
            use std::io::Write;
            let mut f = crate::sys::create_private(&conf)
                .with_context(|| format!("writing {}", conf.display()))?;
            f.write_all(profile.text.as_bytes())?;
        }
        let socket = openvpn::management_path(&self.cfg.hook_socket, name);
        let _ = std::fs::remove_file(&socket);
        let mut child = self.spawn_openvpn(name, &conf, &socket, no_dco)?;
        let heard = Arc::new(Mutex::new(Heard::default()));
        let (dco_tx, mut dco_rx) = mpsc::unbounded_channel();
        if let Some(out) = child.stdout.take() {
            tokio::spawn(pump(
                name.clone(),
                out,
                Arc::clone(&heard),
                dco_tx.clone(),
                Arc::clone(&self.transport),
            ));
        }
        if let Some(err) = child.stderr.take() {
            tokio::spawn(pump(
                name.clone(),
                err,
                Arc::clone(&heard),
                dco_tx,
                Arc::clone(&self.transport),
            ));
        }
        let Some(stream) = management(&socket, &mut child).await else {
            let _ = child.kill().await;
            let last = heard.lock().unwrap().last.clone();
            return Ok(Run::Ended(TunnelState::Trouble, Some(exited(None, &last))));
        };
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        send(&mut writer, "state on").await;
        let token = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .and_then(|e| e.token.clone());
        let mut sign_in = SignIn::new(token, profile.credentials);
        let (answers, mut answer_rx) = mpsc::unbounded_channel::<(u32, String)>();
        let mut asked_id = None;
        let mut fatal = None;
        loop {
            let todo = tokio::select! {
                status = child.wait() => {
                    let last = heard.lock().unwrap().last.clone();
                    return Ok(match status {
                        Ok(s) if s.success() && fatal.is_none() => Run::Ended(TunnelState::Off, None),
                        _ => Run::Ended(TunnelState::Trouble, Some(exited(fatal, &last))),
                    });
                }
                how = &mut *stop_rx => {
                    if !matches!(how, Ok(Stop::KeepSession))
                        && let Some(e) = self.tunnels.lock().unwrap().get_mut(name)
                    {
                        e.token = None;
                    }
                    stop(&mut child, &mut writer).await;
                    return Ok(Run::Ended(TunnelState::Off, None));
                }
                Some(()) = dco_rx.recv(), if !no_dco => {
                    stop(&mut child, &mut writer).await;
                    return Ok(Run::WithoutDco);
                }
                line = lines.next_line() => match line {
                    Ok(Some(line)) => sign_in.said(mgmt::parse(&line)),
                    // openvpn is going: its exit says why.
                    _ => {
                        let _ = child.wait().await;
                        continue;
                    }
                },
                Some((id, text)) = answer_rx.recv() => {
                    if asked_id != Some(id) {
                        continue;
                    }
                    asked_id = None;
                    self.asked(name);
                    sign_in.answered(text)
                }
            };
            for d in todo {
                match d {
                    Do::Send(cmd) => send(&mut writer, &cmd).await,
                    Do::Ask(kind, label, url, retry) => {
                        asked_id = Some(self.ask(name, (kind, label, url, retry), &answers));
                    }
                    Do::Keep(token) => self.keep_token(name, Some(token)),
                    Do::Forget => self.keep_token(name, None),
                    Do::Done => {
                        asked_id = None;
                        self.asked(name);
                    }
                    Do::Fatal(why) => fatal = Some(why),
                }
            }
        }
    }

    fn spawn_openvpn(&self, name: &str, conf: &Path, socket: &Path, no_dco: bool) -> Result<Child> {
        let hook = self.cfg.hook_path.display().to_string();
        let mut cmd = Command::new(&self.cfg.openvpn);
        cmd.arg("--config").arg(conf);
        if crate::sys::NAMES_DEVICES {
            cmd.args(["--dev", &interface_name(name), "--dev-type", "tun"]);
        } else {
            // macOS: the next free utun; the hook says which.
            cmd.args(["--dev", "utun"]);
        }
        cmd.args(["--route-noexec", "--ifconfig-noexec"])
            .args([
                "--script-security",
                "2",
                "--up",
                &hook,
                "--down",
                &hook,
                "--up-restart",
            ])
            .arg("--management")
            .arg(socket)
            .arg("unix")
            .args(["--management-hold", "--management-query-passwords"])
            .args(["--auth-retry", "interact"])
            // It can open a sign-in page, and answer a code (CR_TEXT).
            .args(["--setenv", "IV_SSO", "webauth,crtext"])
            // Windows' DNS lockdown, and 2.7's own DNS set-up: Crake's.
            .args(["--pull-filter", "ignore", "block-outside-dns"])
            .args([
                "--ignore-unknown-option",
                "dns-updown",
                "--dns-updown",
                "disable",
            ])
            .args(["--setenv", ENV_TUNNEL, name])
            .arg("--setenv")
            .arg(ENV_SOCKET)
            .arg(&self.cfg.hook_socket)
            .args(["--verb", "3"]);
        if no_dco {
            cmd.arg("--disable-dco");
        }
        cmd.env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        crate::sys::own_group(&mut cmd);
        cmd.spawn()
            .with_context(|| format!("starting {}", self.cfg.openvpn.display()))
    }

    /// A question for the user, shown by the app's sign-in window.
    fn ask(
        &self,
        name: &str,
        ask: (AskKind, String, Option<String>, bool),
        answers: &Answers,
    ) -> u32 {
        let id = NEXT_ASK.fetch_add(1, Ordering::Relaxed);
        let (kind, label, url, retry) = ask;
        let ask = openvpn::Ask {
            id,
            kind,
            label,
            url,
            retry,
        };
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.asking = Some((ask, answers.clone()));
        }
        let _ = self.changes.send(name.to_owned());
        id
    }

    /// Nothing more to ask.
    fn asked(&self, name: &str) {
        let had = self
            .tunnels
            .lock()
            .unwrap()
            .get_mut(name)
            .and_then(|e| e.asking.take())
            .is_some();
        if had {
            let _ = self.changes.send(name.to_owned());
        }
    }

    fn keep_token(&self, name: &str, token: Option<openvpn::Token>) {
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.token = token;
        }
    }
}

/// openvpn's management socket, once it's there; None if openvpn ended
/// first or took too long.
async fn management(socket: &Path, child: &mut Child) -> Option<UnixStream> {
    let deadline = Instant::now() + MANAGEMENT_WAIT;
    while Instant::now() < deadline {
        if let Ok(stream) = UnixStream::connect(socket).await {
            return Some(stream);
        }
        if child.try_wait().ok().flatten().is_some() {
            return None;
        }
        tokio::time::sleep(MANAGEMENT_POLL).await;
    }
    None
}

/// One command; never logged with its words (they may be a password).
async fn send(w: &mut (impl AsyncWrite + Unpin), cmd: &str) {
    let verb = cmd.split_whitespace().next().unwrap_or("");
    tracing::debug!(target: "openvpn", "management: {verb} …");
    if let Err(e) = w.write_all(format!("{cmd}\n").as_bytes()).await {
        tracing::debug!(target: "openvpn", "management: {e}");
    }
}

/// Tell openvpn to stop (it calls the hook's `down` on its way); killed
/// if it doesn't within STOP_WAIT.
async fn stop(child: &mut Child, w: &mut (impl AsyncWrite + Unpin)) {
    send(w, "signal SIGTERM").await;
    if tokio::time::timeout(STOP_WAIT, child.wait()).await.is_err() {
        let _ = child.kill().await;
    }
}

/// Why it stopped, for people: what it said last.
fn exited(fatal: Option<String>, last: &str) -> String {
    /// Its log lines start with the date and the time.
    const STAMP_WORDS: usize = 2;
    let said = fatal.unwrap_or_else(|| last.to_owned());
    let stamped = said.chars().next().is_some_and(|c| c.is_ascii_digit());
    let said = said
        .splitn(STAMP_WORDS + 1, ' ')
        .nth(STAMP_WORDS)
        .filter(|_| stamped)
        .unwrap_or(&said);
    format!("openvpn stopped. {said}").trim().to_owned()
}

/// openvpn's output goes to the journal; craked notices how it travels and
/// a gateway that can't take data channel offload.
async fn pump<R: tokio::io::AsyncRead + Unpin>(
    tunnel: String,
    r: R,
    heard: Arc<Mutex<Heard>>,
    no_dco: mpsc::UnboundedSender<()>,
    transport: Arc<Mutex<HashMap<String, String>>>,
) {
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        tracing::info!(target: "openvpn", tunnel = %tunnel, "{line}");
        if line.contains(NO_DCO) {
            let _ = no_dco.send(());
        }
        let how = if line.contains("UDP link remote")
            || line.contains("UDPv4 link remote")
            || line.contains("UDPv6 link remote")
        {
            Some("UDP")
        } else if line.contains("TCP_CLIENT link remote")
            || line.contains("TCPv4_CLIENT link remote")
            || line.contains("TCPv6_CLIENT link remote")
        {
            Some("TCP")
        } else {
            None
        };
        if let Some(how) = how {
            transport
                .lock()
                .unwrap()
                .insert(tunnel.clone(), how.to_owned());
        }
        heard.lock().unwrap().last = line;
    }
}
