//! One OpenVPN tunnel's openvpn: started with craked's copy of the profile,
//! answered over its management interface (`listen`), stopped with
//! `signal SIGTERM`, and started again without data channel offload where
//! the gateway can't take it. On Windows each run gets an adapter of its
//! own, `crake-<name>`, made with OpenVPN's tapctl (`sys::adapters`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::io::AsyncWrite;
use tokio::sync::mpsc;

use super::super::entry::Answers;
use super::super::*;
use super::listen::Listen;
use super::sign_in::{Do, SignIn};
use crate::openvpn::{self, AskKind, mgmt};

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
        #[cfg(windows)]
        crate::sys::adapters::remove(&self.cfg.openvpn, &interface_name(&name)).await;
        self.asked(&name);
        self.finish(&name, &label, state, reason).await;
    }

    /// Where a run's files go: craked's runtime folder beside the hook's
    /// socket (tmpfs); on Windows, whose hook is a named pipe, a private
    /// folder beside its state.
    fn run_dir(&self) -> PathBuf {
        if cfg!(windows) {
            self.cfg.state_path.with_file_name("run")
        } else {
            self.cfg.hook_socket.parent().map_or_else(
                || self.cfg.state_path.with_file_name("run"),
                Path::to_path_buf,
            )
        }
    }

    /// A run's config: the profile as checked, root's (SYSTEM's) only.
    fn run_config(&self, name: &str) -> PathBuf {
        self.run_dir().join(format!("openvpn-{name}.conf"))
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
        let profile = openvpn::profile::check(&text).map_err(anyhow::Error::msg)?;
        let conf = self.run_config(name);
        crate::sys::create_private_dir(&self.run_dir())
            .with_context(|| format!("making {}", self.run_dir().display()))?;
        {
            use std::io::Write;
            let mut f = crate::sys::create_private(&conf)
                .with_context(|| format!("writing {}", conf.display()))?;
            f.write_all(profile.text.as_bytes())?;
        }
        #[cfg(windows)]
        crate::sys::adapters::make(&self.cfg.openvpn, &interface_name(name), !no_dco).await?;
        #[cfg(unix)]
        let listen = Listen::new(&self.run_dir(), name)?;
        #[cfg(windows)]
        let listen = Listen::new()?;
        let mut child = self.spawn_openvpn(name, &conf, &listen, no_dco)?;
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
        let Some((reader, mut writer)) = listen.connect(&mut child).await else {
            let _ = child.kill().await;
            let last = heard.lock().unwrap().last.clone();
            return Ok(Run::Ended(TunnelState::Trouble, Some(exited(None, &last))));
        };
        let mut lines = BufReader::new(reader).lines();
        let mut paced = Paced::default();
        paced.send(&mut writer, "state on".into()).await;
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
                    Ok(Some(line)) => {
                        // What it said, but never a token.
                        let shown = if line.contains("Auth-Token") { "(its token)" } else { line.as_str() };
                        tracing::debug!("openvpn's management said: {shown}");
                        paced.heard(&mut writer, &line).await;
                        sign_in.said(mgmt::parse(&line))
                    }
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
                    Do::Send(cmd) => paced.send(&mut writer, cmd).await,
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

    fn spawn_openvpn(
        &self,
        name: &str,
        conf: &Path,
        listen: &Listen,
        no_dco: bool,
    ) -> Result<Child> {
        let hook = self.openvpn_hook()?;
        let mut cmd = Command::new(&self.cfg.openvpn);
        cmd.arg("--config").arg(conf);
        if cfg!(windows) {
            // Its own adapter, made for this run; Windows' address set-up
            // is craked's (IP Helper), not openvpn's.
            // ovpn-dco by default; --disable-dco takes tap-windows6.
            cmd.args(["--dev", "tun", "--dev-node", &interface_name(name)])
                .args(["--ip-win32", "manual"]);
        } else if crate::sys::NAMES_DEVICES {
            cmd.args(["--dev", &interface_name(name), "--dev-type", "tun"]);
        } else {
            // macOS: the next free utun; the hook says which. openvpn
            // takes tun or tap from a device's name, and "utun" says
            // neither ("problem with tun vs. tap setting" at the push).
            cmd.args(["--dev", "utun", "--dev-type", "tun"]);
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
        listen.args(&mut cmd);
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

    /// The hook, as openvpn runs it. Windows: crake-hook.exe (openvpn runs
    /// programs, not openconnect's JScript shim), copied into the private
    /// run folder: openvpn there takes quotes around a path as part of its
    /// name, and Program Files has a space in it.
    fn openvpn_hook(&self) -> Result<String> {
        if cfg!(windows) {
            let hook = self.cfg.hook_path.with_file_name("crake-hook.exe");
            let copy = self.run_dir().join("crake-hook.exe");
            std::fs::copy(&hook, &copy).with_context(|| format!("copying {}", hook.display()))?;
            return Ok(copy.display().to_string());
        }
        Ok(self.cfg.hook_path.display().to_string())
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

/// Commands one at a time, each once openvpn has answered the one before
/// (`SUCCESS:` or `ERROR:`, which every command gets): Windows' openvpn
/// 2.7 took the second of two commands that came in one read for the
/// first (a password went in as the username again, and sign-in hung).
#[derive(Default)]
struct Paced {
    waiting: bool,
    queue: std::collections::VecDeque<String>,
}

impl Paced {
    async fn send(&mut self, w: &mut (impl AsyncWrite + Unpin), cmd: String) {
        if self.waiting {
            self.queue.push_back(cmd);
        } else {
            send(w, &cmd).await;
            self.waiting = true;
        }
    }

    /// A line from openvpn: an answer lets the next command go.
    async fn heard(&mut self, w: &mut (impl AsyncWrite + Unpin), line: &str) {
        if !(line.starts_with("SUCCESS:") || line.starts_with("ERROR:")) {
            return;
        }
        self.waiting = false;
        if let Some(next) = self.queue.pop_front() {
            send(w, &next).await;
            self.waiting = true;
        }
    }
}

/// One command; never logged with its words (they may be a password).
async fn send(w: &mut (impl AsyncWrite + Unpin), cmd: &str) {
    /// The interface reads a line to its end: CR LF on Windows.
    const END: &str = if cfg!(windows) { "\r\n" } else { "\n" };
    let verb = cmd.split_whitespace().next().unwrap_or("");
    tracing::debug!("openvpn's management: {verb} \u{2026}");
    if let Err(e) = w.write_all(format!("{cmd}{END}").as_bytes()).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_command_waits_for_the_answer_to_the_one_before() {
        let mut out = Vec::new();
        let mut paced = Paced::default();
        paced
            .send(&mut out, "username \"Auth\" \"alice\"".into())
            .await;
        paced
            .send(&mut out, "password \"Auth\" \"hunter2\"".into())
            .await;
        let sent = |out: &Vec<u8>| String::from_utf8_lossy(out).lines().count();
        assert_eq!(sent(&out), 1, "the password waits");
        paced
            .heard(&mut out, ">PASSWORD:Need 'Auth' username/password")
            .await;
        assert_eq!(sent(&out), 1, "a real-time message isn't an answer");
        paced
            .heard(
                &mut out,
                "SUCCESS: 'Auth' username entered, but not yet verified",
            )
            .await;
        assert_eq!(sent(&out), 2);
        assert!(String::from_utf8_lossy(&out).contains("password \"Auth\""));
        paced.heard(&mut out, "ERROR: bad").await;
        paced.send(&mut out, "state on".into()).await;
        assert_eq!(sent(&out), 3, "nothing waiting: it goes at once");
    }
}
