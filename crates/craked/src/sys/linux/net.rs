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

    /// Give `dev` its IPv6 address (no duplicate-address detection: the
    /// tunnel's other end is the gateway). Idempotent.
    pub async fn assign6(&self, dev: &str, addr: std::net::Ipv6Addr, prefix: u8) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        ip(&[
            "-6",
            "addr",
            "replace",
            &format!("{addr}/{prefix}"),
            "dev",
            dev,
            "nodad",
        ])
        .await
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

    /// The same for an IPv6 DNS server.
    pub async fn route_dns6(&self, dev: &str, dns: std::net::Ipv6Addr) -> Result<()> {
        let Self::Ip = self else { return Ok(()) };
        ip(&["-6", "route", "replace", &format!("{dns}/128"), "dev", dev]).await
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

/// IFLA_LINKINFO and, inside it, IFLA_INFO_KIND (linux/if_link.h).
const IFLA_LINKINFO: u16 = 18;
const IFLA_INFO_KIND: u16 = 1;
/// Room for a link's description (RTM_NEWLINK; a few hundred bytes to a
/// few kilobytes).
const LINK_BUFFER: usize = 32 * 1024;

/// `dev` is a tun device (as openvpn3 makes) in craked's own network
/// namespace; false when there's no such device. Asked over netlink, which,
/// unlike /sys, always speaks of the namespace craked runs in.
pub fn is_tun(dev: &str) -> bool {
    link_kind(dev).as_deref() == Some("tun")
}

/// What kind of link `dev` is ("tun", "dummy", ...; IFLA_INFO_KIND).
fn link_kind(dev: &str) -> Option<String> {
    use std::mem::offset_of;
    let name = std::ffi::CString::new(dev).ok()?;
    // SAFETY: a valid NUL-terminated name.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    if index == 0 {
        return None;
    }
    let mut body = vec![0u8; size_of::<libc::ifinfomsg>()];
    let at = offset_of!(libc::ifinfomsg, ifi_index);
    body[at..at + size_of::<i32>()].copy_from_slice(&index.to_ne_bytes());
    let reply = exchange(&netlink::request(libc::RTM_GETLINK, &body), LINK_BUFFER).ok()?;
    netlink::messages(&reply)
        .into_iter()
        .filter(|(kind, _, _)| *kind == libc::RTM_NEWLINK)
        .find_map(|(_, _, link)| {
            let attrs = netlink::attrs(link.get(size_of::<libc::ifinfomsg>()..)?);
            let (_, info) = attrs.into_iter().find(|(t, _)| *t == IFLA_LINKINFO)?;
            let (_, kind) = netlink::attrs(info)
                .into_iter()
                .find(|(t, _)| *t == IFLA_INFO_KIND)?;
            Some(
                String::from_utf8_lossy(kind)
                    .trim_end_matches('\0')
                    .to_owned(),
            )
        })
}

/// Send one rtnetlink request and read its acknowledgement.
fn set_link(msg: &[u8]) -> Result<()> {
    let reply = exchange(msg, netlink::ACK_BUFFER)?;
    // NLMSG_ERROR with error 0 is the acknowledgement.
    match netlink::ack_code(&reply) {
        Some(0) => Ok(()),
        Some(code) => Err(std::io::Error::from_raw_os_error(-code).into()),
        None => bail!("no acknowledgement from the kernel"),
    }
}

/// Send one rtnetlink request; what the kernel answers first (up to
/// `room` bytes).
fn exchange(msg: &[u8], room: usize) -> Result<Vec<u8>> {
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
    let mut buf = vec![0u8; room];
    // SAFETY: receiving into a valid buffer of the given length.
    let n = unsafe { libc::recv(sock.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
    if n < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    buf.truncate(usize::try_from(n).unwrap_or(0));
    Ok(buf)
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

#[cfg(test)]
mod tests {
    #[test]
    fn only_a_tun_device_is_one() {
        assert!(!super::is_tun("lo"), "loopback");
        assert!(!super::is_tun("crake-no-such-device"));
    }
}
