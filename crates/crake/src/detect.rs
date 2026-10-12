//! New tunnel, "Detect automatically": ask the gateway, with openconnect
//! itself and nothing signed in, whether it is an AnyConnect gateway, and
//! which sign-in groups it offers. As AnyConnect, so gateways that only
//! admit Cisco's client answer too (CLAUDE.md, 7d).

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How often a running detection is looked at.
const POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// An AnyConnect (or ocserv) gateway; the groups it offers, if any,
    /// and when its certificate stops being valid, if it showed one.
    AnyConnect {
        groups: Vec<String>,
        expires: Option<jiff::Timestamp>,
    },
    /// It answered, but not as AnyConnect: the user picks the protocol.
    Unknown,
    /// Nothing answered.
    Unreachable,
}

/// Ask `gateway` (blocking, at most `wait`). A gateway whose certificate
/// this computer doesn't trust (a company's own authority) is asked again,
/// pinned to the certificate it showed: nothing is signed in here, and
/// sign-in still asks the user to trust it.
pub fn gateway(gateway: &str, wait: Duration) -> Found {
    let start = Instant::now();
    let text = ask(gateway, None, wait);
    let found = read(&text);
    let found = match (&found, pin(&text)) {
        (Found::Unknown, Some(pin)) => read(&ask(
            gateway,
            Some(&pin),
            wait.saturating_sub(start.elapsed()),
        )),
        _ => found,
    };
    match found {
        Found::AnyConnect { groups, .. } => Found::AnyConnect {
            groups,
            expires: crate::gateway_cert::expiry(gateway, wait.saturating_sub(start.elapsed())),
        },
        other => other,
    }
}

/// A certificate that runs out this soon is said to (with its day).
const SOON_DAYS: u64 = 30;
const SOON: jiff::SignedDuration =
    jiff::SignedDuration::from_secs((SOON_DAYS * crate::units::SECS_PER_DAY) as i64);

/// What New tunnel says about the gateway's certificate, after "Found a
/// Cisco AnyConnect gateway": valid until a month, running out on a day,
/// or run out. Nothing when it showed none.
pub fn certificate_said(expires: Option<jiff::Timestamp>, now: jiff::Timestamp) -> String {
    let found = "Found a Cisco AnyConnect gateway";
    let Some(expires) = expires else {
        return format!("{found}.");
    };
    let day = || expires.strftime("%-d %B %Y").to_string();
    if expires <= now {
        format!("{found}, but its certificate expired on {}.", day())
    } else if expires.duration_since(now) < SOON {
        format!("{found}. Its certificate expires on {}.", day())
    } else {
        format!(
            "{found}. Its certificate is valid until {}.",
            expires.strftime("%B %Y")
        )
    }
}

/// What openconnect says on stderr, asking as AnyConnect within `wait`.
fn ask(gateway: &str, servercert: Option<&str>, wait: Duration) -> String {
    let program = crate::engines::openconnect();
    let mut cmd = Command::new(program);
    crate::sys::no_console(&mut cmd);
    cmd.args([
        "--authenticate",
        "--non-inter",
        "--protocol=anyconnect",
        "--useragent=AnyConnect",
    ]);
    if let Some(pin) = servercert {
        cmd.arg(format!("--servercert={pin}"));
    }
    cmd.args(["--", gateway])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let Ok(mut child) = crate::engines::spawn(&mut cmd) else {
        return String::new();
    };
    let start = Instant::now();
    while start.elapsed() < wait {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(POLL);
    }
    let _ = child.kill();
    let _ = child.wait();
    let mut text = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut text);
    }
    text
}

/// The certificate openconnect wouldn't trust, as it suggests pinning it
/// ("--servercert pin-sha256:...").
fn pin(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let pin = l.trim().strip_prefix("--servercert ")?;
        pin.starts_with("pin-sha256:").then(|| pin.to_owned())
    })
}

/// What openconnect's output says about the gateway.
fn read(text: &str) -> Found {
    let lines: Vec<&str> = text.lines().collect();
    let anyconnect = lines.iter().any(|l| {
        l.starts_with("XML POST enabled")
            || l.starts_with("Server requested SSL client certificate")
            || l.starts_with("Please enter your")
            || l.contains("GROUP:")
            || l.starts_with("XML response has no \"auth\" node")
    });
    if anyconnect {
        // "GROUP: [Employees|Contractors]:"
        let groups = lines
            .iter()
            .find_map(|l| {
                let i = l.find("GROUP:")?;
                let rest = &l[i..];
                let (open, close) = (rest.find('[')?, rest.rfind(']')?);
                Some(
                    rest[open + 1..close]
                        .split('|')
                        .map(|g| g.trim().to_owned())
                        .filter(|g| !g.is_empty())
                        .collect(),
                )
            })
            .unwrap_or_default();
        return Found::AnyConnect {
            groups,
            expires: None,
        };
    }
    let reached = lines
        .iter()
        .any(|l| l.starts_with("Connected to ") || l.starts_with("SSL negotiation"));
    if reached {
        Found::Unknown
    } else {
        Found::Unreachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_how_long_the_certificate_is_valid() {
        let now: jiff::Timestamp = "2026-10-12T12:00:00Z".parse().unwrap();
        let at = |s: &str| Some(s.parse::<jiff::Timestamp>().unwrap());
        assert_eq!(
            certificate_said(at("2027-03-31T12:00:00Z"), now),
            "Found a Cisco AnyConnect gateway. Its certificate is valid until March 2027."
        );
        assert_eq!(
            certificate_said(at("2026-11-04T12:00:00Z"), now),
            "Found a Cisco AnyConnect gateway. Its certificate expires on 4 November 2026."
        );
        assert_eq!(
            certificate_said(at("2026-10-03T12:00:00Z"), now),
            "Found a Cisco AnyConnect gateway, but its certificate expired on 3 October 2026."
        );
        assert_eq!(
            certificate_said(None, now),
            "Found a Cisco AnyConnect gateway."
        );
    }

    #[test]
    fn reads_what_the_gateway_said() {
        // vpn.acme.example as AnyConnect, no certificate given.
        let acme = "POST https://vpn.acme.example/\nConnected to 203.0.113.10:443\n\
                  Server requested SSL client certificate; none was configured\n\
                  POST https://vpn.acme.example/\nXML response has no \"auth\" node\n";
        assert_eq!(
            read(acme),
            Found::AnyConnect {
                groups: vec![],
                expires: None
            }
        );
        let groups = "Connected to 192.0.2.1:443\nXML POST enabled\nPlease enter your username and password.\n\
                      GROUP: [Employees|Contractors]:\nUser input required in non-interactive mode\n";
        assert_eq!(
            read(groups),
            Found::AnyConnect {
                groups: vec!["Employees".into(), "Contractors".into()],
                expires: None
            }
        );
        assert_eq!(
            read("Connected to 192.0.2.1:443\nGot HTTP response: HTTP/1.1 404 Not Found\n"),
            Found::Unknown
        );
        assert_eq!(
            read("getaddrinfo failed for host 'nope.example': Name or service not known\n"),
            Found::Unreachable
        );
    }

    #[test]
    fn sees_past_a_certificate_this_computer_does_not_trust() {
        // A company's own certificate authority, not installed here.
        let untrusted = "POST https://vpn.acme.example/\nConnected to 203.0.113.10:443\n\
                         SSL negotiation with vpn.acme.example\n\
                         Server certificate verify failed: signer not found\n\n\
                         Certificate from VPN server \"vpn.acme.example\" failed verification.\n\
                         Reason: signer not found\n\
                         To trust this server in future, perhaps add this to your command line:\n    \
                         --servercert pin-sha256:QUNNRSBleGFtcGxlIGNlcnRpZmljYXRlIHBpbg==\n\
                         SSL connection failure: Error in the certificate.\n";
        assert_eq!(read(untrusted), Found::Unknown);
        assert_eq!(
            pin(untrusted).as_deref(),
            Some("pin-sha256:QUNNRSBleGFtcGxlIGNlcnRpZmljYXRlIHBpbg==")
        );
        assert_eq!(pin("Connected to 192.0.2.1:443\nXML POST enabled\n"), None);
    }
}
