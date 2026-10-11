//! Types shared by `craked`, `crake-hook` and (later) the UI.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where `craked` listens for the hook, unless configured otherwise.
#[cfg(unix)]
pub const DEFAULT_HOOK_SOCKET: &str = "/run/crake/hook.sock";
/// Windows: a named pipe.
#[cfg(windows)]
pub const DEFAULT_HOOK_SOCKET: &str = r"\\.\pipe\crake-hook";
/// Where the hook binary is installed.
#[cfg(unix)]
pub const DEFAULT_HOOK_PATH: &str = "/usr/libexec/crake/hook";
/// Windows: openconnect runs its script through cscript.exe, so the hook
/// is a JScript shim that starts crake-hook.exe beside it.
#[cfg(windows)]
pub const DEFAULT_HOOK_PATH: &str = r"C:\Program Files\Crake\crake-hook.js";
/// Windows: where craked serves the app (D-Bus, loopback only; the app
/// checks it's the `craked` service listening, craked that the caller is
/// in the console session).
#[cfg(windows)]
pub const APP_ADDRESS: &str = "127.0.0.1:47366";
/// Windows: craked's service name.
#[cfg(windows)]
pub const SERVICE_NAME: &str = "craked";
/// Set by `craked` in openconnect's environment so the hook knows which tunnel it serves.
pub const ENV_TUNNEL: &str = "CRAKE_TUNNEL";
/// Overrides the hook socket path (used by the hook and by tests).
pub const ENV_SOCKET: &str = "CRAKE_HOOK_SOCKET";

/// Well-known D-Bus name, object path and interface of the daemon.
pub const BUS_NAME: &str = "software.dorhawk.Crake1";
pub const BUS_PATH: &str = "/software/dorhawk/Crake1";

/// Tunnel names become part of the interface name (`crake-<name>`, 15 bytes max).
pub const MAX_TUNNEL_NAME: usize = 9;

pub fn valid_tunnel_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_TUNNEL_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
}

pub fn interface_name(tunnel: &str) -> String {
    format!("crake-{tunnel}")
}

/// Matches the tray icon states in `design/DESIGN.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelState {
    Off,
    Connecting,
    Connected,
    Trouble,
}

impl TunnelState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Trouble => "trouble",
        }
    }
}

/// One call from openconnect's script hook, forwarded to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMessage {
    pub tunnel: String,
    /// `pre-init`, `connect`, `disconnect`, `reconnect`, `attempt-reconnect`, ...
    pub reason: String,
    /// Only the variables Crake uses; see [`collect_env`].
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookReply {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
}

const ENV_PREFIXES: &[&str] = &[
    "TUNDEV",
    "VPNGATEWAY",
    "INTERNAL_IP4_",
    "INTERNAL_IP6_",
    "CISCO_",
    "IDLE_TIMEOUT",
];

/// Keep only the variables openconnect documents for its script.
pub fn collect_env(vars: impl IntoIterator<Item = (String, String)>) -> BTreeMap<String, String> {
    vars.into_iter()
        .filter(|(k, _)| ENV_PREFIXES.iter().any(|p| k.starts_with(p)))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    Routed,
    LeftAlone,
    Undone,
    System,
    /// "Ask me first": a host needs the tunnel; the user decides.
    Asked,
}

/// One line of the append-only JSON log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// RFC 3339, UTC.
    pub ts: String,
    pub kind: LogKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tunnel: Option<String>,
    /// One plain sentence, in Crake's voice.
    pub message: String,
    /// The reason, on the second line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The addresses a decision was about, so it can be undone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    /// Other tunnels that answered for these hosts too (by name): the user
    /// can move them there ("When two tunnels answer: Ask me").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub others: Vec<String>,
}

/// A signed-in session: what `openconnect --authenticate` prints. Sign-in
/// runs in the user's session (`crake`), the connection in `craked`, which
/// gets this. The cookie is a credential: it travels over stdin or the
/// private API, never on a command line, and is never logged.
#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    pub cookie: String,
    pub connect_url: String,
    pub fingerprint: Option<String>,
    pub resolve: Option<String>,
    /// How the client introduced itself at sign-in (e.g. "AnyConnect" for
    /// gateways that only let Cisco's client in); the connection must match.
    pub user_agent: Option<String>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("connect_url", &self.connect_url)
            .field("cookie", &"<hidden>")
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Parse `KEY='value'` lines, shell-quoted as openconnect writes them.
    pub fn parse(stdout: &str) -> Result<Self, &'static str> {
        let get = |key: &str| {
            stdout.lines().find_map(|l| {
                let v = l.strip_prefix(key)?.strip_prefix('=')?;
                Some(unquote(v))
            })
        };
        let cookie = get("COOKIE")
            .filter(|c| !c.is_empty())
            .ok_or("no COOKIE in sign-in output")?;
        let connect_url = get("CONNECT_URL")
            .or_else(|| get("HOST"))
            .filter(|u| !u.is_empty() && !u.starts_with('-'))
            .ok_or("no CONNECT_URL in sign-in output")?;
        Ok(Self {
            cookie,
            connect_url,
            fingerprint: get("FINGERPRINT").filter(|v| !v.is_empty()),
            resolve: get("RESOLVE").filter(|v| !v.is_empty()),
            user_agent: None,
        })
    }

    /// As a string map, for the API.
    pub fn to_map(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("cookie".into(), self.cookie.clone());
        m.insert("connect_url".into(), self.connect_url.clone());
        if let Some(f) = &self.fingerprint {
            m.insert("fingerprint".into(), f.clone());
        }
        if let Some(r) = &self.resolve {
            m.insert("resolve".into(), r.clone());
        }
        if let Some(u) = &self.user_agent {
            m.insert("user_agent".into(), u.clone());
        }
        m
    }

    /// From the API. Values end up as openconnect arguments, so nothing that
    /// could read as an option is accepted.
    pub fn from_map(m: &BTreeMap<String, String>) -> Result<Self, &'static str> {
        let cookie = m
            .get("cookie")
            .filter(|c| !c.is_empty())
            .ok_or("no cookie")?
            .clone();
        let connect_url = m
            .get("connect_url")
            .filter(|u| !u.is_empty() && !u.starts_with('-') && !u.contains(char::is_whitespace))
            .ok_or("no usable connect_url")?
            .clone();
        let safe =
            |v: &&String| !v.is_empty() && !v.starts_with('-') && !v.contains(char::is_whitespace);
        Ok(Self {
            cookie,
            connect_url,
            fingerprint: m.get("fingerprint").filter(safe).cloned(),
            resolve: m.get("resolve").filter(safe).cloned(),
            // A short plain name only: letters, digits and a few separators.
            user_agent: m
                .get("user_agent")
                .filter(|u| {
                    (1..=64).contains(&u.len())
                        && !u.starts_with('-')
                        && u.chars()
                            .all(|c| c.is_ascii_alphanumeric() || " ._/-".contains(c))
                })
                .cloned(),
        })
    }
}

fn unquote(v: &str) -> String {
    match v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        Some(inner) => inner.replace("'\\''", "'"),
        None => v.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_names() {
        assert!(valid_tunnel_name("acme"));
        assert!(valid_tunnel_name("nord-1"));
        assert!(!valid_tunnel_name(""));
        assert!(!valid_tunnel_name("Acme"));
        assert!(!valid_tunnel_name("-acme"));
        assert!(!valid_tunnel_name("abcdefghij"));
        assert!(!valid_tunnel_name("a b"));
        assert_eq!(interface_name("acme").len(), 10);
        assert!(interface_name("abcdefghi").len() <= 15);
    }

    #[test]
    fn sessions() {
        let s = Session::parse(
            "POST https://vpn.example/\nCOOKIE='abc'\\''def'\nHOST='192.0.2.1'\n\
             CONNECT_URL='https://vpn.example/'\nFINGERPRINT='pin-sha256:xyz='\n",
        )
        .unwrap();
        assert_eq!(s.cookie, "abc'def");
        assert_eq!(s.fingerprint.as_deref(), Some("pin-sha256:xyz="));
        assert_eq!(Session::from_map(&s.to_map()).unwrap(), s);
        assert!(!format!("{s:?}").contains("abc"));
        let mut bad = s.to_map();
        bad.insert("connect_url".into(), "--script=/tmp/x".into());
        assert!(
            Session::from_map(&bad).is_err(),
            "nothing that reads as an option"
        );
        assert!(Session::parse("HOST=''\n").is_err());
        let mut acting = s.clone();
        acting.user_agent = Some("AnyConnect".into());
        assert_eq!(Session::from_map(&acting.to_map()).unwrap(), acting);
        let mut odd = s.to_map();
        odd.insert("user_agent".into(), "--script=/tmp/x".into());
        assert_eq!(
            Session::from_map(&odd).unwrap().user_agent,
            None,
            "a user agent that reads as an option is dropped"
        );
    }

    #[test]
    fn env_whitelist() {
        let env = collect_env([
            ("TUNDEV".into(), "crake-acme".into()),
            ("CISCO_DEF_DOMAIN".into(), "acme.internal".into()),
            ("HOME".into(), "/root".into()),
            ("AWS_SECRET".into(), "x".into()),
        ]);
        assert_eq!(env.len(), 2);
        assert!(!env.contains_key("HOME"));
    }
}
