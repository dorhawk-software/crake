//! Why a tunnel stopped (design/proposals/trouble-reason.html, option A):
//! the sentences craked logs for the reasons Crake recognises, and the few
//! words the panel shows for each instead of "Couldn't stay connected".

/// openvpn3's SESSION_EXPIRED, or openconnect saying the session expired.
pub const EXPIRED: &str = "Its session expired";
/// openconnect: the gateway ended the session, or refused its cookie.
pub const GATEWAY_ENDED: &str = "Its gateway ended the session";
/// openvpn3: the gateway turned the sign-in down.
pub const SIGN_IN_REFUSED: &str = "The gateway didn\u{2019}t accept the sign-in";
/// craked: the session's device went away (openvpn3 ended it).
pub const OPENVPN3_ENDED: &str = "OpenVPN 3 ended its session";
/// IPsec: the gateway's key, as IT gave it, doesn't match the gateway.
pub const KEY_MISMATCH: &str = "The key doesn\u{2019}t match";
/// IPsec: the gateway's certificate isn't one to trust for its name
/// (design/proposals/ipsec-certificates.html, 5: the trust question comes
/// once Crake reads the certificate itself).
pub const UNTRUSTED: &str = "Crake can\u{2019}t vouch for";
/// IPsec: the gateway's certificate is for another name; craked read it
/// (never offered to be trusted).
pub const WRONG_NAME: &str = "The gateway\u{2019}s certificate is for";
/// IPsec: the gateway's certificate is out of its dates, or not in them
/// yet ("Nordbank's gateway shows a certificate that ran out on …").
pub const RAN_OUT: &str = "shows a certificate that ran out on";
pub const NOT_YET: &str = "shows a certificate that\u{2019}s valid only from";
/// IPsec: the gateway turned the user's own certificate down
/// ("Nordbank's gateway didn't accept your certificate. …").
pub const CERT_REFUSED: &str = "didn\u{2019}t accept your certificate";
/// IPsec: the user's certificate is out of its dates, read before
/// anything is sent.
pub const YOUR_CERT_RAN_OUT: &str = "Your certificate ran out on";
pub const YOUR_CERT_NOT_YET: &str = "Your certificate is valid only from";
/// IPsec: the app has no copy of the user's certificate to give.
pub const NO_CERTIFICATE: &str = "Crake has no copy of your certificate";
/// What to do about it.
pub const SIGN_IN_AGAIN: &str = "Switch it on to sign in again.";

/// The status's few words for a reason Crake recognises.
pub fn label(why: &str) -> Option<&'static str> {
    [
        (EXPIRED, "Session expired"),
        (GATEWAY_ENDED, "Ended by the gateway"),
        (SIGN_IN_REFUSED, "Sign-in refused"),
        (OPENVPN3_ENDED, "Ended by OpenVPN 3"),
        (KEY_MISMATCH, "Key doesn\u{2019}t match"),
        (UNTRUSTED, "Gateway not trusted"),
        (WRONG_NAME, "Certificate for another name"),
        (YOUR_CERT_RAN_OUT, "Your certificate ran out"),
        (YOUR_CERT_NOT_YET, "Certificate not valid yet"),
        (NO_CERTIFICATE, "No certificate"),
    ]
    .into_iter()
    .find(|(start, _)| why.starts_with(start))
    .or_else(|| {
        // Said after the tunnel's name ("Nordbank's gateway shows …").
        [
            (RAN_OUT, "Gateway\u{2019}s certificate ran out"),
            (NOT_YET, "Gateway\u{2019}s certificate not valid yet"),
            (CERT_REFUSED, "Certificate not accepted"),
        ]
        .into_iter()
        .find(|(part, _)| why.contains(part))
    })
    .map(|(_, words)| words)
}

/// openconnect's last words (openconnect 9.12, run with LC_ALL=C), put
/// plainly, when they're a reason Crake recognises.
pub fn from_openconnect(last: &str) -> Option<String> {
    const EXPIRED_LINES: [&str; 3] = [
        "Server terminated connection (session expired)",
        "VPN session expired",
        "Cookie is no longer valid, ending session",
    ];
    const ENDED_LINES: [&str; 2] = [
        "Session terminated by server; exiting.",
        "Cookie was rejected by server; exiting.",
    ];
    if EXPIRED_LINES.iter().any(|l| last.contains(l)) {
        Some(format!("{EXPIRED}. {SIGN_IN_AGAIN}"))
    } else if ENDED_LINES.iter().any(|l| last.contains(l)) {
        Some(format!("{GATEWAY_ENDED}. {SIGN_IN_AGAIN}"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_that_doesnt_match_is_said_in_a_few_words() {
        let why = format!(
            "{KEY_MISMATCH} Nordbank\u{2019}s gateway. Your IT team can tell you the right one."
        );
        assert_eq!(label(&why), Some("Key doesn\u{2019}t match"));
    }

    #[test]
    fn openconnects_words_are_put_plainly() {
        for line in [
            "Server terminated connection (session expired)",
            "VPN session expired",
            "Cookie is no longer valid, ending session",
        ] {
            let why = from_openconnect(line).unwrap_or_default();
            assert_eq!(label(&why), Some("Session expired"), "{line}: {why}");
        }
        for line in [
            "Session terminated by server; exiting.",
            "Cookie was rejected by server; exiting.",
        ] {
            let why = from_openconnect(line).unwrap_or_default();
            assert_eq!(label(&why), Some("Ended by the gateway"), "{line}: {why}");
            assert!(why.ends_with(SIGN_IN_AGAIN), "{why}");
        }
        assert_eq!(from_openconnect("SSL negotiation failed"), None);
    }

    #[test]
    fn each_reason_has_its_few_words() {
        let expired = format!("{EXPIRED}: the server ends sessions after a while. {SIGN_IN_AGAIN}");
        assert_eq!(label(&expired), Some("Session expired"));
        assert_eq!(
            label(&format!("{SIGN_IN_REFUSED}. AUTH_FAILED")),
            Some("Sign-in refused")
        );
        assert_eq!(
            label(&format!("{OPENVPN3_ENDED}: tun0 is gone.")),
            Some("Ended by OpenVPN 3")
        );
        assert_eq!(
            label(&format!("{UNTRUSTED} Nordbank\u{2019}s gateway.")),
            Some("Gateway not trusted")
        );
        assert_eq!(
            label(&format!(
                "{WRONG_NAME} vpn.globex.example, not gw.nordbank.example."
            )),
            Some("Certificate for another name")
        );
        assert_eq!(
            label(&format!("Nordbank\u{2019}s gateway {RAN_OUT} 18 October.")),
            Some("Gateway\u{2019}s certificate ran out")
        );
        assert_eq!(
            label(&format!("Nordbank\u{2019}s gateway {NOT_YET} 1 November.")),
            Some("Gateway\u{2019}s certificate not valid yet")
        );
        assert_eq!(
            label(&format!("Nordbank\u{2019}s gateway {CERT_REFUSED}.")),
            Some("Certificate not accepted")
        );
        assert_eq!(
            label(&format!("{YOUR_CERT_RAN_OUT} 3 March 2026.")),
            Some("Your certificate ran out")
        );
        assert_eq!(label(&format!("{NO_CERTIFICATE}.")), Some("No certificate"));
        assert_eq!(label("openconnect exited with exit status: 1."), None);
        assert_eq!(label(""), None);
    }
}
