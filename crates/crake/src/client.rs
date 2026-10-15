//! The connection to craked, on its own thread with its own runtime: the
//! system bus normally, or the private socket a development run of craked
//! opens in the user's runtime directory. Keeps trying while craked is away.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Traffic is fetched this often; the log's last LOG_ENTRIES entries are
/// enough for the log window's days (kept 30 days).
const TRAFFIC_EVERY: Duration = Duration::from_secs(20);
const LOG_ENTRIES: u32 = 5000;

use crake_proto::{BUS_NAME, BUS_PATH, LogEntry};
use futures_util::StreamExt;
use zbus::Connection;
use zbus::zvariant::OwnedValue;

/// What the UI asks of craked.
#[derive(Clone, Debug)]
pub enum Command {
    Connect(String),
    /// Connect with a sign-in done here (keys as `Session::to_map`).
    ConnectWithSession(String, crake_proto::Session),
    Disconnect(String),
    Undo(String, Vec<String>),
    Redo(String, Vec<String>),
    /// Send hosts through another tunnel that answered too (from, to, hosts).
    Move(String, String, Vec<String>),
    /// Send hosts through a tunnel (an "Ask me first" answer).
    AddHosts(String, Vec<String>),
    /// Keep hosts or networks on a tunnel.
    Pin(String, Vec<String>),
    Unpin(String, String),
    Forget(String, String),
    ForgetLearned(String),
    DismissTrouble(String),
    /// An OpenVPN 3 session's process outlived its session (tunnel or "",
    /// profile, process).
    LeftRunning(String, String, u32),
    /// Tunnel detail: its hosts and its traffic today.
    Hosts(String),
    TunnelTraffic(String),
    /// The Traffic window: the counters right now.
    Live,
    /// OpenVPN 3: route through a session's device (its fields) / it ended.
    Attach(String, HashMap<String, String>),
    Detach(String, String),
    /// New tunnel: its fields; Settings → Tunnels: change or remove one.
    AddTunnel(HashMap<String, String>),
    UpdateTunnel(String, HashMap<String, String>),
    RemoveTunnel(String),
    /// A Settings window value, in its text form.
    SetSetting(String, String),
    ClearLog,
}

/// What craked tells the UI.
#[derive(Clone, Debug)]
pub enum Update {
    /// craked can be reached (or not).
    Reachable(bool),
    /// craked's version, as it starts serving ("" when it doesn't say).
    ServiceVersion(String),
    Tunnels(Vec<Tunnel>),
    /// The recent log, oldest first (on every new connection).
    Log(Vec<LogEntry>),
    /// One new entry, as it's written.
    Appended(LogEntry),
    /// A command failed; the message is craked's sentence.
    Failed(String),
    /// Direct traffic per minute, last hour.
    Traffic(Vec<(u64, u64)>),
    /// All traffic per minute, two days (the log's day chart).
    Days(Vec<(u64, u64)>),
    /// craked's settings, each in its text form.
    Settings(HashMap<String, String>),
    /// A tunnel was added (its name), from New tunnel.
    Added(String),
    /// A tunnel's hosts (craked's HostDetails rows).
    Hosts(String, Vec<HashMap<String, String>>),
    /// Bytes through a tunnel per minute, two days.
    TunnelTraffic(String, Vec<(u64, u64)>),
    /// The counters right now (the Traffic window).
    Live(crake_proto::live::Live),
    /// craked wouldn't route through an OpenVPN 3 session (it clashes with
    /// a tunnel that's up): the app ends it.
    AttachRefused(String),
    /// craked's view of the computer, line by line, for helpdesk copies.
    Diagnostics(Vec<String>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tunnel {
    pub name: String,
    pub display: String,
    /// off | connecting | connected | trouble
    pub state: String,
    pub hosts: u32,
    pub address: String,
    pub gateway: String,
    pub protocol: String,
    pub user: String,
    pub auth_args: Vec<String>,
    pub extra_args: Vec<String>,
    /// craked keeps a sign-in: switching on needs no new one.
    pub signed_in: bool,
    /// From IT (managed.d): read-only, with its policy in plain words.
    pub managed: bool,
    /// "ACME IT allows learning for *.acme.internal" (empty for your own).
    pub policy: String,
    pub full_tunnel: bool,
    pub managed_file: String,
    /// A managed tunnel's scope: names and networks it may learn.
    pub learn: Vec<String>,
    /// Domains whose names go to its DNS (pushed, plus configured).
    pub dns_domains: Vec<String>,
    /// "app" (added in Crake: may be changed), "config" or "managed".
    pub source: String,
    /// Networks the gateway pushed when it last connected (hints).
    pub split_include: Vec<String>,
    /// Who set a managed tunnel up ("ACME IT").
    pub organisation: String,
    /// An app tunnel's fields in text form (AddTunnel's keys), for editing.
    pub own: HashMap<String, String>,
    /// When it came up (seconds since the epoch; 0 when off).
    pub since: u64,
    /// How its packets travel now: DTLS, ESP, TLS, UDP, TCP (or unknown).
    pub transport: String,
    /// Its DNS servers, while up.
    pub dns: Vec<String>,
    /// Why it can't be switched on now: "Turn ACME off to use Nordbank.
    /// Both use 10.0.0.2 for DNS." (empty when it can).
    pub clash: String,
    /// Its DNS doesn't answer (craked answers known names from memory).
    pub dns_down: bool,
    /// In trouble: why, as the log says it (crake_proto::trouble).
    pub why: String,
}

pub fn start(updates: async_channel::Sender<Update>) -> async_channel::Sender<Command> {
    spawn(None, updates)
}

/// The same, always on this socket (tests: a craked of their own).
#[cfg(all(test, target_os = "linux"))]
pub fn start_on(
    socket: PathBuf,
    updates: async_channel::Sender<Update>,
) -> async_channel::Sender<Command> {
    spawn(Some(socket), updates)
}

fn spawn(
    socket: Option<PathBuf>,
    updates: async_channel::Sender<Update>,
) -> async_channel::Sender<Command> {
    let (commands, incoming) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("craked-client".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime for the D-Bus client");
            rt.block_on(run(socket, updates, incoming));
        })
        .expect("a thread for the D-Bus client");
    commands
}

/// The development socket, if a development craked is running.
fn dev_socket() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CRAKE_SOCKET") {
        return Some(p.into());
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let p = PathBuf::from(dir).join("crake-dev.sock");
    p.exists().then_some(p)
}

async fn connect(socket: Option<&Path>) -> zbus::Result<(Connection, bool)> {
    match socket.map(Path::to_path_buf).or_else(dev_socket) {
        Some(path) => Ok((crate::sys::connect_dev(&path).await?, true)),
        None => crate::sys::connect_craked().await,
    }
}

async fn run(
    socket: Option<PathBuf>,
    updates: async_channel::Sender<Update>,
    commands: async_channel::Receiver<Command>,
) {
    let mut reachable = None;
    loop {
        let result = session(socket.as_deref(), &updates, &commands, &mut reachable).await;
        if let Err(e) = result {
            tracing_like(&format!("craked: {e}"));
        }
        if reachable != Some(false) {
            reachable = Some(false);
            if updates.send(Update::Reachable(false)).await.is_err() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn tracing_like(msg: &str) {
    if std::env::var_os("CRAKE_DEBUG").is_some() {
        eprintln!("{msg}");
    }
}

struct Api {
    conn: Connection,
    p2p: bool,
}

impl Api {
    async fn call<B, R>(&self, method: &str, body: &B) -> zbus::Result<R>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
    {
        let dest = (!self.p2p).then_some(BUS_NAME);
        let reply = self
            .conn
            .call_method(dest, BUS_PATH, Some(BUS_NAME), method, body)
            .await?;
        reply.body().deserialize()
    }

    async fn tunnels(&self) -> zbus::Result<Vec<Tunnel>> {
        let raw: Vec<HashMap<String, OwnedValue>> = self.call("ListTunnels", &()).await?;
        Ok(raw.iter().map(tunnel).collect())
    }

    /// craked's version (its `Version` property).
    async fn version(&self) -> zbus::Result<String> {
        let dest = (!self.p2p).then_some(BUS_NAME);
        let reply = self
            .conn
            .call_method(
                dest,
                BUS_PATH,
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(BUS_NAME, "Version"),
            )
            .await?;
        let value: OwnedValue = reply.body().deserialize()?;
        String::try_from(value).map_err(zbus::Error::Variant)
    }

    async fn log(&self, count: u32) -> zbus::Result<Vec<LogEntry>> {
        let raw: Vec<String> = self.call("RecentLog", &(count,)).await?;
        Ok(raw
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }
}

fn tunnel(d: &HashMap<String, OwnedValue>) -> Tunnel {
    let text = |k: &str| {
        d.get(k)
            .and_then(|v| String::try_from(v.clone()).ok())
            .unwrap_or_default()
    };
    let strings = |k: &str| {
        d.get(k)
            .and_then(|v| Vec::<String>::try_from(v.clone()).ok())
            .unwrap_or_default()
    };
    let flag = |k: &str| {
        d.get(k)
            .and_then(|v| bool::try_from(v.clone()).ok())
            .unwrap_or(false)
    };
    Tunnel {
        name: text("name"),
        display: text("display_name"),
        state: text("state"),
        hosts: d
            .get("hosts")
            .and_then(|v| u32::try_from(v.clone()).ok())
            .unwrap_or(0),
        address: text("address"),
        gateway: text("gateway"),
        protocol: text("protocol"),
        user: text("user"),
        auth_args: strings("auth_args"),
        extra_args: strings("extra_args"),
        signed_in: flag("signed_in"),
        managed: flag("managed"),
        policy: text("policy"),
        full_tunnel: flag("full_tunnel"),
        managed_file: text("managed_file"),
        learn: strings("learn"),
        dns_domains: strings("dns_domains"),
        source: text("source"),
        split_include: strings("split_include"),
        organisation: text("organisation"),
        own: d
            .get("own")
            .and_then(|v| HashMap::<String, String>::try_from(v.clone()).ok())
            .unwrap_or_default(),
        since: d
            .get("since")
            .and_then(|v| u64::try_from(v.clone()).ok())
            .unwrap_or(0),
        transport: text("transport"),
        dns: strings("dns"),
        clash: text("clash"),
        dns_down: flag("dns_down"),
        why: text("why"),
    }
}

async fn session(
    socket: Option<&Path>,
    updates: &async_channel::Sender<Update>,
    commands: &async_channel::Receiver<Command>,
    reachable: &mut Option<bool>,
) -> zbus::Result<()> {
    let (conn, p2p) = connect(socket).await?;
    let api = Api {
        conn: conn.clone(),
        p2p,
    };
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(BUS_NAME)?
        .build();
    let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, None).await?;

    let send = |u: Update| async { updates.send(u).await.is_ok() };
    let mut traffic_tick = tokio::time::interval(TRAFFIC_EVERY);
    let tunnels = api.tunnels().await?;
    // Enough for the log window's days (kept for 30 days).
    let log = api.log(LOG_ENTRIES).await?;
    // Older craked without settings: the window shows the defaults.
    let settings = api
        .call::<_, HashMap<String, String>>("Settings", &())
        .await
        .unwrap_or_default();
    let version = api.version().await.unwrap_or_default();
    *reachable = Some(true);
    if !(send(Update::Reachable(true)).await
        && send(Update::ServiceVersion(version)).await
        && send(Update::Tunnels(tunnels)).await
        && send(Update::Log(log)).await
        && send(Update::Settings(settings)).await)
    {
        return Ok(());
    }

    loop {
        tokio::select! {
            _ = traffic_tick.tick() => {
                // Older craked without the counters: no bars, nothing else breaks.
                if let Ok(d) = api.call::<_, Vec<(u64, u64)>>("TrafficDays", &()).await
                    && !send(Update::Days(d)).await
                {
                    return Ok(());
                }
                if let Ok(t) = api.call::<_, Vec<(u64, u64)>>("TrafficLastHour", &()).await
                    && !send(Update::Traffic(t)).await
                {
                    return Ok(());
                }
                // craked's view of the computer, for helpdesk copies (an
                // older craked has none: nothing is sent).
                if let Ok(d) = api.call::<_, Vec<String>>("Diagnostics", &()).await
                    && !send(Update::Diagnostics(d)).await
                {
                    return Ok(());
                }
            }
            msg = signals.next() => {
                let Some(Ok(msg)) = msg else {
                    return Err(zbus::Error::Failure("the connection to craked ended".into()));
                };
                let header = msg.header();
                match header.member().map(|m| m.as_str()) {
                    Some("SettingsChanged") => {
                        let settings = api.call::<_, HashMap<String, String>>("Settings", &()).await?;
                        if !send(Update::Settings(settings)).await { return Ok(()); }
                    }
                    Some("TunnelChanged") => {
                        let tunnels = api.tunnels().await?;
                        if !send(Update::Tunnels(tunnels)).await { return Ok(()); }
                        // What changed with it: devices, DNS.
                        if let Ok(d) = api.call::<_, Vec<String>>("Diagnostics", &()).await
                            && !send(Update::Diagnostics(d)).await { return Ok(()); }
                    }
                    Some("LogAppended") => {
                        let json: String = msg.body().deserialize()?;
                        if let Ok(entry) = serde_json::from_str::<LogEntry>(&json)
                            && !send(Update::Appended(entry)).await { return Ok(()); }
                        // Host counts change with decisions.
                        let tunnels = api.tunnels().await?;
                        if !send(Update::Tunnels(tunnels)).await { return Ok(()); }
                    }
                    _ => {}
                }
            }
            cmd = commands.recv() => {
                let Ok(cmd) = cmd else { return Ok(()) };
                let result: zbus::Result<()> = match cmd {
                                        Command::Connect(t) => api.call::<_, ()>("Connect", &(t,)).await,
                    Command::ConnectWithSession(t, session) => {
                        let map: HashMap<String, String> = session.to_map().into_iter().collect();
                        api.call::<_, ()>("ConnectWithSession", &(t, map)).await
                    }
                    Command::Disconnect(t) => api.call::<_, ()>("Disconnect", &(t,)).await,
                    Command::Undo(t, hosts) => api.call::<_, String>("Undo", &(t, hosts)).await.map(drop),
                    Command::Redo(t, hosts) => api.call::<_, String>("Redo", &(t, hosts)).await.map(drop),
                    Command::Move(from, to, hosts) => {
                        api.call::<_, String>("MoveHosts", &(from, to, hosts)).await.map(drop)
                    }
                    Command::AddHosts(t, hosts) => {
                        let mut done = Ok(());
                        for h in hosts {
                            done = done.and(api.call::<_, String>("AddHost", &(t.clone(), h)).await.map(drop));
                        }
                        done
                    }
                    Command::Attach(t, fields) => match api.call::<_, ()>("Attach", &(t.clone(), fields)).await {
                        Err(e) => {
                            if !send(Update::AttachRefused(t)).await { return Ok(()); }
                            Err(e)
                        }
                        ok => ok,
                    },
                    Command::Detach(t, why) => api.call::<_, ()>("Detach", &(t, why)).await,
                    Command::Pin(t, targets) => {
                        let mut done = Ok(());
                        for x in targets {
                            done = done.and(api.call::<_, String>("Pin", &(t.clone(), x)).await.map(drop));
                        }
                        done
                    }
                    Command::Unpin(t, x) => api.call::<_, String>("Unpin", &(t, x)).await.map(drop),
                    Command::Forget(t, x) => api.call::<_, String>("Forget", &(t, x)).await.map(drop),
                    Command::ForgetLearned(t) => api.call::<_, String>("ForgetLearned", &(t,)).await.map(drop),
                    Command::DismissTrouble(t) => api.call::<_, ()>("DismissTrouble", &(t,)).await,
                    Command::LeftRunning(t, profile, pid) => api.call::<_, ()>("LeftRunning", &(t, profile, pid)).await,
                    Command::Hosts(t) => match api.call::<_, Vec<HashMap<String, String>>>("HostDetails", &(t.clone(),)).await {
                        Ok(rows) => {
                            if !send(Update::Hosts(t, rows)).await { return Ok(()); }
                            Ok(())
                        }
                        Err(e) => Err(e),
                    },
                    Command::Live => match api.call::<_, String>("Live", &()).await {
                        Ok(json) => {
                            let live = serde_json::from_str(&json).unwrap_or_default();
                            if !send(Update::Live(live)).await { return Ok(()); }
                            Ok(())
                        }
                        Err(e) => Err(e),
                    },
                    Command::TunnelTraffic(t) => match api.call::<_, Vec<(u64, u64)>>("TunnelTraffic", &(t.clone(),)).await {
                        Ok(minutes) => {
                            if !send(Update::TunnelTraffic(t, minutes)).await { return Ok(()); }
                            Ok(())
                        }
                        Err(e) => Err(e),
                    },
                    Command::AddTunnel(fields) => match api.call::<_, String>("AddTunnel", &(fields,)).await {
                        Ok(name) => {
                            if !send(Update::Added(name)).await { return Ok(()); }
                            Ok(())
                        }
                        Err(e) => Err(e),
                    },
                    Command::UpdateTunnel(t, fields) => api.call::<_, ()>("UpdateTunnel", &(t, fields)).await,
                    Command::RemoveTunnel(t) => api.call::<_, ()>("RemoveTunnel", &(t,)).await,
                    Command::SetSetting(key, value) => {
                        api.call::<_, ()>("SetSetting", &(key, value)).await
                    }
                    Command::ClearLog => {
                        let done = api.call::<_, ()>("ClearLog", &()).await;
                        if done.is_ok() {
                            let log = api.log(LOG_ENTRIES).await?;
                            if !send(Update::Log(log)).await { return Ok(()); }
                        }
                        done
                    }
                };
                if let Err(e) = result {
                    let text = match &e {
                        zbus::Error::MethodError(_, Some(text), _) => text.clone(),
                        other => other.to_string(),
                    };
                    if !send(Update::Failed(text)).await { return Ok(()); }
                }
            }
        }
    }
}
