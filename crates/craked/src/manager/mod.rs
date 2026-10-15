use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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

mod app_tunnels;
mod attached;
mod clash;
mod diagnostics;
mod handover;
mod host_ops;
mod live;
mod naming;
mod outage;
mod probing;
mod routes;
mod settings;
mod tunnels;

// Helpers one part uses and others need too.
use self::clash::own_net;
use self::host_ops::guard;
use self::naming::{ask, ask_within};

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
    /// Its DNS doesn't answer (7h).
    pub dns_down: bool,
    /// Why it can't be switched on now (it clashes with a tunnel that's up),
    /// as far as craked knows from its last connection.
    pub clash: Option<String>,
    /// When it came up (seconds since the epoch).
    pub since: Option<u64>,
    /// How its packets travel: DTLS, ESP, TLS (TCP only), UDP or TCP.
    pub transport: Option<String>,
    /// In trouble: why, as the log says it (crake_proto::trouble).
    pub why: Option<String>,
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
    /// Handed over to the new craked of a package upgrade (`handover`): it
    /// stops, and starts again there, without a line in the log.
    handover: bool,
    /// OpenVPN 3: what the app attached it with, for a handover.
    attached: Option<HashMap<String, String>>,
    /// In trouble: why (cleared when it leaves trouble).
    why: Option<String>,
}

/// A UDP conversation's first datagram, and when it was copied.
type Datagram = (Arc<Vec<u8>>, Instant);

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
    /// The domains kept private last (`refresh_hush` changes them only
    /// when they differ).
    hush_last: Mutex<Option<Vec<String>>>,
    /// Tunnels whose DNS doesn't answer (7h): tunnel -> the names already
    /// said to be answered from memory during this outage.
    dns_down: Mutex<HashMap<String, std::collections::HashSet<String>>>,
    /// Checks in a row each tunnel's DNS didn't answer.
    dns_misses: Mutex<HashMap<String, u8>>,
    /// The memory answerer (`keep.rs`) runs, and its link is changed one at
    /// a time.
    keep_serving: std::sync::atomic::AtomicBool,
    keep: tokio::sync::Mutex<()>,
    /// Serializes host changes so the store and the kernel sets move together.
    ops: tokio::sync::Mutex<()>,
    log: Log,
    net: Net,
    routing: Routing,
    resolved: Resolved,
    /// "This computer encrypts its DNS" has been said (once per start).
    encrypted_dns_said: std::sync::atomic::AtomicBool,
    traffic: Arc<crate::traffic::Traffic>,
    /// How each tunnel's packets travel, as openconnect (or the app, for
    /// OpenVPN 3) last said: tunnel -> "DTLS", "ESP", "TLS", "UDP", "TCP".
    transport: Arc<Mutex<HashMap<String, String>>>,
    /// Names no tunnel knew lately; not asked again for a while.
    unknown: Mutex<HashMap<(String, RecordType), Instant>>,
    /// Addresses another tunnel's DNS also gave (tunnel, address), when
    /// last said: once per 10 min, not on every lookup.
    contested: Mutex<HashMap<(String, IpAddr), Instant>>,
    /// "Ask me first" questions lately (tunnel, host): not asked again soon.
    asked: Mutex<HashMap<(String, IpAddr), Instant>>,
    /// Names under an organisation's domain whose tunnel DNS had no private
    /// address lately; not asked again for a while.
    no_split: Mutex<HashMap<(String, RecordType), Instant>>,
    /// Hosts probed lately (or being probed): not probed again for a while.
    probed: Mutex<HashMap<IpAddr, Instant>>,
    /// Hosts lately that answered neither directly nor through a tunnel:
    /// several at once mean the network is in trouble (`network_trouble`).
    down_everywhere: Mutex<HashMap<IpAddr, Instant>>,
    /// New UDP conversations' first datagrams (copied as they left), to try
    /// their hosts through the tunnels with (`probe::udp_first`).
    datagrams: Mutex<HashMap<crate::flows::Flow, Datagram>>,
    /// Names from recent public DNS answers, so decisions can say a name.
    seen_names: Mutex<HashMap<IpAddr, (String, Instant)>>,
    changes: broadcast::Sender<String>,
}

/// How long a name no tunnel knew stays unasked.
const UNKNOWN_FOR: Duration = Duration::from_secs(300);

/// How long a probed host stays unprobed.
const PROBED_FOR: Duration = Duration::from_secs(600);

/// A network in trouble: this many other hosts answering nowhere within
/// `TROUBLE_WINDOW`. A public host is then tried again after `TROUBLE_RETRY`
/// instead of `PROBED_FOR`.
const TROUBLE_HOSTS: usize = 2;
const TROUBLE_WINDOW: Duration = Duration::from_secs(30);
const TROUBLE_RETRY: Duration = Duration::from_secs(60);

/// How long a probe through one tunnel may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
/// Does an address answer through a tunnel? Its web ports, each given a
/// third of the probe's time.
const WEB_PORTS: [u16; 2] = [443, 80];
const WEB_PROBE: Duration = Duration::from_secs(PROBE_TIMEOUT.as_secs() / 3);
/// Changes queued for the D-Bus signal before slow listeners miss some.
const CHANGES_BACKLOG: usize = 64;
/// How often the log is pruned to its retention.
const PRUNE_EVERY: Duration = Duration::from_secs(6 * crate::units::SECS_PER_HOUR);

/// How long a name from a public answer is remembered.
const SEEN_NAME_FOR: Duration = Duration::from_secs(3600);

/// How long to wait for one tunnel DNS server.
const TUNNEL_DNS_WAIT: Duration = Duration::from_millis(800);

/// How often each tunnel's DNS is asked whether it still answers (7h), how
/// long it gets, and how many silences in a row mean it stopped.
const DNS_CHECK_EVERY: Duration = Duration::from_secs(30);

const DNS_CHECK_WAIT: Duration = Duration::from_secs(3);

const DNS_CHECK_MISSES: u8 = 2;

/// How often an OpenVPN 3 tunnel's device is looked at.
const DEVICE_CHECK_EVERY: Duration = Duration::from_secs(2);

/// How often every tunnel's DNS settings in resolved are looked at: openvpn3
/// rewrites them when any of its sessions reconnects, and an app that looks
/// a name up in that moment (Firefox's proxy auto-detect, right after the
/// network change) sends it to the company. One property read per tunnel.
const DNS_LINKS_EVERY: Duration = Duration::from_millis(200);

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

/// "ACME", "ACME and Nordbank", "ACME, Globex and Nordbank".
fn and_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn join(addrs: &[IpAddr]) -> String {
    addrs
        .iter()
        .map(IpAddr::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn th_name(store: &HostStore, tunnel: &str, addr: IpAddr) -> Option<String> {
    store.tunnels.get(tunnel)?.names.get(&addr).cloned()
}

fn snapshot(e: &Entry) -> Snapshot {
    Snapshot {
        interface: interface_name(&e.config.name),
        config: e.config.clone(),
        state: e.state,
        info: e.info.clone(),
        hosts: 0,
        signed_in: e.session.is_some(),
        dns_down: false,
        clash: None,
        since: e.up_since,
        transport: None,
        why: e.why.clone(),
    }
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
                    handover: false,
                    attached: None,
                    why: None,
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
        let (changes, _) = broadcast::channel(CHANGES_BACKLOG);
        Ok(Arc::new(Self {
            cfg,
            tunnels: Mutex::new(tunnels),
            hosts: Mutex::new(hosts),
            networks: Mutex::new(networks),
            settings: Mutex::new(settings),
            hush: tokio::sync::Mutex::new(()),
            hush_serving: std::sync::atomic::AtomicBool::new(false),
            hush_last: Mutex::new(None),
            dns_down: Mutex::new(HashMap::new()),
            dns_misses: Mutex::new(HashMap::new()),
            keep_serving: std::sync::atomic::AtomicBool::new(false),
            keep: tokio::sync::Mutex::new(()),
            ops: tokio::sync::Mutex::new(()),
            log,
            net,
            routing,
            resolved,
            encrypted_dns_said: std::sync::atomic::AtomicBool::new(false),
            traffic: Arc::default(),
            transport: Arc::default(),
            unknown: Mutex::new(HashMap::new()),
            contested: Mutex::new(HashMap::new()),
            asked: Mutex::new(HashMap::new()),
            no_split: Mutex::new(HashMap::new()),
            probed: Mutex::new(HashMap::new()),
            down_everywhere: Mutex::new(HashMap::new()),
            datagrams: Mutex::new(HashMap::new()),
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
                tokio::time::sleep(PRUNE_EVERY).await;
            }
        });
        self.traffic
            .spawn(self.cfg.state_path.with_file_name("traffic.json"));
        if self.routing.is_real()
            && let Err(e) = crate::dns_queue::spawn(Arc::clone(self))
        {
            // Not fatal: names still resolve, they're just not routed ahead.
            tracing::error!("DNS learning is off: {e:#}");
        }
        if self.routing.is_real()
            && self.cfg.probing
            && let Err(e) = crate::probe::spawn(Arc::clone(self))
        {
            tracing::error!("connection probing is off: {e:#}");
        }
        // The first SYN of a new connection, held so even the first one is
        // routed (Linux NFQUEUE; a no-op elsewhere).
        if self.routing.is_real()
            && self.cfg.probing
            && let Err(e) = crate::syn_queue::spawn(Arc::clone(self))
        {
            tracing::error!("the first-SYN race is off: {e:#}");
        }
        // OpenVPN 3 sessions that ended behind the app's back (their device
        // is gone), and domains another VPN client now serves (not kept
        // private then). Tunnels' DNS settings another program rewrote, at
        // once.
        if self.routing.is_real() {
            let this = Arc::clone(self);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(DNS_LINKS_EVERY).await;
                    this.keep_dns_links().await;
                }
            });
            let this = Arc::clone(self);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(DEVICE_CHECK_EVERY).await;
                    this.check_devices().await;
                    this.refresh_hush().await;
                    // The Traffic window closed a while ago: conntrack's
                    // accounting back off, if craked switched it on.
                    crate::conntrack::connections_idle(false);
                }
            });
        }
        // Each tunnel's DNS, asked now and then whether it still answers.
        if self.routing.is_real() && self.cfg.use_resolved {
            let this = Arc::clone(self);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(DNS_CHECK_EVERY).await;
                    this.check_dns().await;
                }
            });
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
        self.forget_after(name).unwrap_or(crate::config::NEVER_DAYS) * crate::units::SECS_PER_DAY
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
    fn never(&self, name: Option<&str>, addr: impl Into<IpAddr>) -> bool {
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

    /// Every setting, in its text form (the D-Bus API).
    pub fn settings(&self) -> std::collections::HashMap<String, String> {
        self.settings.lock().unwrap().to_map()
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
        let down = self.dns_down.lock().unwrap().clone();
        for s in &mut list {
            if matches!(s.state, TunnelState::Off | TunnelState::Trouble) {
                s.clash = self.remembered_clash(&s.config.name);
            }
            s.dns_down = s.state == TunnelState::Connected && down.contains_key(&s.config.name);
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

    fn label(&self, name: &str) -> String {
        self.tunnels
            .lock()
            .unwrap()
            .get(name)
            .map_or_else(|| name.to_owned(), |e| e.config.display().to_owned())
    }

    /// The tunnel's device: the one it came up on, or the one openconnect is
    /// told to make (`crake-<name>`).
    pub(crate) fn device_of(&self, name: &str) -> String {
        self.tunnels
            .lock()
            .unwrap()
            .get(name)
            .and_then(|e| e.info.as_ref().map(|i| i.device.clone()))
            .unwrap_or_else(|| interface_name(name))
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
            if state != TunnelState::Trouble {
                e.why = None;
            }
            if clear_info {
                if let Some(info) = e.info.take() {
                    self.traffic.untrack(&info.device);
                }
                e.up_since = None;
            }
        }
        let _ = self.changes.send(name.to_owned());
    }

    /// The tunnel is in trouble, and why (crake_proto::trouble).
    fn set_trouble(&self, name: &str, why: Option<String>) {
        if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
            e.why = why;
        }
        self.set_state(name, TunnelState::Trouble, true);
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
}
