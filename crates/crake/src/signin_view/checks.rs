//! Sign-in that couldn't reach the gateway: why, checked one step at a
//! time (design/screens/sign-in.html, "When it can't connect"): the
//! internet, the gateway's name, its port.

use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use super::*;

/// How long a lookup, or a connection, may take (as craked's report).
const CHECK_WAIT: Duration = Duration::from_secs(5);
/// The port a gateway's address implies when it names none (HTTPS).
const HTTPS: u16 = 443;
/// Seconds of silence worth saying ("didn't answer for 20 seconds").
const SAID_FROM: u64 = 2;

/// What a port did when tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Port {
    Answers,
    Refused,
    Silent,
}

/// The checks so far; `None` is still being tried.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Checks {
    /// Which run this is: a run that's been retried since is ignored.
    run: u64,
    /// How long sign-in waited before it gave up (whole seconds).
    pub(super) waited: u64,
    pub(super) internet: Option<bool>,
    /// The gateway's address, or `Some(None)` when its name isn't known.
    pub(super) name: Option<Option<IpAddr>>,
    pub(super) port: Option<Port>,
}

/// The gateway's host and port, as openconnect would take them:
/// "https://vpn.acme.example:10443/group" is ("vpn.acme.example", 10443).
pub(super) fn host_port(gateway: &str) -> (String, u16) {
    let rest = gateway.split_once("://").map_or(gateway, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if let Some(v6) = authority.strip_prefix('[') {
        let (host, after) = v6.split_once(']').unwrap_or((v6, ""));
        let port = after.strip_prefix(':').and_then(|p| p.parse().ok());
        return (host.to_owned(), port.unwrap_or(HTTPS));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            (host.to_owned(), port.parse().unwrap_or(HTTPS))
        }
        _ => (authority.to_owned(), HTTPS),
    }
}

/// One check's row: done (a plain check), failed (a red cross), or under
/// way (dots); a machine fact in mono after the words.
pub(super) struct CheckRow {
    pub(super) mark: Mark,
    pub(super) words: String,
    pub(super) fact: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mark {
    Done,
    Failed,
    UnderWay,
}

impl Checks {
    /// The rows to show, up to the first that failed or is still going.
    pub(super) fn rows(&self, gateway: &str) -> Vec<CheckRow> {
        let (host, port) = host_port(gateway);
        let row = |mark, words: &str, fact: Option<String>| CheckRow {
            mark,
            words: words.to_owned(),
            fact,
        };
        let mut rows = Vec::new();
        match self.internet {
            None => return vec![row(Mark::UnderWay, "Checking the internet", None)],
            Some(false) => return vec![row(Mark::Failed, "No internet", None)],
            Some(true) => rows.push(row(Mark::Done, "Internet is up", None)),
        }
        // An address given as the gateway needs no name looked up.
        if host.parse::<IpAddr>().is_err() {
            match self.name {
                None => {
                    rows.push(row(Mark::UnderWay, "Looking up", Some(host)));
                    return rows;
                }
                Some(None) => {
                    rows.push(row(Mark::Failed, "Name not found:", Some(host)));
                    return rows;
                }
                Some(Some(addr)) => {
                    rows.push(row(Mark::Done, "Name found:", Some(addr.to_string())));
                }
            }
        }
        rows.push(match self.port {
            None => row(Mark::UnderWay, &format!("Trying port {port}"), None),
            Some(Port::Answers) => row(Mark::Done, &format!("Port {port} answers"), None),
            Some(Port::Refused) => row(Mark::Failed, &format!("Refused on port {port}"), None),
            Some(Port::Silent) => row(Mark::Failed, &format!("No answer on port {port}"), None),
        });
        rows
    }

    /// The card's sentence for what the checks found.
    pub(super) fn sentence(&self, spec: &Spec) -> String {
        let (host, _) = host_port(&spec.gateway);
        let blocked = "Your internet works, so it\u{2019}s either down or blocked on this network.";
        match (self.internet, self.name, self.port) {
            (Some(false), ..) => {
                "This computer isn\u{2019}t reaching the internet just now, so the gateway can\u{2019}t be reached either. Try again once it\u{2019}s back.".into()
            }
            (_, Some(None), _) => format!(
                "Your internet works, but {host} isn\u{2019}t a name it knows. Check the gateway\u{2019}s address, or ask {}\u{2019}s IT.",
                spec.display
            ),
            (_, _, Some(Port::Silent)) if self.waited >= SAID_FROM => format!(
                "The gateway didn\u{2019}t answer for {} seconds. {blocked}",
                self.waited
            ),
            (_, _, Some(Port::Silent)) => format!("The gateway didn\u{2019}t answer. {blocked}"),
            (_, _, Some(Port::Refused)) => {
                format!("The gateway turned the connection away. {blocked}")
            }
            (_, _, Some(Port::Answers)) => {
                "The gateway answers now: it may have been a moment\u{2019}s trouble. Try again.".into()
            }
            _ => "The gateway didn\u{2019}t answer. Crake is checking why.".into(),
        }
    }

    /// The checks in plain text, for Copy for helpdesk.
    pub(super) fn as_text(&self, gateway: &str) -> String {
        self.rows(gateway)
            .into_iter()
            .map(|r| {
                let mark = match r.mark {
                    Mark::Done => "ok",
                    Mark::Failed => "failed",
                    Mark::UnderWay => "checking",
                };
                let fact = r.fact.map(|f| format!(" {f}")).unwrap_or_default();
                format!("  {}{fact} ({mark})\n", r.words)
            })
            .collect()
    }
}

/// Whether anything answers at `host` on `port` within CHECK_WAIT: the
/// first address that answers, or what the last one did.
fn try_port(addrs: &[SocketAddr]) -> Port {
    let mut found = Port::Silent;
    for addr in addrs {
        match TcpStream::connect_timeout(addr, CHECK_WAIT) {
            Ok(_) => return Port::Answers,
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => found = Port::Refused,
            Err(_) => {}
        }
    }
    found
}

/// A name's addresses, as any app would look it up.
fn look_up(host: &str, port: u16) -> Vec<SocketAddr> {
    (host, port)
        .to_socket_addrs()
        .map(Iterator::collect)
        .unwrap_or_default()
}

impl SignIn {
    /// Check why the gateway couldn't be reached, step by step, each
    /// step shown as it's done. Only openconnect's gateways: OpenVPN's and
    /// IPsec's are reached by craked.
    pub(super) fn check_why(&mut self, since: Instant, cx: &mut Context<Self>) {
        let run = self.checks.as_ref().map_or(0, |c| c.run) + 1;
        self.checks = Some(Checks {
            run,
            waited: since.elapsed().as_secs(),
            ..Checks::default()
        });
        let (host, port) = host_port(&self.spec.gateway);
        cx.spawn(async move |this, cx| {
            let step = |cx: &mut gpui_kit::AsyncApp, f: Box<dyn FnOnce(&mut Checks)>| {
                this.update(cx, |this, cx| {
                    if let Some(c) = this.checks.as_mut().filter(|c| c.run == run) {
                        f(c);
                        cx.notify();
                    }
                })
                .is_ok()
            };
            let public = cx
                .background_executor()
                .spawn(async move { try_port(&look_up(crake_proto::report::PUBLIC_NAME, HTTPS)) })
                .await;
            let internet = public == Port::Answers;
            if !step(cx, Box::new(move |c| c.internet = Some(internet))) || !internet {
                return;
            }
            let (found, addrs) = cx
                .background_executor()
                .spawn(async move {
                    let addrs = look_up(&host, port);
                    (addrs.first().map(SocketAddr::ip), addrs)
                })
                .await;
            if !step(cx, Box::new(move |c| c.name = Some(found))) || found.is_none() {
                return;
            }
            let answered = cx
                .background_executor()
                .spawn(async move { try_port(&addrs) })
                .await;
            step(cx, Box::new(move |c| c.port = Some(answered)));
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gateways_host_and_port() {
        let hp = |g: &str| host_port(g);
        assert_eq!(hp("vpn.acme.example"), ("vpn.acme.example".into(), 443));
        assert_eq!(
            hp("https://vpn.acme.example:10443/group?x=1"),
            ("vpn.acme.example".into(), 10443)
        );
        assert_eq!(hp("203.0.113.7"), ("203.0.113.7".into(), 443));
        assert_eq!(hp("203.0.113.7:8443"), ("203.0.113.7".into(), 8443));
        assert_eq!(hp("[2001:db8::1]:4443"), ("2001:db8::1".into(), 4443));
        assert_eq!(hp("2001:db8::1"), ("2001:db8::1".into(), 443));
    }

    fn spec() -> Spec {
        Spec {
            tunnel: "nord".into(),
            display: "Nordbank".into(),
            gateway: "gw.nordbank.example".into(),
            protocol: "gp".into(),
            user: String::new(),
            auth_args: Vec::new(),
            extra_args: Vec::new(),
            user_agent: None,
            keyring: false,
            openconnect: None,
            trusted: None,
        }
    }

    fn words(c: &Checks) -> Vec<(Mark, String)> {
        c.rows("gw.nordbank.example")
            .into_iter()
            .map(|r| {
                (
                    r.mark,
                    format!(
                        "{}{}",
                        r.words,
                        r.fact.map(|f| format!(" {f}")).unwrap_or_default()
                    ),
                )
            })
            .collect()
    }

    /// As design/screens/sign-in.html: the internet and the name fine, no
    /// answer on the port.
    #[test]
    fn the_checks_say_where_it_stopped() {
        let addr: IpAddr = "203.0.113.40".parse().unwrap();
        let silent = Checks {
            waited: 20,
            internet: Some(true),
            name: Some(Some(addr)),
            port: Some(Port::Silent),
            ..Checks::default()
        };
        assert_eq!(
            words(&silent),
            [
                (Mark::Done, "Internet is up".into()),
                (Mark::Done, "Name found: 203.0.113.40".into()),
                (Mark::Failed, "No answer on port 443".into()),
            ]
        );
        assert_eq!(
            silent.sentence(&spec()),
            "The gateway didn\u{2019}t answer for 20 seconds. Your internet works, so it\u{2019}s either down or blocked on this network."
        );
        let offline = Checks {
            internet: Some(false),
            ..Checks::default()
        };
        assert_eq!(words(&offline), [(Mark::Failed, "No internet".into())]);
        let unknown = Checks {
            internet: Some(true),
            name: Some(None),
            ..Checks::default()
        };
        assert_eq!(
            words(&unknown)[1],
            (Mark::Failed, "Name not found: gw.nordbank.example".into())
        );
        assert!(
            unknown
                .sentence(&spec())
                .contains("isn\u{2019}t a name it knows")
        );
        let going = Checks {
            internet: Some(true),
            ..Checks::default()
        };
        assert_eq!(words(&going)[1].0, Mark::UnderWay);
        assert_eq!(
            going.as_text("gw.nordbank.example"),
            "  Internet is up (ok)\n  Looking up gw.nordbank.example (checking)\n"
        );
    }
}
