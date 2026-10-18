//! Types shared by `craked`, `crake-hook` and (later) the UI.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod live;
pub mod trouble;

/// Where `craked` listens for the hook, unless configured otherwise.
#[cfg(target_os = "linux")]
pub const DEFAULT_HOOK_SOCKET: &str = "/run/crake/hook.sock";
/// macOS: /var/run (there's no /run).
#[cfg(target_os = "macos")]
pub const DEFAULT_HOOK_SOCKET: &str = "/var/run/crake/hook.sock";
/// Windows: a named pipe.
#[cfg(windows)]
pub const DEFAULT_HOOK_SOCKET: &str = r"\\.\pipe\crake-hook";
/// Where the hook binary is installed.
#[cfg(target_os = "linux")]
pub const DEFAULT_HOOK_PATH: &str = "/usr/libexec/crake/hook";
/// macOS: /usr/local (the system's /usr is sealed).
#[cfg(target_os = "macos")]
pub const DEFAULT_HOOK_PATH: &str = "/usr/local/libexec/crake/hook";
/// Windows: openconnect runs its script through cscript.exe, so the hook
/// is a JScript shim that starts crake-hook.exe beside it.
#[cfg(windows)]
pub const DEFAULT_HOOK_PATH: &str = r"C:\Program Files\Crake\crake-hook.js";
/// Windows: where craked serves the app (D-Bus, loopback only; the app
/// checks it's the `craked` service listening, craked that the caller is
/// in the console session).
#[cfg(windows)]
pub const APP_ADDRESS: &str = "127.0.0.1:47366";
/// macOS: where craked serves the app (D-Bus over a Unix socket; anyone
/// may read, the console's user may change things).
#[cfg(target_os = "macos")]
pub const APP_SOCKET: &str = "/var/run/crake.sock";
/// Windows: craked's service name.
#[cfg(windows)]
pub const SERVICE_NAME: &str = "craked";
/// What openconnect opens a browser sign-in's page with
/// (`--external-browser`): the desktop's own opener.
#[cfg(not(target_os = "macos"))]
pub const BROWSER: &str = "xdg-open";
#[cfg(target_os = "macos")]
pub const BROWSER: &str = "open";
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

/// What openvpn gives its scripts that craked reads (`craked::openvpn::env`).
const OPENVPN_PREFIXES: &[&str] = &[
    "dev",
    "dns_",
    "foreign_option_",
    "ifconfig_",
    "route_",
    "script_",
    "trusted_ip",
    "tun_mtu",
];

/// Keep only the variables openconnect documents for its script, or, when
/// openvpn runs it (it sets `script_type`), openvpn's.
pub fn collect_env(vars: impl IntoIterator<Item = (String, String)>) -> BTreeMap<String, String> {
    let vars: Vec<(String, String)> = vars.into_iter().collect();
    let prefixes = if vars.iter().any(|(k, _)| k == OPENVPN_SCRIPT_TYPE) {
        OPENVPN_PREFIXES
    } else {
        ENV_PREFIXES
    };
    vars.into_iter()
        .filter(|(k, _)| prefixes.iter().any(|p| k.starts_with(p)))
        .collect()
}

/// Set by openvpn for its scripts: `up`, `down`, ...
pub const OPENVPN_SCRIPT_TYPE: &str = "script_type";

/// The hook's reason for one of openvpn's script calls, in openconnect's
/// words: `up` is a connect, or a reconnect when openvpn restarted
/// (`script_context`); `down` the end, or a restart.
pub fn openvpn_reason(script_type: &str, context: &str) -> &'static str {
    match (script_type, context) {
        ("up", "restart") => "reconnect",
        ("up", _) => "connect",
        ("down", "restart") => "attempt-reconnect",
        ("down", _) => "disconnect",
        _ => "other",
    }
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

/// How the log says a tunnel stopped without being asked to:
/// "{label} stopped unexpectedly." (craked writes it; the app notifies on
/// it, with a way to switch the tunnel back on).
pub fn stopped_unexpectedly(label: &str) -> String {
    format!("{label}{STOPPED}")
}

const STOPPED: &str = " stopped unexpectedly.";

/// How the log says an OpenVPN 3 session's process outlived its session
/// (the app finds it, craked logs it; the app notifies on it). openvpn3
/// has kept such a process reconnecting on its own for days, and each
/// reconnect rewrote the other tunnels' DNS.
pub const LEFT_RUNNING: &str = "OpenVPN 3 left a connection running.";

/// The reason under `LEFT_RUNNING`: what it does and how to stop it
/// (Crake can't: the process is openvpn3's).
pub fn left_running_reason(profile: &str, pid: u32) -> String {
    format!(
        "Its session for the profile {profile} ended, but the process behind it \
         ({pid}) didn\u{2019}t: it reconnects on its own and can change your other \
         tunnels\u{2019} DNS. Stop it with: sudo kill -9 {pid}"
    )
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

impl LogEntry {
    /// A tunnel stopped without being asked to (`stopped_unexpectedly`).
    pub fn is_unexpected_stop(&self) -> bool {
        self.kind == LogKind::System && self.tunnel.is_some() && self.message.ends_with(STOPPED)
    }

    /// An OpenVPN 3 session's process outlived its session (`LEFT_RUNNING`).
    pub fn is_left_running(&self) -> bool {
        self.kind == LogKind::System && self.message == LEFT_RUNNING
    }
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

/// The longest user agent a session may carry to craked (a plain client
/// name such as "AnyConnect", never a whole header).
const USER_AGENT_MAX: usize = 64;

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
                    (1..=USER_AGENT_MAX).contains(&u.len())
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
    fn openvpns_script_calls_in_openconnects_words() {
        assert_eq!(openvpn_reason("up", "init"), "connect");
        assert_eq!(openvpn_reason("up", "restart"), "reconnect");
        assert_eq!(openvpn_reason("down", "restart"), "attempt-reconnect");
        assert_eq!(openvpn_reason("down", "init"), "disconnect");
    }

    #[test]
    fn openvpns_variables_are_kept_when_openvpn_calls() {
        let vars = [
            ("script_type", "up"),
            ("dev", "crake-acme"),
            ("foreign_option_1", "dhcp-option DNS 10.40.0.2"),
            ("PATH", "/usr/bin"),
            ("TUNDEV", "x"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()));
        let kept = collect_env(vars.clone());
        assert!(kept.contains_key("dev") && kept.contains_key("foreign_option_1"));
        assert!(!kept.contains_key("PATH") && !kept.contains_key("TUNDEV"));
        // openconnect's call: its own.
        let kept = collect_env(vars.into_iter().filter(|(k, _)| k != "script_type"));
        assert!(kept.contains_key("TUNDEV") && !kept.contains_key("dev"));
    }

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
    fn a_process_left_running_says_how_to_stop_it() {
        let entry = LogEntry {
            ts: String::new(),
            kind: LogKind::System,
            tunnel: None,
            message: LEFT_RUNNING.into(),
            reason: Some(left_running_reason("globex", 4242)),
            hosts: Vec::new(),
            others: Vec::new(),
        };
        assert!(entry.is_left_running());
        assert!(!entry.is_unexpected_stop());
        assert_eq!(
            entry.reason.as_deref(),
            Some(
                "Its session for the profile globex ended, but the process behind it (4242) \
                 didn\u{2019}t: it reconnects on its own and can change your other tunnels\u{2019} \
                 DNS. Stop it with: sudo kill -9 4242"
            )
        );
    }

    #[test]
    fn states_have_the_names_the_api_uses() {
        for (state, name) in [
            (TunnelState::Off, "off"),
            (TunnelState::Connecting, "connecting"),
            (TunnelState::Connected, "connected"),
            (TunnelState::Trouble, "trouble"),
        ] {
            assert_eq!(state.as_str(), name);
        }
    }

    #[test]
    fn a_session_with_a_resolve_and_unquoted_values() {
        // openconnect quotes what needs it; plain values come bare.
        let s = Session::parse(
            "COOKIE=abc\nCONNECT_URL=https://vpn.example/\nRESOLVE='vpn.example:192.0.2.1'\n",
        )
        .unwrap();
        assert_eq!(s.cookie, "abc");
        assert_eq!(s.resolve.as_deref(), Some("vpn.example:192.0.2.1"));
        assert_eq!(Session::from_map(&s.to_map()).unwrap(), s);
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

    #[test]
    fn a_tunnel_that_stopped_by_itself_is_told_apart() {
        let entry = |message: String, tunnel: Option<&str>| LogEntry {
            ts: String::new(),
            kind: LogKind::System,
            tunnel: tunnel.map(str::to_owned),
            message,
            reason: None,
            hosts: Vec::new(),
            others: Vec::new(),
        };
        assert!(entry(stopped_unexpectedly("Initech"), Some("initech")).is_unexpected_stop());
        assert!(!entry("Switched Initech off.".into(), Some("initech")).is_unexpected_stop());
        assert!(!entry(stopped_unexpectedly("Initech"), None).is_unexpected_stop());
    }
}
