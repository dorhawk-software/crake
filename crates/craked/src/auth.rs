//! Sign-in kept apart from the connection: `openconnect --authenticate` yields a
//! session cookie, and the tunnel is then started with `--cookie-on-stdin`.

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

use crate::config::TunnelConfig;

pub use crake_proto::Session;

/// Run the sign-in on the current terminal (prompts go to stderr, results to stdout).
pub async fn authenticate_on_terminal(openconnect: &Path, t: &TunnelConfig) -> Result<Session> {
    let out = Command::new(openconnect)
        .arg("--authenticate")
        .arg(format!("--protocol={}", t.protocol.as_str()))
        .args(t.user.iter().map(|u| format!("--user={u}")))
        .args(&t.auth_args)
        .args(&t.extra_args)
        .arg("--")
        .arg(&t.gateway)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped())
        // Not `output()`: tokio's forces stderr to a pipe, which would hide
        // openconnect's prompts.
        .spawn()
        .with_context(|| format!("starting {}", openconnect.display()))?
        .wait_with_output()
        .await?;
    if !out.status.success() {
        bail!("sign-in to {} failed ({})", t.display(), out.status);
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

/// Parse what `openconnect --authenticate` printed.
pub fn parse(stdout: &str) -> Result<Session> {
    Session::parse(stdout).map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_authenticate_output() {
        let s = parse(
            "POST https://vpn.example/\nCOOKIE='abc'\\''def'\nHOST='192.0.2.1'\n\
             CONNECT_URL='https://vpn.example/'\nFINGERPRINT='pin-sha256:xyz='\n\
             RESOLVE='vpn.example:192.0.2.1'\n",
        )
        .unwrap();
        assert_eq!(s.cookie, "abc'def");
        assert_eq!(s.connect_url, "https://vpn.example/");
        assert_eq!(s.fingerprint.as_deref(), Some("pin-sha256:xyz="));
        assert_eq!(s.resolve.as_deref(), Some("vpn.example:192.0.2.1"));
        assert!(!format!("{s:?}").contains("abc"));
    }

    #[test]
    fn needs_a_cookie() {
        assert!(parse("HOST='192.0.2.1'\n").is_err());
    }
}
