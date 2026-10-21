//! craked's own charon: one for every IPsec tunnel, started with the first
//! and stopped with craked. Its settings are craked's (STRONGSWAN_CONF), not
//! the system's: random local ports (a system strongSwan may hold 500 and
//! 4500), no routes or addresses of its own (Crake's routing decides what
//! goes in), the DNS servers handed to updown (the hook) rather than
//! written to resolv.conf, its vici socket in craked's runtime folder, its
//! log on stderr, which craked reads.
//!
//! charon refuses to start while another runs on the computer (its pid
//! file's path is built in, /var/run/charon.pid), and craked's service may
//! not write /run: the package runs it as a unit with a /run of its own
//! (crake-charon.service, `unit`), which craked starts. Run by hand or in
//! the tests, craked runs charon itself.

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
/// How long it must keep running after making its socket.
const SETTLE: Duration = Duration::from_millis(300);
/// How long it gets to stop once told to, before it's killed.
const STOP_WAIT: Duration = Duration::from_secs(5);
/// The folder the unit binds as /run/strongswan.
const PID_DIR: &str = "run";
/// What charon says when another one runs.
const ALREADY: &str = "already running";

/// The plugins craked's charon loads: no resolve (resolv.conf is
/// resolved's), no stroke or starter (vici only). Its crypto is openssl's
/// (libstrongswan-standard-plugins on Debian and Ubuntu: ECP groups, GCM);
/// strongSwan's own (aes, sha1, sha2, hmac, gmp, kdf) stand in where it's
/// missing, as on Ubuntu 24.04 without the recommended packages, where
/// charon refused to start (2026-10-19, CI). One that isn't installed is
/// skipped.
const PLUGINS: &str = "random nonce openssl aes sha1 sha2 hmac gmp kdf curve25519 pem pkcs1 \
                       pkcs8 x509 revocation constraints pubkey drbg kernel-netlink \
                       socket-default vici updown eap-identity eap-mschapv2 eap-md5 eap-gtc \
                       eap-dynamic";

/// The file charon dumps what it decrypts into, while craked looks at a
/// gateway's certificate (`Charon::dump`): in craked's own folder, root's
/// alone, never the journal (design/proposals/ipsec-certificates.html, 5).
pub const DUMP: &str = "dump.log";
/// charon's log level for its decryption dumps ("enc 3"), and off.
const DUMPING: i8 = 3;
const QUIET: i8 = -1;

/// craked's charon settings; `dump`, its dump's file and whether it's on.
pub fn conf(socket: &Path, dump: (&Path, bool)) -> String {
    let (dump_path, on) = dump;
    let enc = if on { DUMPING } else { QUIET };
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
         \x20   dump {{\n\
         \x20     path = {}\n\
         \x20     ike_name = yes\n\
         \x20     default = {QUIET}\n\
         \x20     enc = {enc}\n\
         \x20   }}\n\
         \x20 }}\n\
         }}\n",
        socket.display(),
        dump_path.display(),
    )
}

/// What charon said that tells why it didn't start: its failures, else
/// everything it said.
fn last_words(said: &str) -> String {
    let telling: Vec<&str> = said
        .lines()
        .filter(|l| l.contains("failed") || l.contains("unmet") || l.contains("abort"))
        .collect();
    if telling.is_empty() {
        said.to_owned()
    } else {
        telling.join("\n")
    }
}

/// How charon runs: craked's child, or the package's unit.
enum Runs {
    Child(Child),
    Unit(super::unit::Unit),
}

pub struct Charon {
    runs: Runs,
    pub socket: PathBuf,
    settings: PathBuf,
    /// Its decryption dump's file, and how many looks at a gateway want it.
    pub dump: PathBuf,
    dumping: usize,
    /// Its last lines (run as craked's child), for why it didn't start.
    heard: Arc<Mutex<Vec<String>>>,
}

/// How many of charon's last lines are kept.
const HEARD: usize = 20;

impl Charon {
    /// Start `program` with its settings and socket in `dir` (root's), or
    /// `unit` with them when the package has one (its unit reads the same
    /// settings: craked.service names it, crake-charon.service says where).
    pub async fn start(program: &Path, dir: &Path, unit: Option<&str>) -> Result<Self> {
        crate::sys::create_private_dir(dir).with_context(|| format!("making {}", dir.display()))?;
        // Where the unit puts Fedora's charon's pid file (its
        // /run/strongswan; crake-charon.service).
        crate::sys::create_private_dir(&dir.join(PID_DIR))
            .with_context(|| format!("making {}", dir.display()))?;
        let socket = dir.join("charon.vici");
        let settings = dir.join("strongswan.conf");
        let dump = dir.join(DUMP);
        write_settings(&settings, &conf(&socket, (&dump, false)))?;
        // Made by craked, root's alone, before charon writes to it.
        drop(
            crate::sys::create_private(&dump)
                .with_context(|| format!("making {}", dump.display()))?,
        );
        let _ = std::fs::remove_file(&socket);
        let heard = Arc::new(Mutex::new(Vec::new()));
        let runs = match unit {
            Some(name) => Runs::Unit(super::unit::Unit::start(name).await?),
            None => Runs::Child(spawn(program, &settings, &heard)?),
        };
        let mut charon = Self {
            runs,
            socket,
            settings,
            dump,
            dumping: 0,
            heard,
        };
        let deadline = tokio::time::Instant::now() + START_WAIT;
        // Up once its socket is there and it still runs a moment later: it
        // makes the socket before its plugins can fail it.
        loop {
            if charon.socket.exists() {
                tokio::time::sleep(SETTLE).await;
                if charon.running().await {
                    break;
                }
            }
            if !charon.running().await || tokio::time::Instant::now() > deadline {
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
                if let Runs::Unit(u) = &charon.runs {
                    bail!(
                        "strongSwan's charon didn't start ({}): journalctl -u {} says why.",
                        u.name,
                        u.name
                    );
                }
                bail!("strongSwan's charon didn't start: {}", last_words(&said));
            }
            tokio::time::sleep(POLL).await;
        }
        Ok(charon)
    }

    /// Its decryption dump on (for one more look at a gateway) or off (one
    /// look fewer), written to its settings; charon reads them again when
    /// told (vici's `reload-settings`). Off, the dump's file is emptied:
    /// charon keeps it open, so it isn't removed.
    pub fn dump(&mut self, on: bool) -> Result<()> {
        self.dumping = if on {
            self.dumping + 1
        } else {
            self.dumping.saturating_sub(1)
        };
        let dumping = self.dumping > 0;
        write_settings(&self.settings, &conf(&self.socket, (&self.dump, dumping)))?;
        if !dumping {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&self.dump)
                .and_then(|f| f.set_len(0))
                .with_context(|| format!("emptying {}", self.dump.display()))?;
        }
        Ok(())
    }

    /// It hasn't ended.
    pub async fn running(&mut self) -> bool {
        match &mut self.runs {
            Runs::Child(child) => matches!(child.try_wait(), Ok(None)),
            Runs::Unit(unit) => unit.running().await,
        }
    }

    pub async fn stop(&mut self) {
        match &mut self.runs {
            Runs::Child(child) => {
                if let Some(pid) = child.id() {
                    let _ = crake_os::unix::signal(pid, libc::SIGTERM);
                }
                if tokio::time::timeout(STOP_WAIT, child.wait()).await.is_err() {
                    let _ = child.kill().await;
                }
            }
            Runs::Unit(unit) => unit.stop().await,
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// charon's settings, root's alone.
fn write_settings(path: &Path, text: &str) -> Result<()> {
    use std::io::Write;
    let mut f =
        crate::sys::create_private(path).with_context(|| format!("writing {}", path.display()))?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

/// charon as craked's child, its log kept in `heard`.
fn spawn(program: &Path, settings: &Path, heard: &Arc<Mutex<Vec<String>>>) -> Result<Child> {
    let mut child = Command::new(program)
        .env("STRONGSWAN_CONF", settings)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("starting {}", program.display()))?;
    if let Some(err) = child.stderr.take() {
        let heard = Arc::clone(heard);
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
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn why_charon_didnt_start_is_its_failures() {
        let said = "00[DMN] Starting IKE charon daemon\n\
                    00[LIB] feature CUSTOM:libcharon has unmet dependency: HASHER:HASH_SHA1\n\
                    00[JOB] spawning 16 worker threads\n\
                    00[DMN] initialization failed - aborting charon";
        let why = last_words(said);
        assert!(
            why.contains("unmet dependency") && why.contains("aborting"),
            "{why}"
        );
        assert!(!why.contains("worker threads"));
        assert_eq!(last_words("one\ntwo"), "one\ntwo");
    }

    #[test]
    fn craked_s_charon_leaves_the_system_alone() {
        let dump = Path::new("/run/crake/ipsec/dump.log");
        let c = conf(Path::new("/run/crake/ipsec/charon.vici"), (dump, false));
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

    #[test]
    fn the_dump_is_on_only_while_a_gateway_is_looked_at() {
        let socket = Path::new("/run/crake/ipsec/charon.vici");
        let dump = Path::new("/run/crake/ipsec/dump.log");
        let off = conf(socket, (dump, false));
        assert!(off.contains("path = /run/crake/ipsec/dump.log"), "{off}");
        assert!(off.contains("enc = -1"), "{off}");
        assert!(off.contains("ike_name = yes"), "this connection's alone");
        let on = conf(socket, (dump, true));
        assert!(on.contains("enc = 3"), "{on}");
        // Only the dump's file: the journal (stderr) never gets it.
        let stderr = on.split("dump {").next().unwrap();
        assert!(!stderr.contains("enc = 3"), "{on}");
    }
}
