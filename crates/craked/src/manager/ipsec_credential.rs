//! The user's own certificate for an IPsec tunnel
//! (design/proposals/ipsec-certificates.html, 2 and 3): asked of the app,
//! which answers from the keyring without a window; opened here
//! (`ipsec::credential`), its password asked as "Unlock your certificate"
//! when it has one, again when it didn't open it; its dates read before
//! anything is sent.

use tokio::sync::mpsc;

use super::entry::Answers;
use super::*;
use crate::ipsec::credential::{self, Credential, Opened};
use crate::openvpn::AskKind;

/// Where a question's answer comes from, and what stops waiting for it.
pub(super) struct Asking<'a> {
    pub(super) answers: &'a Answers,
    pub(super) answer_rx: &'a mut mpsc::UnboundedReceiver<(u32, String)>,
    pub(super) stop_rx: &'a mut oneshot::Receiver<Stop>,
}

impl Manager {
    /// The user's certificate, ready for charon; else how the run ended
    /// (switched off meanwhile, or why it can't sign in).
    pub(super) async fn ipsec_credential(
        &self,
        name: &str,
        who: &str,
        asking: &mut Asking<'_>,
    ) -> std::result::Result<Credential, (TunnelState, Option<String>)> {
        use base64::Engine as _;
        let off = || (TunnelState::Off, None);
        let file = self
            .answer_from(name, (AskKind::Certificate, false), asking)
            .await
            .ok_or_else(off)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(file.trim())
            .unwrap_or_default();
        if bytes.is_empty() {
            return Err(trouble(format!(
                "{}. Choose it again in {who}\u{2019}s Edit.",
                crake_proto::trouble::NO_CERTIFICATE
            )));
        }
        let mut password: Option<String> = None;
        let mut retry = false;
        let credential = loop {
            match credential::open(&bytes, password.as_deref()) {
                Opened::Ready(c) => break c,
                Opened::Unusable(why) => return Err(trouble(why)),
                Opened::Password => {
                    retry = retry || password.is_some();
                    password = Some(
                        self.answer_from(name, (AskKind::KeyPassword, retry), asking)
                            .await
                            .ok_or_else(off)?,
                    );
                }
            }
        };
        let today = time::OffsetDateTime::now_utc().date();
        if credential.cert.not_after < today {
            return Err(trouble(format!(
                "{} {}. Your IT team can give you a new one.",
                crake_proto::trouble::YOUR_CERT_RAN_OUT,
                super::ipsec_trust::day(credential.cert.not_after, today)
            )));
        }
        if credential.cert.not_before > today {
            return Err(trouble(format!(
                "{} {}. If this computer\u{2019}s date is wrong, correct it and try again.",
                crake_proto::trouble::YOUR_CERT_NOT_YET,
                super::ipsec_trust::day(credential.cert.not_before, today)
            )));
        }
        Ok(credential)
    }

    /// Ask, and wait for the answer; None when it's switched off meanwhile.
    async fn answer_from(
        &self,
        name: &str,
        (kind, retry): (AskKind, bool),
        asking: &mut Asking<'_>,
    ) -> Option<String> {
        let id = self.ask(name, (kind, String::new(), None, retry), asking.answers);
        loop {
            tokio::select! {
                _ = &mut *asking.stop_rx => {
                    self.asked(name);
                    return None;
                }
                Some((got, text)) = asking.answer_rx.recv() => {
                    if got == id {
                        self.asked(name);
                        return Some(text);
                    }
                }
            }
        }
    }
}

fn trouble(why: String) -> (TunnelState, Option<String>) {
    (TunnelState::Trouble, Some(why))
}

/// Nordbank's gateway turned the user's certificate down.
pub(super) fn refused(who: &str) -> String {
    format!(
        "{who}\u{2019}s gateway {}. Your IT team can tell you which one it expects.",
        crake_proto::trouble::CERT_REFUSED
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_refused_certificate_is_said_in_a_few_words() {
        let why = super::refused("Nordbank");
        assert_eq!(
            crake_proto::trouble::label(&why),
            Some("Certificate not accepted")
        );
    }
}
