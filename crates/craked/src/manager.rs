use std::collections::{BTreeMap, HashMap};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use crake_proto::{
    ENV_SOCKET, ENV_TUNNEL, HookMessage, LogEntry, LogKind, TunnelState, interface_name,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, oneshot};

use crate::auth::Session;
use crate::config::{Config, TunnelConfig};
use crate::dns::{self, Answer};
use crate::hosts::{self, HostStore, Target};
use crate::info::TunnelInfo;
use crate::log::Log;
use crate::net::Net;
use crate::resolved::Resolved;
use crate::routing::{Plan, Routing};
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::RecordType;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub config: TunnelConfig,
    pub interface: String,
    pub state: TunnelState,
    pub info: Option<TunnelInfo>,
    /// Pinned and learned destinations.
    pub hosts: usize,
    /// A kept sign-in: switching on needs no new one.
    pub signed_in: bool,
    /// Why it can't be switched on now (it clashes with a tunnel that's up),
    /// as far as craked knows from its last connection.
    pub clash: Option<String>,
    /// When it came up (seconds since the epoch).
    pub since: Option<u64>,
    /// How its packets travel: DTLS, ESP, TLS (TCP only), UDP or TCP.
    pub transport: Option<String>,
}

/// How to stop openconnect.
#[derive(Debug, Clone, Copy)]
enum Stop {
    /// SIGHUP: disconnect without logging the session off, so it can be
    /// resumed with the same cookie.
    KeepSession,
    /// SIGTERM: log off.
    LogOff,
}

/// One row of a tunnel's host list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRow {
    pub target: Target,
    /// The DNS name it was learned under, if any.
    pub name: Option<String>,
    pub pinned: bool,
    /// Seconds until a learned host goes direct again; `None` when pinned or direct.
    pub expires_in: Option<u64>,
    /// You sent it direct; Crake won't learn it until you redo.
    pub direct: bool,
    /// When and how it was learned ("probe", "dns", "inside", "split", "you").
    pub learned: Option<(u64, String)>,
    /// Seconds since a learned host last carried traffic (its timeout is
    /// refreshed on use), when Crake forgets hosts at all.
    pub used_ago: Option<u64>,
}

struct Entry {
    config: TunnelConfig,
    /// Position in the config; picks the tunnel's mark, rule and table.
    index: usize,
    state: TunnelState,
    info: Option<TunnelInfo>,
    stop: Option<oneshot::Sender<Stop>>,
    /// The last sign-in, reused to switch the tunnel on again.
    session: Option<Session>,
    /// openconnect's supervisor hasn't finished yet (stays true while stopping).
    running: bool,
    /// The kernel sets and rule for this tunnel are in place.
    routed: bool,
    /// Restarted over TCP (UDP blocked): its next "connect" is a return.
    resumed: bool,
    /// When it last came up (seconds since the epoch); kept across a
    /// reconnect, cleared when it goes off.
    up_since: Option<u64>,
    /// Refused because it clashes with a tunnel that's up: it's stopping,
    /// already said why, and goes Off quietly.
    refused: bool,
}

pub struct Manager {
    cfg: Config,
    tunnels: Mutex<BTreeMap<String, Entry>>,
    hosts: Mutex<HostStore>,
    /// Networks where UDP doesn't get through.
    networks: Mutex<crate::networks::Networks>,
    /// What the user set in the Settings window (`settings.json`).
    settings: Mutex<crate::settings::Settings>,
    /// One hush update at a time; the REFUSED answerer runs once.
    hush: tokio::sync::Mutex<()>,
    hush_serving: std::sync::atomic::AtomicBool,
    /// Serializes host changes so the store and the kernel sets move together.
    ops: tokio::sync::Mutex<()>,
    log: Log,
    net: Net,
    routing: Routing,
    resolved: Resolved,
    traffic: Arc<crate::traffic::Traffic>,
    /// How each tunnel's packets travel, as openconnect (or the app, for
    /// OpenVPN 3) last said: tunnel -> "DTLS", "ESP", "TLS", "UDP", "TCP".
    transport: Arc<Mutex<HashMap<String, String>>>,
    /// Names no tunnel knew lately; not asked again for a while.
    unknown: Mutex<HashMap<(String, RecordType), Instant>>,
    /// Addresses another tunnel's DNS also gave (tunnel, address), when
    /// last said: once per 10 min, not on every lookup.
    contested: Mutex<HashMap<(String, Ipv4Addr), Instant>>,
    /// "Ask me first" questions lately (tunnel, host): not asked again soon.
    asked: Mutex<HashMap<(String, Ipv4Addr), Instant>>,
    /// Names under an organisation's domain whose tunnel DNS had no private
    /// address lately; not asked again for a while.
    no_split: Mutex<HashMap<String, Instant>>,
    /// Hosts probed lately (or being probed): not probed again for a while.
    probed: Mutex<HashMap<Ipv4Addr, Instant>>,
    /// Names from recent public DNS answers, so decisions can say a name.
    seen_names: Mutex<HashMap<Ipv4Addr, (String, Instant)>>,
    changes: broadcast::Sender<String>,
}

/// How long a name no tunnel knew stays unasked.
const UNKNOWN_FOR: Duration = Duration::from_secs(300);
/// How long a probed host stays unprobed.
const PROBED_FOR: Duration = Duration::from_secs(600);
/// How long a probe through one tunnel may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a name from a public answer is remembered.
const SEEN_NAME_FOR: Duration = Duration::from_secs(3600);
/// How long to wait for one tunnel DNS server.
const TUNNEL_DNS_WAIT: Duration = Duration::from_millis(800);

/// Why a DNS answer sent a host through a tunnel.
#[derive(Clone, Copy)]
enum Why {
    /// The name is under the tunnel's domains.
    TunnelDomain,
    /// Public DNS said it doesn't exist; the tunnel's DNS knew it.
    PublicFailed,
    /// Public DNS gave a public address; the tunnel's DNS knows a private one
    /// (split-horizon DNS, e.g. a service that only lets the company in).
    PrivateAddress,
}

impl Manager {
    pub fn new(cfg: Config) -> Result<Arc<Self>> {
        let log = Log::open(&cfg.log_path)?;
        // The config's and IT's tunnels, then those added in the app.
        let mut all = cfg.tunnels.clone();
        for own in crate::own::load(&crate::own::path_for(&cfg.state_path)) {
            if all.len() < crate::routing::MAX_TUNNELS && !all.iter().any(|t| t.name == own.name) {
                all.push(own.to_config());
            }
        }
        let tunnels = all
            .iter()
            .enumerate()
            .map(|(index, t)| {
                let e = Entry {
                    config: t.clone(),
                    index,
                    state: TunnelState::Off,
                    info: None,
                    stop: None,
                    session: None,
                    running: false,
                    routed: false,
                    resumed: false,
                    up_since: None,
                    refused: false,
                };
                (t.name.clone(), e)
            })
            .collect();
        let net = Net::new(cfg.apply_network);
        let routing = Routing::new(cfg.apply_network);
        let resolved = Resolved::new(cfg.apply_network && cfg.use_resolved);
        let hosts = HostStore::load(&cfg.state_path);
        let networks =
            crate::networks::Networks::load(&cfg.state_path.with_file_name("networks.json"));
        let settings =
            crate::settings::Settings::load(&crate::settings::path_for(&cfg.state_path), &cfg);
        log.keep(settings.log_days);
        let (changes, _) = broadcast::channel(64);
        Ok(Arc::new(Self {
            cfg,
            tunnels: Mutex::new(tunnels),
            hosts: Mutex::new(hosts),
            networks: Mutex::new(networks),
            settings: Mutex::new(settings),
            hush: tokio::sync::Mutex::new(()),
            hush_serving: std::sync::atomic::AtomicBool::new(false),
            ops: tokio::sync::Mutex::new(()),
            log,
            net,
            routing,
            resolved,
            traffic: Arc::default(),
            transport: Arc::default(),
            unknown: Mutex::new(HashMap::new()),
            contested: Mutex::new(HashMap::new()),
            asked: Mutex::new(HashMap::new()),
            no_split: Mutex::new(HashMap::new()),
            probed: Mutex::new(HashMap::new()),
            seen_names: Mutex::new(HashMap::new()),
            changes,
        }))
    }

    /// Clear routing state a previous run may have left behind, and start
    /// listening for DNS answers from tunnels.
    pub async fn init(self: &Arc<Self>) -> Result<()> {
        self.routing.reset_all().await?;
        // Old log entries go as they pass the retention (Settings).
        let this = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let days = this.settings.lock().unwrap().log_days;
                if let Some(d) = days {
                    this.log.prune(d);
                }
                tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
            }
        });
        self.traffic
            .spawn(self.cfg.state_path.with_file_name("traffic.json"));
        if let Routing::Real = self.routing
            && let Err(e) = crate::dns_queue::spawn(Arc::clone(self))
        {
            // Not fatal: names still resolve, they're just not routed ahead.
            tracing::error!("DNS learning is off: {e:#}");
        }
        if let Routing::Real = self.routing
            && self.cfg.probing
            && let Err(e) = crate::probe::spawn(Arc::clone(self))
        {
            tracing::error!("connection probing is off: {e:#}");
        }
        // Tunnels start off: their remembered domains stay private.
        self.update_hush().await;
        Ok(())
    }

    /// The domains whose names belong to a tunnel: pushed by the gateway, plus
    /// any from the config.
    fn domains(&self, name: &str) -> Vec<String> {
        let map = self.tunnels.lock().unwrap();
        let Some(e) = map.get(name) else {
            return Vec::new();
        };
        let pushed = e.info.iter().flat_map(|i| i.domains.iter());
        let mut out: Vec<String> = Vec::new();
        for d in pushed.chain(e.config.dns_domains.iter()) {
            let d = dns::normalize(d);
            if dns::valid_domain(&d) && !out.contains(&d) {
                out.push(d);
            }
        }
        out
    }

    /// How long a learned host stays without traffic, for this tunnel: its
    /// own setting (New tunnel, step 3), or Settings'.
    fn forget_after(&self, name: &str) -> Option<u64> {
        let own = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .and_then(|e| e.config.own.as_ref().and_then(|o| o.forget_after_days));
        own.unwrap_or_else(|| self.settings.lock().unwrap().forget_after_days)
    }

    fn timeout_for(&self, name: &str) -> u64 {
        self.forget_after(name).unwrap_or(3650) * 86_400
    }

    /// How a learned host leaves again, in a sentence.
    fn expiry_sentence(&self, name: &str) -> String {
        match self.forget_after(name) {
            Some(1) => "It goes direct again after a day without traffic.".to_owned(),
            Some(d) => format!("It goes direct again after {d} days without traffic."),
            None => "It stays until you forget it.".to_owned(),
        }
    }

    /// Where a tunnel comes in the try order (Settings), then config order.
    fn rank(&self, name: &str, index: usize) -> usize {
        let settings = self.settings.lock().unwrap();
        settings
            .order
            .iter()
            .position(|n| n == name)
            .unwrap_or(settings.order.len() + index)
    }

    /// On the "never send through a tunnel" list?
    fn never(&self, name: Option<&str>, addr: Ipv4Addr) -> bool {
        self.settings.lock().unwrap().never().covers(name, addr)
    }

    /// The tunnel is set to "Ask me first".
    fn asks(&self, name: &str) -> bool {
        self.tunnels
            .lock()
            .unwrap()
            .get(name)
            .is_some_and(|e| e.config.asks())
    }

    /// "Ask me first": say what was found and route nothing; the app offers
    /// "Send through …". Not asked again about a host for a while.
    fn ask_first(&self, name: &str, label: &str, shown: &str, why: String, addrs: &[Ipv4Addr]) {
        let fresh: Vec<Ipv4Addr> = {
            let mut asked = self.asked.lock().unwrap();
            asked.retain(|_, at| at.elapsed() < PROBED_FOR);
            addrs
                .iter()
                .copied()
                .filter(|a| {
                    asked
                        .insert((name.to_owned(), *a), Instant::now())
                        .is_none()
                })
                .collect()
        };
        if fresh.is_empty() {
            return;
        }
        self.record_hosts(
            LogKind::Asked,
            name,
            &format!("{shown} needs {label}."),
            format!("{why} You asked to decide these yourself."),
            fresh.iter().map(Ipv4Addr::to_string).collect(),
        );
    }

    /// Every setting, in its text form (the D-Bus API).
    pub fn settings(&self) -> std::collections::HashMap<String, String> {
        self.settings.lock().unwrap().to_map()
    }

    /// Change one setting; it takes effect at once.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let (expiry_changed, log_days) = {
            let mut s = self.settings.lock().unwrap();
            let before = s.expiry_secs();
            s.set(key, value)?;
            s.save(&crate::settings::path_for(&self.cfg.state_path));
            (s.expiry_secs() != before, s.log_days)
        };
        match key {
            "log_days" => self.log.keep(log_days),
            // Learned hosts' timeouts live in the kernel sets: reload them.
            "forget_after_days" if expiry_changed => {
                let routed: Vec<(String, String, TunnelInfo)> = {
                    let map = self.tunnels.lock().unwrap();
                    map.values()
                        .filter(|e| e.routed)
                        .filter_map(|e| {
                            Some((
                                e.config.name.clone(),
                                e.config.display().to_owned(),
                                e.info.clone()?,
                            ))
                        })
                        .collect()
                };
                for (name, label, info) in routed {
                    if let Err(e) = self.apply_routing(&name, &label, &info).await {
                        tracing::error!("reloading {name} for the new expiry: {e:#}");
                    }
                }
            }
            "ask_unknown" => self.unknown.lock().unwrap().clear(),
            "keep_private" => self.update_hush().await,
            _ => {}
        }
        let _ = self.changes.send(String::new());
        Ok(())
    }

    /// "Keep internal names private": the domains of tunnels that aren't up
    /// go to the hush link (see `hush`); with none, or the setting off, it goes.
    pub async fn update_hush(&self) {
        if !(self.cfg.apply_network && self.cfg.use_resolved) {
            return;
        }
        let private = self.settings.lock().unwrap().keep_private;
        let domains: Vec<String> = if private {
            let map = self.tunnels.lock().unwrap();
            let store = self.hosts.lock().unwrap();
            let mut v: Vec<String> = Vec::new();
            for e in map.values().filter(|e| !e.routed) {
                let saved = store.tunnels.get(&e.config.name).map(|t| t.domains.clone());
                for d in saved
                    .unwrap_or_default()
                    .iter()
                    .chain(&e.config.dns_domains)
                {
                    let d = dns::normalize(d);
                    if dns::valid_domain(&d) && !v.contains(&d) {
                        v.push(d);
                    }
                }
            }
            v
        } else {
            Vec::new()
        };
        let _hush = self.hush.lock().await;
        if domains.is_empty() {
            self.resolved.revert(crate::hush::DEVICE).await;
            self.net.remove(crate::hush::DEVICE).await;
            return;
        }
        if let Err(e) = self.net.dummy(crate::hush::DEVICE, crate::hush::ADDR).await {
            tracing::error!("keeping internal names private: {e:#}");
            return;
        }
        if !self
            .hush_serving
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            tokio::spawn(async {
                if let Err(e) = crate::hush::serve().await {
                    tracing::error!("answering for private names: {e:#}");
                }
            });
        }
        if let Err(e) = self
            .resolved
            .set_link(crate::hush::DEVICE, &[crate::hush::ADDR], &domains, false)
            .await
        {
            tracing::error!("keeping internal names private: {e:#}");
        }
        self.resolved.flush().await;
    }

    /// The tunnels added in the app, as they stand.
    fn own_tunnels(&self) -> Vec<crate::own::OwnTunnel> {
        let map = self.tunnels.lock().unwrap();
        let mut v: Vec<(usize, crate::own::OwnTunnel)> = map
            .values()
            .filter_map(|e| Some((e.index, e.config.own.clone()?)))
            .collect();
        v.sort_by_key(|(i, _)| *i);
        v.into_iter().map(|(_, o)| o).collect()
    }

    fn save_own(&self) {
        crate::own::save(
            &crate::own::path_for(&self.cfg.state_path),
            &self.own_tunnels(),
        );
    }

    /// New tunnel: add one of the user's own. Returns its name.
    pub fn add_tunnel(&self, fields: &HashMap<String, String>) -> Result<String> {
        let name = {
            let map = self.tunnels.lock().unwrap();
            let taken: Vec<String> = map.keys().cloned().collect();
            crate::own::name_for(
                fields.get("display_name").map_or("", |s| s.as_str()),
                &taken,
            )
        };
        let own = crate::own::OwnTunnel::from_map(&name, fields)?;
        {
            let mut map = self.tunnels.lock().unwrap();
            if map.len() >= crate::routing::MAX_TUNNELS {
                bail!("Crake can hold {} tunnels.", crate::routing::MAX_TUNNELS);
            }
            // Its own mark, rule and table: the first index nobody uses.
            let used: Vec<usize> = map.values().map(|e| e.index).collect();
            let index = (0..).find(|i| !used.contains(i)).unwrap_or(0);
            map.insert(
                name.clone(),
                Entry {
                    config: own.to_config(),
                    index,
                    state: TunnelState::Off,
                    info: None,
                    stop: None,
                    session: None,
                    running: false,
                    routed: false,
                    resumed: false,
                    up_since: None,
                    refused: false,
                },
            );
        }
        self.save_own();
        let _ = self.changes.send(name.clone());
        self.log.system(
            Some(&name),
            format!("Added {}.", own.display_name),
            Some(format!("{} at {}.", own.protocol.as_str(), own.gateway)),
        );
        Ok(name)
    }

    /// Edit one of the user's own tunnels. Sign-in changes apply the next
    /// time it connects; what may go through it, at once.
    pub async fn update_tunnel(&self, name: &str, fields: &HashMap<String, String>) -> Result<()> {
        let own = crate::own::OwnTunnel::from_map(name, fields)?;
        let (was, routed) = {
            let mut map = self.tunnels.lock().unwrap();
            let e = map
                .get_mut(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if e.config.own.is_none() {
                bail!(
                    "{} isn't one you added here, so it can't be changed here.",
                    e.config.display()
                );
            }
            let was = e.config.learning();
            e.config = own.to_config();
            (was, e.routed)
        };
        self.save_own();
        // A new scope or a full tunnel: rebuild its routing now.
        if routed && was != own.learning() {
            let info = self
                .tunnels
                .lock()
                .unwrap()
                .get(name)
                .and_then(|e| e.info.clone());
            if let Some(info) = info {
                self.apply_routing(name, &own.display_name, &info).await?;
            }
        }
        let _ = self.changes.send(name.to_owned());
        self.log
            .system(Some(name), format!("Changed {}.", own.display_name), None);
        Ok(())
    }

    /// Remove one of the user's own tunnels: switched off first, its hosts
    /// forgotten. The log keeps its history.
    pub async fn remove_tunnel(&self, name: &str) -> Result<()> {
        let label = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if e.config.own.is_none() {
                bail!(
                    "{} isn't one you added here, so it can't be removed here.",
                    e.config.display()
                );
            }
            e.config.display().to_owned()
        };
        // Off first: log off, so no session is left behind on the gateway.
        let stop = self.tunnels.lock().unwrap().get_mut(name).and_then(|e| {
            e.session = None;
            e.stop.take()
        });
        if let Some(tx) = stop {
            let _ = tx.send(Stop::LogOff);
        }
        for _ in 0..150 {
            let running = self
                .tunnels
                .lock()
                .unwrap()
                .get(name)
                .is_some_and(|e| e.running);
            if !running {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .is_some_and(|e| e.running)
        {
            bail!("{label} didn't switch off. Try again in a moment.");
        }
        self.tunnels.lock().unwrap().remove(name);
        {
            let mut store = self.hosts.lock().unwrap();
            store.tunnels.remove(name);
            store.save();
        }
        {
            let mut settings = self.settings.lock().unwrap();
            settings.order.retain(|n| n != name);
            settings.save(&crate::settings::path_for(&self.cfg.state_path));
        }
        self.save_own();
        self.update_hush().await;
        let _ = self.changes.send(name.to_owned());
        self.log.system(
            Some(name),
            format!("Removed {label}."),
            Some("What Crake learned for it went too.".into()),
        );
        Ok(())
    }

    /// One of the user's own tunnels in text form, for editing.
    pub fn own_fields(&self, name: &str) -> Option<BTreeMap<String, String>> {
        let map = self.tunnels.lock().unwrap();
        Some(map.get(name)?.config.own.as_ref()?.to_map())
    }

    /// Empty the log. Learned hosts stay.
    pub fn clear_log(&self) {
        self.log.clear();
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Names of tunnels whose state changed.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.changes.subscribe()
    }

    pub fn list(&self) -> Vec<Snapshot> {
        let mut list: Vec<Snapshot> = self
            .tunnels
            .lock()
            .unwrap()
            .values()
            .map(snapshot)
            .collect();
        let transport = self.transport.lock().unwrap();
        for s in &mut list {
            s.transport = transport
                .get(&s.config.name)
                .filter(|_| s.state == TunnelState::Connected)
                .cloned();
        }
        drop(transport);
        for s in &mut list {
            if matches!(s.state, TunnelState::Off | TunnelState::Trouble) {
                s.clash = self.remembered_clash(&s.config.name);
            }
        }
        let store = self.hosts.lock().unwrap();
        for s in &mut list {
            s.hosts = store
                .tunnels
                .get(&s.config.name)
                .map_or(0, |t| t.pinned.len() + t.learned.len());
        }
        list
    }

    pub fn get(&self, name: &str) -> Option<Snapshot> {
        self.list().into_iter().find(|s| s.config.name == name)
    }

    /// A tunnel that's up and can't be up together with `name`, and why:
    /// they share a DNS server (craked routes each server through its own
    /// tunnel), their own networks overlap, or a network is pinned to both.
    fn clash(
        &self,
        name: &str,
        dns: &[Ipv4Addr],
        net: Option<Target>,
    ) -> Option<(String, String)> {
        // The tunnels that are up, copied out: one lock at a time.
        let up: Vec<(String, String, TunnelInfo)> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(other, e)| *other != name && e.state == TunnelState::Connected)
            .filter_map(|(other, e)| {
                e.info
                    .clone()
                    .map(|i| (other.clone(), e.config.display().to_owned(), i))
            })
            .collect();
        let pinned_of = |t: &str| -> Vec<Target> {
            self.hosts
                .lock()
                .unwrap()
                .tunnels
                .get(t)
                .map(|h| h.pinned.iter().copied().collect())
                .unwrap_or_default()
        };
        let pinned = pinned_of(name);
        for (other, label, info) in up {
            if let Some(d) = dns.iter().find(|d| info.dns.contains(d)) {
                return Some((label, format!("Both use {d} for DNS.")));
            }
            if let (Some(a), Some(b)) = (net, own_net(&info))
                && overlap(a, b)
            {
                let wider = if a.prefix_len() <= b.prefix_len() { a } else { b };
                return Some((label, format!("Both are on {wider}.")));
            }
            let theirs = pinned_of(&other);
            if let Some(p) = pinned
                .iter()
                .find(|p| theirs.iter().any(|q| overlap(**p, *q)))
            {
                return Some((label, format!("{p} is pinned to both.")));
            }
        }
        None
    }

    /// The tunnel that's up and already sends `addr` through itself
    /// (learned or pinned), other than `except`.
    fn owner_of(&self, addr: Ipv4Addr, except: &str) -> Option<String> {
        let up: Vec<String> = self
            .tunnels
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, e)| *n != except && e.state == TunnelState::Connected)
            .map(|(n, _)| n.clone())
            .collect();
        let store = self.hosts.lock().unwrap();
        up.into_iter()
            .find(|t| store.tunnels.get(t).is_some_and(|h| h.covers(addr)))
    }

    /// Say (once per 10 min) that `host` on `tunnel` has an address that
    /// already goes through `owner`, and offer to move it.
    fn contested(&self, tunnel: &str, label: &str, owner: &str, host: &str, addr: Ipv4Addr) {
        {
            let mut seen = self.contested.lock().unwrap();
            let key = (tunnel.to_owned(), addr);
            if seen
                .get(&key)
                .is_some_and(|t| t.elapsed() < Duration::from_secs(600))
            {
                return;
            }
            seen.insert(key, Instant::now());
        }
        let owner_label = self.label(owner);
        let theirs = th_name(&self.hosts.lock().unwrap(), owner, addr)
            .map(|n| format!(" ({n})"))
            .unwrap_or_default();
        self.log.append(LogEntry {
            ts: crate::log::now(),
            kind: LogKind::System,
            tunnel: Some(owner.to_owned()),
            message: format!("{host} can't go through {label}."),
            reason: Some(format!(
                "Its address, {addr}, already goes through {owner_label}{theirs}. One address goes through one tunnel at a time; you can move it to {label}."
            )),
            hosts: vec![addr.to_string()],
            others: vec![tunnel.to_owned()],
        });
    }

    fn label(&self, name: &str) -> String {
        self.tunnels
            .lock()
            .unwrap()
            .get(name)
            .map_or_else(|| name.to_owned(), |e| e.config.display().to_owned())
    }

    /// `clash` with what `name` got when it was last up, before it's
    /// switched on again: said before signing in, not after.
    fn remembered_clash(&self, name: &str) -> Option<String> {
        let (dns, net) = {
            let store = self.hosts.lock().unwrap();
            let t = store.tunnels.get(name)?;
            (t.last_dns.clone(), t.last_net)
        };
        let display = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .map(|e| e.config.display().to_owned())?;
        self.clash(name, &dns, net)
            .map(|(other, why)| format!("Turn {other} off to use {display}. {why}"))
    }

    /// The tunnel whose device this is: `crake-<name>` for openconnect's,
    /// the attached device (tun0, ...) for OpenVPN 3's.
    pub fn tunnel_of_device(&self, device: &str) -> Option<String> {
        if let Some(name) = device.strip_prefix("crake-") {
            return Some(name.to_owned());
        }
        self.tunnels
            .lock()
            .unwrap()
            .iter()
            .find(|(_, e)| e.info.as_ref().is_some_and(|i| i.device == device))
            .map(|(name, _)| name.clone())
    }

    /// Bytes through `name` per minute, two days (tunnel detail's chart).
    pub fn tunnel_traffic(&self, name: &str) -> Vec<(u64, u64)> {
        self.traffic.tunnel(name)
    }

    pub fn log(&self) -> &Log {
        &self.log
    }

    /// Direct traffic per minute, last hour: (minute since the epoch, bytes).
    pub fn traffic(&self) -> Vec<(u64, u64)> {
        self.traffic.last_hour()
    }

    /// All traffic per minute, two days.
    pub fn traffic_days(&self) -> Vec<(u64, u64)> {
        self.traffic.days()
    }

    fn set_state(&self, name: &str, state: TunnelState, clear_info: bool) {
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.state = state;
            if clear_info {
                if let Some(info) = e.info.take() {
                    self.traffic.untrack(&info.device);
                }
                e.up_since = None;
            }
        }
        let _ = self.changes.send(name.to_owned());
    }

    pub fn connect(self: &Arc<Self>, name: &str) -> Result<()> {
        self.connect_with(name, None)
    }

    /// Connect with a session from a separate sign-in ([`crate::auth`]).
    pub fn connect_with(self: &Arc<Self>, name: &str, session: Option<Session>) -> Result<()> {
        let (config, stop_rx, session) = {
            let mut map = self.tunnels.lock().unwrap();
            let e = map
                .get_mut(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if matches!(e.state, TunnelState::Connecting | TunnelState::Connected) {
                bail!("{} is already on", e.config.display());
            }
            // OpenVPN 3 sessions are the user's: the app starts them.
            if e.config.protocol == crate::config::Protocol::Openvpn3 {
                bail!("{} is switched on from the app.", e.config.display());
            }
            let (tx, rx) = oneshot::channel();
            if e.running {
                bail!("{} is still switching off", e.config.display());
            }
            e.stop = Some(tx);
            e.running = true;
            e.state = TunnelState::Connecting;
            e.info = None;
            // A new sign-in replaces the kept one; none given, resume the kept one.
            if session.is_some() {
                e.session = session.clone();
            }
            let session = session.or_else(|| e.session.clone());
            (e.config.clone(), rx, session)
        };
        let _ = self.changes.send(name.to_owned());
        self.log.system(
            Some(name),
            format!("Switched {} on.", config.display()),
            None,
        );
        let this = Arc::clone(self);
        tokio::spawn(async move { this.supervise(config, session, stop_rx).await });
        Ok(())
    }

    pub fn disconnect(&self, name: &str) -> Result<()> {
        let mut map = self.tunnels.lock().unwrap();
        let e = map
            .get_mut(name)
            .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
        // With a kept session, switch off without logging off, so switching
        // on again needs no new sign-in (unless the tunnel says otherwise).
        let keep = e.config.own.as_ref().is_none_or(|o| o.keep_signed_in);
        let how = if e.session.is_some() && keep {
            Stop::KeepSession
        } else {
            Stop::LogOff
        };
        match e.stop.take() {
            Some(tx) => {
                let _ = tx.send(how);
                Ok(())
            }
            // Trouble or Off: nothing is running; just settle to Off.
            None => {
                e.state = TunnelState::Off;
                drop(map);
                let _ = self.changes.send(name.to_owned());
                Ok(())
            }
        }
    }

    /// Stop every running tunnel and wait for them to finish (daemon shutdown).
    pub async fn shutdown(&self) {
        self.traffic
            .save(&self.cfg.state_path.with_file_name("traffic.json"));
        {
            // Leaving: log every session off.
            let mut map = self.tunnels.lock().unwrap();
            for e in map.values_mut() {
                e.session = None;
                if let Some(tx) = e.stop.take() {
                    let _ = tx.send(Stop::LogOff);
                }
            }
        }
        for _ in 0..120 {
            let busy = self.tunnels.lock().unwrap().values().any(|e| e.running);
            if !busy {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        // Leave nothing behind: Crake off is Crake gone.
        if self.cfg.apply_network && self.cfg.use_resolved {
            self.resolved.revert(crate::hush::DEVICE).await;
            self.net.remove(crate::hush::DEVICE).await;
        }
    }

    /// A tunnel's device is up with this info (openconnect's hook, or an
    /// OpenVPN 3 session attached by the app): DNS routes, routing, DNS,
    /// state, and a line in the log.
    async fn tunnel_up(
        &self,
        name: &str,
        display: &str,
        info: TunnelInfo,
        first: bool,
    ) -> Result<()> {
        let display = display.to_owned();
        // What it got this time, for the next check before switching on.
        {
            let mut store = self.hosts.lock().unwrap();
            let t = store.tunnel(name);
            t.last_dns = info.dns.clone();
            t.last_net = own_net(&info);
            store.save();
        }
        // Two tunnels that share a DNS server, a network or a pinned network
        // can't both work: don't half-work, say which to switch off.
        if let Some((other, why)) = self.clash(name, &info.dns, own_net(&info)) {
            let text = format!("Turn {other} off to use {display}.");
            self.log.system(Some(name), text.clone(), Some(why.clone()));
            let stop = {
                let mut map = self.tunnels.lock().unwrap();
                map.get_mut(name).and_then(|e| {
                    e.refused = true;
                    e.stop.take()
                })
            };
            return match stop {
                // openconnect: stop it, keeping the sign-in for later.
                Some(tx) => {
                    let _ = tx.send(Stop::KeepSession);
                    Ok(())
                }
                // An OpenVPN 3 session: the app ends it.
                None => {
                    if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
                        e.refused = false;
                    }
                    self.set_state(name, TunnelState::Off, true);
                    Err(anyhow!("{text} {why}"))
                }
            };
        }
        for dns in &info.dns {
            self.net.route_dns(&info.device, *dns).await?;
        }
        let routed = self.apply_routing(name, &display, &info).await?;
        let mut first = first;
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.info = Some(info.clone());
            first &= !std::mem::take(&mut e.resumed);
            if first || e.up_since.is_none() {
                e.up_since = Some(hosts::now());
            }
            self.traffic.track(&info.device, name);
        }
        let domains = self.domains(name);
        self.register_dns(name, &display, &info, &domains).await;
        // Remember them, to keep them private while it's off.
        {
            let mut store = self.hosts.lock().unwrap();
            store.tunnel(name).domains = domains.clone();
            store.save();
        }
        self.update_hush().await;
        let summary = describe(&info, &domains);
        self.set_state(name, TunnelState::Connected, false);
        let full = self.learning(name) == crate::policy::Learning::FullTunnel;
        let text = match (first, routed) {
            (false, _) => format!("{display} is back."),
            (true, _) if full => {
                format!("{display} is up. Everything goes through it (full tunnel).")
            }
            (true, 0) => format!("{display} is up. Routing nothing yet."),
            (true, 1) => {
                format!("{display} is up. Sending 1 known destination through it.")
            }
            (true, n) => {
                format!("{display} is up. Sending {n} known destinations through it.")
            }
        };
        self.log.system(Some(name), text, Some(summary));
        Ok(())
    }

    /// OpenVPN 3: the app started the session (as the user, who owns the
    /// profile) and hands over its device and DNS. Routing works as for
    /// openconnect tunnels from here.
    pub async fn attach(&self, name: &str, fields: &HashMap<String, String>) -> Result<()> {
        let display = {
            let mut map = self.tunnels.lock().unwrap();
            let e = map
                .get_mut(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            if e.config.protocol != crate::config::Protocol::Openvpn3 {
                bail!("{} isn't an OpenVPN 3 tunnel", e.config.display());
            }
            // Already routed through this device (the app taking the
            // session back after a restart): nothing to do.
            if e.state == TunnelState::Connected
                && e.info.as_ref().map(|i| i.device.as_str()) == fields.get("device").map(String::as_str)
            {
                return Ok(());
            }
            e.state = TunnelState::Connecting;
            e.config.display().to_owned()
        };
        let device = fields.get("device").map(String::as_str).unwrap_or("");
        // Only a real tun device of this machine.
        if device.is_empty()
            || !device
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            || !std::path::Path::new(&format!("/sys/class/net/{device}/tun_flags")).exists()
                && self.cfg.apply_network
        {
            bail!("{device:?} isn't a VPN device on this machine");
        }
        let list = |k: &str| -> Vec<String> {
            fields
                .get(k)
                .map(|v| {
                    v.split([',', '\n', ' '])
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut dns: Vec<Ipv4Addr> = list("dns").iter().filter_map(|a| a.parse().ok()).collect();
        let mut domains = list("domains");
        // The app can't read openvpn3's network settings (netcfg is closed
        // to users): take what openvpn3 gave resolved for the device, which
        // is what the server pushed. A moment's grace: it may still be
        // setting it.
        if dns.is_empty() {
            for _ in 0..10 {
                if let Ok((servers, names)) = self.resolved.link(device).await
                    && !servers.is_empty()
                {
                    dns = servers;
                    if domains.is_empty() {
                        domains = names;
                    }
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
        if let Some(how) = fields.get("transport").filter(|t| !t.is_empty()) {
            self.transport
                .lock()
                .unwrap()
                .insert(name.to_owned(), how.to_ascii_uppercase());
        }
        let own = crate::probe::ipv4_of(device);
        let info = TunnelInfo {
            device: device.to_owned(),
            gateway: fields.get("gateway").and_then(|g| g.parse().ok()),
            address: own.map(|(a, _)| a),
            prefix: own.map(|(_, p)| p),
            mtu: None,
            dns,
            domains: domains
                .into_iter()
                .map(|d| dns::normalize(&d))
                .filter(|d| dns::valid_domain(d))
                .collect(),
            split_include: list("routes"),
            split_exclude: Vec::new(),
        };
        self.tunnel_up(name, &display, info, true).await
    }

    /// OpenVPN 3: the session ended (switched off in the app, or it dropped).
    pub async fn detach(&self, name: &str, failed: Option<String>) -> Result<()> {
        let label = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .map(|e| e.config.display().to_owned())
            .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
        // Refused at attach (it clashed): it never came up, nothing to undo.
        let never_up = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .is_some_and(|e| e.state == TunnelState::Off && e.info.is_none());
        if never_up {
            return Ok(());
        }
        self.remove_routing(name).await;
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.info = None;
        }
        let state = if failed.is_some() {
            TunnelState::Trouble
        } else {
            TunnelState::Off
        };
        self.set_state(name, state, true);
        self.update_hush().await;
        let msg = match state {
            TunnelState::Trouble => format!("{label} stopped unexpectedly."),
            _ => format!("Switched {label} off."),
        };
        self.log.system(Some(name), msg, failed);
        Ok(())
    }

    /// Apply one call from the hook. An error makes the hook fail, which makes
    /// openconnect give up on that step.
    pub async fn handle_hook(&self, msg: HookMessage) -> Result<()> {
        let (display, state) = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(&msg.tunnel)
                .ok_or_else(|| anyhow!("unknown tunnel {}", msg.tunnel))?;
            (e.config.display().to_owned(), e.state)
        };
        // Only a tunnel Crake started may report in.
        if state == TunnelState::Off || state == TunnelState::Trouble {
            bail!("{display} is not running");
        }
        let name = msg.tunnel.as_str();

        match msg.reason.as_str() {
            "pre-init" => Ok(()),
            "connect" | "reconnect" => {
                let info = TunnelInfo::from_env(&msg.env)?;
                let expected = interface_name(name);
                if info.device != expected {
                    bail!("device {} does not belong to {display}", info.device);
                }
                if let Some(addr) = info.address {
                    self.net
                        .assign(&info.device, addr, info.prefix.unwrap_or(32), info.mtu)
                        .await?;
                }
                self.tunnel_up(name, &display, info, msg.reason == "connect")
                    .await
            }
            "attempt-reconnect" => {
                self.set_state(name, TunnelState::Connecting, false);
                self.log.system(
                    Some(name),
                    format!("{display} lost the gateway. Trying again."),
                    None,
                );
                Ok(())
            }
            "disconnect" => Ok(()), // the supervisor logs the end once the process exits
            other => {
                tracing::debug!(reason = other, "ignoring hook reason");
                Ok(())
            }
        }
    }

    async fn supervise(
        self: Arc<Self>,
        config: TunnelConfig,
        session: Option<Session>,
        mut stop_rx: oneshot::Receiver<Stop>,
    ) {
        let name = config.name.clone();
        let label = config.display().to_owned();
        // Where UDP is known not to get through, start without DTLS (7b).
        let network = crate::networks::current();
        let configured = config.extra_args.iter().any(|a| a == "--no-dtls");
        let mut no_dtls = configured
            || network
                .as_deref()
                .is_some_and(|n| self.networks.lock().unwrap().udp_blocked(n));
        let (state, reason) = loop {
            let mut child = match self
                .spawn_openconnect(&config, session.clone(), no_dtls)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("{label}: {e:#}");
                    break (TunnelState::Trouble, Some(format!("{e:#}")));
                }
            };
            let last_line = Arc::new(Mutex::new(String::new()));
            let (udp_tx, mut udp_rx) = tokio::sync::mpsc::unbounded_channel();
            {
                // Over TCP from the start; otherwise openconnect says.
                let mut transport = self.transport.lock().unwrap();
                if no_dtls {
                    transport.insert(name.clone(), "TLS".to_owned());
                } else {
                    transport.remove(&name);
                }
            }
            let output = Output {
                tunnel: name.clone(),
                last: Arc::clone(&last_line),
                udp_failed: udp_tx,
                transport: Arc::clone(&self.transport),
            };
            forward_output(&mut child, output);

            tokio::select! {
                status = child.wait() => break match status {
                    Ok(s) if s.success() => (TunnelState::Off, None),
                    Ok(s) => {
                        let tail = last_line.lock().unwrap().clone();
                        (TunnelState::Trouble, Some(format!("openconnect exited with {s}. {tail}").trim().to_owned()))
                    }
                    Err(e) => (TunnelState::Trouble, Some(e.to_string())),
                },
                how = &mut stop_rx => {
                    let signal = match how {
                        Ok(Stop::KeepSession) => libc::SIGHUP,
                        _ => libc::SIGTERM,
                    };
                    terminate(&mut child, signal).await;
                    break (TunnelState::Off, None);
                }
                Some(()) = udp_rx.recv(), if !no_dtls => {
                    // UDP doesn't get through here: remember the network and
                    // carry on over TCP at once, keeping the sign-in.
                    if let Some(n) = &network {
                        self.networks.lock().unwrap().block_udp(n);
                    }
                    let can_resume = session.is_some() || config.passwd_file.is_some();
                    let why = "Its UDP channel (DTLS) didn\u{2019}t get through on this network.";
                    if !can_resume {
                        self.log.system(
                            Some(&name),
                            "UDP is blocked here.".to_owned(),
                            Some(format!("{why} Next time {label} starts over TCP here.")),
                        );
                        // openconnect carries on over HTTPS by itself.
                        let (state, reason) = tokio::select! {
                            status = child.wait() => match status {
                                Ok(s) if s.success() => (TunnelState::Off, None),
                                Ok(s) => (TunnelState::Trouble, Some(format!("openconnect exited with {s}."))),
                                Err(e) => (TunnelState::Trouble, Some(e.to_string())),
                            },
                            how = &mut stop_rx => {
                                let signal = match how {
                                    Ok(Stop::KeepSession) => libc::SIGHUP,
                                    _ => libc::SIGTERM,
                                };
                                terminate(&mut child, signal).await;
                                (TunnelState::Off, None)
                            }
                        };
                        break (state, reason);
                    }
                    self.log.system(
                        Some(&name),
                        format!("UDP is blocked here. {label} runs over TCP on this network."),
                        Some(format!("{why} Crake reconnected without it, keeping the sign-in.")),
                    );
                    if let Some(e) = self.tunnels.lock().unwrap().get_mut(&name) {
                        e.resumed = true;
                    }
                    terminate(&mut child, libc::SIGHUP).await;
                    no_dtls = true;
                }
            }
        };
        self.finish(&name, &label, state, reason).await;
    }

    async fn finish(&self, name: &str, label: &str, state: TunnelState, reason: Option<String>) {
        self.remove_routing(name).await;
        self.update_hush().await;
        let refused = {
            let mut map = self.tunnels.lock().unwrap();
            match map.get_mut(name) {
                Some(e) => {
                    e.stop = None;
                    e.running = false;
                    // A failed run may mean the session is no good any more.
                    if state == TunnelState::Trouble && !e.refused {
                        e.session = None;
                    }
                    std::mem::take(&mut e.refused)
                }
                None => false,
            }
        };
        if refused {
            // Already said why ("Turn ACME off to use Nordbank.").
            self.set_state(name, TunnelState::Off, true);
            return;
        }
        self.set_state(name, state, true);
        let msg = match state {
            TunnelState::Trouble => format!("{label} stopped unexpectedly."),
            _ => format!("Switched {label} off."),
        };
        self.log.system(Some(name), msg, reason);
    }

    /// Load the tunnel's pinned and learned hosts into the kernel. Returns how
    /// many destinations now go through it.
    async fn apply_routing(&self, name: &str, label: &str, info: &TunnelInfo) -> Result<usize> {
        let _ops = self.ops.lock().await;
        let index = self.entry(name)?.0;
        if self.is_routed(name) {
            // Reconnect: keep what traffic refreshed in the kernel.
            self.sync_learned(name).await;
        }
        let now = hosts::now();
        let (pinned, learned, skipped) = {
            let mut store = self.hosts.lock().unwrap();
            let t = store.tunnel(name);
            t.gateway = info.gateway.or(t.gateway);
            t.expire(now);
            let ok = |x: Target| guard(label, x, t.gateway).is_ok();
            let pinned: Vec<Target> = t.pinned.iter().copied().filter(|x| ok(*x)).collect();
            let learned: Vec<(Ipv4Addr, u64)> = t
                .learned_remaining(now)
                .into_iter()
                .filter(|(a, _)| ok(Target::host(*a)))
                .collect();
            let skipped = t.pinned.len() + t.learned.len() - pinned.len() - learned.len();
            store.save();
            (pinned, learned, skipped)
        };
        if skipped > 0 {
            self.log.system(
                Some(name),
                format!("Left {skipped} saved destination(s) direct."),
                Some(format!("They cover {label}'s gateway.")),
            );
        }
        // A full tunnel takes everything but its own gateway and the LAN.
        let full_except = (self.learning(name) == crate::policy::Learning::FullTunnel).then(|| {
            let mut v: Vec<Target> = crate::probe::Local::load().networks();
            if let Some(gw) = info.gateway {
                v.push(Target::host(gw));
            }
            v
        });
        self.routing
            .up(&Plan {
                name,
                index,
                device: &info.device,
                timeout: self.timeout_for(name),
                dns: &info.dns,
                pinned: &pinned,
                learned: &learned,
                full_except: full_except.as_deref(),
            })
            .await?;
        self.set_routed(name, true);
        self.routing.public_up().await?;
        // A tunnel that just came up may know names that failed before.
        self.unknown.lock().unwrap().clear();
        Ok(pinned.len() + learned.len())
    }

    /// Hand the tunnel's DNS servers and domains to systemd-resolved. Done after
    /// the routing is in place, so the first answer is already caught.
    async fn register_dns(&self, name: &str, label: &str, info: &TunnelInfo, domains: &[String]) {
        let everything = self.learning(name) == crate::policy::Learning::FullTunnel;
        if info.dns.is_empty() || (domains.is_empty() && !everything) {
            return;
        }
        let res = self
            .resolved
            .set_link(&info.device, &info.dns, domains, everything)
            .await;
        // Forget answers cached while the tunnel was off (failures included).
        self.resolved.flush().await;
        if let Err(e) = res {
            tracing::error!("registering {label}'s DNS: {e:#}");
            self.log.system(
                Some(name),
                format!("Couldn't hand {label}'s DNS to the system."),
                Some(format!("Names under its domains won't resolve. {e:#}")),
            );
        }
    }

    async fn remove_routing(&self, name: &str) {
        let _ops = self.ops.lock().await;
        if !self.is_routed(name) {
            return;
        }
        self.sync_learned(name).await;
        self.resolved.revert(&interface_name(name)).await;
        let Ok((index, _, _)) = self.entry(name) else {
            return;
        };
        let device = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .and_then(|e| e.info.as_ref().map(|i| i.device.clone()));
        if let Err(e) = self.routing.down(name, index, device.as_deref()).await {
            tracing::error!("removing routing for {name}: {e:#}");
        }
        self.set_routed(name, false);
        let any_left = self.tunnels.lock().unwrap().values().any(|e| e.routed);
        if !any_left && let Err(e) = self.routing.public_down().await {
            tracing::error!("removing the public DNS watch: {e:#}");
        }
        // Answers from this tunnel's DNS are no use any more.
        self.resolved.flush().await;
    }

    /// Pull learned hosts' expiry back from the kernel, where traffic refreshes
    /// it. Hosts the kernel has expired are dropped.
    async fn sync_learned(&self, name: &str) {
        if !self.is_routed(name) {
            return;
        }
        match self.routing.read_learned(name).await {
            Ok(Some(kernel)) => {
                let now = hosts::now();
                let mut store = self.hosts.lock().unwrap();
                let t = store.tunnel(name);
                let old = std::mem::take(&mut t.learned);
                for (addr, expires) in kernel {
                    let exp = if expires > 0 {
                        now + expires
                    } else {
                        old.get(&addr).copied().unwrap_or(now)
                    };
                    t.learned.insert(addr, exp);
                }
                store.save();
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("reading learned hosts for {name}: {e:#}"),
        }
    }

    fn entry(&self, name: &str) -> Result<(usize, String, Option<Ipv4Addr>)> {
        let map = self.tunnels.lock().unwrap();
        let e = map
            .get(name)
            .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
        let gw = e.info.as_ref().and_then(|i| i.gateway);
        Ok((e.index, e.config.display().to_owned(), gw))
    }

    /// What may go through the tunnel (its managed policy, if any).
    fn learning(&self, name: &str) -> crate::policy::Learning {
        self.tunnels
            .lock()
            .unwrap()
            .get(name)
            .map(|e| e.config.learning())
            .unwrap_or_default()
    }

    /// Refuse what a managed tunnel's policy doesn't allow, in its words.
    fn check_scope(
        &self,
        name: &str,
        label: &str,
        t: Target,
        dns_name: Option<&str>,
    ) -> Result<()> {
        use crate::policy::Learning;
        let (learning, org) = {
            let map = self.tunnels.lock().unwrap();
            let Some(m) = map.get(name).and_then(|e| e.config.managed.clone()) else {
                return Ok(());
            };
            (m.learning, m.organisation)
        };
        let ok = if t.is_host() {
            learning.allows(dns_name, t.addr())
        } else {
            learning.allows_target(t)
        };
        match learning {
            _ if ok => Ok(()),
            Learning::FullTunnel => bail!("Everything already goes through {label} (full tunnel)."),
            _ => bail!("{org} only allows {} through {label}.", learning.scope()),
        }
    }

    fn is_routed(&self, name: &str) -> bool {
        self.tunnels
            .lock()
            .unwrap()
            .get(name)
            .is_some_and(|e| e.routed)
    }

    fn set_routed(&self, name: &str, routed: bool) {
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.routed = routed;
        }
    }

    /// Label and the gateway to protect (current, or last seen).
    fn prepare(&self, name: &str) -> Result<(String, Option<Ipv4Addr>)> {
        let (_, label, gw) = self.entry(name)?;
        let saved = self
            .hosts
            .lock()
            .unwrap()
            .tunnels
            .get(name)
            .and_then(|t| t.gateway);
        Ok((label, gw.or(saved)))
    }

    /// "Add a host" with a name: what the tunnel's DNS says it is goes
    /// through the tunnel, as if learned.
    async fn add_name(&self, name: &str, host: &str) -> Result<String> {
        let (device, servers, label) = {
            let map = self.tunnels.lock().unwrap();
            let e = map
                .get(name)
                .ok_or_else(|| anyhow!("no tunnel named {name}"))?;
            let label = e.config.display().to_owned();
            match (&e.info, e.state) {
                (Some(i), TunnelState::Connected) if !i.dns.is_empty() => {
                    (i.device.clone(), i.dns.clone(), label)
                }
                (Some(_), TunnelState::Connected) => {
                    bail!("{label} has no DNS to ask about {host}. Add its address instead.")
                }
                _ => bail!("Switch {label} on first: Crake asks its DNS what {host} is."),
            }
        };
        let reply = ask(&device, &servers, host, RecordType::A)
            .await
            .ok_or_else(|| anyhow!("{label}'s DNS didn't answer."))?;
        let addrs = dns::a_records(&reply);
        if addrs.is_empty() {
            bail!("{label}'s DNS doesn't know {host}.");
        }
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        for a in &addrs {
            guard(&label, Target::host(*a), gw)?;
            self.check_scope(name, &label, Target::host(*a), Some(host))?;
            if let Some(owner) = self.owner_of(*a, name) {
                let owner = self.label(&owner);
                bail!(
                    "{host} is {a}, which already goes through {owner}. One address goes through one tunnel at a time: forget it there first."
                );
            }
        }
        for a in &addrs {
            self.hosts.lock().unwrap().tunnel(name).direct.remove(a);
            self.learn_one(name, *a, Some(host), "you").await?;
        }
        let expiry = self.expiry_sentence(name);
        let msg = format!("Sent {host} through {label}.");
        self.record_hosts(
            LogKind::Routed,
            name,
            &msg,
            format!("You added it. {expiry}"),
            addrs.iter().map(|a| a.to_string()).collect(),
        );
        Ok(msg)
    }

    /// Send one host through the tunnel until it has gone unused for the expiry time.
    pub async fn add_host(&self, name: &str, input: &str) -> Result<String> {
        let input = input.trim();
        // A name: asked of the tunnel's DNS, its addresses sent through.
        if input.parse::<Target>().is_err() && dns::valid_domain(input) {
            return self.add_name(name, &dns::normalize(input)).await;
        }
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        let t: Target = input.parse()?;
        if !t.is_host() {
            bail!("Only single hosts are learned. To send {t} through {label}, pin it.");
        }
        guard(&label, t, gw)?;
        self.check_scope(name, &label, t, None)?;
        if let Some(owner) = self.owner_of(t.addr(), name) {
            let owner = self.label(&owner);
            bail!(
                "{t} already goes through {owner}. One address goes through one tunnel at a time: forget it there first."
            );
        }
        let expiry = self.expiry_sentence(name);
        {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if th.pinned.contains(&t) {
                bail!("{t} is already pinned to {label}.");
            }
            // Adding by hand overrides an earlier undo.
            th.direct.remove(&t.addr());
        }
        self.learn_one(name, t.addr(), None, "you").await?;
        let msg = format!("Sent {t} through {label}.");
        self.record_hosts(
            LogKind::Routed,
            name,
            &msg,
            format!("You added it. {expiry}"),
            vec![t.addr().to_string()],
        );
        Ok(msg)
    }

    /// Put one host in the learned set (fresh expiry). Caller holds `ops`.
    /// `how`: why it was learned ("probe", "dns", "inside", "split",
    /// "you"), kept with when, for tunnel detail.
    async fn learn_one(
        &self,
        name: &str,
        addr: Ipv4Addr,
        dns_name: Option<&str>,
        how: &str,
    ) -> Result<()> {
        {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            let now = hosts::now();
            if !th.learned.contains_key(&addr) || !th.learned_how.contains_key(&addr) {
                th.learned_how.insert(addr, (now, how.to_owned()));
            }
            th.learned.insert(addr, now + self.timeout_for(name));
            if let Some(n) = dns_name {
                th.names.insert(addr, n.to_owned());
            }
            store.save();
        }
        if self.is_routed(name) {
            self.routing
                .add_learned(name, addr, self.timeout_for(name))
                .await?;
        }
        Ok(())
    }

    /// Called with each DNS answer from a tunnel's DNS servers, before the
    /// answer is let through. Names outside the tunnel's domains are ignored.
    pub async fn learn_from_dns(&self, name: &str, answer: Answer) {
        if answer.addrs.is_empty() || !dns::in_domains(&answer.name, &self.domains(name)) {
            return;
        }
        self.learn_answer(name, &answer, Why::TunnelDomain).await;
    }

    /// Called with each public DNS answer, before it's let through. When it
    /// says a name doesn't exist, ask each tunnel that is up, in order. If one
    /// knows the name, route its addresses and return the packet carrying the
    /// tunnel's answer instead.
    pub async fn rescue(&self, packet: &[u8]) -> Option<Vec<u8>> {
        let payload = dns::dns_payload(packet)?;
        if let Some(answer) = dns::parse_message(payload) {
            self.remember_names(&answer);
            if !self.settings.lock().unwrap().ask_unknown {
                return None;
            }
            return self.split_horizon(packet, payload).await;
        }
        // Settings: "Ask every tunnel when a name is unknown".
        if !self.settings.lock().unwrap().ask_unknown {
            return None;
        }
        let failure = dns::public_failure(payload)?;
        let key = (failure.name.clone(), failure.qtype);
        {
            let mut unknown = self.unknown.lock().unwrap();
            unknown.retain(|_, at| at.elapsed() < UNKNOWN_FOR);
            if unknown.contains_key(&key) {
                return None;
            }
        }
        for (tunnel, device, servers) in self.routed_tunnels() {
            if dns::in_domains(&failure.name, &self.domains(&tunnel)) {
                continue; // already this tunnel's name, and it said no
            }
            let learning = self.learning(&tunnel);
            if learning == crate::policy::Learning::FullTunnel {
                continue; // its DNS answers everything already
            }
            let Some(reply) = ask(&device, &servers, &failure.name, failure.qtype).await else {
                continue;
            };
            if reply.metadata.response_code != ResponseCode::NoError {
                continue;
            }
            let addrs = dns::a_records(&reply);
            // A managed tunnel answers only for what its policy allows.
            if !addrs.is_empty()
                && !addrs
                    .iter()
                    .any(|a| learning.allows(Some(&failure.name), *a))
            {
                continue;
            }
            if !addrs.is_empty() {
                let answer = Answer {
                    name: failure.name.clone(),
                    addrs,
                };
                self.learn_answer(&tunnel, &answer, Why::PublicFailed).await;
            }
            return dns::replace_payload(packet, &dns::rewrite(&failure, reply)?);
        }
        self.unknown.lock().unwrap().insert(key, Instant::now());
        None
    }

    /// A public answer for a name under a tunnel's organisation (the
    /// gateway's own domain, e.g. acme.example): ask that tunnel's DNS too. When it
    /// knows a private address the public DNS doesn't, that is the one that
    /// works (the public one often only lets the company's addresses in):
    /// route it and hand the app the tunnel's answer instead.
    async fn split_horizon(&self, packet: &[u8], payload: &[u8]) -> Option<Vec<u8>> {
        let (question, public) = dns::public_answer(payload)?;
        let tunnels: Vec<_> = self
            .routed_tunnels()
            .into_iter()
            .filter(|(t, _, _)| {
                self.organisation(t)
                    .is_some_and(|org| dns::in_domains(&question.name, &[org]))
            })
            .collect();
        if tunnels.is_empty() {
            return None;
        }
        {
            let mut no_split = self.no_split.lock().unwrap();
            no_split.retain(|_, at| at.elapsed() < UNKNOWN_FOR);
            if no_split.contains_key(&question.name) {
                return None;
            }
        }
        for (tunnel, device, servers) in tunnels {
            let learning = self.learning(&tunnel);
            if learning == crate::policy::Learning::FullTunnel {
                continue;
            }
            let Some(reply) = ask(&device, &servers, &question.name, question.qtype).await else {
                continue;
            };
            if reply.metadata.response_code != ResponseCode::NoError {
                continue;
            }
            let private: Vec<Ipv4Addr> = dns::a_records(&reply)
                .into_iter()
                .filter(|a| dns::is_private(*a) && !public.contains(a))
                .filter(|a| learning.allows(Some(&question.name), *a))
                .collect();
            if private.is_empty() {
                continue;
            }
            // Only if the private address answers through this tunnel (web
            // ports): some internal answers are for the office network only,
            // and the public address would have been the better bet.
            let Ok((index, _, _)) = self.entry(&tunnel) else {
                continue;
            };
            let mark = crate::routing::ids(index).mark;
            let mut answers = false;
            for port in [443, 80] {
                let addr = SocketAddrV4::new(private[0], port);
                if crate::probe::through_tunnel(addr, mark, PROBE_TIMEOUT / 3)
                    .await
                    .is_some()
                {
                    answers = true;
                    break;
                }
            }
            if !answers {
                continue;
            }
            let answer = Answer {
                name: question.name.clone(),
                addrs: private.clone(),
            };
            self.learn_answer(&tunnel, &answer, Why::PrivateAddress)
                .await;
            // Only hand out the private address if it now goes through (not
            // when you sent it direct, or the never list has it).
            let routed = {
                let store = self.hosts.lock().unwrap();
                store
                    .tunnels
                    .get(&tunnel)
                    .is_some_and(|t| private.iter().all(|a| t.covers(*a)))
            };
            if !routed {
                return None;
            }
            return dns::replace_payload(packet, &dns::rewrite(&question, reply)?);
        }
        self.no_split
            .lock()
            .unwrap()
            .insert(question.name.clone(), Instant::now());
        None
    }

    /// The tunnel's organisation domain, from its gateway.
    fn organisation(&self, tunnel: &str) -> Option<String> {
        let map = self.tunnels.lock().unwrap();
        dns::organisation(&map.get(tunnel)?.config.gateway)
    }

    fn remember_names(&self, answer: &Answer) {
        if answer.addrs.is_empty() {
            return;
        }
        let mut seen = self.seen_names.lock().unwrap();
        if seen.len() > 4096 {
            seen.retain(|_, (_, at)| at.elapsed() < SEEN_NAME_FOR);
        }
        for a in &answer.addrs {
            seen.insert(*a, (answer.name.clone(), Instant::now()));
        }
    }

    fn name_for(&self, addr: Ipv4Addr) -> Option<String> {
        let seen = self.seen_names.lock().unwrap();
        seen.get(&addr)
            .filter(|(_, at)| at.elapsed() < SEEN_NAME_FOR)
            .map(|(n, _)| n.clone())
    }

    pub fn probe_wait(&self) -> Duration {
        Duration::from_secs(self.settings.lock().unwrap().probe_wait_secs)
    }

    /// At least one tunnel's routing is in place.
    pub fn any_routed(&self) -> bool {
        self.tunnels.lock().unwrap().values().any(|e| e.routed)
    }

    /// A direct connection to `flow.dst` got no answer, or was refused. Try it
    /// through each tunnel that's up, in order; route it through the first that
    /// answers. The stuck connection is forgotten by conntrack, so the app's
    /// next retry already goes through the tunnel.
    pub async fn probe(&self, flow: crate::conntrack::Flow, why: crate::probe::Direct) {
        use crate::probe::{Direct, Local};
        let dst = flow.dst;
        let target = Target::host(dst);
        if target.check_routable().is_err() || self.never(self.name_for(dst).as_deref(), dst) {
            return;
        }
        let local = Local::load();
        // Only this machine's own connections, and never the local network.
        if !local.is_own(flow.src) || local.on_link(dst) {
            return;
        }
        {
            let store = self.hosts.lock().unwrap();
            if store.tunnels.values().any(|t| t.covers(dst)) {
                return;
            }
        }
        {
            let mut probed = self.probed.lock().unwrap();
            probed.retain(|_, at| at.elapsed() < PROBED_FOR);
            if probed.insert(dst, Instant::now()).is_some() {
                return;
            }
        }
        let candidates = self.probe_candidates(dst);
        if candidates.is_empty() {
            return;
        }
        let shown = self.name_for(dst).unwrap_or_else(|| dst.to_string());
        let addr = SocketAddrV4::new(dst, flow.dport);
        let both = self.settings.lock().unwrap().both;
        // Every eligible tunnel is tried at once; the setting decides which
        // answer counts (CLAUDE.md, "When two tunnels answer").
        let mut tries = tokio::task::JoinSet::new();
        for (i, (_, _, mark)) in candidates.iter().enumerate() {
            let mark = *mark;
            tries.spawn(async move {
                (
                    i,
                    crate::probe::through_tunnel(addr, mark, PROBE_TIMEOUT).await,
                )
            });
        }
        let mut results: Vec<Option<Option<Duration>>> = vec![None; candidates.len()];
        let chosen = loop {
            if let Some(i) = pick(&results, both) {
                break Some(i);
            }
            match tries.join_next().await {
                Some(Ok((i, r))) => results[i] = Some(r),
                Some(Err(_)) => {}
                None => break pick(&results, both),
            }
        };
        if let Some(i) = chosen {
            let (name, label, _) = &candidates[i];
            let took = results[i].flatten().unwrap_or_default();
            if self.asks(name) {
                let ms = took.as_millis().max(1);
                let why =
                    format!("It didn't answer directly. Through {label} it answered in {ms} ms.");
                self.ask_first(name, label, &shown, why, &[dst]);
                return;
            }
            let _ops = self.ops.lock().await;
            let dns_name = self.name_for(dst);
            if let Err(e) = self.learn_one(name, dst, dns_name.as_deref(), "probe").await {
                tracing::error!("learning {dst} for {name}: {e:#}");
                return;
            }
            drop(_ops);
            // The app's stuck attempt: forget it, so its next retry is routed afresh.
            let _ = tokio::task::spawn_blocking(move || crate::conntrack::delete(flow)).await;
            let ms = took.as_millis().max(1);
            let mut reason = match why {
                Direct::NoAnswer => {
                    format!("It didn't answer directly. Through {label} it answered in {ms} ms.")
                }
                Direct::Refused => {
                    format!("Refused directly. Through {label} it answered in {ms} ms.")
                }
            };
            if both == crate::settings::Both::Faster
                && results
                    .iter()
                    .filter(|r| matches!(r, Some(Some(_))))
                    .count()
                    > 1
            {
                reason += " The fastest of the tunnels that answered.";
            }
            self.record_hosts(
                LogKind::Routed,
                name,
                &format!("Sent {shown} through {label}."),
                reason,
                vec![dst.to_string()],
            );
            // "Ask me": hear the others out, and say which answered too.
            if both == crate::settings::Both::Ask {
                while let Some(r) = tries.join_next().await {
                    if let Ok((j, r)) = r {
                        results[j] = Some(r);
                    }
                }
                let others: Vec<usize> = (0..candidates.len())
                    .filter(|j| *j != i && matches!(results[*j], Some(Some(_))))
                    .collect();
                if !others.is_empty() {
                    let labels: Vec<&str> =
                        others.iter().map(|j| candidates[*j].1.as_str()).collect();
                    let verb = if labels.len() == 1 {
                        "answers"
                    } else {
                        "answer"
                    };
                    self.log.append(LogEntry {
                        ts: crate::log::now(),
                        kind: LogKind::System,
                        tunnel: Some(name.clone()),
                        message: format!("{} {verb} for {shown} too.", and_list(&labels)),
                        reason: Some(format!(
                            "Crake sent it through {label}, first in order. You can move it."
                        )),
                        hosts: vec![dst.to_string()],
                        others: others.iter().map(|j| candidates[*j].0.clone()).collect(),
                    });
                }
            }
            return;
        }
        let labels: Vec<&str> = candidates.iter().map(|(_, l, _)| l.as_str()).collect();
        let through = labels.join(" or ");
        let reason = match why {
            Direct::NoAnswer => {
                format!("It didn't answer directly or through {through}. Probably down.")
            }
            Direct::Refused => format!("Refused directly, and no answer through {through}."),
        };
        let log_tunnel = candidates[0].0.clone();
        self.record_hosts(
            LogKind::LeftAlone,
            &log_tunnel,
            &format!("Left {shown} alone."),
            reason,
            vec![dst.to_string()],
        );
    }

    /// Tunnels a host may be probed through, in order: routed, not sent direct
    /// by you for that tunnel, and not covering the tunnel's gateway.
    fn probe_candidates(&self, dst: Ipv4Addr) -> Vec<(String, String, u32)> {
        let name = self.name_for(dst);
        let map = self.tunnels.lock().unwrap();
        let store = self.hosts.lock().unwrap();
        let mut v: Vec<_> = map
            .values()
            .filter(|e| e.routed)
            .filter(|e| {
                let t = store.tunnels.get(&e.config.name);
                !t.is_some_and(|t| t.direct.contains(&dst))
            })
            .filter(|e| {
                let gw = e.info.as_ref().and_then(|i| i.gateway);
                guard(e.config.display(), Target::host(dst), gw).is_ok()
            })
            .filter(|e| e.config.learning().allows(name.as_deref(), dst))
            .map(|e| {
                (
                    (self.rank(&e.config.name, e.index), e.index),
                    e.config.name.clone(),
                    e.config.display().to_owned(),
                )
            })
            .collect();
        v.sort_by_key(|t| t.0);
        v.into_iter()
            .map(|((_, i), n, l)| (n, l, crate::routing::ids(i).mark))
            .collect()
    }

    /// Send learned hosts direct and keep them direct: Crake won't learn them
    /// for this tunnel again until you redo. Several addresses are one
    /// decision (a name that resolved to several).
    pub async fn undo(&self, name: &str, inputs: &[&str]) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, _) = self.prepare(name)?;
        let addrs = hosts_of(inputs, &label)?;
        let shown = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if let Some(a) = addrs
                .iter()
                .find(|a| th.pinned.contains(&Target::host(**a)))
            {
                bail!("{a} is pinned to {label}. Unpin it first.");
            }
            let mut any = false;
            for a in &addrs {
                any |= th.learned.remove(a).is_some();
                th.direct.insert(*a);
            }
            if !any {
                bail!("{} isn't sent through {label}.", join(&addrs));
            }
            let shown = th
                .names
                .get(&addrs[0])
                .cloned()
                .unwrap_or_else(|| join(&addrs));
            store.save();
            shown
        };
        if self.is_routed(name) {
            for a in &addrs {
                self.routing.remove_learned(name, *a).await?;
            }
        }
        let msg = format!("{shown} goes direct again.");
        self.record_hosts(
            LogKind::Undone,
            name,
            &msg,
            "You undid the route. Crake won\u{2019}t learn this host again on its own.".into(),
            addrs.iter().map(Ipv4Addr::to_string).collect(),
        );
        Ok(msg)
    }

    /// Send hosts through `to` instead of `from` (another tunnel answered for
    /// them too). They stay direct for `from` until redone there.
    pub async fn move_hosts(&self, from: &str, to: &str, inputs: &[&str]) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (from_label, _) = self.prepare(from)?;
        let (to_label, gw) = self.prepare(to)?;
        let addrs = hosts_of(inputs, &to_label)?;
        for a in &addrs {
            guard(&to_label, Target::host(*a), gw)?;
            let known = th_name(&self.hosts.lock().unwrap(), from, *a);
            self.check_scope(to, &to_label, Target::host(*a), known.as_deref())?;
        }
        let shown = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(from);
            if let Some(a) = addrs
                .iter()
                .find(|a| th.pinned.contains(&Target::host(**a)))
            {
                bail!("{a} is pinned to {from_label}. Unpin it first.");
            }
            for a in &addrs {
                th.learned.remove(a);
                th.direct.insert(*a);
            }
            let shown = th.names.get(&addrs[0]).cloned();
            store.save();
            shown.unwrap_or_else(|| join(&addrs))
        };
        if self.is_routed(from) {
            for a in &addrs {
                self.routing.remove_learned(from, *a).await?;
            }
        }
        for a in &addrs {
            {
                let mut store = self.hosts.lock().unwrap();
                store.tunnel(to).direct.remove(a);
            }
            let known = th_name(&self.hosts.lock().unwrap(), from, *a);
            self.learn_one(to, *a, known.as_deref(), "you").await?;
        }
        let msg = format!("Sent {shown} through {to_label} instead of {from_label}.");
        self.record_hosts(
            LogKind::Routed,
            to,
            &msg,
            format!("You chose {to_label}. {}", self.expiry_sentence(to)),
            addrs.iter().map(Ipv4Addr::to_string).collect(),
        );
        Ok(msg)
    }

    /// Take back an undo: send the hosts through the tunnel again.
    pub async fn redo(&self, name: &str, inputs: &[&str]) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        let addrs = hosts_of(inputs, &label)?;
        for a in &addrs {
            guard(&label, Target::host(*a), gw)?;
            let known = th_name(&self.hosts.lock().unwrap(), name, *a);
            self.check_scope(name, &label, Target::host(*a), known.as_deref())?;
        }
        let shown = {
            let mut store = self.hosts.lock().unwrap();
            let mut any = false;
            {
                let th = store.tunnel(name);
                for a in &addrs {
                    any |= th.direct.remove(a);
                }
            }
            if !any {
                bail!("{} wasn't sent direct by you.", join(&addrs));
            }
            store.save();
            th_name(&store, name, addrs[0]).unwrap_or_else(|| join(&addrs))
        };
        for a in &addrs {
            self.learn_one(name, *a, None, "you").await?;
        }
        let msg = format!("Sent {shown} through {label} again.");
        self.record_hosts(
            LogKind::Routed,
            name,
            &msg,
            "You redid it.".into(),
            addrs.iter().map(Ipv4Addr::to_string).collect(),
        );
        Ok(msg)
    }

    /// Tunnels whose routing is in place, in config order (the try order),
    /// with their device and DNS servers.
    fn routed_tunnels(&self) -> Vec<(String, String, Vec<Ipv4Addr>)> {
        let map = self.tunnels.lock().unwrap();
        let mut v: Vec<_> = map
            .values()
            .filter(|e| e.routed)
            .filter_map(|e| {
                let info = e.info.as_ref()?;
                (!info.dns.is_empty()).then(|| {
                    (
                        self.rank(&e.config.name, e.index),
                        e.config.name.clone(),
                        info.device.clone(),
                        info.dns.clone(),
                    )
                })
            })
            .collect();
        v.sort_by_key(|t| t.0);
        v.into_iter().map(|(_, n, d, s)| (n, d, s)).collect()
    }

    /// Route the addresses of one answer through the tunnel, and say so.
    async fn learn_answer(&self, name: &str, answer: &Answer, why: Why) {
        let _ops = self.ops.lock().await;
        let Ok((label, gw)) = self.prepare(name) else {
            return;
        };
        let learning = self.learning(name);
        let asks = self.asks(name);
        let mut sent = Vec::new();
        for addr in &answer.addrs {
            // Outside a managed tunnel's scope: not this tunnel's business.
            if !learning.allows(Some(&answer.name), *addr) {
                continue;
            }
            {
                let mut store = self.hosts.lock().unwrap();
                let th = store.tunnel(name);
                // Already sent, or sent direct by you.
                if th.covers(*addr) || th.direct.contains(addr) {
                    continue;
                }
            }
            // Already another tunnel's: two companies using the same
            // private address. One address goes through one tunnel; it
            // stays where it is, and the user may move it.
            if let Some(owner) = self.owner_of(*addr, name) {
                self.contested(name, &label, &owner, &answer.name, *addr);
                continue;
            }
            if self.never(Some(&answer.name), *addr) {
                continue;
            }
            if let Err(e) = guard(&label, Target::host(*addr), gw) {
                self.record(
                    LogKind::LeftAlone,
                    name,
                    &format!("Left {} alone.", answer.name),
                    format!("{e:#}"),
                );
                continue;
            }
            // "Ask me first": found, not routed; the user decides.
            if asks {
                sent.push(addr.to_string());
                continue;
            }
            let how = match &why {
                Why::TunnelDomain => "dns",
                Why::PublicFailed => "inside",
                Why::PrivateAddress => "split",
            };
            match self.learn_one(name, *addr, Some(&answer.name), how).await {
                Ok(()) => sent.push(addr.to_string()),
                Err(e) => tracing::error!("learning {addr} for {name}: {e:#}"),
            }
        }
        if asks {
            let addrs: Vec<Ipv4Addr> = sent.iter().filter_map(|a| a.parse().ok()).collect();
            let why = match why {
                Why::TunnelDomain => format!("Only {label}'s DNS knows it."),
                Why::PublicFailed => format!("Public DNS didn't know it; {label}'s DNS did."),
                Why::PrivateAddress => {
                    format!("Public name, private address: {label}'s DNS knows it.")
                }
            };
            self.ask_first(name, &label, &answer.name, why, &addrs);
            return;
        }
        if !sent.is_empty() {
            self.record_hosts(
                LogKind::Routed,
                name,
                &format!("Sent {} through {label}.", answer.name),
                                match why {
                    Why::TunnelDomain => format!(
                        "Only {label}'s DNS knows it ({}). Routed before the first packet.",
                        sent.join(", ")
                    ),
                    Why::PublicFailed => format!(
                        "Public DNS didn't know it; {label}'s DNS did ({}). Routed before the first packet.",
                        sent.join(", ")
                    ),
                    Why::PrivateAddress => format!(
                        "Public name, private address: {label}'s DNS knows it as {}. Routed before the first packet.",
                        sent.join(", ")
                    ),
                },
                sent.clone(),
            );
        }
    }

    /// Keep a host or network on the tunnel until unpinned.
    pub async fn pin(&self, name: &str, input: &str) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, gw) = self.prepare(name)?;
        let t: Target = input.parse()?;
        guard(&label, t, gw)?;
        self.check_scope(name, &label, t, None)?;
        let was_learned = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if !th.pinned.insert(t) {
                bail!("{t} is already pinned to {label}.");
            }
            if t.is_host() {
                th.direct.remove(&t.addr());
            }
            let was = t.is_host() && th.learned.remove(&t.addr()).is_some();
            store.save();
            was
        };
        if self.is_routed(name) {
            self.routing.add_pinned(name, t).await?;
            if was_learned {
                self.routing.remove_learned(name, t.addr()).await?;
            }
        }
        let msg = format!("Pinned {t} to {label}.");
        self.record(
            LogKind::Routed,
            name,
            &msg,
            "It stays until you unpin it.".into(),
        );
        Ok(msg)
    }

    /// Unpin. A host goes back to being learned (and expires); a network goes direct.
    pub async fn unpin(&self, name: &str, input: &str) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, _) = self.prepare(name)?;
        let t: Target = input.parse()?;
        {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            if !th.pinned.remove(&t) {
                bail!("{t} isn't pinned to {label}.");
            }
            if t.is_host() {
                th.learned
                    .insert(t.addr(), hosts::now() + self.timeout_for(name));
            }
            store.save();
        }
        if self.is_routed(name) {
            if t.is_host() {
                self.routing
                    .add_learned(name, t.addr(), self.timeout_for(name))
                    .await?;
            }
            self.routing.remove_pinned(name, t).await?;
        }
        let msg = format!("Unpinned {t} from {label}.");
        let reason = if t.is_host() {
            self.expiry_sentence(name)
        } else {
            "It goes direct again.".to_owned()
        };
        self.record(LogKind::Undone, name, &msg, reason);
        Ok(msg)
    }

    /// Stop sending a host or network through the tunnel, now.
    pub async fn forget(&self, name: &str, input: &str) -> Result<String> {
        let _ops = self.ops.lock().await;
        let (label, _) = self.prepare(name)?;
        let t: Target = input.parse()?;
        let (was_pinned, was_learned) = {
            let mut store = self.hosts.lock().unwrap();
            let th = store.tunnel(name);
            let p = th.pinned.remove(&t);
            let l = t.is_host() && th.learned.remove(&t.addr()).is_some();
            if t.is_host() {
                th.names.remove(&t.addr());
                th.learned_how.remove(&t.addr());
            }
            store.save();
            (p, l)
        };
        if !was_pinned && !was_learned {
            bail!("{t} isn't sent through {label}.");
        }
        if self.is_routed(name) {
            if was_pinned {
                self.routing.remove_pinned(name, t).await?;
            }
            if was_learned {
                self.routing.remove_learned(name, t.addr()).await?;
            }
        }
        let msg = format!("Stopped sending {t} through {label}.");
        self.record(
            LogKind::Undone,
            name,
            &msg,
            "You forgot it. It goes direct again.".into(),
        );
        Ok(msg)
    }

    /// Pinned first, then learned hosts with time left.
    pub async fn hosts(&self, name: &str) -> Result<Vec<HostRow>> {
        let _ops = self.ops.lock().await;
        self.entry(name)?;
        self.sync_learned(name).await;
        let now = hosts::now();
        let store = self.hosts.lock().unwrap();
        let Some(t) = store.tunnels.get(name) else {
            return Ok(Vec::new());
        };
        let named = |a: Ipv4Addr| t.names.get(&a).cloned();
        let how = |a: Ipv4Addr| t.learned_how.get(&a).cloned();
        let forgets = self.forget_after(name).is_some();
        let timeout = self.timeout_for(name);
        let pinned = t.pinned.iter().map(|p| HostRow {
            target: *p,
            name: if p.is_host() { named(p.addr()) } else { None },
            pinned: true,
            expires_in: None,
            direct: false,
            learned: if p.is_host() { how(p.addr()) } else { None },
            used_ago: None,
        });
        let learned = t
            .learned_remaining(now)
            .into_iter()
            .map(|(a, left)| HostRow {
                target: Target::host(a),
                name: named(a),
                pinned: false,
                expires_in: Some(left),
                direct: false,
                learned: how(a),
                used_ago: forgets.then(|| timeout.saturating_sub(left)),
            });
        let direct = t.direct.iter().map(|a| HostRow {
            target: Target::host(*a),
            name: named(*a),
            pinned: false,
            expires_in: None,
            direct: true,
            learned: None,
            used_ago: None,
        });
        Ok(pinned.chain(learned).chain(direct).collect())
    }

    fn record(&self, kind: LogKind, tunnel: &str, message: &str, reason: String) {
        self.record_hosts(kind, tunnel, message, reason, Vec::new());
    }

    /// A decision about specific addresses (so it can be undone).
    fn record_hosts(
        &self,
        kind: LogKind,
        tunnel: &str,
        message: &str,
        reason: String,
        hosts: Vec<String>,
    ) {
        self.log.append(LogEntry {
            ts: crate::log::now(),
            kind,
            tunnel: Some(tunnel.to_owned()),
            message: message.to_owned(),
            reason: Some(reason),
            hosts,
            others: Vec::new(),
        });
    }

    async fn spawn_openconnect(
        &self,
        t: &TunnelConfig,
        session: Option<Session>,
        no_dtls: bool,
    ) -> Result<Child> {
        let mut cmd = Command::new(&self.cfg.openconnect);
        cmd.arg(format!("--protocol={}", t.protocol.as_str()))
            .arg(format!("--interface={}", interface_name(&t.name)))
            .arg(format!("--script={}", self.cfg.hook_path.display()))
            .arg("--non-inter");
        if let Some(user) = &t.user {
            cmd.arg(format!("--user={user}"));
        }
        let mut target = t.gateway.clone();
        let stdin_line = match (&session, &t.passwd_file) {
            (Some(s), _) => {
                cmd.arg("--cookie-on-stdin");
                if let Some(fp) = &s.fingerprint {
                    cmd.arg(format!("--servercert={fp}"));
                }
                if let Some(r) = &s.resolve {
                    cmd.arg(format!("--resolve={r}"));
                }
                // Introduce itself as at sign-in, unless the config says how.
                if let Some(ua) = &s.user_agent
                    && !t.extra_args.iter().any(|a| a.starts_with("--useragent"))
                {
                    cmd.arg(format!("--useragent={ua}"));
                }
                target = s.connect_url.clone();
                Some(s.cookie.clone())
            }
            (None, Some(p)) => {
                cmd.arg("--passwd-on-stdin");
                let text = tokio::fs::read_to_string(p)
                    .await
                    .with_context(|| format!("reading {}", p.display()))?;
                Some(text.lines().next().unwrap_or("").to_owned())
            }
            (None, None) => None,
        };
        if no_dtls && !t.extra_args.iter().any(|a| a == "--no-dtls") {
            cmd.arg("--no-dtls");
        }
        cmd.args(&t.extra_args)
            .arg("--")
            .arg(&target)
            // English output, whatever the system language, so it can be read.
            .env("LC_ALL", "C")
            .env(ENV_TUNNEL, &t.name)
            .env(ENV_SOCKET, &self.cfg.hook_socket)
            .stdin(if stdin_line.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Its own process group: Ctrl+C on a terminal reaches craked only,
            // which then stops openconnect in order.
            .process_group(0)
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .with_context(|| format!("starting {}", self.cfg.openconnect.display()))?;
        if let (Some(pw), Some(mut stdin)) = (stdin_line, child.stdin.take()) {
            stdin.write_all(format!("{pw}\n").as_bytes()).await.ok();
        }
        Ok(child)
    }
}

/// Which tunnel's answer counts so far: `results[i]` is None while tunnel i
/// is still being tried, then whether (and how fast) it answered.
fn pick(results: &[Option<Option<Duration>>], both: crate::settings::Both) -> Option<usize> {
    use crate::settings::Both;
    match both {
        // The first to answer is the fastest.
        Both::Faster => results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.flatten().map(|t| (i, t)))
            .min_by_key(|(_, t)| *t)
            .map(|(i, _)| i),
        // The first in order that answers, once those before it haven't.
        Both::First | Both::Ask => {
            for (i, r) in results.iter().enumerate() {
                match r {
                    Some(Some(_)) => return Some(i),
                    Some(None) => continue,
                    None => return None,
                }
            }
            None
        }
    }
}

/// "ACME", "ACME and Nordbank", "ACME, Globex and Nordbank".
fn and_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Single hosts from user input.
fn hosts_of(inputs: &[&str], label: &str) -> Result<Vec<Ipv4Addr>> {
    let mut out = Vec::new();
    for i in inputs {
        let t: Target = i.parse()?;
        if !t.is_host() {
            bail!(
                "Only single hosts can be undone. To stop sending {t} through {label}, forget it."
            );
        }
        out.push(t.addr());
    }
    if out.is_empty() {
        bail!("Which host?");
    }
    Ok(out)
}

fn join(addrs: &[Ipv4Addr]) -> String {
    addrs
        .iter()
        .map(Ipv4Addr::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn th_name(store: &HostStore, tunnel: &str, addr: Ipv4Addr) -> Option<String> {
    store.tunnels.get(tunnel)?.names.get(&addr).cloned()
}

/// Ask a tunnel's DNS servers, in order, about `name` (e.g. the question
/// that failed publicly). The socket is bound to the tunnel, so the
/// question can't leak elsewhere.
async fn ask(device: &str, servers: &[Ipv4Addr], name: &str, qtype: RecordType) -> Option<Message> {
    use std::hash::BuildHasher;
    for server in servers {
        let id = std::collections::hash_map::RandomState::new().hash_one(Instant::now()) as u16;
        let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
        sock.bind_device(Some(device.as_bytes())).ok()?;
        if sock.connect((*server, 53)).await.is_err() {
            continue;
        }
        if sock
            .send(&dns::query(id, name, qtype))
            .await
            .is_err()
        {
            continue;
        }
        let mut buf = vec![0u8; 4096];
        let wait = tokio::time::timeout(TUNNEL_DNS_WAIT, async {
            loop {
                let n = sock.recv(&mut buf).await.ok()?;
                if let Some(m) = dns::parse_reply(&buf[..n], id, name) {
                    return Some(m);
                }
            }
        });
        if let Ok(Some(reply)) = wait.await {
            return Some(reply);
        }
    }
    None
}

/// Nothing that would break the machine, and nothing that covers the tunnel's
/// own gateway (that would cut the tunnel off).
fn guard(label: &str, t: Target, gateway: Option<Ipv4Addr>) -> Result<()> {
    t.check_routable()?;
    if let Some(gw) = gateway.filter(|gw| t.contains(*gw)) {
        bail!(
            "{t} covers {label}'s gateway ({gw}). Sending it through {label} would cut the tunnel off."
        );
    }
    Ok(())
}

fn snapshot(e: &Entry) -> Snapshot {
    Snapshot {
        interface: interface_name(&e.config.name),
        config: e.config.clone(),
        state: e.state,
        info: e.info.clone(),
        hosts: 0,
        signed_in: e.session.is_some(),
        clash: None,
        since: e.up_since,
        transport: None,
    }
}

/// The network a tunnel's own address is on (none for a single address).
fn own_net(info: &TunnelInfo) -> Option<Target> {
    let (addr, len) = (info.address?, info.prefix?);
    if len == 0 || len >= 32 {
        return None;
    }
    let mask = u32::MAX << (32 - u32::from(len));
    format!("{}/{len}", Ipv4Addr::from(u32::from(addr) & mask))
        .parse()
        .ok()
}

/// Two networks (or hosts) share an address.
fn overlap(a: Target, b: Target) -> bool {
    a.contains(b.addr()) || b.contains(a.addr())
}

fn describe(info: &TunnelInfo, domains: &[String]) -> String {
    let addr = info
        .address
        .map_or("no address".to_owned(), |a| a.to_string());
    let names = if domains.is_empty() || info.dns.is_empty() {
        "No names go to its DNS.".to_owned()
    } else {
        format!("Names under {} go to its DNS.", domains.join(", "))
    };
    format!(
        "Address {addr} on {}. {names} {} pushed network(s) kept as hints.",
        info.device,
        info.split_include.len()
    )
}

/// openconnect's output goes to the journal; its last line explains a
/// failure, and a sign that UDP doesn't get through is passed on.
/// Where one openconnect's output goes.
#[derive(Clone)]
struct Output {
    tunnel: String,
    last: Arc<Mutex<String>>,
    udp_failed: tokio::sync::mpsc::UnboundedSender<()>,
    transport: Arc<Mutex<HashMap<String, String>>>,
}

fn forward_output(child: &mut Child, output: Output) {
    if let Some(out) = child.stdout.take() {
        tokio::spawn(pump(BufReader::new(out), output.clone()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(pump(BufReader::new(err), output));
    }
}

async fn pump<R: tokio::io::AsyncRead + Unpin>(r: BufReader<R>, o: Output) {
    let mut lines = r.lines();
    // Once UDP has worked here, a later hiccup isn't a blocked network.
    let mut udp_worked = false;
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        tracing::info!(target: "openconnect", tunnel = %o.tunnel, "{line}");
        let worked = crate::networks::udp_worked(&line);
        udp_worked |= worked;
        let failed = crate::networks::udp_failed(&line);
        if !udp_worked && failed {
            let _ = o.udp_failed.send(());
        }
        // How packets travel now, for tunnel detail.
        let now = if worked {
            Some(if line.starts_with("ESP") { "ESP" } else { "DTLS" })
        } else if failed || line.contains("using HTTPS instead") {
            Some("TLS")
        } else {
            None
        };
        if let Some(how) = now {
            o.transport.lock().unwrap().insert(o.tunnel.clone(), how.to_owned());
        }
        *o.last.lock().unwrap() = line;
    }
}

/// SIGTERM lets openconnect call the hook with `disconnect`; kill after a grace period.
async fn terminate(child: &mut Child, signal: i32) {
    if let Some(pid) = child.id() {
        // SAFETY: plain signal to a child we own.
        unsafe { libc::kill(pid as i32, signal) };
    }
    if tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Both;

    #[test]
    fn which_tunnels_answer_counts() {
        let ms = |n| Some(Some(Duration::from_millis(n)));
        let silent = Some(None);
        // ACME (first in order) still trying, Nordbank answered.
        let r = [None, ms(40)];
        assert_eq!(pick(&r, Both::First), None, "wait for the first in order");
        assert_eq!(pick(&r, Both::Ask), None);
        assert_eq!(pick(&r, Both::Faster), Some(1), "the first to answer");
        // ACME gave up: Nordbank it is.
        assert_eq!(pick(&[silent, ms(40)], Both::First), Some(1));
        assert_eq!(pick(&[ms(90), ms(40)], Both::First), Some(0));
        assert_eq!(pick(&[ms(90), ms(40)], Both::Faster), Some(1));
        assert_eq!(pick(&[silent, silent], Both::Ask), None);
        assert_eq!(
            and_list(&["ACME", "Globex", "Nordbank"]),
            "ACME, Globex and Nordbank"
        );
    }
}
