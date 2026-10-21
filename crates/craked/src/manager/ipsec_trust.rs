//! An IPsec gateway whose certificate charon can't trust
//! (design/proposals/ipsec-certificates.html, 3 and 5): craked looks at
//! the certificate (charon's decryption dump, on only for the look),
//! checks its name first, then its dates, and only then asks the user
//! whether to trust it, as AnyConnect's question asks; trusted, it's kept
//! beside the tunnel's secret and handed to charon, which checks it again
//! on every connection.

use std::collections::BTreeMap;

use super::*;
use crate::ipsec::certificate::{self, Certificate};
use crate::ipsec::vici::{Tunnel, Vici, look_name};

/// What a look at the gateway's certificate found.
pub(super) enum Looked {
    /// No certificate in the dump: said as charon's refusal.
    Unread,
    /// For another name than the gateway's (never offered to be trusted).
    WrongName(Certificate),
    /// Out of its dates, or not in them yet.
    RanOut(Certificate),
    NotYet(Certificate),
    /// One to ask the user about.
    Ask(Certificate),
}

impl Manager {
    /// Look at the gateway's certificate: charon dumps what it decrypts
    /// while a connection that names no gateway tries it (it fails at the
    /// certificate), then the dump goes off and its file is emptied.
    pub(super) async fn look_at_gateway(
        &self,
        vici: &mut Vici,
        tunnel: &Tunnel<'_>,
        password: &str,
        expected: &str,
    ) -> Looked {
        let Some(dump) = self.charon_dump(true).await else {
            return Looked::Unread;
        };
        let _ = vici.reload_settings().await;
        let _ = vici.look(tunnel, password).await;
        let text = std::fs::read_to_string(&dump).unwrap_or_default();
        let _ = self.charon_dump(false).await;
        let _ = vici.reload_settings().await;
        let Some(cert) = certificate::from_dump(&text, &look_name(tunnel.name))
            .and_then(|d| certificate::parse(&d))
        else {
            return Looked::Unread;
        };
        let today = time::OffsetDateTime::now_utc().date();
        if !cert.is_for(expected) {
            Looked::WrongName(cert)
        } else if cert.not_after < today {
            Looked::RanOut(cert)
        } else if cert.not_before > today {
            Looked::NotYet(cert)
        } else {
            Looked::Ask(cert)
        }
    }

    /// charon's dump on or off; its file, when charon runs.
    async fn charon_dump(&self, on: bool) -> Option<std::path::PathBuf> {
        let mut charon = self.charon.lock().await;
        let c = charon.as_mut()?;
        if let Err(e) = c.dump(on) {
            tracing::warn!("charon's dump: {e:#}");
            return None;
        }
        Some(c.dump.clone())
    }

    /// The trust question's details: the certificate's pin, who it's for
    /// and who signed it, and the day the one it replaces was trusted.
    pub(super) fn trust_question(
        cert: &Certificate,
        expected: &str,
        replaced: Option<&crate::ipsec::Pin>,
    ) -> BTreeMap<String, String> {
        let mut extra = BTreeMap::from([
            ("fingerprint".to_owned(), cert.pin()),
            ("for".to_owned(), expected.to_owned()),
            ("issuer".to_owned(), cert.issuer.clone()),
        ]);
        if let Some(p) = replaced {
            extra.insert("trusted_on".to_owned(), p.trusted_on.clone());
        }
        extra
    }

    /// The certificate the user trusted for `name`'s gateway, kept.
    pub(super) fn keep_pin(&self, name: &str, cert: &Certificate) -> Result<crate::ipsec::Pin> {
        use std::io::Write;
        let pin = crate::ipsec::Pin::new(&cert.der, time::OffsetDateTime::now_utc().date());
        let path = crate::ipsec::pin_path(&self.cfg.state_path, name);
        if let Some(dir) = path.parent() {
            crate::sys::create_private_dir(dir)?;
        }
        let mut f = crate::sys::create_private(&path)?;
        f.write_all(serde_json::to_string(&pin)?.as_bytes())?;
        Ok(pin)
    }
}

/// A day as the sign-in window says it: "18 October", with its year when
/// it isn't this one's.
fn day(date: time::Date, today: time::Date) -> String {
    if date.year() == today.year() {
        format!("{} {}", date.day(), date.month())
    } else {
        format!("{} {} {}", date.day(), date.month(), date.year())
    }
}

/// Why a look ended the sign-in, in the sign-in window's words
/// (ipsec-certificates.html, 3 and 4); None to ask.
pub(super) fn why_not(looked: &Looked, who: &str, expected: &str) -> Option<String> {
    use crake_proto::trouble;
    let today = time::OffsetDateTime::now_utc().date();
    Some(match looked {
        Looked::Ask(_) => return None,
        Looked::Unread => format!(
            "{} {who}\u{2019}s gateway: its certificate isn\u{2019}t for {expected}, or isn\u{2019}t signed by an authority this computer knows. Ask your IT team for its certificate authority, or whether its name is right.",
            trouble::UNTRUSTED
        ),
        Looked::WrongName(c) => format!(
            "{} {}, not {expected}. Your IT team can tell you whether that\u{2019}s right.",
            trouble::WRONG_NAME,
            c.names_said()
        ),
        Looked::RanOut(c) => format!(
            "{who}\u{2019}s gateway {} {}. Your IT team needs to renew it.",
            trouble::RAN_OUT,
            day(c.not_after, today)
        ),
        Looked::NotYet(c) => format!(
            "{who}\u{2019}s gateway {} {}. If this computer\u{2019}s date is wrong, correct it and try again.",
            trouble::NOT_YET,
            day(c.not_before, today)
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_is_said_with_its_year_only_when_it_isnt_this_ones() {
        let today = time::Date::from_calendar_date(2026, time::Month::October, 21).unwrap();
        let soon = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
        let later = time::Date::from_calendar_date(2028, time::Month::March, 1).unwrap();
        assert_eq!(day(soon, today), "1 November");
        assert_eq!(day(later, today), "1 March 2028");
    }

    #[test]
    fn only_a_certificate_for_the_gateway_is_asked_about() {
        assert_eq!(
            why_not(&Looked::Unread, "Nordbank", "gw.nordbank.example")
                .map(|w| w.starts_with(crake_proto::trouble::UNTRUSTED)),
            Some(true)
        );
    }
}
