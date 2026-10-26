//! Asking one of several DNS servers (Windows and macOS: craked's own DNS
//! server, `sys::dns_server`, for the system's servers and a tunnel's).

use std::future::Future;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a server gets before the next one is asked too.
pub const NEXT_AFTER: Duration = Duration::from_millis(400);

/// Which servers answered, and when: the one that answered last is asked
/// first, so a dead server costs a moment once, not on every name.
#[derive(Debug, Default)]
pub struct Order(Mutex<Vec<(IpAddr, Instant)>>);

impl Order {
    pub const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    fn seen(&self) -> std::sync::MutexGuard<'_, Vec<(IpAddr, Instant)>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The servers in the order they're asked: the latest to answer first,
    /// then those that never did, as given.
    pub fn ranked(&self, servers: &[IpAddr]) -> Vec<IpAddr> {
        let seen = self.seen();
        let last = |server: &IpAddr| {
            seen.iter()
                .find(|(s, _)| s == server)
                .map(|(_, at)| std::cmp::Reverse(*at))
        };
        let mut ranked = servers.to_vec();
        // Those that answered first (false < true), the latest first.
        ranked.sort_by_key(|s| (last(s).is_none(), last(s)));
        ranked
    }

    /// `server` answered at `now`.
    pub fn answered(&self, server: IpAddr, now: Instant) {
        let mut seen = self.seen();
        match seen.iter_mut().find(|(s, _)| *s == server) {
            Some(entry) => entry.1 = now,
            None => seen.push((server, now)),
        }
    }
}

/// The first answer of `servers`, asked with `ask_one` in `order`'s order:
/// the next is asked too when the ones asked so far are quiet for
/// `next_after`, or at once when they failed. The questions still out are
/// dropped once one answers.
pub async fn first_answer<T, F, Fut>(
    order: &Order,
    servers: &[IpAddr],
    next_after: Duration,
    ask_one: F,
) -> Option<T>
where
    T: Send + 'static,
    F: Fn(IpAddr) -> Fut,
    Fut: Future<Output = Option<T>> + Send + 'static,
{
    let mut waiting = order.ranked(servers).into_iter();
    let mut asked = tokio::task::JoinSet::new();
    loop {
        if let Some(server) = waiting.next() {
            let question = ask_one(server);
            asked.spawn(async move { (server, question.await) });
        }
        let done = if waiting.len() > 0 {
            match tokio::time::timeout(next_after, asked.join_next()).await {
                Ok(done) => done,
                // Quiet so far: ask the next one too.
                Err(_) => continue,
            }
        } else {
            asked.join_next().await
        };
        match done {
            Some(Ok((server, Some(reply)))) => {
                order.answered(server, Instant::now());
                return Some(reply);
            }
            // That one failed: the next is asked at once.
            Some(_) => {}
            None if waiting.len() == 0 => return None,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const DEAD: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
    const GOOD: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2));
    const FAILS: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 3));
    /// How long a server that never answers is given (`sys::dns_server`'s
    /// WAIT).
    const WAIT: Duration = Duration::from_secs(3);
    const NEXT: Duration = Duration::from_millis(50);

    /// A dead server stays quiet for WAIT; FAILS fails at once (a send
    /// that fails, a refused TCP connection); GOOD answers at once.
    async fn fake(server: IpAddr) -> Option<IpAddr> {
        match server {
            DEAD => {
                tokio::time::sleep(WAIT).await;
                None
            }
            FAILS => None,
            _ => Some(server),
        }
    }

    /// The network's first DNS server doesn't answer, its second does (a
    /// real network, 2026-10-26: every new name on a Mac took three
    /// seconds while a tunnel was up). The second is asked once the first
    /// had its moment, not after the first's whole wait.
    #[tokio::test]
    async fn a_dead_first_server_costs_a_moment_not_the_whole_wait() {
        let order = Order::new();
        let start = Instant::now();
        let reply = first_answer(&order, &[DEAD, GOOD], NEXT, fake).await;
        assert_eq!(reply, Some(GOOD));
        assert!(start.elapsed() < WAIT / 2, "took {:?}", start.elapsed());
    }

    /// Once the second answered, it's asked first: the dead one costs
    /// nothing at all after that.
    #[tokio::test]
    async fn the_server_that_answered_is_asked_first() {
        let order = Order::new();
        first_answer(&order, &[DEAD, GOOD], NEXT, fake).await;
        assert_eq!(order.ranked(&[DEAD, GOOD]), vec![GOOD, DEAD]);
        let start = Instant::now();
        let reply = first_answer(&order, &[DEAD, GOOD], NEXT, fake).await;
        assert_eq!(reply, Some(GOOD));
        assert!(start.elapsed() < NEXT, "took {:?}", start.elapsed());
    }

    /// The one that answered most recently comes first; those that never
    /// did keep their order, after the ones that did.
    #[test]
    fn ranked_by_the_latest_answer() {
        let order = Order::new();
        let t0 = Instant::now();
        assert_eq!(order.ranked(&[DEAD, GOOD, FAILS]), vec![DEAD, GOOD, FAILS]);
        order.answered(FAILS, t0);
        order.answered(GOOD, t0 + NEXT);
        assert_eq!(order.ranked(&[DEAD, GOOD, FAILS]), vec![GOOD, FAILS, DEAD]);
        order.answered(FAILS, t0 + WAIT);
        assert_eq!(order.ranked(&[DEAD, GOOD, FAILS]), vec![FAILS, GOOD, DEAD]);
    }

    /// A server that fails at once hands over at once; none answering is
    /// no answer, after the last one's wait.
    #[tokio::test]
    async fn failing_servers_hand_over_at_once() {
        let order = Order::new();
        let start = Instant::now();
        let reply = first_answer(&order, &[FAILS, GOOD], WAIT, fake).await;
        assert_eq!(reply, Some(GOOD));
        assert!(start.elapsed() < NEXT, "took {:?}", start.elapsed());
        assert_eq!(first_answer(&order, &[FAILS], WAIT, fake).await, None);
        assert_eq!(first_answer(&order, &[], WAIT, fake).await, None);
    }
}
