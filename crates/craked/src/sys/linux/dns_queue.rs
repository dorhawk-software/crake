//! Holds DNS answers (NFQUEUE) while craked acts on them:
//!
//! - answers from a tunnel's DNS servers are held until the addresses in them
//!   are in the tunnel's set, so the first packet already goes through it;
//! - public answers saying a name doesn't exist are held while craked asks the
//!   tunnels; if one knows the name, its answer replaces the failure.
//!
//! The queue rules use `bypass`: if craked isn't listening, answers pass untouched.
//! Each answer is handled on its own task, so a slow tunnel never holds up the rest;
//! an answer over TCP spread over segments is put together first, in the order
//! they came (`dns_tcp`), and only the segment that completes it is held.

use std::ffi::CStr;
use std::io::ErrorKind;
use std::sync::Arc;

use anyhow::{Context, Result};
use nfq_updated::{Message, Queue, Verdict};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;

use crate::dns;
use crate::manager::Manager;

pub const QUEUE_NUM: u16 = 7300;

/// Listen for the daemon's lifetime.
pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    let mut q = Queue::open().context("opening NFQUEUE")?;
    q.bind(QUEUE_NUM).context("binding NFQUEUE")?;
    // If craked falls behind, let answers through rather than drop them.
    q.set_fail_open(QUEUE_NUM, true).ok();
    q.set_nonblocking(true);
    // SAFETY: the queue owns its netlink socket and the AsyncFd owns the
    // queue: open, the same descriptor, for as long as the AsyncFd holds it.
    let mut fd = unsafe { AsyncFd::register(q) }
        .map_err(std::io::Error::from)
        .context("watching NFQUEUE")?;
    let (done_tx, mut done) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        let mut tcp_answers = crate::dns_tcp::Reassembly::default();
        loop {
            tokio::select! {
                ready = fd.readable_mut() => {
                    let mut guard = match ready {
                        Ok(g) => g,
                        Err(e) => {
                            tracing::error!("DNS queue: {e}");
                            return;
                        }
                    };
                    loop {
                        match guard.get_inner_mut().recv() {
                            Ok(msg) => {
                                let whole = tcp_answers.segment(msg.get_payload());
                                let (mgr, done_tx) = (Arc::clone(&mgr), done_tx.clone());
                                tokio::spawn(async move {
                                    let _ = done_tx.send(handle(&mgr, msg, whole).await);
                                });
                            }
                            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                guard.clear_ready();
                                break;
                            }
                            Err(e) => {
                                tracing::warn!("DNS queue: {e}");
                                break;
                            }
                        }
                    }
                }
                Some(msg) = done.recv() => {
                    if let Err(e) = fd.get_mut().verdict(msg) {
                        tracing::error!("DNS queue verdict: {e}");
                    }
                }
            }
        }
    });
    Ok(())
}

/// `whole`: the TCP answers this segment completes (`dns_tcp`).
async fn handle(mgr: &Arc<Manager>, mut msg: Message, whole: Vec<Vec<u8>>) -> Message {
    match device_of(msg.get_indev()).and_then(|d| mgr.tunnel_of_device(&d)) {
        Some(tunnel) => {
            let answers: Vec<dns::Answer> = if whole.is_empty() {
                dns::parse_packet(msg.get_payload()).into_iter().collect()
            } else {
                whole.iter().filter_map(|m| dns::parse_message(m)).collect()
            };
            for answer in answers {
                mgr.learn_from_dns(&tunnel, answer).await;
            }
        }
        None => {
            if let Some(flow) = crate::probe::unreachable(msg.get_payload()) {
                let mgr = Arc::clone(mgr);
                tokio::spawn(
                    async move { mgr.probe(flow, crate::probe::Direct::Unreachable).await },
                );
            } else if let Some(packet) = mgr.rescue(msg.get_payload()).await {
                msg.set_payload(packet);
            }
        }
    }
    msg.set_verdict(Verdict::Accept);
    msg
}

/// The interface's name (`crake-acme`, `tun0`).
fn device_of(ifindex: u32) -> Option<String> {
    let mut buf = [0 as libc::c_char; libc::IF_NAMESIZE];
    // SAFETY: buffer is IF_NAMESIZE as required.
    let p = unsafe { libc::if_indextoname(ifindex, buf.as_mut_ptr()) };
    if p.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().ok()?;
    Some(name.to_owned())
}
