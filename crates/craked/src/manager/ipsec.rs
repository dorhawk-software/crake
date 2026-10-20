//! IPsec tunnels (CLAUDE.md, 7k): craked's charon signs in with the
//! gateway's key and the user's password, which the user is asked for as
//! OpenVPN's sign-in asks (`Asking`); the tunnel's XFRM interface is made
//! before and removed after; the hook brings the tunnel up when charon's
//! updown says it is; charon is asked every LOOK_EVERY whether it still is.

use std::path::PathBuf;

use tokio::sync::mpsc;

use super::entry::Answers;
use super::*;
use crate::ipsec::charon::Charon;
use crate::ipsec::vici::{Signed, Tunnel, Vici};
use crate::ipsec::{self, Secret};
use crate::openvpn::AskKind;

/// How often charon is asked whether the tunnel is still up.
const LOOK_EVERY: Duration = Duration::from_secs(2);

/// How an IPsec tunnel's run ended.
type Ended = (TunnelState, Option<String>);

impl Manager {
    pub(super) async fn supervise_ipsec(
        self: Arc<Self>,
        config: TunnelConfig,
        mut stop_rx: oneshot::Receiver<Stop>,
    ) {
        let name = config.name.clone();
        let label = config.display().to_owned();
        let dev = interface_name(&name);
        let (state, reason) = match self.run_ipsec(&config, &dev, &mut stop_rx).await {
            Ok(end) => end,
            Err(e) => {
                tracing::error!("{label}: {e:#}");
                (TunnelState::Trouble, Some(format!("{e:#}")))
            }
        };
        self.asked(&name);
        self.net.remove(&dev).await;
        self.finish(&name, &label, state, reason).await;
    }

    async fn run_ipsec(
        &self,
        config: &TunnelConfig,
        dev: &str,
        stop_rx: &mut oneshot::Receiver<Stop>,
    ) -> Result<Ended> {
        let name = &config.name;
        // IT's key (or craked.toml's), read at each start; else craked's own
        // copy of the one given in the app.
        let secret = match &config.gateway_key_file {
            Some(file) => ipsec::key_from_file(file, config.gateway_id.as_deref())?,
            None => Secret::read(&ipsec::secret_path(&self.cfg.state_path, name))?,
        };
        let socket = self.charon_socket().await?;
        let index = self
            .tunnels
            .lock()
            .unwrap()
            .get(name)
            .map(|e| e.index)
            .context("the tunnel is gone")?;
        let if_id = ipsec::if_id(index);
        self.net.xfrm(dev, if_id, ipsec::MTU).await?;
        let mut vici = Vici::connect(&socket).await?;
        let (answers, mut answer_rx) = mpsc::unbounded_channel();
        let mut retry = false;
        loop {
            let user = match &config.user {
                Some(u) => u.clone(),
                None => {
                    let asked = (AskKind::User, retry);
                    match self
                        .answer_of(name, asked, &answers, &mut answer_rx, stop_rx)
                        .await
                    {
                        Some(u) => u,
                        None => return Ok((TunnelState::Off, None)),
                    }
                }
            };
            let asked = (AskKind::Password, retry && config.user.is_some());
            let Some(password) = self
                .answer_of(name, asked, &answers, &mut answer_rx, stop_rx)
                .await
            else {
                return Ok((TunnelState::Off, None));
            };
            let tunnel = Tunnel {
                name,
                gateway: &config.gateway,
                secret: &secret,
                user: &user,
                if_id,
                updown: self.updown(name),
            };
            vici.load(&tunnel, &password).await?;
            let signed = tokio::select! {
                s = vici.initiate(name) => Some(s),
                _ = &mut *stop_rx => None,
            };
            match signed {
                Some(Signed::In) => break,
                Some(Signed::Refused) => {
                    vici.forget(name).await;
                    retry = true;
                }
                Some(Signed::KeyRefused) => {
                    vici.forget(name).await;
                    let who = config.display();
                    return Ok((
                        TunnelState::Trouble,
                        Some(format!(
                            "{} {who}\u{2019}s gateway. Your IT team can tell you the right one; change it in {who}\u{2019}s Edit.",
                            crake_proto::trouble::KEY_MISMATCH
                        )),
                    ));
                }
                Some(Signed::Failed(why)) => {
                    vici.forget(name).await;
                    return Ok((TunnelState::Trouble, Some(why)));
                }
                None => {
                    vici.end(name).await;
                    return Ok((TunnelState::Off, None));
                }
            }
        }
        // Up (the hook said so); until it's switched off or ends.
        loop {
            tokio::select! {
                _ = &mut *stop_rx => {
                    vici.end(name).await;
                    return Ok((TunnelState::Off, None));
                }
                () = tokio::time::sleep(LOOK_EVERY) => {
                    if !vici.up(name).await.unwrap_or(false) {
                        vici.forget(name).await;
                        return Ok((
                            TunnelState::Trouble,
                            Some(format!(
                                "{}. {}",
                                crake_proto::trouble::GATEWAY_ENDED,
                                crake_proto::trouble::SIGN_IN_AGAIN
                            )),
                        ));
                    }
                }
            }
        }
    }

    /// Ask the user, and wait for the answer; None when it's switched off
    /// meanwhile.
    async fn answer_of(
        &self,
        name: &str,
        (kind, retry): (AskKind, bool),
        answers: &Answers,
        answer_rx: &mut mpsc::UnboundedReceiver<(u32, String)>,
        stop_rx: &mut oneshot::Receiver<Stop>,
    ) -> Option<String> {
        let id = self.ask(name, (kind, String::new(), None, retry), answers);
        loop {
            tokio::select! {
                _ = &mut *stop_rx => {
                    self.asked(name);
                    return None;
                }
                Some((got, text)) = answer_rx.recv() => {
                    if got == id {
                        self.asked(name);
                        return Some(text);
                    }
                }
            }
        }
    }

    /// strongSwan's updown for this tunnel: the hook, with the tunnel's
    /// name and craked's socket (charon gives its script nothing else of
    /// its own environment).
    fn updown(&self, name: &str) -> String {
        format!(
            "{ENV_TUNNEL}={name} {ENV_SOCKET}={} {}",
            quoted(&self.cfg.hook_socket.display().to_string()),
            quoted(&self.cfg.hook_path.display().to_string()),
        )
    }

    /// craked's charon's vici socket, started first if it isn't running.
    async fn charon_socket(&self) -> Result<PathBuf> {
        let mut charon = self.charon.lock().await;
        if let Some(c) = charon.as_mut()
            && c.running().await
        {
            return Ok(c.socket.clone());
        }
        let dir = self.cfg.hook_socket.parent().map_or_else(
            || self.cfg.state_path.with_file_name("ipsec"),
            |p| p.join("ipsec"),
        );
        let c = Charon::start(&self.cfg.charon, &dir, self.cfg.charon_unit.as_deref()).await?;
        let socket = c.socket.clone();
        *charon = Some(c);
        Ok(socket)
    }

    /// Stop craked's charon (craked is leaving).
    pub(super) async fn stop_charon(&self) {
        if let Some(mut c) = self.charon.lock().await.take() {
            c.stop().await;
        }
    }
}

/// `s` as one word for the shell strongSwan runs updown with.
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_one_word_for_the_shell() {
        assert_eq!(quoted("/run/crake/hook.sock"), "'/run/crake/hook.sock'");
        assert_eq!(quoted("/a b/it's"), r"'/a b/it'\''s'");
    }
}
