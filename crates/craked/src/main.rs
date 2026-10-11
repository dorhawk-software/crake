use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, bail};
use clap::Parser;
use crake_proto::TunnelState;
use craked::config::{Config, DEFAULT_CONFIG};
use craked::manager::Manager;
use craked::{auth, dbus, hook_server};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::signal::unix::{SignalKind, signal};

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
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();

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

    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;

    if let Some(name) = args.connect {
        // The Crake app (run as the user who ran sudo) talks to this run over
        // a private socket in their runtime directory.
        let ui_socket = ui_socket();
        if let Some((path, uid)) = &ui_socket {
            match dbus::serve_p2p(mgr.clone(), path, *uid) {
                Ok(()) => println!("The Crake app can connect now (cargo run -p crake)."),
                Err(e) => eprintln!("No app connection: {e:#}"),
            }
        }
        let result = try_tunnel(&mgr, &name, &mut term, &mut int).await;
        mgr.shutdown().await;
        let _ = std::fs::remove_file(&socket);
        if let Some((path, _)) = &ui_socket {
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
        let Some((path, uid)) = ui_socket() else {
            bail!("--dev needs sudo (SUDO_UID) and the user's /run/user directory");
        };
        dbus::serve_p2p(mgr.clone(), &path, uid)?;
        println!("Crake is running for the app (cargo run -p crake). Ctrl+C stops it.");
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
        println!("Switching everything off...");
        mgr.shutdown().await;
        let _ = std::fs::remove_file(&socket);
        let _ = std::fs::remove_file(&path);
        // The runtime would wait for nothing useful; end here.
        std::process::exit(0);
    }

    let _bus = dbus::serve(mgr.clone(), !args.session_bus).await?;
    tracing::info!("craked {} ready", env!("CARGO_PKG_VERSION"));
    // A package upgrade: the tunnels the previous craked handed over.
    mgr.resume_after_update().await;
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
    tracing::info!("shutting down");
    mgr.shutdown().await;
    let _ = std::fs::remove_file(&socket);
    Ok(())
}

/// `/run/user/<uid>/crake-dev.sock` for the user who ran sudo.
fn ui_socket() -> Option<(PathBuf, u32)> {
    let uid: u32 = std::env::var("SUDO_UID").ok()?.parse().ok()?;
    let dir = PathBuf::from(format!("/run/user/{uid}"));
    dir.is_dir().then(|| (dir.join("crake-dev.sock"), uid))
}

async fn try_tunnel(
    mgr: &Arc<Manager>,
    name: &str,
    term: &mut tokio::signal::unix::Signal,
    int: &mut tokio::signal::unix::Signal,
) -> Result<()> {
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
        _ = int.recv() => return Ok(()),
    };
    println!("Signed in. Connecting {label}...");

    let mut changes = mgr.subscribe();
    mgr.connect_with(name, Some(session))?;
    let mut was_up = false;
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    loop {
        tokio::select! {
            _ = term.recv() => break,
            _ = int.recv() => break,
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
