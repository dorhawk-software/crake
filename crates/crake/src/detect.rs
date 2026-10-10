//! New tunnel, "Detect automatically": ask the gateway, with openconnect
//! itself and nothing signed in, whether it is an AnyConnect gateway, and
//! which sign-in groups it offers. As AnyConnect, so gateways that only
//! admit Cisco's client answer too (CLAUDE.md, 7d).

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// An AnyConnect (or ocserv) gateway; the groups it offers, if any.
    AnyConnect { groups: Vec<String> },
    /// It answered, but not as AnyConnect: the user picks the protocol.
    Unknown,
    /// Nothing answered.
    Unreachable,
}

/// Ask `gateway` (blocking, at most `wait`).
pub fn gateway(gateway: &str, wait: Duration) -> Found {
    let program = std::env::var_os("CRAKE_OPENCONNECT").unwrap_or_else(|| "openconnect".into());
    let Ok(mut child) = Command::new(program)
        .args([
            "--authenticate",
            "--non-inter",
            "--protocol=anyconnect",
            "--useragent=AnyConnect",
            "--",
            gateway,
        ])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return Found::Unknown;
    };
    let start = Instant::now();
    while start.elapsed() < wait {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    let mut text = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut text);
    }
    read(&text)
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
        return Found::AnyConnect { groups };
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
    fn reads_what_the_gateway_said() {
        // vpn.acme.example as AnyConnect, no certificate given.
        let acme = "POST https://vpn.acme.example/\nConnected to 203.0.113.10:443\n\
                  Server requested SSL client certificate; none was configured\n\
                  POST https://vpn.acme.example/\nXML response has no \"auth\" node\n";
        assert_eq!(read(acme), Found::AnyConnect { groups: vec![] });
        let groups = "Connected to 192.0.2.1:443\nXML POST enabled\nPlease enter your username and password.\n\
                      GROUP: [Employees|Contractors]:\nUser input required in non-interactive mode\n";
        assert_eq!(
            read(groups),
            Found::AnyConnect {
                groups: vec!["Employees".into(), "Contractors".into()]
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
}
