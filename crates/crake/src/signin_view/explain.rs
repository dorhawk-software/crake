//! Sign-in: why it didn't work, in Crake's words.

use super::*;

/// A failure in Crake's words (DESIGN.md, copy voice): the title, one
/// sentence with what it means, and the failed step's own short line.
pub(super) struct Why {
    pub(super) title: String,
    pub(super) sentence: String,
    pub(super) row: String,
}

/// OpenVPN 3: openvpn3 reaches the server; what fails before that is
/// openvpn3 itself or the profile, not the network.
pub(super) fn explain_openvpn3(r: &str, at: usize, spec: &Spec) -> Why {
    let (display, profile) = (&spec.display, &spec.gateway);
    if r.contains("no profile called") || r.contains("look up the profile") {
        return Why {
            title: format!("Couldn\u{2019}t start {display}"),
            sentence: format!(
                "OpenVPN 3 has no profile called {profile} for you. It may have been removed or renamed."
            ),
            row: format!("No profile {profile}"),
        };
    }
    if r.contains("reach openvpn 3")
        || r.contains("start a session")
        || r.contains("wasn\u{2019}t ready")
    {
        return Why {
            title: format!("Couldn\u{2019}t start {display}"),
            sentence: "OpenVPN 3 didn\u{2019}t start the session. Its own words are below.".into(),
            row: "OpenVPN 3 didn\u{2019}t start it".into(),
        };
    }
    if r.contains("accept the sign-in") {
        return Why {
            title: format!("Couldn\u{2019}t sign in to {display}"),
            sentence: format!(
                "{display} didn\u{2019}t accept the sign-in. Ask {display}\u{2019}s IT whether your account is allowed in."
            ),
            row: "Sign-in not accepted".into(),
        };
    }
    Why {
        title: format!("{display} didn\u{2019}t come up"),
        sentence: format!("OpenVPN 3 couldn\u{2019}t connect {display}. Its own words are below."),
        row: if at >= 2 {
            "Tunnel didn\u{2019}t come up".into()
        } else {
            "Connection failed".into()
        },
    }
}

pub(super) fn explain(raw: &str, at: usize, spec: &Spec) -> Why {
    let (display, gateway) = (&spec.display, &spec.gateway);
    let r = raw.to_ascii_lowercase();
    if spec.protocol == "openvpn3" {
        return explain_openvpn3(&r, at, spec);
    }
    if r.contains("couldn't start openconnect") || r.contains("couldn\u{2019}t start openconnect") {
        return Why {
            title: "openconnect isn\u{2019}t installed".into(),
            sentence: format!(
                "Crake uses openconnect to talk to {display}\u{2019}s gateway. Install it with: sudo apt install openconnect"
            ),
            row: "openconnect not installed".into(),
        };
    }
    let unreachable = at == 0
        || [
            "connect to",
            "timed out",
            "refused",
            "resolve",
            "no route",
            "unreachable",
            "https connection",
        ]
        .iter()
        .any(|w| r.contains(w));
    if unreachable {
        return Why {
            title: format!("Couldn\u{2019}t reach {display}"),
            sentence: "The gateway didn\u{2019}t answer. It\u{2019}s either down or blocked on this network.".into(),
            row: format!("No answer from {gateway}"),
        };
    }
    if at >= 2 {
        return Why {
            title: format!("{display} didn\u{2019}t stay up"),
            sentence: format!(
                "{display} accepted the sign-in, but the tunnel didn\u{2019}t come up."
            ),
            row: "Tunnel didn\u{2019}t come up".into(),
        };
    }
    if ["certificate", "pkcs", "pass phrase", "decrypt"]
        .iter()
        .any(|w| r.contains(w))
    {
        return Why {
            title: format!("Couldn\u{2019}t sign in to {display}"),
            sentence: "The certificate didn\u{2019}t open. Its password may be wrong, or the file damaged.".into(),
            row: "Certificate didn\u{2019}t open".into(),
        };
    }
    Why {
        title: format!("Couldn\u{2019}t sign in to {display}"),
        sentence: format!(
            "{display}\u{2019}s gateway didn\u{2019}t accept the sign-in. Check what you typed, or ask {display}\u{2019}s IT whether your account is allowed in."
        ),
        row: "Sign-in not accepted".into(),
    }
}
