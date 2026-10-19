//! Holds the first SYN of a new outbound connection (NFQUEUE) while craked
//! decides where it goes, so even the first connection to a host that needs
//! a tunnel succeeds (CLAUDE.md milestone 4, "race the direct and tunnel
//! attempts while holding the first SYN"). The nftables rule (`routing`)
//! queues only SYNs worth racing (a tunnel's scope, or a private address
//! not on the local network); craked races a direct attempt against the
//! tunnels (`race_held`), learns the host if a tunnel takes it, then lets
//! the SYN through — now routed. `bypass`: if craked isn't listening, SYNs
//! pass untouched; each is handled on its own task. New UDP conversations'
//! first datagrams to the same addresses come here too: copied and let go
//! at once, for probing a UDP host through the tunnels with them.

use std::io::ErrorKind;
use std::sync::Arc;

use anyhow::{Context, Result};
use nfq_updated::{Message, Queue, Verdict};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;

use crate::manager::Manager;

pub const QUEUE_NUM: u16 = 7301;

pub fn spawn(mgr: Arc<Manager>) -> Result<()> {
    let mut q = Queue::open().context("opening the SYN NFQUEUE")?;
    q.bind(QUEUE_NUM).context("binding the SYN NFQUEUE")?;
    // If craked falls behind, let connections through rather than hold them.
    q.set_fail_open(QUEUE_NUM, true).ok();
    q.set_nonblocking(true);
    // SAFETY: the queue owns its netlink socket and the AsyncFd owns the
    // queue: open, the same descriptor, for as long as the AsyncFd holds it.
    let mut fd = unsafe { AsyncFd::register(q) }
        .map_err(std::io::Error::from)
        .context("watching the SYN NFQUEUE")?;
    let (done_tx, mut done) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                ready = fd.readable_mut() => {
                    let mut guard = match ready {
                        Ok(g) => g,
                        Err(e) => {
                            tracing::error!("SYN queue: {e}");
                            return;
                        }
                    };
                    loop {
                        match guard.get_inner_mut().recv() {
                            Ok(msg) => {
                                let (mgr, done_tx) = (Arc::clone(&mgr), done_tx.clone());
                                tokio::spawn(async move {
                                    let _ = done_tx.send(handle(&mgr, msg).await);
                                });
                            }
                            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                guard.clear_ready();
                                break;
                            }
                            Err(e) => {
                                tracing::warn!("SYN queue: {e}");
                                break;
                            }
                        }
                    }
                }
                Some(msg) = done.recv() => {
                    if let Err(e) = fd.get_mut().verdict(msg) {
                        tracing::error!("SYN queue verdict: {e}");
                    }
                }
            }
        }
    });
    Ok(())
}

async fn handle(mgr: &Arc<Manager>, mut msg: Message) -> Message {
    if let Some(flow) = crate::probe::syn_flow(msg.get_payload()) {
        // Routes the host (populates the tunnel's set) if a tunnel takes it,
        // so the SYN we now let through is marked and routed by the output
        // chain. Direct or down: let it through unchanged.
        mgr.race_held(flow).await;
    } else if let Some((flow, payload)) = crate::probe::udp_first(msg.get_payload()) {
        // Copied before it goes (its conntrack entry is made after): the
        // probe's way to try a UDP host through the tunnels.
        mgr.first_datagram(flow, payload);
    }
    msg.set_verdict(Verdict::Accept);
    msg
}
