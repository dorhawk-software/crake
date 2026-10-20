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

/// IPsec's gateway (Linux's containers only).
#[cfg(target_os = "linux")]
pub(crate) const IPSEC_GATEWAY: &str = "198.51.100.10";

/// craked started for one test, and where it serves the app.
pub(crate) struct Craked {
    /// Stopped by the scenario itself: nothing left to clean up.
    pub stopped: bool,
    #[cfg(unix)]
    dir: tempfile::TempDir,
    #[cfg(unix)]
    pub mgr: std::sync::Arc<craked::manager::Manager>,
}

#[cfg(unix)]
impl Craked {
    /// craked in this test, as root, with a folder of its own (`config`).
    pub async fn start() -> (Self, App) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config(dir.path());
        let hook_socket = cfg.hook_socket.clone();
        let mgr = craked::manager::Manager::new(cfg).unwrap();
        mgr.init().await.unwrap();
        craked::hook_server::serve(mgr.clone(), &hook_socket).unwrap();
        let app = crate::test_app::serve(mgr.clone(), dir.path());
        (
            Self {
                stopped: false,
                dir,
                mgr,
            },
            app,
        )
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
}

#[cfg(windows)]
impl Craked {
    /// The craked service the installer put there.
    pub async fn start() -> (Self, App) {
        let (tx, updates) = async_channel::unbounded();
        (
            Self { stopped: false },
            App::new(crate::client::start(tx), updates),
        )
    }

    pub fn socket(&self) -> Option<std::path::PathBuf> {
        None
    }

    pub fn device(&self, tunnel: &str) -> String {
        format!("crake-{tunnel}")
    }

    /// The service keeps running for the next test.
    pub async fn stop(&mut self) {
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
