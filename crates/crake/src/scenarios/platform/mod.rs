//! What the scenarios need of each system: craked started fresh, the
//! tunnel's device, where an address goes, how a name is asked over TCP,
//! what a tunnel or craked leaves behind. One file per system; what's
//! shared is here.

use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs as _};
use std::time::Duration;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub(crate) use linux::*;
#[cfg(target_os = "macos")]
pub(crate) use macos::*;
#[cfg(windows)]
pub(crate) use windows::*;

use crate::test_app::{App, get};

/// How often craked asks a tunnel's DNS in these tests (30 s by default:
/// one outage scenario took 3 minutes of CI's Windows). Windows' installed
/// service reads it from the craked.toml ci-through-cloudflare.sh writes.
#[cfg(unix)]
pub(crate) const DNS_CHECK_SECS: u64 = 5;

/// ACME's DNS server, which every test gateway pushes.
pub(crate) const ACME_DNS: &str = "10.40.0.2";

/// The AnyConnect gateway as New tunnel takes it: through Cloudflare, the
/// local end of `cloudflared access`.
#[cfg(not(target_os = "linux"))]
pub(crate) fn anyconnect_gateway() -> String {
    crate::test_app::gateway()
}

/// carol's certificate (a PKCS#12 file, its password hunter2): the
/// gateway's copy, in the wire's folder on Linux, CI's artifact elsewhere.
pub(crate) fn user_certificate() -> String {
    if cfg!(target_os = "linux") {
        "/mnt/link/acme-user.p12".into()
    } else {
        std::env::var("CRAKE_GATEWAY_USER_CERT").expect("CRAKE_GATEWAY_USER_CERT")
    }
}

/// ACME's AnyConnect-only front and its gateway with groups, through
/// Cloudflare where CI reaches them (scripts/ci-through-cloudflare.sh).
#[cfg(not(target_os = "linux"))]
pub(crate) fn anyconnect_only_gateway() -> Option<String> {
    std::env::var("CRAKE_GATEWAY_PICKY").ok()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn anyconnect_groups_gateway() -> Option<String> {
    std::env::var("CRAKE_GATEWAY_GROUPS").ok()
}

/// Whether ACME's DNS answers `name` with `at` when asked directly (over
/// TCP), as a check that a block on it took.
#[cfg(unix)]
pub(crate) fn acme_dns_answers(name: &str, at: &str) -> bool {
    dns_over_tcp(ACME_DNS, name).contains(at)
}

/// craked started for one test, and where it serves the app.
pub(crate) struct Craked {
    /// Stopped by the scenario itself: nothing left to clean up.
    pub stopped: bool,
    #[cfg(unix)]
    dir: tempfile::TempDir,
    #[cfg(unix)]
    pub mgr: std::sync::Arc<craked::manager::Manager>,
    /// What it was started with, for the craked that follows it.
    #[cfg(unix)]
    cfg: craked::config::Config,
    /// IT's files this test put in the service's managed.d.
    #[cfg(windows)]
    managed: Vec<std::path::PathBuf>,
}

/// Where the installed service reads IT's files.
#[cfg(windows)]
const MANAGED_DIR: &str = r"C:\ProgramData\Crake\managed.d";

#[cfg(unix)]
impl Craked {
    /// craked in this test, as root, with a folder of its own (`config`)
    /// and IT's files (name, text) in its managed.d, read as the service
    /// reads them when it starts.
    pub async fn start_with(managed: &[(&str, String)]) -> (Self, App) {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.managed_dir = dir.path().join("managed.d");
        std::fs::create_dir_all(&cfg.managed_dir).unwrap();
        for (file, text) in managed {
            std::fs::write(cfg.managed_dir.join(file), text).unwrap();
        }
        cfg.add_managed();
        let (mgr, app) = Self::launch(&cfg, dir.path()).await;
        (
            Self {
                stopped: false,
                dir,
                mgr,
                cfg,
            },
            app,
        )
    }

    async fn launch(
        cfg: &craked::config::Config,
        dir: &std::path::Path,
    ) -> (std::sync::Arc<craked::manager::Manager>, App) {
        let mgr = craked::manager::Manager::new(cfg.clone()).unwrap();
        mgr.init().await.unwrap();
        craked::hook_server::serve(mgr.clone(), &cfg.hook_socket).unwrap();
        let app = crate::test_app::serve(mgr.clone(), dir);
        (mgr, app)
    }

    /// An upgrade, as the packages' scripts make one: the flag beside the
    /// hook's socket, craked stopped (handing its tunnels over), and the
    /// new one started in its folder, which switches them on again. The
    /// app that follows it.
    pub async fn upgrade(&mut self) -> Option<App> {
        let flag = self.cfg.hook_socket.parent().unwrap().join("handover");
        std::fs::write(flag, "").unwrap();
        self.mgr.shutdown().await;
        let (mgr, app) = Self::launch(&self.cfg, self.dir.path()).await;
        mgr.resume_after_update().await;
        self.mgr = mgr;
        Some(app)
    }

    /// The socket the app reaches craked on (for a report).
    pub fn socket(&self) -> Option<std::path::PathBuf> {
        Some(self.dir.path().join("crake.sock"))
    }

    /// The device `tunnel` runs on, as craked was told by the hook.
    pub fn device(&self, tunnel: &str) -> String {
        self.mgr.get(tunnel).unwrap().info.unwrap().device
    }

    /// craked stopping, as the service does.
    pub async fn stop(&mut self) {
        self.mgr.shutdown().await;
        self.stopped = true;
    }

    /// craked ending with its tunnels up, as the service stops.
    pub async fn end(&mut self) {
        self.stop().await;
    }
}

#[cfg(windows)]
impl Craked {
    /// The craked service the installer put there, with IT's files (name,
    /// text) in its managed.d: the service restarted to read them, as it reads them when Windows
    /// starts it (and again without them once the test is done).
    pub async fn start_with(managed: &[(&str, String)]) -> (Self, App) {
        // A test that stopped the service ("stopping craked leaves
        // nothing") left it for this one to start.
        sh("if ((Get-Service craked).Status -ne 'Running') { Start-Service craked }");
        let dir = std::path::Path::new(MANAGED_DIR);
        std::fs::create_dir_all(dir).unwrap();
        // A test that failed left its files (it never got to `stop`).
        let mut restart = false;
        for left in std::fs::read_dir(dir).unwrap().flatten() {
            restart |= std::fs::remove_file(left.path()).is_ok();
        }
        let mut files = Vec::new();
        for (file, text) in managed {
            std::fs::write(dir.join(file), text).unwrap();
            files.push(dir.join(file));
        }
        if restart || !files.is_empty() {
            sh("Restart-Service craked");
        }
        let (tx, updates) = async_channel::unbounded();
        (
            Self {
                stopped: false,
                managed: files,
            },
            App::new(crate::client::start(tx), updates),
        )
    }

    pub fn socket(&self) -> Option<std::path::PathBuf> {
        None
    }

    pub fn device(&self, tunnel: &str) -> String {
        format!("crake-{tunnel}")
    }

    /// The service keeps running for the next test (without IT's files
    /// this test gave it).
    pub async fn stop(&mut self) {
        if !self.managed.is_empty() {
            for file in self.managed.drain(..) {
                let _ = std::fs::remove_file(file);
            }
            sh("Restart-Service craked");
        }
        self.stopped = true;
    }

    /// An upgrade: this run's installer again, silently, over the running
    /// service (CRAKE_INSTALLER; without one, None). The app that follows.
    pub async fn upgrade(&mut self) -> Option<App> {
        let setup = std::env::var("CRAKE_INSTALLER").ok()?;
        let out = sh(&format!(
            "$p = Start-Process '{setup}' -ArgumentList '/S' -Wait -PassThru; $p.ExitCode"
        ));
        assert_eq!(out.trim(), "0", "the installer: {out}");
        let (tx, updates) = async_channel::unbounded();
        Some(App::new(crate::client::start(tx), updates))
    }

    /// The service stopped with its tunnels up, as Windows stops it (the
    /// next test starts it again).
    pub async fn end(&mut self) {
        let out = sh("Stop-Service craked; (Get-Service craked).Status");
        assert!(out.contains("Stopped"), "the service didn't stop: {out}");
        self.stopped = true;
    }
}

/// craked's OpenVPN profile for the test gateway: a password sign-in,
/// nothing inline but the gateway's CA. Over UDP where the gateway is
/// reached directly (Linux), over TCP through Cloudflare.
pub(crate) fn openvpn_profile() -> String {
    let (remote, proto, ca) = openvpn_gateway();
    let ca = std::fs::read_to_string(&ca).unwrap_or_else(|e| panic!("{ca}: {e}"));
    format!(
        "client\ndev tun\nproto {proto}\nremote {remote}\nnobind\nauth-user-pass\n\
         remote-cert-tls server\n<ca>\n{ca}</ca>\n"
    )
}

pub(crate) fn sh(cmd: &str) -> String {
    #[cfg(windows)]
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", cmd])
        .output()
        .unwrap();
    #[cfg(unix)]
    let out = std::process::Command::new("sh")
        .args(["-c", cmd])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// Whether `ip` answers an HTTP GET within 2 s.
pub(crate) async fn answers(ip: &'static str) -> bool {
    tokio::task::spawn_blocking(move || get(ip, Duration::from_secs(2)).is_ok())
        .await
        .unwrap()
}

/// `name`'s IPv4 addresses, asked as an app asks the system.
pub(crate) async fn look_up(name: &'static str) -> Vec<Ipv4Addr> {
    tokio::task::spawn_blocking(move || {
        (name, 80)
            .to_socket_addrs()
            .map(|a| {
                a.filter_map(|a| match a {
                    SocketAddr::V4(v4) => Some(*v4.ip()),
                    SocketAddr::V6(_) => None,
                })
                .collect()
            })
            .unwrap_or_default()
    })
    .await
    .unwrap()
}

/// `name`'s IPv6 addresses, asked as an app asks the system.
pub(crate) async fn look_up6(name: &'static str) -> Vec<std::net::Ipv6Addr> {
    tokio::task::spawn_blocking(move || {
        (name, 80)
            .to_socket_addrs()
            .map(|a| {
                a.filter_map(|a| match a {
                    SocketAddr::V6(v6) => Some(*v6.ip()),
                    SocketAddr::V4(_) => None,
                })
                .collect()
            })
            .unwrap_or_default()
    })
    .await
    .unwrap()
}

/// Whether the public name answers, looked up as an app looks it up.
pub(crate) fn public_name_answers() -> bool {
    PUBLIC_NAME
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.find(SocketAddr::is_ipv4))
        .is_some_and(|a| TcpStream::connect_timeout(&a, Duration::from_secs(5)).is_ok())
}

/// `name`'s addresses asked of `server` over TCP, as an app asking a DNS
/// server directly does when an answer is long or it prefers TCP.
#[cfg(unix)]
pub(crate) fn dns_over_tcp(server: &str, name: &str) -> String {
    use std::io::{Read as _, Write as _};
    let mut q = vec![0xab, 0xcd, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        q.push(u8::try_from(label.len()).unwrap());
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 1, 0, 1]);
    let asked = || -> std::io::Result<Vec<u8>> {
        let addr: SocketAddr = format!("{server}:53").parse().unwrap();
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        s.write_all(&u16::try_from(q.len()).unwrap().to_be_bytes())?;
        s.write_all(&q)?;
        let mut len = [0u8; 2];
        s.read_exact(&mut len)?;
        let mut answer = vec![0u8; usize::from(u16::from_be_bytes(len))];
        s.read_exact(&mut answer)?;
        Ok(answer)
    };
    match asked() {
        Ok(answer) => a_records(&answer, q.len())
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" "),
        Err(e) => format!("{server} over TCP: {e}"),
    }
}

/// The A records of a DNS answer to a question `asked` bytes long (names
/// in the answers compressed, as every server sends them).
#[cfg(unix)]
fn a_records(answer: &[u8], asked: usize) -> Vec<Ipv4Addr> {
    let mut at = asked;
    let mut found = Vec::new();
    while at + 12 <= answer.len() {
        let rr = &answer[at + 2..];
        let (kind, len) = (
            u16::from_be_bytes([rr[0], rr[1]]),
            usize::from(u16::from_be_bytes([rr[8], rr[9]])),
        );
        let data = at + 12;
        if kind == 1 && len == 4 && data + 4 <= answer.len() {
            found.push(Ipv4Addr::new(
                answer[data],
                answer[data + 1],
                answer[data + 2],
                answer[data + 3],
            ));
        }
        at = data + len;
    }
    found
}
