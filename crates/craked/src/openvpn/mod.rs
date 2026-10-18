//! OpenVPN with the community `openvpn` program (CLAUDE.md, 7i): craked runs
//! it per tunnel, sets nothing up through it (no address, route or DNS: the
//! hook hands craked what was pushed) and answers its management interface,
//! asking the app when the user has to answer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod env;
pub mod mgmt;
/// What may run (shared with the app, which checks a profile as it is chosen).
pub use crake_proto::openvpn_profile as profile;

/// The sign-in the gateway gave (`>PASSWORD:Auth-Token:`), sent as the
/// password the next time, with the user name it was given for: switching
/// on again, starting again without DCO, and the next craked after an
/// upgrade sign in with it, as with openconnect's cookie.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub user: String,
    pub token: String,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Token {{ user: {:?}, .. }}", self.user)
    }
}

/// What the user is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskKind {
    User,
    Password,
    /// The password of the profile's private key.
    KeyPassword,
    /// A one-time code or a challenge's answer; `echo`: it may be shown.
    Code {
        echo: bool,
    },
    /// A page to finish signing in on, in the browser (nothing to answer).
    Browser,
}

impl AskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Password => "password",
            Self::KeyPassword => "key-password",
            Self::Code { echo: true } => "code-visible",
            Self::Code { echo: false } => "code",
            Self::Browser => "browser",
        }
    }
}

/// One question for the app's sign-in window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// New for every question, so an answer can't go to the wrong one.
    pub id: u32,
    pub kind: AskKind,
    /// The gateway's own words, when it has any ("Enter your PIN").
    pub label: String,
    /// The page, for `Browser`.
    pub url: Option<String>,
    /// The last answer was refused.
    pub retry: bool,
}

impl Ask {
    /// The D-Bus API's text form (`Asking`).
    pub fn to_map(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("id".into(), self.id.to_string());
        m.insert("kind".into(), self.kind.as_str().into());
        m.insert("label".into(), self.label.clone());
        m.insert("url".into(), self.url.clone().unwrap_or_default());
        m.insert("retry".into(), self.retry.to_string());
        m
    }
}

/// Where craked keeps a tunnel's profile: root's only, next to the host store.
pub fn profile_path(state_path: &Path, tunnel: &str) -> PathBuf {
    state_path
        .with_file_name("profiles")
        .join(format!("{tunnel}.ovpn"))
}

/// The tunnel's management socket, beside the hook's.
pub fn management_path(hook_socket: &Path, tunnel: &str) -> PathBuf {
    hook_socket.with_file_name(format!("openvpn-{tunnel}.sock"))
}
