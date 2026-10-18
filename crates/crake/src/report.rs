//! A report to send when something's wrong (design/proposals/report.html,
//! option A, chosen by the user): the log window's "Save a report…" and
//! `crake report` write the same plain-text file, always the same parts
//! whatever the log window shows: craked's tunnels, the computer's side
//! (craked's D-Bus `Report`: the internet and each tunnel's names tried
//! now, the system's DNS, routes and Crake's rules), the last 24 hours of
//! decisions and craked's own log. Sign-ins never go in (craked leaves them
//! out); with "Hide names and addresses when sharing" on, the hosts
//! reached become "a host" and the computer's plumbing stays. Crake sends
//! it nowhere.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crake_proto::LogEntry;
use crake_proto::report::{Computer, OWN_LOG_LINES};

use crate::client::{Api, connect};
use crate::model::{Keep, scrub_keeping};

/// How far back a report's decisions go.
const DECISIONS_FOR: Duration = Duration::from_secs(24 * 60 * 60);
/// Enough of the log for a day's decisions.
const LOG_ENTRIES: u32 = 5000;

/// Why the computer's side is missing, in the file's words.
const NOT_READ: &str = "this computer's DNS, routes and Crake's rules weren't read.";

/// What craked said, for one report.
#[derive(Debug)]
pub struct Gathered {
    /// craked's version ("" when it couldn't be asked).
    pub service: String,
    /// craked's line per tunnel (its `Diagnostics`).
    pub tunnels: Vec<String>,
    /// The log's last 24 hours, oldest first.
    pub decisions: Vec<LogEntry>,
    /// The computer's side, or the sentence saying why it's missing.
    pub computer: Result<Computer, String>,
}

impl Gathered {
    /// Nothing from craked, and why.
    fn away(why: String) -> Self {
        Self {
            service: String::new(),
            tunnels: Vec::new(),
            decisions: Vec::new(),
            computer: Err(why),
        }
    }
}

/// craked isn't there to ask.
fn not_running() -> String {
    if cfg!(target_os = "macos") {
        format!(
            "craked isn't running (Crake's service may still need your OK in System Settings): {NOT_READ}"
        )
    } else {
        format!("craked isn't running: {NOT_READ}")
    }
}

/// Everything craked can say, asked over a connection of its own to the
/// craked the app talks to.
pub async fn gather() -> Gathered {
    gather_from(crate::client::socket()).await
}

/// The same, from the craked on `socket` (none: the system's).
pub async fn gather_from(socket: Option<PathBuf>) -> Gathered {
    let away = Gathered::away;
    let Ok((conn, p2p)) = connect(socket.as_deref()).await else {
        return away(not_running());
    };
    let api = Api { conn, p2p };
    let service = match api.version().await {
        Ok(v) => v,
        Err(e) if is_absent(&e) => return away(not_running()),
        Err(_) => String::new(),
    };
    let tunnels: Vec<String> = api.call("Diagnostics", &()).await.unwrap_or_default();
    let since = jiff::Timestamp::now() - DECISIONS_FOR;
    let decisions = api
        .log(LOG_ENTRIES)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.ts.parse::<jiff::Timestamp>().is_ok_and(|t| t >= since))
        .collect();
    let computer = match api.call::<_, String>("Report", &()).await {
        Ok(json) => serde_json::from_str(&json)
            .map_err(|e| format!("craked's answer didn't read ({e}): {NOT_READ}")),
        Err(zbus::Error::MethodError(name, ..)) if name.contains("AccessDenied") => Err(format!(
            "Crake's service tells only the person at the screen, or an administrator: {NOT_READ}"
        )),
        Err(zbus::Error::MethodError(name, ..)) if name.contains("UnknownMethod") => Err(format!(
            "craked {service} is older than this Crake and can't say: {NOT_READ}"
        )),
        Err(e) => Err(format!("craked couldn't be asked ({e}): {NOT_READ}")),
    };
    Gathered {
        service,
        tunnels,
        decisions,
        computer,
    }
}

/// The system bus has no craked on it.
fn is_absent(e: &zbus::Error) -> bool {
    matches!(e, zbus::Error::MethodError(name, ..)
        if name.contains("ServiceUnknown") || name.contains("NameHasNoOwner"))
}

/// `gather`, on a thread and runtime of its own: the app's windows keep
/// going meanwhile, and `crake report` has no runtime.
pub fn gather_apart() -> async_channel::Receiver<Gathered> {
    let (tx, rx) = async_channel::bounded(1);
    // A thread that can't start drops `tx`: the caller reads a closed
    // channel and says so.
    let _ = std::thread::Builder::new()
        .name("crake-report".into())
        .spawn(move || {
            let gathered = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_or_else(
                    |e| Gathered::away(format!("Crake couldn't ask ({e}): {NOT_READ}")),
                    |rt| rt.block_on(gather()),
                );
            let _ = tx.send_blocking(gathered);
        });
    rx
}

/// Who gathers for the log window: craked (`gather_apart`, when there's
/// none of these), or a window test's stand-in.
#[derive(Clone)]
pub struct Gatherer(pub std::rc::Rc<dyn Fn() -> async_channel::Receiver<Gathered>>);

impl gpui_kit::Global for Gatherer {}

/// The file's name: crake-report-2026-10-18.txt.
pub fn file_name(now: &jiff::Zoned) -> String {
    format!("crake-report-{}.txt", now.strftime("%Y-%m-%d"))
}

/// Where a report goes unless told otherwise: Documents, else home.
pub fn default_folder() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    [home.join("Documents"), home.clone()]
        .into_iter()
        .find(|d| d.is_dir())
        .unwrap_or(home)
}

/// The file, as text.
pub fn text(g: &Gathered, hide: bool, now: &jiff::Zoned) -> String {
    let keep = Keep::new(g.computer.as_ref().map_or(&[][..], |c| &c.keep));
    let say = |t: &str| {
        if hide {
            scrub_keeping(t, &keep)
        } else {
            t.to_owned()
        }
    };
    let mut s = format!(
        "Crake report, {}\n{}\nHost names and addresses: {} (Settings \u{2192} Privacy & log).\n",
        now.strftime("%-d %B %Y %H:%M"),
        about(&g.service),
        if hide { "hidden" } else { "shown" },
    );
    let missing = g.computer.as_ref().err();
    let mut section = |title: &str, lines: &mut dyn Iterator<Item = String>, empty: &str| {
        s += &format!("\n{title}\n");
        let mut any = false;
        for l in lines {
            any = true;
            s += &format!("  {}\n", say(&l));
        }
        if !any {
            s += &format!("  {empty}\n");
        }
    };
    let away = missing.map_or("(none)", |_| "(craked couldn't be asked)");
    section("Tunnels", &mut g.tunnels.iter().cloned(), away);
    match &g.computer {
        Ok(c) => {
            section(
                "This computer",
                &mut c.checks.iter().cloned(),
                "(nothing tried)",
            );
            for part in &c.parts {
                section(&part.title, &mut part.lines.iter().cloned(), "(nothing)");
            }
        }
        Err(why) => section("This computer", &mut std::iter::once(why.clone()), ""),
    }
    let decisions = g.decisions.iter().flat_map(|e| {
        let time =
            e.ts.parse::<jiff::Timestamp>()
                .map(|t| {
                    t.to_zoned(jiff::tz::TimeZone::system())
                        .strftime("%H:%M")
                        .to_string()
                })
                .unwrap_or_default();
        let mut lines = vec![format!("{time}  {}", e.message)];
        lines.extend(e.reason.as_ref().map(|r| format!("       {r}")));
        lines
    });
    section(
        "What Crake did (the last 24 hours)",
        &mut decisions.collect::<Vec<_>>().into_iter(),
        missing.map_or("(nothing)", |_| "(craked couldn't be asked)"),
    );
    let own: Vec<String> = g
        .computer
        .as_ref()
        .map(|c| c.own_log.clone())
        .unwrap_or_default();
    section(
        &format!("craked's own log (the last hour, at most {OWN_LOG_LINES} lines)"),
        &mut own.into_iter(),
        missing.map_or("(nothing)", |_| "(craked couldn't be asked)"),
    );
    s
}

/// "Crake 0.10.4 on Ubuntu 26.04 LTS." (and craked's version when it's
/// another).
fn about(service: &str) -> String {
    let mine = env!("CARGO_PKG_VERSION");
    let service = if service.is_empty() || service == mine {
        String::new()
    } else {
        format!(" (its service: {service})")
    };
    format!("Crake {mine}{service} on {}.", system())
}

/// The system's own name for itself.
fn system() -> String {
    #[cfg(target_os = "linux")]
    {
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        release
            .lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|n| n.trim_matches('"').to_owned())
            .unwrap_or_else(|| "Linux".to_owned())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .map(|o| format!("macOS {}", String::from_utf8_lossy(&o.stdout).trim()))
            .unwrap_or_else(|| "macOS".to_owned())
    }
    #[cfg(windows)]
    {
        "Windows".to_owned()
    }
}

/// `text` written to `path`; the system's reason when it can't be.
pub fn save(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// `crake report [file] [--hide-hosts|--show-hosts]`: None when the app
/// wasn't started for one (it starts as usual); else the exit code.
pub fn cli(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("report") {
        return None;
    }
    crate::sys::attach_console();
    let mut hide = None;
    let mut file = None;
    for a in &args[1..] {
        match a.as_str() {
            "--hide-hosts" => hide = Some(true),
            "--show-hosts" => hide = Some(false),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Some(0);
            }
            other if other.starts_with('-') => {
                eprintln!("crake report: no option {other}\n{USAGE}");
                return Some(USAGE_ERROR);
            }
            other => file = Some(PathBuf::from(other)),
        }
    }
    let hide = hide.unwrap_or_else(|| crate::prefs::Prefs::load().hide_hosts);
    let now = jiff::Zoned::now();
    let path = file.unwrap_or_else(|| default_folder().join(file_name(&now)));
    let gathered = gather_apart()
        .recv_blocking()
        .unwrap_or_else(|_| Gathered::away(format!("Crake couldn't ask: {NOT_READ}")));
    match save(&path, &text(&gathered, hide, &now)) {
        Ok(()) => {
            println!("Saved {}", path.display());
            Some(0)
        }
        Err(why) => {
            eprintln!("Couldn't save the report: {why}");
            Some(1)
        }
    }
}

const USAGE: &str = "crake report [file] [--hide-hosts | --show-hosts]

Saves what someone helping you needs to see (this computer's DNS and
routes, what Crake added, whether the internet answers, and the logs) as
a text file, by default crake-report-<date>.txt in Documents. Crake sends
it nowhere. --hide-hosts leaves out the names and addresses of the hosts
you reached, --show-hosts keeps them; without either, as Settings says.";

/// Wrong arguments, as Unix commands say it.
const USAGE_ERROR: i32 = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use crake_proto::LogKind;
    use crake_proto::report::Part;

    fn sample() -> Gathered {
        Gathered {
            service: env!("CARGO_PKG_VERSION").into(),
            tunnels: vec![
                "ACME: utun4, address 10.40.255.50, DNS 10.40.0.2 for acme.internal; 3 hosts learned, 1 pinned".into(),
            ],
            decisions: vec![LogEntry {
                ts: "2026-10-18T15:40:00Z".into(),
                kind: LogKind::Routed,
                tunnel: Some("acme".into()),
                message: "Sent git.acme.internal through ACME.".into(),
                reason: Some("It didn't answer directly. Through ACME it answered in 31 ms.".into()),
                hosts: vec!["10.40.1.10".into()],
                others: Vec::new(),
            }],
            computer: Ok(Computer {
                checks: vec![
                    "ACME's names: git.acme.internal is 10.40.1.10 (DNS answered in 4 ms); a connection to it on 443 took 31 ms, through ACME.".into(),
                ],
                parts: vec![Part::new(
                    "Routes (netstat -rn)",
                    [
                        "default 192.0.2.1 UGScg en0".to_owned(),
                        "10.40.0.2 utun4    ← Crake: ACME's DNS".to_owned(),
                        "10.40.1.10 utun4    ← Crake: learned through ACME".to_owned(),
                    ],
                )],
                own_log: vec!["INFO craked::manager: ACME connected".into()],
                keep: vec!["192.0.2.1".into(), "10.40.255.50".into(), "10.40.0.2".into()],
            }),
        }
    }

    fn now() -> jiff::Zoned {
        "2026-10-18T17:52:00+02:00[Europe/Prague]".parse().unwrap()
    }

    #[test]
    fn a_report_has_every_part() {
        let text = text(&sample(), false, &now());
        assert!(
            text.starts_with("Crake report, 18 October 2026 17:52\nCrake "),
            "{text}"
        );
        assert!(text.contains("Host names and addresses: shown"));
        for part in [
            "\nTunnels\n  ACME: utun4",
            "\nThis computer\n  ACME's names: git.acme.internal is 10.40.1.10",
            "\nRoutes (netstat -rn)\n  default 192.0.2.1",
            "\nWhat Crake did (the last 24 hours)\n",
            "  Sent git.acme.internal through ACME.\n         It didn't answer directly.",
            "\ncraked's own log (the last hour, at most 200 lines)\n  INFO craked::manager: ACME connected",
        ] {
            assert!(text.contains(part), "{part:?} in\n{text}");
        }
    }

    #[test]
    fn hidden_the_hosts_go_and_the_plumbing_stays() {
        let text = text(&sample(), true, &now());
        assert!(text.contains("Host names and addresses: hidden"));
        assert!(!text.contains("acme.internal"), "{text}");
        assert!(!text.contains("10.40.1.10"), "{text}");
        assert!(
            text.contains("address 10.40.255.50, DNS 10.40.0.2 for a host"),
            "{text}"
        );
        assert!(text.contains("default 192.0.2.1 UGScg en0"), "{text}");
        assert!(
            text.contains("a host utun4    ← Crake: learned through ACME"),
            "{text}"
        );
        assert!(text.contains("ACME's names: a host is a host"), "{text}");
    }

    #[test]
    fn without_craked_it_says_so_and_still_saves() {
        let away = Gathered::away(not_running());
        let text = text(&away, false, &now());
        assert!(
            text.contains("\nThis computer\n  craked isn't running"),
            "{text}"
        );
        assert!(text.contains("weren't read."));
        assert!(
            text.contains("\nTunnels\n  (craked couldn't be asked)"),
            "{text}"
        );
        assert_eq!(file_name(&now()), "crake-report-2026-10-18.txt");
    }

    #[test]
    fn the_command_line() {
        assert_eq!(cli(&["--autostart".into()]), None);
        assert_eq!(cli(&[]), None);
        assert_eq!(
            cli(&["report".into(), "--nonsense".into()]),
            Some(USAGE_ERROR)
        );
    }
}
