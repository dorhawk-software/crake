//! Each OpenVPN tunnel's adapter on Windows, `crake-<name>`: made for its run
//! with OpenVPN's tapctl (beside openvpn.exe) and removed after it. ovpn-dco
//! (data channel offload, openvpn 2.7's default), or tap-windows6 for a
//! gateway that can't take offload. Both drivers come with Crake's
//! installer.

use std::path::Path;

use anyhow::{Result, bail};
use tokio::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// tapctl, beside the openvpn Crake runs.
fn tapctl(openvpn: &Path) -> std::path::PathBuf {
    openvpn.with_file_name("tapctl.exe")
}

/// Make `name` (any of that name first goes: a run that didn't end well,
/// or the other driver's).
pub async fn make(openvpn: &Path, name: &str, dco: bool) -> Result<()> {
    remove(openvpn, name).await;
    let hwid = if dco { "ovpn-dco" } else { r"root\tap0901" };
    let out = Command::new(tapctl(openvpn))
        .args(["create", "--hwid", hwid, "--name", name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .await?;
    if !out.status.success() {
        bail!(
            "making its adapter {name}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Remove `name`, if it's there.
pub async fn remove(openvpn: &Path, name: &str) {
    let _ = Command::new(tapctl(openvpn))
        .args(["delete", name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .await;
}
