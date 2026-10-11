//! macOS: the command-line tools craked drives (`ifconfig`, `route`), as
//! Linux's craked drives `ip` and `nft`.

use std::net::Ipv4Addr;

use anyhow::{Result, bail};
use tokio::process::Command;

/// Run `program` with `args`; its complaint, if it fails.
pub async fn run(program: &str, args: &[&str]) -> Result<()> {
    let out = Command::new(program).args(args).output().await?;
    if !out.status.success() {
        bail!(
            "{program} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// `dest/prefix` as `route` takes it.
fn destination(dest: &str, prefix: u8) -> Vec<String> {
    if prefix == 32 {
        vec!["-host".into(), dest.into()]
    } else {
        vec!["-net".into(), format!("{dest}/{prefix}")]
    }
}

/// `dest/prefix` through the interface (already there: fine).
pub async fn route_add(dest: &str, prefix: u8, dev: &str) -> Result<()> {
    let mut args = vec!["-n".to_owned(), "add".into()];
    args.extend(destination(dest, prefix));
    args.extend(["-interface".into(), dev.into()]);
    route(&args, true).await
}

/// `dest/prefix` through the interface, for sockets bound to it only
/// (IP_BOUND_IF: macOS' scoped routing).
pub async fn route_add_scoped(dest: &str, prefix: u8, dev: &str) -> Result<()> {
    let mut args = vec!["-n".to_owned(), "add".into()];
    args.extend(destination(dest, prefix));
    args.extend([
        "-interface".into(),
        dev.into(),
        "-ifscope".into(),
        dev.into(),
    ]);
    route(&args, true).await
}

/// `dest/prefix` to a next hop (a full tunnel's gateway kept on its way).
pub async fn route_add_via(dest: &str, prefix: u8, via: Ipv4Addr) -> Result<()> {
    let mut args = vec!["-n".to_owned(), "add".into()];
    args.extend(destination(dest, prefix));
    args.push(via.to_string());
    route(&args, true).await
}

/// Remove `dest/prefix` (from `dev`'s scope with `scoped`); gone already:
/// fine.
pub async fn route_delete(dest: &str, prefix: u8, scoped: Option<&str>) -> Result<()> {
    let mut args = vec!["-n".to_owned(), "delete".into()];
    args.extend(destination(dest, prefix));
    if let Some(dev) = scoped {
        args.extend(["-ifscope".into(), dev.into()]);
    }
    route(&args, false).await
}

async fn route(args: &[String], adding: bool) -> Result<()> {
    let out = Command::new("route").args(args).output().await?;
    let said =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    let fine = if adding {
        said.contains("File exists")
    } else {
        said.contains("not in table")
    };
    if !out.status.success() && !fine {
        bail!("route {}: {}", args.join(" "), said.trim());
    }
    Ok(())
}

/// How macOS sends to `dest` now: (whether by the default route, its next
/// hop if any), from `route -n get`.
pub async fn way_to(dest: Ipv4Addr) -> Result<(bool, Option<Ipv4Addr>)> {
    let out = Command::new("route")
        .args(["-n", "get", &dest.to_string()])
        .output()
        .await?;
    Ok(parse_route_get(&String::from_utf8_lossy(&out.stdout)))
}

/// `route -n get`'s answer: whether its destination is the default route,
/// and its gateway.
pub fn parse_route_get(text: &str) -> (bool, Option<Ipv4Addr>) {
    let field = |name: &str| {
        text.lines()
            .filter_map(|l| l.trim().strip_prefix(name))
            .map(str::trim)
            .next()
    };
    let default = field("destination:") == Some("default");
    let gateway = field("gateway:").and_then(|g| g.parse().ok());
    (default, gateway)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_route_get() {
        let text = "   route to: 198.51.100.10\ndestination: default\n       mask: default\n    gateway: 192.0.2.1\n  interface: en0\n";
        assert_eq!(
            super::parse_route_get(text),
            (true, Some("192.0.2.1".parse().unwrap()))
        );
        let lan = "   route to: 192.0.2.7\ndestination: 192.0.2.0\n  interface: en0\n";
        assert_eq!(super::parse_route_get(lan), (false, None));
    }
}
