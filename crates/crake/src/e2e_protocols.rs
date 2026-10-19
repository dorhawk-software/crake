//! The protocols with no open-source gateway (Fortinet, GlobalProtect, F5,
//! Juniper/Pulse) against openconnect's own fake servers, in the e2e
//! gateway's container (scripts/e2e/start.sh, one port each): Crake's
//! sign-in, as the window drives it, with the system's openconnect, asks
//! what each gateway asks (the password, a code, a group) and comes back
//! with a session; switched on with it, craked's tunnel ends in trouble
//! when the gateway refuses the session at the tunnel stage, as the fake
//! servers do (they carry no traffic). Skipped unless CRAKE_E2E=1; run it
//! with `scripts/e2e-test.sh`. On CI's Mac and Windows the sign-ins run
//! too, against the same servers through Cloudflare
//! (scripts/ci-through-cloudflare.sh sets CRAKE_FAKE_GATEWAY when it
//! reaches them; the tunnel stage needs the e2e container's craked: Linux).

use std::time::Duration;

use crate::signin::{self, Ask, Event, Reply, Spec};

/// The fake servers' address: the e2e gateway's on the wire, or where
/// CRAKE_FAKE_GATEWAY says (CI's Mac and Windows: 127.0.0.1, through
/// Cloudflare). None: not reachable from here, nothing to test.
fn gateway() -> Option<String> {
    match std::env::var("CRAKE_FAKE_GATEWAY") {
        Ok(g) if !g.is_empty() => Some(g),
        _ if cfg!(target_os = "linux") => Some("198.51.100.10".to_owned()),
        _ => None,
    }
}

/// The tests run: CRAKE_E2E=1 and the fake servers reachable.
fn enabled() -> bool {
    crate::test_app::enabled() && gateway().is_some()
}
/// How long a sign-in, and a tunnel, get.
const WAIT: Duration = Duration::from_secs(30);
/// More questions than any of these sign-ins asks: the same one again
/// and again means the answers aren't taken.
const MOST_ASKED: usize = 8;

/// One fake gateway: its protocol (openconnect's name), port, and the
/// server's own name (its log, and the tunnel's).
struct Fake {
    protocol: &'static str,
    port: u16,
    server: &'static str,
}

const FORTINET: Fake = Fake {
    protocol: "fortinet",
    port: 4431,
    server: "fortinet",
};
const GLOBALPROTECT: Fake = Fake {
    protocol: "gp",
    port: 4432,
    server: "gp",
};
const F5: Fake = Fake {
    protocol: "f5",
    port: 4433,
    server: "f5",
};
const JUNIPER: Fake = Fake {
    protocol: "nc",
    port: 4434,
    server: "juniper",
};

impl Fake {
    fn url(&self, path: &str) -> String {
        let gateway = gateway().unwrap_or_default();
        format!("https://{gateway}:{}{path}", self.port)
    }

    /// The fake server's own switches (two-factor, groups, ...).
    fn configure(&self, form: &str) {
        let nowhere = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let out = std::process::Command::new("curl")
            .args(["-sk", &self.url("/CONFIGURE"), "-d", form, "-o", nowhere])
            .args(["-w", "%{http_code}"])
            .output()
            .expect("curl");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        let code = said.trim();
        assert!(
            code.len() >= 3 && code[code.len() - 3..].starts_with('2'),
            "{}'s CONFIGURE: {said}",
            self.protocol
        );
    }
}

/// What the window was asked, as it shows it.
#[derive(Debug, PartialEq, Eq)]
enum Asked {
    Trust,
    Password,
    /// A second factor or another secret that isn't the password.
    Code(String),
    Text(String),
    Group(Vec<String>),
}

/// Crake's sign-in to `url` as user "test", answering as a person would:
/// trust the certificate, the password ("test", what the fake servers
/// take), a code, the group `group` when there's a choice. Returns what
/// was asked and the session.
async fn sign_in(
    fake: &Fake,
    url: &str,
    group: Option<&str>,
) -> (Vec<Asked>, Result<crake_proto::Session, String>) {
    let spec = Spec {
        tunnel: fake.protocol.into(),
        display: fake.protocol.to_uppercase(),
        gateway: url.to_owned(),
        protocol: fake.protocol.into(),
        user: "test".into(),
        auth_args: vec![],
        extra_args: vec![],
        user_agent: None,
        keyring: false,
        openconnect: None,
    };
    let (tx, events) = async_channel::unbounded();
    let replies = signin::start(spec, tx);
    let mut asked = Vec::new();
    loop {
        let event = match tokio::time::timeout(WAIT, events.recv()).await {
            Ok(Ok(e)) => e,
            _ => return (asked, Err("sign-in went quiet".into())),
        };
        let answer = match event {
            Event::Reached | Event::Attached(..) | Event::Browser(_) => continue,
            Event::Done(session) => return (asked, Ok(session)),
            Event::Failed(why) => return (asked, Err(why)),
            Event::Prompt(p) if asked.len() >= MOST_ASKED => {
                return (asked, Err(format!("asked again and again; last {p:?}")));
            }
            Event::Prompt(p) => match p.ask {
                Ask::TrustServer { .. } => {
                    asked.push(Asked::Trust);
                    "yes".to_owned()
                }
                Ask::Secret if p.label.to_lowercase().contains("password") => {
                    asked.push(Asked::Password);
                    "test".to_owned()
                }
                Ask::Secret => {
                    asked.push(Asked::Code(p.label));
                    "123456".to_owned()
                }
                Ask::Text => {
                    asked.push(Asked::Text(p.label.clone()));
                    if p.label.to_lowercase().contains("user") {
                        "test".to_owned()
                    } else {
                        "123456".to_owned()
                    }
                }
                Ask::Choice(options) => {
                    let pick = group
                        .and_then(|g| options.iter().find(|o| o.as_str() == g))
                        .or(options.first())
                        .cloned()
                        .unwrap_or_default();
                    asked.push(Asked::Group(options));
                    pick
                }
                Ask::CertificatePassword { .. } => {
                    return (asked, Err("asked a certificate's password".into()));
                }
            },
        };
        replies
            .send(Reply::Answer {
                text: answer,
                remember: false,
                keep_certificate: false,
            })
            .await
            .unwrap();
    }
}

/// A sign-in that must work, asking at least the password.
async fn signs_in(
    fake: &Fake,
    url: &str,
    group: Option<&str>,
) -> (Vec<Asked>, crake_proto::Session) {
    let (asked, session) = sign_in(fake, url, group).await;
    let session = session.unwrap_or_else(|why| {
        let log = std::fs::read_to_string(format!("/mnt/link/fake-{}.log", fake.server))
            .unwrap_or_default();
        panic!(
            "{} at {url}: {why}\nasked: {asked:?}\nthe fake server said:\n{log}",
            fake.protocol
        )
    });
    assert!(
        asked.contains(&Asked::Password),
        "{}: the password asked: {asked:?}",
        fake.protocol
    );
    (asked, session)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_protocols_sign_in() {
    if !enabled() {
        return;
    }
    // Fortinet: a password, then a code (tokeninfo two-factor).
    let (_, session) = signs_in(&FORTINET, &FORTINET.url(""), None).await;
    assert!(!session.cookie.is_empty(), "Fortinet's SVPNCOOKIE");
    FORTINET.configure("want_2fa=1&type_2fa=tokeninfo");
    let (asked, _) = signs_in(&FORTINET, &FORTINET.url(""), None).await;
    FORTINET.configure("");
    assert!(
        asked
            .iter()
            .any(|a| matches!(a, Asked::Code(_) | Asked::Text(_))),
        "Fortinet's code asked: {asked:?}"
    );

    // GlobalProtect: the portal, then its gateway.
    signs_in(&GLOBALPROTECT, &GLOBALPROTECT.url(""), None).await;

    // F5: a password; with domains, the group asked.
    signs_in(&F5, &F5.url(""), None).await;
    F5.configure("domains=xyz,abc,def");
    let (asked, _) = signs_in(&F5, &F5.url(""), Some("abc")).await;
    F5.configure("");
    assert!(
        asked
            .iter()
            .any(|a| matches!(a, Asked::Group(g) if g.iter().any(|o| o == "abc"))),
        "F5's domain asked: {asked:?}"
    );

    // Juniper/Pulse: a password; realms (its form's choice); a token form
    // after the password. Its switches are /CONFIGURE's, not the address.
    signs_in(&JUNIPER, &JUNIPER.url(""), None).await;
    JUNIPER.configure("realms=xyz,abc,def");
    let (asked, _) = signs_in(&JUNIPER, &JUNIPER.url(""), Some("abc")).await;
    assert!(
        asked
            .iter()
            .any(|a| matches!(a, Asked::Group(g) if g.iter().any(|o| o == "abc"))),
        "Juniper's realm asked: {asked:?}"
    );
    JUNIPER.configure("token_form=frmTotpToken");
    let (asked, _) = signs_in(&JUNIPER, &JUNIPER.url(""), None).await;
    JUNIPER.configure("");
    assert!(
        asked
            .iter()
            .any(|a| matches!(a, Asked::Code(_) | Asked::Text(_))),
        "Juniper's token asked: {asked:?}"
    );
}

/// Switched on with a session the gateway then refuses at the tunnel
/// stage (as the fake servers do): craked's tunnel ends in trouble, saying
/// why, instead of hanging in "connecting". Linux: the e2e container's
/// craked.
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_session_refused_at_the_tunnel_stage_says_so() {
    use craked::config::{Config, Protocol, TunnelConfig};
    use craked::manager::Manager;

    use crate::client::Command;
    use crate::e2e_tests::config;
    use crate::wire_tests;
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let protocols = [
        (&FORTINET, Protocol::Fortinet),
        (&GLOBALPROTECT, Protocol::Gp),
        (&F5, Protocol::F5),
        (&JUNIPER, Protocol::Nc),
    ];
    let cfg = Config {
        tunnels: protocols
            .iter()
            .map(|(fake, protocol)| TunnelConfig {
                user: Some("test".into()),
                ..TunnelConfig::new(fake.server, &fake.url(""), *protocol)
            })
            .collect(),
        ..config(dir.path())
    };
    let hook_socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg).unwrap();
    mgr.init().await.unwrap();
    craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
    let mut app = wire_tests::serve(mgr.clone(), dir.path());
    for (fake, _) in protocols {
        let name = fake.server;
        let (_, session) = signs_in(fake, &fake.url(""), None).await;
        app.send(Command::ConnectWithSession(name.into(), session));
        app.within(WAIT, &format!("{name} in trouble"), |m| {
            m.tunnels
                .iter()
                .any(|t| t.name == name && t.state == "trouble" && !t.why.is_empty())
        })
        .await;
    }
    mgr.shutdown().await;
}
