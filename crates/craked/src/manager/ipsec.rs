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
        if let Some(ended) = self
            .sign_in_ipsec(config, &secret, if_id, &mut vici, stop_rx)
            .await?
        {
            return Ok(ended);
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

    /// Sign in: the user's name and password (asked unless the tunnel has
    /// the name, asked again after a refusal), and, for a gateway whose
    /// certificate no authority here vouches for, the trust question
    /// (`ipsec_trust`). None once signed in; else how the run ended.
    async fn sign_in_ipsec(
        &self,
        config: &TunnelConfig,
        secret: &Secret,
        if_id: u32,
        vici: &mut Vici,
        stop_rx: &mut oneshot::Receiver<Stop>,
    ) -> Result<Option<Ended>> {
        use super::ipsec_trust::{Looked, why_not};
        use crate::ipsec::certificate;
        let name = &config.name;
        let who = config.display();
        // The name the gateway's certificate must be for.
        let expected = secret
            .gateway_id
            .clone()
            .unwrap_or_else(|| config.gateway.clone());
        let pin_path = ipsec::pin_path(&self.cfg.state_path, name);
        let (answers, mut answer_rx) = mpsc::unbounded_channel();
        let mut retry = false;
        let method = config
            .own
            .as_ref()
            .map_or(crate::own::SignIn::Password, |o| o.sign_in);
        // The user's certificate: once, before the gateway is asked
        // anything (ipsec-certificates.html, 2).
        let credential = if method.certificate() {
            let mut asking = super::ipsec_credential::Asking {
                answers: &answers,
                answer_rx: &mut answer_rx,
                stop_rx: &mut *stop_rx,
            };
            match self.ipsec_credential(name, who, &mut asking).await {
                Ok(c) => Some(c),
                Err(ended) => return Ok(Some(ended)),
            }
        } else {
            None
        };
        // The name and password, kept across the trust question.
        let mut known: Option<(String, String)> = None;
        loop {
            let (user, password) = match known.take() {
                Some(k) => k,
                None if !method.password() => (String::new(), String::new()),
                None => {
                    let Some(asked) = self
                        .user_and_password(config, retry, &answers, &mut answer_rx, stop_rx)
                        .await
                    else {
                        return Ok(Some((TunnelState::Off, None)));
                    };
                    asked
                }
            };
            let tunnel = Tunnel {
                name,
                gateway: &config.gateway,
                secret,
                credential: credential.as_ref(),
                user: &user,
                if_id,
                updown: self.updown(name),
            };
            // A certificate the user trusted for this gateway, by its key.
            let pin = secret
                .key
                .is_none()
                .then(|| ipsec::Pin::read(&pin_path))
                .flatten();
            if let Some(der) = pin.as_ref().and_then(ipsec::Pin::der) {
                vici.load_trusted(&certificate::pem(&der)).await?;
            }
            vici.load(&tunnel, &password).await?;
            let signed = tokio::select! {
                s = vici.initiate(name) => Some(s),
                _ = &mut *stop_rx => None,
            };
            match signed {
                Some(Signed::In) => return Ok(None),
                Some(refusal @ (Signed::Refused | Signed::RefusedAtFirst)) => {
                    vici.forget(name).await;
                    // A gateway asked for a name it isn't refuses before it
                    // shows its certificate (the spike): not the password.
                    if secret.key.is_none() {
                        let looked = self
                            .look_at_gateway(vici, &tunnel, &password, &expected)
                            .await;
                        if let Looked::WrongName(_) = looked {
                            return Ok(Some((
                                TunnelState::Trouble,
                                why_not(&looked, who, &expected),
                            )));
                        }
                    }
                    // Before any password, or with no password at all: the
                    // user's certificate.
                    let certificate =
                        matches!(refusal, Signed::RefusedAtFirst) || !method.password();
                    if method.certificate() && certificate {
                        return Ok(Some((
                            TunnelState::Trouble,
                            Some(super::ipsec_credential::refused(who)),
                        )));
                    }
                    retry = true;
                }
                Some(Signed::KeyRefused) => {
                    vici.forget(name).await;
                    return Ok(Some((
                        TunnelState::Trouble,
                        Some(format!(
                            "{} {who}\u{2019}s gateway. Your IT team can tell you the right one; change it in {who}\u{2019}s Edit.",
                            crake_proto::trouble::KEY_MISMATCH
                        )),
                    )));
                }
                Some(Signed::Untrusted) => {
                    vici.forget(name).await;
                    let mut looked = self
                        .look_at_gateway(vici, &tunnel, &password, &expected)
                        .await;
                    // IT's authority is the only one for this gateway, and
                    // a certificate already trusted can't be asked again:
                    // nothing to ask either way.
                    if let Looked::Ask(cert) = &looked {
                        let same = pin.as_ref().and_then(ipsec::Pin::der).as_deref()
                            == Some(cert.der.as_slice());
                        if secret.ca.is_some() || same {
                            looked = Looked::Unread;
                        }
                    }
                    if let Some(why) = why_not(&looked, who, &expected) {
                        return Ok(Some((TunnelState::Trouble, Some(why))));
                    }
                    let Looked::Ask(cert) = looked else {
                        return Ok(Some((TunnelState::Trouble, None)));
                    };
                    let extra = Self::trust_question(&cert, &expected, pin.as_ref());
                    let asked = (AskKind::Trust, pin.is_some());
                    let answer = self
                        .answer_more(name, asked, extra, &answers, &mut answer_rx, stop_rx)
                        .await;
                    if answer.as_deref() != Some("yes") {
                        return Ok(Some((TunnelState::Off, None)));
                    }
                    self.keep_pin(name, &cert)?;
                    known = Some((user, password));
                    retry = false;
                }
                Some(Signed::Failed(why)) => {
                    vici.forget(name).await;
                    return Ok(Some((TunnelState::Trouble, Some(why))));
                }
                None => {
                    vici.end(name).await;
                    return Ok(Some((TunnelState::Off, None)));
                }
            }
        }
    }

    /// The username (unless the tunnel has it) and password, asked; again,
    /// saying so, after a refusal. None when it's switched off meanwhile.
    async fn user_and_password(
        &self,
        config: &TunnelConfig,
        retry: bool,
        answers: &Answers,
        answer_rx: &mut mpsc::UnboundedReceiver<(u32, String)>,
        stop_rx: &mut oneshot::Receiver<Stop>,
    ) -> Option<(String, String)> {
        let name = &config.name;
        let user = match &config.user {
            Some(u) => u.clone(),
            None => {
                self.answer_of(name, (AskKind::User, retry), answers, answer_rx, stop_rx)
                    .await?
            }
        };
        let asked = (AskKind::Password, retry && config.user.is_some());
        let password = self
            .answer_of(name, asked, answers, answer_rx, stop_rx)
            .await?;
        Some((user, password))
    }

    /// Ask the user, and wait for the answer; None when it's switched off
    /// meanwhile.
    async fn answer_of(
        &self,
        name: &str,
        asked: (AskKind, bool),
        answers: &Answers,
        answer_rx: &mut mpsc::UnboundedReceiver<(u32, String)>,
        stop_rx: &mut oneshot::Receiver<Stop>,
    ) -> Option<String> {
        let extra = std::collections::BTreeMap::new();
        self.answer_more(name, asked, extra, answers, answer_rx, stop_rx)
            .await
    }

    /// `answer_of`, the question with more about it.
    async fn answer_more(
        &self,
        name: &str,
        (kind, retry): (AskKind, bool),
        extra: std::collections::BTreeMap<String, String>,
        answers: &Answers,
        answer_rx: &mut mpsc::UnboundedReceiver<(u32, String)>,
        stop_rx: &mut oneshot::Receiver<Stop>,
    ) -> Option<String> {
        let id = self.ask_more(name, (kind, String::new(), None, retry), extra, answers);
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
        trust_the_systems_authorities(&socket).await;
        Ok(socket)
    }

    /// Stop craked's charon (craked is leaving).
    pub(super) async fn stop_charon(&self) {
        if let Some(mut c) = self.charon.lock().await.take() {
            c.stop().await;
        }
    }
}

/// The system's certificate authorities, for gateways that prove
/// themselves with a certificate and name no authority of IT's: charon
/// knows none of its own (craked's settings, not the system's). Without
/// them the tunnels with a key still work, so a failure is only logged.
async fn trust_the_systems_authorities(socket: &std::path::Path) {
    let Some((path, pems)) = ipsec::system_authorities() else {
        tracing::warn!("no certificate authorities found on this computer");
        return;
    };
    let mut loaded = 0;
    match Vici::connect(socket).await {
        Ok(mut vici) => {
            for pem in &pems {
                if vici.load_authority(pem).await.is_ok() {
                    loaded += 1;
                }
            }
        }
        Err(e) => tracing::warn!("charon's vici: {e:#}"),
    }
    tracing::info!(
        "charon trusts {loaded} of {} certificate authorities in {}",
        pems.len(),
        path.display()
    );
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
