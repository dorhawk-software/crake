use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, bail};
use clap::Parser;
use crake_proto::TunnelState;
use craked::config::{Config, DEFAULT_CONFIG};
use craked::manager::Manager;
use craked::{auth, dbus, hook_server};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;

#[derive(Parser)]
#[command(version, about = "Crake system daemon")]
struct Args {
    #[arg(long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,
    /// Use the session bus and skip polkit (development only).
    #[arg(long)]
    session_bus: bool,
    /// Try one tunnel from a terminal: sign in here, connect, and switch it off
    /// on Ctrl+C. No D-Bus. For testing without installing anything.
    #[arg(long, value_name = "TUNNEL")]
    connect: Option<String>,
    /// Development, nothing installed: serve the Crake app over a private
    /// socket for the user who ran sudo (no system bus). The app signs in
    /// and switches tunnels; Ctrl+C or SIGTERM switches everything off.
    #[arg(long, conflicts_with = "connect")]
    dev: bool,
    /// Windows: run in this console rather than as the craked service.
    #[cfg(windows)]
    #[arg(long)]
    console: bool,
    /// Windows, craked's own use: send a console event (c or break) to an
    /// openconnect it started, from this separate process.
    #[cfg(windows)]
    #[arg(long, num_args = 2, value_names = ["PID", "EVENT"], hide = true)]
    console_event: Option<Vec<String>>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    #[cfg(windows)]
    if let Some([pid, event]) = args.console_event.as_deref() {
        let sent = pid
            .parse()
            .is_ok_and(|pid| craked::sys::deliver_console_event(pid, event));
        std::process::exit(if sent { 0 } else { 1 });
    }
    #[cfg(windows)]
    if !args.console && args.connect.is_none() {
        return service::run(args);
    }
    log_to(None);
    run(args, None)
}

/// Log to stderr, or to `file` (a service has no console).
fn log_to(file: Option<std::fs::File>) {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    match file {
        Some(file) => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .init(),
        None => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
}

/// The daemon, until it's stopped: SIGTERM or Ctrl+C, or `external` (the
/// Windows service manager).
#[tokio::main]
async fn run(args: Args, external: Option<oneshot::Receiver<()>>) -> Result<()> {
    let cfg = if args.config.exists() {
        Config::load(&args.config)?
    } else if args.connect.is_some() {
        bail!("{} not found", args.config.display());
    } else {
        tracing::warn!(
            "{} not found; starting with no tunnels",
            args.config.display()
        );
        Config::default()
    };
    let socket = cfg.hook_socket.clone();
    let mgr = Manager::new(cfg)?;
    mgr.init().await?;
    let _hook = hook_server::serve(mgr.clone(), &socket)?;

    let mut stop = Stop::new(external)?;

    if let Some(name) = args.connect {
        let app = match serve_app(&mgr).await {
            Ok(app) => {
                println!("The Crake app can connect now (cargo run -p crake).");
                app
            }
            Err(e) => {
                eprintln!("No app connection: {e:#}");
                None
            }
        };
        let result = try_tunnel(&mgr, &name, &mut stop).await;
        mgr.shutdown().await;
        let _ = std::fs::remove_file(&socket);
        if let Some(path) = &app {
            let _ = std::fs::remove_file(path);
        }
        println!("Log: {}", mgr.config().log_path.display());
        // Exit now: the runtime would otherwise wait for the terminal reader,
        // which is blocked until the next line of input.
        if let Err(e) = result {
            eprintln!("Error: {e:#}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    if args.dev {
        let app = serve_app(&mgr).await?;
        println!("Crake is running for the app (cargo run -p crake). Ctrl+C stops it.");
        stop.recv().await;
        println!("Switching everything off...");
        mgr.shutdown().await;
        let _ = std::fs::remove_file(&socket);
        if let Some(path) = &app {
            let _ = std::fs::remove_file(path);
        }
        // The runtime would wait for nothing useful; end here.
        std::process::exit(0);
    }

    #[cfg(target_os = "linux")]
    let _bus = dbus::serve(mgr.clone(), !args.session_bus).await?;
    #[cfg(target_os = "macos")]
    dbus::serve_console(mgr.clone(), std::path::Path::new(crake_proto::APP_SOCKET))?;
    #[cfg(windows)]
    dbus::serve_local(mgr.clone(), crake_proto::APP_ADDRESS).await?;
    tracing::info!("craked {} ready", env!("CARGO_PKG_VERSION"));
    // A package upgrade: the tunnels the previous craked handed over.
    mgr.resume_after_update().await;
    stop.recv().await;
    tracing::info!("shutting down");
    mgr.shutdown().await;
    let _ = std::fs::remove_file(&socket);
    Ok(())
}

/// What stops craked: SIGTERM (systemd) or Ctrl+C, or the service manager
/// (Windows).
struct Stop {
    #[cfg(unix)]
    term: tokio::signal::unix::Signal,
    #[cfg(unix)]
    int: tokio::signal::unix::Signal,
    external: Option<oneshot::Receiver<()>>,
}

impl Stop {
    #[cfg(unix)]
    fn new(external: Option<oneshot::Receiver<()>>) -> Result<Self> {
        use tokio::signal::unix::{SignalKind, signal};
        Ok(Self {
            term: signal(SignalKind::terminate())?,
            int: signal(SignalKind::interrupt())?,
            external,
        })
    }

    #[cfg(windows)]
    fn new(external: Option<oneshot::Receiver<()>>) -> Result<Self> {
        Ok(Self { external })
    }

    #[cfg(unix)]
    async fn recv(&mut self) {
        tokio::select! {
            _ = self.term.recv() => {}
            _ = self.int.recv() => {}
            () = external(&mut self.external) => {}
        }
    }

    #[cfg(windows)]
    async fn recv(&mut self) {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            () = external(&mut self.external) => {}
        }
    }
}

/// The service manager's stop, or never.
async fn external(rx: &mut Option<oneshot::Receiver<()>>) {
    match rx {
        Some(rx) => {
            let _ = rx.await;
        }
        None => std::future::pending().await,
    }
}

/// Serve the Crake app for a run started by hand. Linux: a private socket
/// for the user who ran sudo, returned to be removed when done. Windows and
/// macOS: where the service serves it.
#[cfg(target_os = "linux")]
async fn serve_app(mgr: &Arc<Manager>) -> Result<Option<PathBuf>> {
    let Some((path, uid)) = ui_socket() else {
        bail!("the app needs sudo (SUDO_UID) and the user's /run/user directory");
    };
    dbus::serve_p2p(mgr.clone(), &path, uid)?;
    Ok(Some(path))
}

#[cfg(target_os = "macos")]
async fn serve_app(mgr: &Arc<Manager>) -> Result<Option<PathBuf>> {
    let path = PathBuf::from(crake_proto::APP_SOCKET);
    dbus::serve_console(mgr.clone(), &path)?;
    Ok(Some(path))
}

#[cfg(windows)]
async fn serve_app(mgr: &Arc<Manager>) -> Result<Option<PathBuf>> {
    dbus::serve_local(mgr.clone(), crake_proto::APP_ADDRESS).await?;
    Ok(None)
}

/// `/run/user/<uid>/crake-dev.sock` for the user who ran sudo.
#[cfg(target_os = "linux")]
fn ui_socket() -> Option<(PathBuf, u32)> {
    let uid: u32 = std::env::var("SUDO_UID").ok()?.parse().ok()?;
    let dir = PathBuf::from(format!("/run/user/{uid}"));
    dir.is_dir().then(|| (dir.join("crake-dev.sock"), uid))
}

async fn try_tunnel(mgr: &Arc<Manager>, name: &str, stop: &mut Stop) -> Result<()> {
    let Some(snap) = mgr.get(name) else {
        bail!("no tunnel named {name} in the config");
    };
    let label = snap.config.display().to_owned();
    println!(
        "Signing in to {label} ({}). Answer openconnect's questions below.",
        snap.config.gateway
    );
    let session = tokio::select! {
        s = auth::authenticate_on_terminal(&mgr.config().openconnect, &snap.config) => s?,
        () = stop.recv() => return Ok(()),
    };
    println!("Signed in. Connecting {label}...");

    let mut changes = mgr.subscribe();
    mgr.connect_with(name, Some(session))?;
    let mut was_up = false;
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    loop {
        tokio::select! {
            () = stop.recv() => break,
            line = input.next_line() => match line {
                Ok(Some(line)) => command(mgr, name, &line).await,
                // stdin closed: keep running until a signal.
                _ => std::future::pending::<()>().await,
            },
            changed = changes.recv() => {
                if changed.as_deref().ok() != Some(name) { continue }
                let Some(s) = mgr.get(name) else { break };
                match s.state {
                    TunnelState::Connected => {
                        report(&label, &s);
                        was_up = true;
                    }
                    TunnelState::Connecting if was_up => println!("{label} is reconnecting..."),
                    TunnelState::Connecting => {}
                    TunnelState::Off => { println!("{label} is off."); return Ok(()) }
                    TunnelState::Trouble => bail!("{label} stopped unexpectedly. See the log."),
                }
            }
        }
    }
    println!("\nSwitching {label} off...");
    Ok(())
}

fn report(label: &str, s: &craked::manager::Snapshot) {
    let Some(i) = &s.info else { return };
    let addr = i
        .address
        .map_or("no IPv4 address".into(), |a| a.to_string());
    println!("\n{label} is up. Routing nothing yet.");
    println!("  interface    {}", s.interface);
    println!("  address      {addr}");
    for d in &i.dns {
        println!("  DNS          {d}");
    }
    if !i.domains.is_empty() {
        println!(
            "  names        {}  (asked of {label}'s DNS)",
            i.domains.join(", ")
        );
    }
    println!(
        "  pushed nets  {}  (hints, not routed)",
        i.split_include.len()
    );
    println!("\nType help for commands. Press Ctrl+C to switch {label} off.");
}

const HELP: &str = "\
  add <ip>              send one host through the tunnel (expires without traffic)
  pin <ip or net/len>   keep a host or network on the tunnel
  unpin <ip or net/len> a pinned host goes back to expiring; a network goes direct
    forget <ip or net/len>  stop sending it through the tunnel, now
  undo <ip>             send a learned host direct, and keep it direct
  redo <ip>             take back an undo
  list                  what goes through the tunnel";

async fn command(mgr: &Manager, tunnel: &str, line: &str) {
    let mut words = line.split_whitespace();
    let (Some(cmd), arg) = (words.next(), words.next()) else {
        return;
    };
    let result = match (cmd, arg) {
        ("add", Some(a)) => mgr.add_host(tunnel, a).await,
        ("pin", Some(a)) => mgr.pin(tunnel, a).await,
        ("unpin", Some(a)) => mgr.unpin(tunnel, a).await,
        ("forget", Some(a)) => mgr.forget(tunnel, a).await,
        ("undo", Some(a)) => mgr.undo(tunnel, &[a]).await,
        ("redo", Some(a)) => mgr.redo(tunnel, &[a]).await,
        ("list", None) => mgr.hosts(tunnel).await.map(|rows| {
            if rows.is_empty() {
                return "Nothing goes through the tunnel.".to_owned();
            }
            rows.iter()
                .map(|r| {
                    let what = match (r.pinned, r.direct, r.expires_in) {
                        (true, _, _) => "pinned".to_owned(),
                        (_, true, _) => "direct (you undid it)".to_owned(),
                        (_, _, Some(s)) => format!("learned, goes direct in {}", human(s)),
                        _ => String::new(),
                    };
                    let name = r.name.as_deref().unwrap_or("");
                    format!("  {:<18} {:<28} {what}", r.target.to_string(), name)
                })
                .collect::<Vec<_>>()
                .join("\n")
        }),
        _ => Ok(HELP.to_owned()),
    };
    match result {
        Ok(text) => println!("{text}"),
        Err(e) => println!("{e:#}"),
    }
}

fn human(secs: u64) -> String {
    match secs {
        s if s >= 86_400 => format!("{}d {}h", s / 86_400, s % 86_400 / 3600),
        s if s >= 3600 => format!("{}h {}m", s / 3600, s % 3600 / 60),
        s => format!("{}m", s.div_ceil(60)),
    }
}

/// Windows: craked as a service (`crake_proto::SERVICE_NAME`), logging to
/// craked.log beside its config.
#[cfg(windows)]
mod service {
    use std::ffi::OsString;
    use std::sync::Mutex;
    use std::time::Duration;

    use anyhow::Result;
    use crake_proto::SERVICE_NAME;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};

    /// The arguments, from `run` to the service's own thread.
    static ARGS: Mutex<Option<super::Args>> = Mutex::new(None);

    define_windows_service!(ffi_service_main, service_main);

    /// Run under the service manager. Started any other way (a console),
    /// run there instead.
    pub fn run(args: super::Args) -> Result<()> {
        *ARGS.lock().unwrap() = Some(args);
        match service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
            Ok(()) => Ok(()),
            // ERROR_FAILED_SERVICE_CONTROLLER_CONNECT: not a service.
            Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1063) => {
                super::log_to(None);
                let args = ARGS.lock().unwrap().take().expect("set above");
                super::run(args, None)
            }
            Err(e) => Err(e.into()),
        }
    }

    fn service_main(_: Vec<OsString>) {
        let log = std::path::Path::new(super::DEFAULT_CONFIG).with_file_name("craked.log");
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        super::log_to(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
                .ok(),
        );
        if let Err(e) = serve() {
            tracing::error!("craked: {e:#}");
        }
    }

    fn serve() -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Mutex::new(Some(tx));
        let status =
            service_control_handler::register(SERVICE_NAME, move |control| match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            })?;
        let report = |state, accept| {
            status.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: accept,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::from_secs(15),
                process_id: None,
            })
        };
        report(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        )?;
        let args = ARGS.lock().unwrap().take().expect("set by run");
        let result = super::run(args, Some(rx));
        report(ServiceState::Stopped, ServiceControlAccept::empty())?;
        result
    }
}
