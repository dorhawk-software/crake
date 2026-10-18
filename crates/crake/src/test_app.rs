//! The app's side in tests against a real craked: its model, fed by the
//! app's own client, and the sign-in a person would do. Shared by the
//! Linux tests (wire, e2e, journey) and Windows' (`windows_tests.rs`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use crate::client::{Command, Update};
use crate::model::Model;
use crate::signin::{self, Ask, Event, Reply, Spec};

/// The app's side: its model, fed by the client's updates.
pub(crate) struct App {
    pub model: Model,
    pub updates: async_channel::Receiver<Update>,
    commands: async_channel::Sender<Command>,
}

impl App {
    pub fn new(
        commands: async_channel::Sender<Command>,
        updates: async_channel::Receiver<Update>,
    ) -> Self {
        Self {
            model: Model::default(),
            updates,
            commands,
        }
    }

    pub fn send(&self, c: Command) {
        self.commands.send_blocking(c).unwrap();
    }

    /// Apply updates until `done` holds for the model (5 s at most).
    pub async fn until(&mut self, what: &str, done: impl Fn(&Model) -> bool) {
        self.within(Duration::from_secs(5), what, done).await;
    }

    pub async fn within(&mut self, wait: Duration, what: &str, done: impl Fn(&Model) -> bool) {
        let end = Instant::now() + wait;
        while !done(&self.model) {
            let left = end.saturating_duration_since(Instant::now());
            match tokio::time::timeout(left, self.updates.recv()).await {
                Ok(Ok(Update::Failed(text))) => {
                    panic!("craked refused while waiting for {what}: {text}")
                }
                Ok(Ok(u)) => {
                    self.model.apply(u);
                }
                _ => panic!("never saw {what}"),
            }
        }
    }

    /// The next question an OpenVPN tunnel's sign-in asks (5 s at most),
    /// applying what comes before it.
    #[cfg(target_os = "linux")]
    pub async fn asked(&mut self, what: &str) -> std::collections::HashMap<String, String> {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let left = end.saturating_duration_since(Instant::now());
            match tokio::time::timeout(left, self.updates.recv()).await {
                Ok(Ok(Update::Asking(_, ask))) => return ask,
                Ok(Ok(Update::Failed(text))) => {
                    panic!("craked refused while waiting for {what}: {text}")
                }
                Ok(Ok(u)) => {
                    self.model.apply(u);
                }
                _ => panic!("never asked {what}"),
            }
        }
    }

    /// Apply whatever arrives for `wait`.
    pub async fn settle(&mut self, wait: Duration) {
        let end = Instant::now() + wait;
        while let Ok(Ok(u)) = tokio::time::timeout(
            end.saturating_duration_since(Instant::now()),
            self.updates.recv(),
        )
        .await
        {
            self.model.apply(u);
        }
    }

    pub fn state(&self, tunnel: &str) -> Option<String> {
        self.model
            .tunnels
            .iter()
            .find(|t| t.name == tunnel)
            .map(|t| t.state.clone())
    }

    #[cfg(target_os = "linux")]
    pub fn logged(&self, message: &str) -> bool {
        self.model.log.iter().any(|e| e.message == message)
    }
}

/// The test gateway: scripts/e2e's container, or wherever CRAKE_E2E_GATEWAY
/// says (the NixOS VM test's gateway VM).
pub(crate) fn gateway() -> String {
    std::env::var("CRAKE_E2E_GATEWAY").unwrap_or_else(|_| "https://198.51.100.10".into())
}

pub(crate) fn enabled() -> bool {
    if std::env::var("CRAKE_E2E").as_deref() == Ok("1") {
        return true;
    }
    eprintln!("skipped: run scripts/e2e-test.sh (needs Docker)");
    false
}

/// The sign-in window's part: trust the gateway's certificate, give the
/// password. Returns the session openconnect got.
pub(crate) async fn sign_in() -> crake_proto::Session {
    sign_in_to(&gateway()).await
}

/// The same at another gateway (the Windows test's second, on 8443).
pub(crate) async fn sign_in_to(gateway: &str) -> crake_proto::Session {
    let spec = Spec {
        tunnel: "acme".into(),
        display: "ACME".into(),
        gateway: gateway.to_owned(),
        protocol: "anyconnect".into(),
        user: "alice".into(),
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
        let event = tokio::time::timeout(Duration::from_secs(20), events.recv())
            .await
            .expect("sign-in went quiet")
            .expect("sign-in ended");
        let answer = match event {
            Event::Reached | Event::Attached(..) => continue,
            Event::Done(session) => {
                assert_eq!(asked, ["trust", "password"], "what the window asked");
                return session;
            }
            Event::Failed(why) => panic!("sign-in failed: {why}"),
            Event::Prompt(p) => match p.ask {
                Ask::TrustServer { fingerprint } => {
                    assert!(fingerprint.is_some(), "the window shows the fingerprint");
                    asked.push("trust");
                    "yes"
                }
                Ask::Secret => {
                    asked.push("password");
                    "hunter2"
                }
                other => panic!("unexpected question {other:?} ({})", p.label),
            },
        };
        replies
            .send(Reply::Answer {
                text: answer.into(),
                remember: false,
                keep_certificate: false,
            })
            .await
            .unwrap();
    }
}

/// An HTTP GET on a fresh connection, as an app would.
pub(crate) fn get(ip: &str, within: Duration) -> std::io::Result<String> {
    let addr: SocketAddr = format!("{ip}:80").parse().unwrap();
    let mut s = TcpStream::connect_timeout(&addr, within)?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    s.write_all(b"GET / HTTP/1.0\r\n\r\n")?;
    let mut body = String::new();
    s.read_to_string(&mut body)?;
    Ok(body)
}

pub(crate) fn connected(m: &Model) -> bool {
    m.tunnels
        .iter()
        .any(|t| t.name == "acme" && t.state == "connected")
}

pub(crate) fn off(m: &Model) -> bool {
    m.tunnels
        .iter()
        .any(|t| t.name == "acme" && t.state == "off")
}
