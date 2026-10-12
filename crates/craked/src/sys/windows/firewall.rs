//! Windows Firewall, for one thing: a full tunnel whose gateway gave no
//! IPv6 address refuses IPv6 rather than let it go around the tunnel
//! (Linux does it with nftables, macOS with reject routes). Windows skips
//! a route whose interface has no address to send from, so routing IPv6
//! into the tunnel doesn't do it; a firewall rule blocking IPv6 out of the
//! machine's own adapters does, and an app's connection fails at once (it
//! falls back to IPv4). Loopback, link-local, multicast, the LAN and the
//! gateway are left out; Crake's own tunnels aren't among the adapters.

use anyhow::{Result, bail};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

use crate::hosts::Target;

/// The rules' group: what's Crake's, found again after a crash.
const GROUP: &str = "Crake";
/// There while a rule of Crake's may exist: only then does a start clean
/// up (PowerShell's firewall module takes seconds to load, too long for a
/// service starting).
const MARKER: &str = r"C:\ProgramData\Crake\firewall-rules";

/// Block IPv6 for `name` (its full tunnel has no IPv6 address), but to
/// `except` and what always stays (`plan::KEPT6`).
pub async fn block6(name: &str, except: &[Target]) -> Result<()> {
    let mut adapters: Vec<String> = super::interfaces6()
        .into_iter()
        .map(|(alias, _, _)| alias)
        .filter(|alias| !super::is_tunnel_device(alias))
        .collect();
    adapters.sort();
    adapters.dedup();
    if adapters.is_empty() {
        // No IPv6 anywhere: nothing to go around the tunnel.
        return Ok(());
    }
    let ranges: Vec<String> = crate::plan::uncovered6(except)
        .into_iter()
        .map(|(first, last)| quoted(&format!("{first}-{last}")))
        .collect();
    let aliases: Vec<String> = adapters.iter().map(|a| quoted(a)).collect();
    // Before the rule: a crash right after it still leaves the marker.
    if let Some(dir) = std::path::Path::new(MARKER).parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(MARKER, b"")?;
    powershell(&format!(
        "New-NetFirewallRule -DisplayName {} -Group {} -Direction Outbound -Action Block \
         -Profile Any -InterfaceAlias @({}) -RemoteAddress @({}) | Out-Null",
        quoted(&rule(name)),
        quoted(GROUP),
        aliases.join(","),
        ranges.join(",")
    ))
    .await
}

/// `name`'s rule gone (none: fine).
pub async fn unblock6(name: &str) -> Result<()> {
    remove(&format!("-DisplayName {}", quoted(&rule(name)))).await
}

/// No rule of Crake's is left: nothing to clean up at the next start.
pub fn none_left() {
    let _ = std::fs::remove_file(MARKER);
}

/// Every rule of Crake's gone (at startup: what a crash left).
pub async fn clear() -> Result<()> {
    if !std::path::Path::new(MARKER).exists() {
        return Ok(());
    }
    remove(&format!("-Group {}", quoted(GROUP))).await?;
    let _ = std::fs::remove_file(MARKER);
    Ok(())
}

/// The rules `which` finds removed. None found isn't a failure (finding
/// none is one to PowerShell, whose exit code would say so).
async fn remove(which: &str) -> Result<()> {
    powershell(&format!(
        "$found = Get-NetFirewallRule {which} -ErrorAction SilentlyContinue; \
         if ($found) {{ $found | Remove-NetFirewallRule }}; exit 0"
    ))
    .await
}

fn rule(name: &str) -> String {
    format!("Crake {name} IPv6")
}

/// A PowerShell string literal.
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

async fn powershell(script: &str) -> Result<()> {
    let out = tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .await?;
    if !out.status.success() {
        bail!(
            "the firewall: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}
