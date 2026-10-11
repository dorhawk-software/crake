//! Windows: craked's own DNS server, where NRPT rules send the tunnels'
//! domains (`resolved`). Linux holds the tunnel DNS's answers in NFQUEUE
//! until their addresses are routed; here craked asks the tunnel's DNS
//! itself, through the tunnel, routes the answer's addresses, and only then
//! replies: routed before the first packet. Names of craked's own links
//! are answered here too: refused (`crake-hush`, kept private) or from
//! memory (`crake-keep`, a tunnel whose DNS stopped answering).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::net::UdpSocket;

use crate::manager::Manager;

/// Where the server listens: a loopback address of its own.
pub const ADDR: Ipv4Addr = Ipv4Addr::new(127, 53, 0, 1);
/// How long a tunnel's DNS gets to answer.
const WAIT: Duration = Duration::from_secs(3);

/// Serve on `ADDR`:53 for the daemon's lifetime.
pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    let std_sock = std::net::UdpSocket::bind((ADDR, 53))?;
    std_sock.set_nonblocking(true)?;
    let sock = Arc::new(UdpSocket::from_std(std_sock)?);
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf).await else {
                continue;
            };
            let query = buf[..n].to_vec();
            let (mgr, sock) = (Arc::clone(&mgr), Arc::clone(&sock));
            tokio::spawn(async move {
                if let Some(reply) = answer(&mgr, &query).await {
                    let _ = sock.send_to(&reply, from).await;
                }
            });
        }
    });
    Ok(())
}

/// The reply to one query.
async fn answer(mgr: &Arc<Manager>, query: &[u8]) -> Option<Vec<u8>> {
    let (name, _) = crate::keep::question(query)?;
    let Some((device, servers)) = super::resolved::owner(&name) else {
        // Not a name of any link now: say so at once.
        return crate::hush::refuse(query);
    };
    match device.as_str() {
        crate::hush::DEVICE => crate::hush::refuse(query),
        crate::keep::DEVICE => mgr.keep_answer(query).await,
        _ => {
            let Some(reply) = ask(query, &device, &servers).await else {
                // SERVFAIL: the tunnel's DNS didn't answer.
                return crate::hush::reply_code(query, 2);
            };
            if let (Some(tunnel), Some(parsed)) = (
                mgr.tunnel_of_device(&device),
                crate::dns::parse_message(&reply),
            ) {
                mgr.learn_from_dns(&tunnel, parsed).await;
            }
            Some(reply)
        }
    }
}

/// Ask the tunnel's DNS servers in turn, through the tunnel.
async fn ask(query: &[u8], device: &str, servers: &[Ipv4Addr]) -> Option<Vec<u8>> {
    for server in servers {
        let Ok(sock) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else {
            continue;
        };
        if super::bind_device(&sock, device).is_err() {
            continue;
        }
        let to = SocketAddr::from((*server, 53));
        if sock.send_to(query, to).await.is_err() {
            continue;
        }
        let mut buf = vec![0u8; 4096];
        if let Ok(Ok(n)) = tokio::time::timeout(WAIT, sock.recv(&mut buf)).await {
            buf.truncate(n);
            return Some(buf);
        }
    }
    None
}
