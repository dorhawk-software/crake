//! A gateway's certificate the user trusted ("Trust it"): remembered per
//! gateway (`prefs::Prefs::trusted`) and handed to openconnect as
//! `--servercert`, so the next sign-in doesn't ask again. Until 2026-10-27
//! every sign-in asked (a Mac, twenty times; its real cause there was a
//! company root only in the Keychain, `sys::trusted_cafile`). A gateway
//! that later shows another certificate is asked about again, saying when
//! the one it replaces was trusted. IPsec and OpenVPN keep their own (craked).

use super::Spec;

/// The pin to hand openconnect: the one trusted before, unless the
/// tunnel's own arguments already say which certificate to accept.
pub(super) fn pinned(spec: &Spec, auth_args: &[String]) -> Option<String> {
    let configured = spec
        .extra_args
        .iter()
        .chain(auth_args)
        .any(|a| a.starts_with("--servercert"));
    spec.trusted
        .as_ref()
        .filter(|_| !configured)
        .map(|t| t.pin.clone())
}

/// openconnect refused the gateway's certificate for not being the pinned
/// one (9.x's words, then 8.x's).
pub(super) fn mismatched(lines: &[String]) -> bool {
    lines.iter().any(|l| {
        l.contains("specified via --servercert match")
            || l.contains("Server SSL certificate didn't match")
    })
}

/// "21 October", the day the replaced certificate was trusted.
pub(super) fn trusted_on(spec: &Spec) -> Option<String> {
    crate::openvpn::said_day(&spec.trusted.as_ref()?.on)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_certificate_other_than_the_trusted_one_is_noticed() {
        let nine = "None of the 1 fingerprint(s) specified via --servercert match server's certificate: pin-sha256:AbC=";
        assert!(super::mismatched(&[nine.into()]));
        let eight = "Server SSL certificate didn't match: pin-sha256:AbC=";
        assert!(super::mismatched(&[eight.into()]));
        assert!(!super::mismatched(&[
            "Failed to complete authentication".into()
        ]));
    }

    /// A gateway that shows another certificate than the one trusted
    /// before: openconnect refuses the pin, and the question comes again,
    /// saying when the old one was trusted.
    #[cfg(unix)]
    #[test]
    fn a_certificate_other_than_the_trusted_one_is_asked_about() {
        use super::super::{Ask, Event, Reply, Spec, run};
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("openconnect");
        std::fs::write(
            &fake,
            "#!/bin/sh\n\
             case \"$*\" in *--servercert=pin-sha256:OLD=*)\n\
               echo \"None of the 1 fingerprint(s) specified via --servercert match server's certificate: pin-sha256:NEW=\" >&2; exit 1;;\n\
             esac\n\
             echo 'To trust this server in future, perhaps add this to your command line:' >&2\n\
             echo '    --servercert pin-sha256:NEW=' >&2\n\
             printf \"Enter 'yes' to accept, 'no' to abort; anything else to view:\" >&2; read a\n\
             [ \"$a\" = yes ] || exit 1\n\
             echo \"COOKIE='c'\"; echo \"CONNECT_URL='https://gw/'\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let (events, shown) = async_channel::unbounded();
        let (answer, replies) = async_channel::unbounded();
        let spec = Spec {
            tunnel: "acme".into(),
            display: "ACME".into(),
            gateway: "gw".into(),
            protocol: "anyconnect".into(),
            user: String::new(),
            auth_args: vec![],
            extra_args: vec![],
            user_agent: None,
            keyring: false,
            openconnect: Some(fake),
            trusted: Some(crate::prefs::Trusted {
                pin: "pin-sha256:OLD=".into(),
                on: "2026-10-21".into(),
            }),
        };
        let asked = std::thread::spawn(move || {
            loop {
                if let Ok(Event::Prompt(p)) = shown.recv_blocking() {
                    answer
                        .send_blocking(Reply::Answer {
                            text: "yes".into(),
                            remember: false,
                            keep_certificate: false,
                        })
                        .unwrap();
                    return p.ask;
                }
            }
        });
        assert!(run(&spec, &events, &replies, &rt).is_ok(), "signed in");
        assert_eq!(
            asked.join().unwrap(),
            Ask::TrustServer {
                fingerprint: Some("pin-sha256:NEW=".into()),
                about: None,
                trusted_on: crate::openvpn::said_day("2026-10-21"),
            }
        );
    }
}
