//! craked's own charon: one for every IPsec tunnel, started with the first
//! and stopped with craked. Its settings are craked's (STRONGSWAN_CONF), not
//! the system's: random local ports (a system strongSwan may hold 500 and
//! 4500), no routes or addresses of its own (Crake's routing decides what
//! goes in), the DNS servers handed to updown (the hook) rather than
//! written to resolv.conf, its vici socket in craked's runtime folder, its
//! log on stderr, which craked reads.
//!
//! charon refuses to start while another runs on the computer (its pid
//! file's path is built in, /var/run/charon.pid): the package runs it as a
//! unit with a /run of its own for that (CLAUDE.md, 7k; not built yet).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// How long charon gets to open its vici socket.
const START_WAIT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(100);
/// How long it gets to stop once told to, before it's killed.
const STOP_WAIT: Duration = Duration::from_secs(5);
/// What charon says when another one runs.
const ALREADY: &str = "already running";

/// The plugins craked's charon loads: no resolve (resolv.conf is
/// resolved's), no stroke or starter (vici only).
const PLUGINS: &str = "random nonce openssl pem pkcs1 pkcs8 x509 revocation constraints \
                       pubkey drbg kernel-netlink socket-default vici updown \
                       eap-identity eap-mschapv2 eap-md5 eap-gtc eap-dynamic";

/// craked's charon settings.
pub fn conf(socket: &Path) -> String {
    format!(
        "charon {{\n\
         \x20 port = 0\n\
         \x20 port_nat_t = 0\n\
         \x20 install_routes = no\n\
         \x20 install_virtual_ip = no\n\
         \x20 load = {PLUGINS}\n\
         \x20 plugins {{\n\
         \x20   vici {{\n\
         \x20     socket = unix://{}\n\
         \x20   }}\n\
         \x20   updown {{\n\
         \x20     dns_handler = yes\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 syslog {{\n\
         \x20   daemon {{\n\
         \x20     default = -1\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 filelog {{\n\
         \x20   stderr {{\n\
         \x20     default = 1\n\
         \x20   }}\n\
         \x20 }}\n\
         }}\n",
        socket.display()
    )
}

pub struct Charon {
    child: Child,
    pub socket: PathBuf,
    /// Its last lines, for a refusal's reason.
    heard: Arc<Mutex<Vec<String>>>,
}

/// How many of charon's last lines are kept.
const HEARD: usize = 20;

impl Charon {
    /// Start `program` with its settings and socket in `dir` (root's).
    pub async fn start(program: &Path, dir: &Path) -> Result<Self> {
        crate::sys::create_private_dir(dir).with_context(|| format!("making {}", dir.display()))?;
        let socket = dir.join("charon.vici");
        let settings = dir.join("strongswan.conf");
        {
            use std::io::Write;
            let mut f = crate::sys::create_private(&settings)
                .with_context(|| format!("writing {}", settings.display()))?;
            f.write_all(conf(&socket).as_bytes())?;
        }
        let _ = std::fs::remove_file(&socket);
        let mut child = Command::new(program)
            .env("STRONGSWAN_CONF", &settings)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("starting {}", program.display()))?;
        let heard = Arc::new(Mutex::new(Vec::new()));
        if let Some(err) = child.stderr.take() {
            let heard = Arc::clone(&heard);
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "charon", "{line}");
                    let mut h = heard.lock().unwrap();
                    if h.len() == HEARD {
                        h.remove(0);
                    }
                    h.push(line);
                }
            });
        }
        let mut charon = Self {
            child,
            socket,
            heard,
        };
        let deadline = tokio::time::Instant::now() + START_WAIT;
        while !charon.socket.exists() {
            if !charon.running() || tokio::time::Instant::now() > deadline {
                // Its last words arrive a moment after it ends.
                tokio::time::sleep(POLL).await;
                let said = charon.heard.lock().unwrap().join("\n");
                if said.contains(ALREADY) {
                    bail!(
                        "Another strongSwan runs on this computer, and Crake's can't start \
                         beside it."
                    );
                }
                charon.stop().await;
                bail!("strongSwan's charon didn't start: {said}");
            }
            tokio::time::sleep(POLL).await;
        }
        Ok(charon)
    }

    /// It hasn't ended.
    pub fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub async fn stop(&mut self) {
        if let Some(pid) = self.child.id() {
            let _ = crake_os::unix::signal(pid, libc::SIGTERM);
        }
        if tokio::time::timeout(STOP_WAIT, self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn craked_s_charon_leaves_the_system_alone() {
        let c = conf(Path::new("/run/crake/ipsec/charon.vici"));
        for line in [
            "port = 0",
            "port_nat_t = 0",
            "install_routes = no",
            "install_virtual_ip = no",
            "socket = unix:///run/crake/ipsec/charon.vici",
            "dns_handler = yes",
        ] {
            assert!(c.contains(line), "{line} in\n{c}");
        }
        assert!(!c.contains("resolve"), "resolv.conf is resolved's");
    }
}
