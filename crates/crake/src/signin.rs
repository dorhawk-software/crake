//! Signing in, in the user's session: `openconnect --authenticate` runs as
//! the user (who can read their certificate), its questions go to Crake's
//! sign-in window, and only the resulting session goes to craked.
//!
//! openconnect asks on stderr without a newline and reads the answer from
//! stdin; a line ending in ':' after a short pause is a question.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crake_proto::Session;

use crate::keyring::{self, Kind};

/// What sign-in needs to know about a tunnel.
#[derive(Clone, Debug)]
pub struct Spec {
    pub tunnel: String,
    pub display: String,
    pub gateway: String,
    pub protocol: String,
    pub user: String,
    pub auth_args: Vec<String>,
    pub extra_args: Vec<String>,
    /// How to introduce itself, when this gateway is known to need it.
    pub user_agent: Option<String>,
    /// Read and write the keyring (off in demo mode: nothing real is stored).
    pub keyring: bool,
    /// The openconnect to run (tests: a fake). None: CRAKE_OPENCONNECT
    /// (development), or openconnect on PATH.
    pub openconnect: Option<PathBuf>,
}

impl Spec {
    /// Who does the signing in, for "… said:".
    pub fn program(&self) -> &'static str {
        if self.protocol == "openvpn3" {
            "OpenVPN 3"
        } else {
            "openconnect"
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// The certificate's password (asked by openconnect, not the gateway).
    CertificatePassword {
        /// The certificate came from a file that could be kept in the keyring.
        from_file: Option<PathBuf>,
        /// The password from the keyring didn't work.
        retry: bool,
    },
    Secret,
    Text,
    Choice(Vec<String>),
    /// openconnect can't verify the gateway's certificate.
    TrustServer {
        fingerprint: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub ask: Ask,
    /// The field's label, e.g. "Username".
    pub label: String,
    /// The gateway's own words before the question, if any.
    pub words: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Event {
    Reached,
    Prompt(Prompt),
    Done(Session),
    Failed(String),
    /// OpenVPN 3: connected; the device and DNS for craked (`ovpn3`).
    Attached(std::collections::HashMap<String, String>),
}

#[derive(Clone, Debug)]
pub enum Reply {
    Answer {
        text: String,
        remember: bool,
        keep_certificate: bool,
    },
    Cancel,
}

pub fn start(spec: Spec, events: async_channel::Sender<Event>) -> async_channel::Sender<Reply> {
    let (replies, incoming) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("sign-in".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            let result = run(&spec, &events, &incoming, &rt);
            let _ = events.send_blocking(match result {
                Ok(session) => Event::Done(session),
                Err(e) => Event::Failed(e),
            });
        })
        .expect("a thread for sign-in");
    replies
}

/// A certificate written for one sign-in; removed when dropped.
struct TempCert(PathBuf);

impl Drop for TempCert {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn private_dir() -> Result<PathBuf, String> {
    use std::os::unix::fs::DirBuilderExt;
    let base = std::env::var_os("XDG_RUNTIME_DIR").ok_or("no XDG_RUNTIME_DIR")?;
    let dir = PathBuf::from(base).join("crake");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

fn certificate_arg(args: &[String]) -> Option<(usize, PathBuf)> {
    args.iter().enumerate().find_map(|(i, a)| {
        a.strip_prefix("--certificate=")
            .or_else(|| a.strip_prefix("-c="))
            .map(|p| (i, PathBuf::from(p)))
    })
}

fn run(
    spec: &Spec,
    events: &async_channel::Sender<Event>,
    replies: &async_channel::Receiver<Reply>,
    rt: &tokio::runtime::Runtime,
) -> Result<Session, String> {
    let mut auth_args = spec.auth_args.clone();
    // A certificate kept in the keyring wins over the file the config names.
    let mut temp = None;
    let mut from_file = certificate_arg(&auth_args).map(|(_, p)| p);
    let stored_certificate = if spec.keyring {
        rt.block_on(keyring::get(&spec.tunnel, Kind::Certificate))
    } else {
        None
    };
    if let Some(bytes) = stored_certificate {
        use std::os::unix::fs::OpenOptionsExt;
        let path = private_dir()?.join(format!("{}-{}.p12", spec.tunnel, std::process::id()));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| e.to_string())?;
        f.write_all(&bytes).map_err(|e| e.to_string())?;
        let arg = format!("--certificate={}", path.display());
        match certificate_arg(&auth_args) {
            Some((i, _)) => auth_args[i] = arg,
            None => auth_args.push(arg),
        }
        from_file = None;
        temp = Some(TempCert(path));
    }
    let stored_password = if spec.keyring {
        rt.block_on(keyring::get(&spec.tunnel, Kind::CertificatePassword))
    } else {
        None
    };
    let stored_login = if spec.keyring {
        rt.block_on(keyring::get(&spec.tunnel, Kind::Password))
    } else {
        None
    };

    // Gateways that only let Cisco's client in turn openconnect away before
    // asking anything; then it tries once more introducing itself as
    // AnyConnect (CLAUDE.md, 7d). Never when the config says how.
    let configured = spec
        .extra_args
        .iter()
        .chain(&auth_args)
        .any(|a| a.starts_with("--useragent"));
    let mut user_agent = spec.user_agent.clone().filter(|_| !configured);
    let state = &mut Attempt {
        stored_password: stored_password.as_deref(),
        from_file: from_file.as_deref(),
        tried_stored: false,
        to_save: None,
        forget_stored: false,
        stored_login: stored_login.as_deref(),
        tried_login: false,
        login_to_save: None,
    };
    let stdout = loop {
        match attempt(
            spec,
            &auth_args,
            user_agent.as_deref(),
            state,
            events,
            replies,
        )? {
            Ok(stdout) => break stdout,
            Err(lines) => {
                if !configured
                    && user_agent.is_none()
                    && spec.protocol == "anyconnect"
                    && turned_away(&lines)
                {
                    user_agent = Some(ANYCONNECT.into());
                    continue;
                }
                return Err(failure(&lines));
            }
        }
    };
    drop(temp);
    let (mut to_save, forget_stored) = (state.to_save.take(), state.forget_stored);
    let login_to_save = state.login_to_save.take();
    if !spec.keyring {
        to_save = None;
    }
    if forget_stored {
        rt.block_on(keyring::forget(&spec.tunnel, Kind::CertificatePassword));
    }
    let mut session = Session::parse(&stdout).map_err(str::to_owned)?;
    session.user_agent = user_agent;
    // The gateway password that worked goes to the keyring.
    if spec.keyring
        && let Some(login) = &login_to_save
    {
        let _ = rt.block_on(keyring::set(
            &spec.tunnel,
            &spec.display,
            Kind::Password,
            login.as_bytes(),
        ));
    }
    // Only a password that worked is kept.
    if let Some((password, keep_certificate)) = to_save {
        if !password.is_empty() {
            let _ = rt.block_on(keyring::set(
                &spec.tunnel,
                &spec.display,
                Kind::CertificatePassword,
                password.as_bytes(),
            ));
        }
        if keep_certificate
            && let Some(path) = &from_file
            && let Ok(bytes) = std::fs::read(path)
        {
            let _ = rt.block_on(keyring::set(
                &spec.tunnel,
                &spec.display,
                Kind::Certificate,
                &bytes,
            ));
        }
    }
    Ok(session)
}

/// How Crake introduces itself to gateways that only let AnyConnect in (the
/// value NetworkManager's openconnect plugin uses too).
pub const ANYCONNECT: &str = "AnyConnect";

/// What one run of `openconnect --authenticate` needs to carry over.
struct Attempt<'a> {
    stored_password: Option<&'a [u8]>,
    from_file: Option<&'a std::path::Path>,
    tried_stored: bool,
    /// The certificate password to remember (empty: none) and whether to
    /// keep the certificate, once sign-in has worked.
    to_save: Option<(String, bool)>,
    /// The keyring's password didn't open the certificate and the user chose
    /// not to remember a new one: the stale one goes.
    forget_stored: bool,
    /// The gateway password from the keyring, tried once.
    stored_login: Option<&'a [u8]>,
    tried_login: bool,
    /// The gateway password typed this time: kept once sign-in worked.
    login_to_save: Option<String>,
}

/// The gateway asking for the account's password (not a one-time code, not
/// the certificate's pass phrase).
fn asks_password(prompt: &Prompt) -> bool {
    prompt.ask == Ask::Secret && prompt.label.to_ascii_lowercase().contains("password")
}

/// One run of `openconnect --authenticate`, its questions answered through
/// the window. Ok(Ok(stdout)) signed in; Ok(Err(lines)) failed, with what
/// openconnect said; Err when cancelled or it couldn't run.
fn attempt(
    spec: &Spec,
    auth_args: &[String],
    user_agent: Option<&str>,
    state: &mut Attempt,
    events: &async_channel::Sender<Event>,
    replies: &async_channel::Receiver<Reply>,
) -> Result<Result<String, Vec<String>>, String> {
    // CRAKE_OPENCONNECT (development): another openconnect to run.
    let program = match &spec.openconnect {
        Some(p) => p.clone().into_os_string(),
        None => std::env::var_os("CRAKE_OPENCONNECT").unwrap_or_else(|| "openconnect".into()),
    };
    let mut cmd = Command::new(program);
    cmd.arg("--authenticate")
        .arg(format!("--protocol={}", spec.protocol));
    if !spec.user.is_empty() {
        cmd.arg(format!("--user={}", spec.user));
    }
    if let Some(ua) = user_agent {
        cmd.arg(format!("--useragent={ua}"));
    }
    cmd.args(auth_args)
        .args(&spec.extra_args)
        .arg("--")
        .arg(&spec.gateway)
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Couldn't start openconnect: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let mut stderr = child.stderr.take().ok_or("no stderr")?;

    let (chunks_tx, chunks) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 || chunks_tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });

    let mut lines: Vec<String> = Vec::new();
    let mut pending = String::new();
    let mut reached = false;
    loop {
        match chunks.recv_timeout(Duration::from_millis(250)) {
            Ok(bytes) => {
                pending.push_str(&String::from_utf8_lossy(&bytes));
                while let Some(nl) = pending.find('\n') {
                    let line = pending[..nl].trim_end().to_owned();
                    pending.drain(..=nl);
                    if !reached
                        && (line.starts_with("Connected to ")
                            || line.starts_with("SSL negotiation"))
                    {
                        reached = true;
                        let _ = events.send_blocking(Event::Reached);
                    }
                    if !line.is_empty() {
                        lines.push(line);
                    }
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let question = pending.trim_end();
        if !question.ends_with(':') {
            continue;
        }
        let mut prompt = classify(question, &lines);
        pending.clear();
        // "Your password goes to the system keyring": answered from it once.
        if asks_password(&prompt)
            && let (Some(pw), false) = (state.stored_login, state.tried_login)
        {
            state.tried_login = true;
            let _ = stdin.write_all(&[pw, b"\n"].concat());
            continue;
        }
        let is_login = asks_password(&prompt);
        if let Ask::CertificatePassword { .. } = prompt.ask {
            if let (Some(pw), false) = (state.stored_password, state.tried_stored) {
                state.tried_stored = true;
                let _ = stdin.write_all(&[pw, b"\n"].concat());
                continue;
            }
            prompt.ask = Ask::CertificatePassword {
                from_file: state.from_file.map(Into::into),
                retry: state.tried_stored,
            };
        }
        let is_cert = matches!(prompt.ask, Ask::CertificatePassword { .. });
        let _ = events.send_blocking(Event::Prompt(prompt));
        match replies.recv_blocking() {
            Ok(Reply::Answer {
                text,
                remember,
                keep_certificate,
            }) => {
                if is_cert && state.tried_stored && !remember {
                    state.forget_stored = true;
                }
                if is_login {
                    state.login_to_save = Some(text.clone());
                }
                if is_cert && (remember || keep_certificate) {
                    state.to_save = Some((
                        if remember {
                            text.clone()
                        } else {
                            String::new()
                        },
                        keep_certificate,
                    ));
                }
                let _ = stdin.write_all(format!("{text}\n").as_bytes());
            }
            Ok(Reply::Cancel) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cancelled.".into());
            }
        }
    }
    drop(stdin);
    let status = child.wait().map_err(|e| e.to_string())?;
    let stdout = out.join().unwrap_or_default();
    Ok(if status.success() {
        Ok(stdout)
    } else {
        Err(lines)
    })
}

/// The gateway refused openconnect's very first request (403/404 before it
/// asked for anything): the way gateways that only admit Cisco's client fail.
fn turned_away(lines: &[String]) -> bool {
    for l in lines {
        if l.starts_with("Server requested SSL client certificate")
            || l.starts_with("XML response")
            || l.starts_with("Please enter")
        {
            return false; // it got past the door
        }
        if l.starts_with("Unexpected 403 result")
            || l.starts_with("Unexpected 404 result")
            || l.starts_with("Got HTTP response: HTTP/1.1 403")
            || l.starts_with("Got HTTP response: HTTP/1.1 404")
        {
            return true;
        }
    }
    false
}

/// Lines openconnect prints about its own progress, not the gateway's words.
fn is_progress(line: &str) -> bool {
    const PREFIXES: [&str; 12] = [
        "POST ",
        "GET ",
        "Connected to ",
        "SSL negotiation",
        "Got HTTP",
        "XML POST",
        "Using client certificate",
        "Attempting to connect",
        "Got CONNECT",
        "Server certificate verify failed",
        "Please enter your",
        "Enter '",
    ];
    PREFIXES.iter().any(|p| line.starts_with(p))
}

pub fn classify(question: &str, lines: &[String]) -> Prompt {
    let q = question.trim_end_matches(':').trim();
    let lower = q.to_ascii_lowercase();
    let words = lines.iter().rev().find(|l| !is_progress(l)).cloned();
    if lower.starts_with("enter 'yes' to accept") {
        let fingerprint = lines.iter().rev().find_map(|l| {
            l.split_whitespace()
                .find(|w| w.starts_with("pin-sha256:"))
                .map(str::to_owned)
        });
        return Prompt {
            ask: Ask::TrustServer { fingerprint },
            label: q.into(),
            words: None,
        };
    }
    if lower.contains("pkcs#12") || lower.contains("pass phrase") || lower.contains("certificate") {
        return Prompt {
            ask: Ask::CertificatePassword {
                from_file: None,
                retry: false,
            },
            label: "Certificate password".into(),
            words: None,
        };
    }
    // "GROUP: [Employees|Contractors]"
    if let (Some(open), Some(close)) = (q.rfind('['), q.rfind(']'))
        && open < close
    {
        let options = q[open + 1..close]
            .split('|')
            .map(|o| o.trim().to_owned())
            .filter(|o| !o.is_empty())
            .collect();
        let label = q[..open].trim().trim_end_matches(':').trim().to_owned();
        return Prompt {
            ask: Ask::Choice(options),
            label,
            words,
        };
    }
    let secret = ["password", "passcode", "pin", "token", "code", "otp"]
        .iter()
        .any(|w| lower.contains(w));
    Prompt {
        ask: if secret { Ask::Secret } else { Ask::Text },
        label: q.into(),
        words,
    }
}

/// The reason to show when sign-in failed: openconnect's last complaint.
fn failure(lines: &[String]) -> String {
    lines
        .iter()
        .rev()
        .find(|l| {
            let l = l.to_ascii_lowercase();
            l.contains("fail")
                || l.contains("error")
                || l.contains("unexpected")
                || l.contains("denied")
                || l.contains("invalid")
        })
        .or(lines.last())
        .cloned()
        .unwrap_or_else(|| "openconnect stopped without saying why.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn recognises_openconnects_questions() {
        let p = classify(
            "Enter PKCS#12 pass phrase:",
            &lines(&[
                "POST https://vpn.acme.example/",
                "Connected to 203.0.113.10:443",
            ]),
        );
        assert!(matches!(p.ask, Ask::CertificatePassword { .. }));

        let p = classify(
            "Password:",
            &lines(&[
                "POST https://vpn/",
                "Please enter your username and password.",
            ]),
        );
        assert_eq!((p.ask, p.label.as_str()), (Ask::Secret, "Password"));

        let p = classify("Username:", &lines(&["Welcome to ACME VPN"]));
        assert_eq!(p.ask, Ask::Text);
        assert_eq!(p.words.as_deref(), Some("Welcome to ACME VPN"));

        let p = classify("GROUP: [Employees|Contractors]:", &[]);
        assert_eq!(
            p.ask,
            Ask::Choice(vec!["Employees".into(), "Contractors".into()])
        );
        assert_eq!(p.label, "GROUP");

        let p = classify(
            "Enter 'yes' to accept, 'no' to abort; anything else to view:",
            &lines(&[
                "To trust this server in future, perhaps add this to your command line:",
                "    --servercert pin-sha256:AbC=",
            ]),
        );
        assert_eq!(
            p.ask,
            Ask::TrustServer {
                fingerprint: Some("pin-sha256:AbC=".into())
            }
        );

        let p = classify(
            "Enter your 6-digit OTP:",
            &lines(&["Enter your 6-digit OTP."]),
        );
        assert_eq!(p.ask, Ask::Secret);
    }

    #[test]
    fn notices_a_gateway_that_only_admits_anyconnect() {
        // vpn.acme.example with openconnect's own user agent (v9.12, LC_ALL=C).
        let refused = lines(&[
            "POST https://vpn.acme.example/",
            "Connected to 203.0.113.10:443",
            "SSL negotiation with vpn.acme.example",
            "Connected to HTTPS on vpn.acme.example with ciphersuite (TLS1.2)-(ECDHE-X25519)-(RSA-SHA256)-(AES-256-GCM)",
            "Got HTTP response: HTTP/1.1 404 Not Found",
            "Unexpected 404 result from server",
            "GET https://vpn.acme.example/",
            "Got HTTP response: HTTP/1.1 404 Not Found",
            "Failed to complete authentication",
        ]);
        assert!(turned_away(&refused));
        // As AnyConnect it gets in and asks for the certificate; a later 404
        // (after the user gave up) is not the door.
        let admitted = lines(&[
            "POST https://vpn.acme.example/",
            "Connected to 203.0.113.10:443",
            "Server requested SSL client certificate; none was configured",
            "POST https://vpn.acme.example/",
            "XML response has no \"auth\" node",
            "Got HTTP response: HTTP/1.1 404 Not Found",
            "Unexpected 404 result from server",
            "Failed to complete authentication",
        ]);
        assert!(!turned_away(&admitted));
        assert!(!turned_away(&lines(&[
            "Failed to open HTTPS connection to vpn"
        ])));
    }

    /// A gateway like ACME's: 404 to openconnect's own user agent, a session
    /// to "AnyConnect".
    #[test]
    fn signs_in_as_anyconnect_where_only_that_is_let_in() {
        let dir = std::env::temp_dir().join(format!("crake-ua-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("openconnect");
        std::fs::write(
            &fake,
            "#!/bin/sh\n\
             case \"$*\" in *--useragent=AnyConnect*) ;; *)\n\
               echo 'POST https://gw/' >&2\n\
               echo 'Got HTTP response: HTTP/1.1 404 Not Found' >&2\n\
               echo 'Unexpected 404 result from server' >&2\n\
               echo 'Failed to complete authentication' >&2; exit 1;;\n\
             esac\n\
             echo \"COOKIE='c'\"; echo \"CONNECT_URL='https://gw/'\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let (events, _rx) = async_channel::unbounded();
        let (_tx, replies) = async_channel::unbounded();
        let mut spec = Spec {
            tunnel: "acme".into(),
            display: "ACME".into(),
            gateway: "gw".into(),
            protocol: "anyconnect".into(),
            user: String::new(),
            auth_args: vec![],
            extra_args: vec![],
            user_agent: None,
            keyring: false,
            openconnect: Some(fake),
        };
        let s = run(&spec, &events, &replies, &rt).unwrap();
        assert_eq!(s.user_agent.as_deref(), Some(ANYCONNECT));
        // Told how in the config: Crake doesn't second-guess it.
        spec.extra_args = vec!["--useragent=Something".into()];
        assert!(run(&spec, &events, &replies, &rt).is_err());
        // Another protocol: no AnyConnect retry.
        spec.extra_args = vec![];
        spec.protocol = "gp".into();
        assert!(run(&spec, &events, &replies, &rt).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finds_the_reason_it_failed() {
        let l = lines(&[
            "POST https://vpn/",
            "Failed to open HTTPS connection to vpn",
            "Failed to complete authentication",
        ]);
        assert_eq!(failure(&l), "Failed to complete authentication");
        assert_eq!(
            certificate_arg(&["--certificate=/a/b.p12".into()]),
            Some((0, PathBuf::from("/a/b.p12")))
        );
    }
}
