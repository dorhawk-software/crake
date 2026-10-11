//! Windows: craked's own DNS server, where NRPT rules send the tunnels'
//! domains (`resolved`). Linux holds the tunnel DNS's answers in NFQUEUE
//! until their addresses are routed; here craked asks the tunnel's DNS
//! itself, through the tunnel, routes the answer's addresses, and only then
//! replies: routed before the first packet. Names of craked's own links
//! are answered here too: refused (`crake-hush`, kept private) or from
//! memory (`crake-keep`, a tunnel whose DNS stopped answering). Over UDP
//! and TCP (Windows asks again over TCP when an answer was truncated); a
//! question that came over TCP is asked of the tunnel's DNS over TCP.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream, UdpSocket};

use crate::manager::Manager;

/// Where the server listens: a loopback address of its own.
pub const ADDR: Ipv4Addr = Ipv4Addr::new(127, 53, 0, 1);
/// How long a tunnel's DNS gets to answer.
const WAIT: Duration = Duration::from_secs(3);

/// How long a TCP client may stay quiet between questions.
const IDLE: Duration = Duration::from_secs(10);

/// Serve on `ADDR`:53, UDP and TCP, for the daemon's lifetime.
pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    let std_sock = std::net::UdpSocket::bind((ADDR, 53))?;
    std_sock.set_nonblocking(true)?;
    let sock = Arc::new(UdpSocket::from_std(std_sock)?);
    let std_listener = std::net::TcpListener::bind((ADDR, 53))?;
    std_listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(std_listener)?;
    let tcp_mgr = Arc::clone(&mgr);
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(serve_tcp(Arc::clone(&tcp_mgr), stream));
        }
    });
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf).await else {
                continue;
            };
            let query = buf[..n].to_vec();
            let (mgr, sock) = (Arc::clone(&mgr), Arc::clone(&sock));
            tokio::spawn(async move {
                if let Some(reply) = answer(&mgr, &query, false).await {
                    let _ = sock.send_to(&reply, from).await;
                }
            });
        }
    });
    Ok(())
}

/// One TCP client's questions, each with its two-byte length, until it
/// goes quiet or away.
async fn serve_tcp(mgr: Arc<Manager>, mut stream: TcpStream) {
    while let Ok(Some(query)) = tokio::time::timeout(IDLE, read_message(&mut stream)).await {
        let Some(reply) = answer(&mgr, &query, true).await else {
            return;
        };
        if write_message(&mut stream, &reply).await.is_err() {
            return;
        }
    }
}

/// One DNS message over TCP (RFC 1035 4.2.2: its length first).
async fn read_message(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let len = stream.read_u16().await.ok()?;
    let mut msg = vec![0u8; usize::from(len)];
    stream.read_exact(&mut msg).await.ok()?;
    Some(msg)
}

async fn write_message(stream: &mut TcpStream, msg: &[u8]) -> std::io::Result<()> {
    let len = u16::try_from(msg.len()).map_err(std::io::Error::other)?;
    let mut out = Vec::with_capacity(msg.len() + 2);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(msg);
    stream.write_all(&out).await
}

/// The reply to one query; `tcp`: it came over TCP, so the whole answer
/// is wanted, not one cut to a datagram.
async fn answer(mgr: &Arc<Manager>, query: &[u8], tcp: bool) -> Option<Vec<u8>> {
    let (name, _) = crate::keep::question(query)?;
    let Some((device, servers)) = super::resolved::owner(&name) else {
        if super::resolved::public() {
            return public(mgr, query, tcp).await;
        }
        // Not a name of any link now: say so at once.
        return crate::hush::refuse(query);
    };
    match device.as_str() {
        crate::hush::DEVICE => crate::hush::refuse(query),
        crate::keep::DEVICE => mgr.keep_answer(query).await,
        _ => {
            let reply = if tcp {
                ask_tcp(query, Some(&device), &servers).await
            } else {
                ask(query, Some(&device), &servers).await
            };
            let Some(reply) = reply else {
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

/// A public name: asked of Windows' own DNS servers, whose answer the
/// manager may rescue (a name only a tunnel knows, a public name with a
/// private address).
async fn public(mgr: &Arc<Manager>, query: &[u8], tcp: bool) -> Option<Vec<u8>> {
    let servers = super::system_dns();
    let reply = if tcp {
        ask_tcp(query, None, &servers).await
    } else {
        ask(query, None, &servers).await
    };
    let Some(reply) = reply else {
        return crate::hush::reply_code(query, 2);
    };
    Some(mgr.rescue_answer(&reply).await.unwrap_or(reply))
}

/// Ask DNS servers in turn: a tunnel's through the tunnel (`device`).
async fn ask(query: &[u8], device: Option<&str>, servers: &[Ipv4Addr]) -> Option<Vec<u8>> {
    for server in servers {
        let Ok(sock) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else {
            continue;
        };
        if let Some(device) = device
            && super::bind_device(&sock, device).is_err()
        {
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

/// The same over TCP.
async fn ask_tcp(query: &[u8], device: Option<&str>, servers: &[Ipv4Addr]) -> Option<Vec<u8>> {
    for server in servers {
        let Ok(sock) = TcpSocket::new_v4() else {
            continue;
        };
        if let Some(device) = device
            && super::bind_device(&sock, device).is_err()
        {
            continue;
        }
        let exchange = async {
            let mut stream = sock.connect(SocketAddr::from((*server, 53))).await.ok()?;
            write_message(&mut stream, query).await.ok()?;
            read_message(&mut stream).await
        };
        if let Ok(Some(reply)) = tokio::time::timeout(WAIT, exchange).await {
            return Some(reply);
        }
    }
    None
}
