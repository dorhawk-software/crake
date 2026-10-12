//! Address and link setup for a tunnel interface. Installs no routes except
//! a /32 to each of the tunnel's DNS servers.

use std::net::Ipv4Addr;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

use super::netlink;

#[derive(Debug, Clone, Copy)]
pub enum Net {
    /// Shell out to iproute2.
    Ip,
    /// Do nothing (development and tests).
    Noop,
}

impl Net {
    pub fn new(apply: bool) -> Self {
        if apply { Self::Ip } else { Self::Noop }
    }

    /// Give `dev` its address, bring it up. Idempotent.
    pub async fn assign(
        &self,
        dev: &str,
        addr: Ipv4Addr,
        prefix: u8,
        mtu: Option<u32>,
    ) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        ip(&["addr", "replace", &format!("{addr}/{prefix}"), "dev", dev]).await?;
        let mut up = vec!["link".to_string(), "set".into(), "dev".into(), dev.into()];
        if let Some(mtu) = mtu {
            up.extend(["mtu".into(), mtu.to_string()]);
        }
        up.push("up".into());
        let up: Vec<&str> = up.iter().map(String::as_str).collect();
        ip(&up).await
    }

    /// A dummy link with one address (see `hush`). Idempotent.
    pub async fn dummy(&self, dev: &str, addr: Ipv4Addr) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        let _ = ip(&["link", "add", dev, "type", "dummy"]).await; // fine if it exists
        ip(&["addr", "replace", &format!("{addr}/32"), "dev", dev]).await?;
        ip(&["link", "set", "dev", dev, "up"]).await
    }

    /// Remove a link; fine if it's gone.
    pub async fn remove(&self, dev: &str) {
        if let Self::Ip = self {
            let _ = ip(&["link", "del", dev]).await;
        }
    }

    /// Replies from the hosts Crake sends through `dev` come in on it, while
    /// the main table would send them elsewhere: strict reverse-path
    /// filtering (rp_filter=1, as on Fedora) drops them. Loose mode on the
    /// tunnel's own interface lets them in; the kernel takes the looser of
    /// `all` and the interface's own value, so nothing else changes.
    ///
    /// Set over netlink (RTM_SETLINK, the interface's IPv4 settings), which
    /// works where /proc/sys is read-only.
    pub fn loose_rp_filter(&self, dev: &str) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        let name = std::ffi::CString::new(dev)?;
        // SAFETY: a valid NUL-terminated name.
        let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
        if index == 0 {
            bail!("no interface {dev}");
        }
        set_link(&rp_filter_message(index, 2))
            .with_context(|| format!("loose reverse-path filtering on {dev}"))
    }

    /// The only route Crake installs up front.
    pub async fn route_dns(&self, dev: &str, dns: Ipv4Addr) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        ip(&["route", "replace", &format!("{dns}/32"), "dev", dev]).await
    }
}

/// IPV4_DEVCONF_RP_FILTER, among an interface's IPv4 settings
/// (linux/ip.h); IFLA_AF_SPEC and IFLA_INET_CONF (linux/if_link.h).
const DEVCONF_RP_FILTER: u16 = 8;
const IFLA_AF_SPEC: u16 = 26;
const IFLA_INET_CONF: u16 = 1;

/// RTM_SETLINK for interface `index`: its rp_filter set to `value`.
fn rp_filter_message(index: u32, value: u32) -> Vec<u8> {
    use std::mem::offset_of;
    let conf = netlink::attr(
        IFLA_INET_CONF,
        &netlink::attr(DEVCONF_RP_FILTER, &value.to_ne_bytes()),
    );
    let spec = netlink::attr(IFLA_AF_SPEC, &netlink::attr(libc::AF_INET as u16, &conf));
    // struct ifinfomsg: all zero (any family, type, flags) but the index.
    let mut body = vec![0u8; size_of::<libc::ifinfomsg>()];
    let at = offset_of!(libc::ifinfomsg, ifi_index);
    body[at..at + size_of::<i32>()].copy_from_slice(&index.to_ne_bytes());
    body.extend_from_slice(&spec);
    netlink::request(libc::RTM_SETLINK, &body)
}

/// Send one rtnetlink request and read its acknowledgement.
fn set_link(msg: &[u8]) -> Result<()> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    // SAFETY: plain socket(2); the descriptor is owned below.
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            libc::NETLINK_ROUTE,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: a fresh descriptor nothing else owns.
    let sock = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: sending from a valid buffer of the given length.
    let sent = unsafe { libc::send(sock.as_raw_fd(), msg.as_ptr().cast(), msg.len(), 0) };
    if sent < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut buf = [0u8; netlink::ACK_BUFFER];
    // SAFETY: receiving into a valid buffer of the given length.
    let n = unsafe { libc::recv(sock.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
    if n < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // NLMSG_ERROR with error 0 is the acknowledgement.
    match netlink::ack_code(&buf[..usize::try_from(n).unwrap_or(0)]) {
        Some(0) => Ok(()),
        Some(code) => Err(std::io::Error::from_raw_os_error(-code).into()),
        None => bail!("no acknowledgement from the kernel"),
    }
}

async fn ip(args: &[&str]) -> Result<()> {
    let out = Command::new("ip")
        .args(args)
        .output()
        .await
        .context("running ip")?;
    if !out.status.success() {
        bail!(
            "ip {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}
