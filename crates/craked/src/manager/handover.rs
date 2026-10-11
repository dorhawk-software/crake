//! Package upgrades: the old craked hands its tunnels to the new one, so
//! they stay signed in and come back within seconds (CLAUDE.md, Packaging).
//!
//! The package's postinst leaves `handover` in craked's runtime directory
//! (`/run/crake`, next to the hook's socket) before restarting it. craked,
//! stopping, then disconnects each openconnect tunnel without logging its
//! session off (SIGHUP) and writes what each tunnel needs to come back to
//! `resume.json` there: the session, or for OpenVPN 3 the device the app
//! attached (its session runs on in openvpn3). Root only, on tmpfs: gone at
//! reboot. The new craked reads it once, deletes it, and switches them on
//! again. Without the flag, stopping logs every session off, as always.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::*;

/// The flag postinst leaves, and the file the old craked writes.
const FLAG: &str = "handover";
const RESUME: &str = "resume.json";
/// A flag older than this is left over from an upgrade that didn't restart
/// craked; a resume file older than this, from one that didn't start it
/// again soon (the sessions may have gone by then).
const FLAG_FRESH: Duration = Duration::from_secs(120);
const RESUME_FRESH: Duration = Duration::from_secs(600);

#[derive(Debug, Default, Serialize, Deserialize)]
struct Resume {
    /// openconnect tunnels: their sessions (`Session::to_map`).
    #[serde(default)]
    sessions: BTreeMap<String, BTreeMap<String, String>>,
    /// OpenVPN 3 tunnels: what the app attached them with.
    #[serde(default)]
    attached: BTreeMap<String, HashMap<String, String>>,
}

/// `path` exists and was written within `fresh`.
fn recent(path: &std::path::Path, fresh: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().is_ok_and(|age| age < fresh))
}

impl Manager {
    fn runtime_file(&self, name: &str) -> Option<PathBuf> {
        Some(self.cfg.hook_socket.parent()?.join(name))
    }

    /// Stop every tunnel (daemon shutdown): log their sessions off, or, when
    /// an upgrade asked for it, hand them over to the next craked.
    pub(super) fn stop_all(&self) {
        let flag = self.runtime_file(FLAG);
        let handover = flag.as_deref().is_some_and(|f| recent(f, FLAG_FRESH));
        if let Some(f) = &flag {
            let _ = std::fs::remove_file(f);
        }
        let mut resume = Resume::default();
        {
            let mut map = self.tunnels.lock().unwrap();
            for (name, e) in map.iter_mut() {
                let up = matches!(e.state, TunnelState::Connecting | TunnelState::Connected);
                if handover && up {
                    if let (Some(session), Some(_)) = (&e.session, &e.stop) {
                        resume.sessions.insert(name.clone(), session.to_map());
                        e.handover = true;
                    }
                    if let Some(fields) = &e.attached {
                        resume.attached.insert(name.clone(), fields.clone());
                    }
                }
                let how = if e.handover {
                    Stop::KeepSession
                } else {
                    e.session = None;
                    Stop::LogOff
                };
                if let Some(tx) = e.stop.take() {
                    let _ = tx.send(how);
                }
            }
        }
        if resume.sessions.is_empty() && resume.attached.is_empty() {
            return;
        }
        if let Some(path) = self.runtime_file(RESUME)
            && let Err(e) = write_private(&path, &resume)
        {
            tracing::error!("handing the tunnels over: {e:#}");
        }
    }

    /// After a package upgrade: switch on again what the old craked handed
    /// over. Call once the hook's socket and D-Bus are being served.
    pub async fn resume_after_update(self: &Arc<Self>) {
        let Some(path) = self.runtime_file(RESUME) else {
            return;
        };
        if !path.exists() {
            return;
        }
        let fresh = recent(&path, RESUME_FRESH);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // Read once: the sessions in it are never left lying around.
        let _ = std::fs::remove_file(&path);
        if !fresh {
            return;
        }
        let Ok(resume) = serde_json::from_str::<Resume>(&text) else {
            return;
        };
        let mut back = 0;
        for (name, map) in &resume.sessions {
            let Ok(session) = crake_proto::Session::from_map(map) else {
                continue;
            };
            if let Some(e) = self.tunnels.lock().unwrap().get_mut(name) {
                e.handover = true;
            }
            match self.connect_with(name, Some(session)) {
                Ok(()) => back += 1,
                Err(e) => tracing::error!("{name} after the update: {e:#}"),
            }
        }
        for (name, fields) in &resume.attached {
            match self.attach(name, fields).await {
                Ok(()) => back += 1,
                Err(e) => tracing::error!("{name} after the update: {e:#}"),
            }
        }
        if back > 0 {
            self.log.system(
                None,
                "Crake was updated. Your tunnels are reconnecting with your sign-ins.".to_owned(),
                None,
            );
        }
    }
}

/// Written for root alone (mode 0600), replacing any older one.
fn write_private(path: &std::path::Path, resume: &Resume) -> anyhow::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(&serde_json::to_vec(resume)?)?;
    Ok(())
}
