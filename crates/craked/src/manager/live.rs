//! What goes where right now, for the app's Traffic window (D-Bus `Live`):
//! the counters of the computer's devices and each tunnel's, and the open
//! connections (where the system lists them) with the way each takes.
//! Looked at only while the window asks; nothing is kept or logged.

use std::net::IpAddr;

use crake_proto::TunnelState;
use crake_proto::live::{Connection, Live};

use super::Manager;
use crate::flows::Flow;

/// DNS questions: lookups, not traffic worth drawing.
const DNS_PORT: u16 = 53;

impl Manager {
    pub async fn live(&self) -> Live {
        let (physical, tunnels) = self.traffic.now();
        // Only where craked really routes (not its tests' stand-ins).
        let flows = if self.routing.is_real() {
            tokio::task::spawn_blocking(crate::conntrack::open_connections)
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        let connections = flows.map(|flows| {
            let local = crate::probe::Local::load();
            let (full, gateways) = self.full_and_gateways();
            flows
                .into_iter()
                .filter(|(f, _)| shown(f, &local))
                .map(|(f, bytes)| Connection {
                    tunnel: self.way(f.dst, full.as_deref(), &gateways, &local),
                    name: self.name_for(f.dst),
                    src: f.src,
                    sport: f.sport,
                    dst: f.dst,
                    dport: f.dport,
                    udp: f.udp,
                    bytes,
                })
                .collect()
        });
        Live {
            physical,
            tunnels,
            connections,
        }
    }

    /// The full tunnel that's up, if any, and every tunnel's gateway (which
    /// is reached directly, a full tunnel's too).
    pub(super) fn full_and_gateways(&self) -> (Option<String>, Vec<IpAddr>) {
        let tunnels = self.tunnels.lock().unwrap();
        let gateways = tunnels
            .values()
            .filter_map(|e| e.info.as_ref().and_then(|i| i.gateway).map(IpAddr::V4))
            .collect();
        let up: Vec<String> = tunnels
            .iter()
            .filter(|(_, e)| e.state == TunnelState::Connected)
            .map(|(n, _)| n.clone())
            .collect();
        drop(tunnels);
        let full = up
            .into_iter()
            .find(|n| self.learning(n) == crate::policy::Learning::FullTunnel);
        (full, gateways)
    }

    /// The tunnel `dst` goes through: the one that learned or pinned it,
    /// else a full tunnel (not for its gateway or the local network); None
    /// for direct.
    pub(super) fn way(
        &self,
        dst: IpAddr,
        full: Option<&str>,
        gateways: &[IpAddr],
        local: &crate::probe::Local,
    ) -> Option<String> {
        self.owner_of(dst, "").or_else(|| {
            full.filter(|_| !gateways.contains(&dst) && !local.on_link(dst))
                .map(str::to_owned)
        })
    }
}

/// This computer's own connections out: not ones coming in, nothing to
/// itself, no DNS questions.
fn shown(f: &Flow, local: &crate::probe::Local) -> bool {
    local.is_own(f.src)
        && !local.is_own(f.dst)
        && !f.dst.is_loopback()
        && !f.dst.is_multicast()
        && !f.dst.is_unspecified()
        && f.dport != DNS_PORT
}
