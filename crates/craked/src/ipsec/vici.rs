//! What craked tells its charon over vici (strongSwan's vici README): a
//! tunnel's connection and secrets loaded under its name, signed in
//! (`initiate`, charon's log streamed back for the reason of a failure),
//! looked at (`list-sas`) and ended (`terminate`, then unloaded).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, bail};
use futures_util::{TryStreamExt, pin_mut};
use serde::{Deserialize, Serialize};

use super::Secret;
use super::credential::Credential;

/// How long sign-in may take, in milliseconds, before charon gives up
/// waiting (the gateway, then the user's EAP, then the tunnel).
const INITIATE_MS: u32 = 30_000;
/// How long ending it may take, in milliseconds.
const TERMINATE_MS: u32 = 5_000;
/// What charon's log says when the gateway turned the sign-in down: its
/// EAP method failing ("EAP-MS-CHAPv2 failed with error
/// ERROR_AUTHENTICATION_FAILURE", "EAP_MSCHAPV2 method failed"), the
/// gateway's notify, or an EAP failure it sent.
const REFUSED: [&str; 3] = [
    "AUTHENTICATION_FAILURE",
    "method failed",
    "received EAP_FAILURE",
];
/// The gateway's refusal before any EAP (its notify in IKE_AUTH's first
/// reply): with the user's certificate, the certificate.
const REFUSED_AT_FIRST: &str = "AUTHENTICATION_FAILED";
/// What charon's log says when the gateway's key doesn't match: the
/// gateway's proof checked with the key Crake has, and it failed ("tried 1
/// shared key for 'a' - 'b', but MAC mismatched").
const KEY_MISMATCH: &str = "MAC mismatched";
/// What charon's log says when the gateway's certificate isn't one it can
/// trust for the gateway's identity: signed by no authority it has, or for
/// another name ("no trusted RSA public key found for 'vpn.acme.example'").
const UNTRUSTED: (&str, &str) = ("no trusted ", " public key found for");
/// Lines of charon's log worth giving as a failure's reason.
const TELLING: [&str; 4] = ["failed", "error", "unable", "giving up"];

pub struct Vici(rsvici::Client);

/// What one tunnel is, for charon.
pub struct Tunnel<'a> {
    pub name: &'a str,
    pub gateway: &'a str,
    pub secret: &'a Secret,
    /// The user's certificate, when it signs in with one; then a password
    /// too when `user` isn't empty (ipsec-certificates.html, 2).
    pub credential: Option<&'a Credential>,
    pub user: &'a str,
    pub if_id: u32,
    /// strongSwan's updown: the hook, told which tunnel and where craked is.
    pub updown: String,
}

#[derive(Serialize)]
struct Conn<'a> {
    version: u8,
    remote_addrs: Vec<&'a str>,
    vips: Vec<&'a str>,
    if_id_in: String,
    if_id_out: String,
    /// Charon notices a gateway that's gone.
    dpd_delay: &'a str,
    /// One round ("local"), or two ("local-1", "local-2": the certificate,
    /// then EAP; RFC 4739).
    #[serde(flatten)]
    locals: BTreeMap<&'a str, Local<'a>>,
    remote: Remote<'a>,
    children: BTreeMap<&'a str, Child<'a>>,
}

#[derive(Serialize)]
struct Local<'a> {
    auth: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    eap_id: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    certs: Vec<&'a str>,
}

#[derive(Serialize)]
struct Key<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    data: &'a str,
}

#[derive(Serialize)]
struct Remote<'a> {
    auth: &'a str,
    id: &'a str,
    /// IT's authority, the only one trusted for this gateway; the
    /// system's (loaded with charon, `load_authority`) when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cacerts: Vec<&'a str>,
}

#[derive(Serialize)]
struct Cert<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    flag: &'a str,
    data: &'a str,
}

#[derive(Serialize)]
struct Child<'a> {
    /// Everything: nothing goes in until Crake's routing sends it there.
    remote_ts: Vec<&'a str>,
    if_id_in: String,
    if_id_out: String,
    updown: &'a str,
}

#[derive(Serialize)]
struct Shared<'a> {
    id: String,
    #[serde(rename = "type")]
    kind: &'a str,
    data: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    owners: Vec<&'a str>,
}

#[derive(Deserialize)]
struct Answer {
    success: bool,
    #[serde(default)]
    errmsg: Option<String>,
}

#[derive(Serialize)]
struct Initiate<'a> {
    child: &'a str,
    ike: &'a str,
    timeout: u32,
    #[serde(rename = "init-limits")]
    init_limits: bool,
}

#[derive(Serialize)]
struct Terminate<'a> {
    ike: &'a str,
    timeout: u32,
}

#[derive(Serialize)]
struct Nothing {}

#[derive(Serialize)]
struct Named<'a> {
    name: &'a str,
}

#[derive(Serialize)]
struct Id {
    id: String,
}

#[derive(Serialize)]
struct ListSas<'a> {
    ike: &'a str,
    noblock: bool,
}

#[derive(Deserialize)]
struct Log {
    #[serde(default)]
    msg: String,
}

#[derive(Deserialize)]
struct IkeSa {
    #[serde(default)]
    state: String,
}

/// How a sign-in ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Signed {
    In,
    /// The gateway turned the user's password (or name) down.
    Refused,
    /// The gateway refused before any password was asked: the user's
    /// certificate, or a name it isn't (a password tunnel's refusal too).
    RefusedAtFirst,
    /// The gateway's key doesn't match it (only IT can help).
    KeyRefused,
    /// The gateway's certificate isn't one to trust for its identity.
    Untrusted,
    /// Anything else, in charon's words.
    Failed(String),
}

impl Vici {
    pub async fn connect(socket: &Path) -> Result<Self> {
        Ok(Self(rsvici::unix::connect(socket).await?))
    }

    async fn ask<T: Serialize>(&mut self, cmd: &str, msg: T) -> Result<()> {
        let a: Answer = self.0.request(cmd, msg).await?;
        if a.success {
            Ok(())
        } else {
            bail!("{cmd}: {}", a.errmsg.unwrap_or_default())
        }
    }

    /// The tunnel's connection, its gateway's key (or what checks its
    /// certificate) and the user's password.
    pub async fn load(&mut self, t: &Tunnel<'_>, password: &str) -> Result<()> {
        self.load_as(t, t.name, password).await
    }

    /// The connection a look at the gateway's certificate makes
    /// (`look_name`): the tunnel's, naming no gateway, since a gateway
    /// asked for another name refuses before it shows its certificate
    /// (the spike, scripts/spikes/ipsec-certificate). It's never trusted
    /// that way: it fails at the certificate, before the user's password.
    pub async fn look(&mut self, t: &Tunnel<'_>, password: &str) -> Signed {
        let name = look_name(t.name);
        if let Err(e) = self.load_as(t, &name, password).await {
            return Signed::Failed(format!("{e:#}"));
        }
        let signed = self.initiate(&name).await;
        self.end(&name).await;
        signed
    }

    async fn load_as(&mut self, t: &Tunnel<'_>, name: &str, password: &str) -> Result<()> {
        let key = t.secret.key.as_deref();
        let look = name != t.name;
        // A certificate is checked for a name: the identity IT gave, else
        // the gateway as typed (strongSwan's "%any" would take any trusted
        // certificate for any name), but by a look.
        let gateway_id = match (&t.secret.gateway_id, key) {
            _ if look => "%any",
            (Some(id), _) => id.as_str(),
            (None, Some(_)) => "%any",
            (None, None) => t.gateway,
        };
        let if_id = t.if_id.to_string();
        let child = Child {
            remote_ts: vec!["0.0.0.0/0", "::/0"],
            if_id_in: if_id.clone(),
            if_id_out: if_id.clone(),
            updown: &t.updown,
        };
        let conn = Conn {
            version: 2,
            remote_addrs: vec![t.gateway],
            vips: vec!["0.0.0.0", "::"],
            if_id_in: if_id.clone(),
            if_id_out: if_id,
            dpd_delay: "30s",
            locals: rounds(t),
            remote: Remote {
                auth: if key.is_some() { "psk" } else { "pubkey" },
                id: gateway_id,
                cacerts: t.secret.ca.as_deref().into_iter().collect(),
            },
            children: BTreeMap::from([(name, child)]),
        };
        self.ask("load-conn", BTreeMap::from([(name, conn)]))
            .await?;
        if look {
            return Ok(());
        }
        if let Some(key) = key {
            let owners = t.secret.gateway_id.as_deref().into_iter().collect();
            self.ask(
                "load-shared",
                Shared {
                    id: shared_id(t.name, "ike"),
                    kind: "IKE",
                    data: key,
                    owners,
                },
            )
            .await?;
        }
        if let Some(c) = t.credential {
            self.ask(
                "load-key",
                Key {
                    kind: "any",
                    data: &c.key_pem,
                },
            )
            .await?;
        }
        if t.user.is_empty() {
            return Ok(());
        }
        self.ask(
            "load-shared",
            Shared {
                id: shared_id(t.name, "eap"),
                kind: "EAP",
                data: password,
                owners: vec![t.user],
            },
        )
        .await
    }

    /// One of the system's certificate authorities (PEM), trusted for any
    /// gateway that proves itself with a certificate and names no
    /// authority of IT's.
    pub async fn load_authority(&mut self, pem: &str) -> Result<()> {
        self.ask(
            "load-cert",
            Cert {
                kind: "X509",
                flag: "CA",
                data: pem,
            },
        )
        .await
    }

    /// A gateway's certificate the user trusted (PEM): charon trusts it as
    /// it is, by its key, for the name the connection asks for.
    pub async fn load_trusted(&mut self, pem: &str) -> Result<()> {
        self.ask(
            "load-cert",
            Cert {
                kind: "X509",
                flag: "NONE",
                data: pem,
            },
        )
        .await
    }

    /// charon reads its settings again (its dump on or off, `Charon::dump`).
    pub async fn reload_settings(&mut self) -> Result<()> {
        self.ask("reload-settings", Nothing {}).await
    }

    /// Sign in and bring the tunnel up.
    pub async fn initiate(&mut self, name: &str) -> Signed {
        let msg = Initiate {
            child: name,
            ike: name,
            timeout: INITIATE_MS,
            init_limits: false,
        };
        let logs = self
            .0
            .stream_request::<_, Log>("initiate", "control-log", msg);
        pin_mut!(logs);
        let mut said = Vec::new();
        loop {
            match logs.try_next().await {
                Ok(Some(log)) => {
                    tracing::debug!(target: "charon", "{}", log.msg);
                    said.push(log.msg);
                }
                Ok(None) => return Signed::In,
                Err(e) => {
                    if said.iter().any(|l| l.contains(KEY_MISMATCH)) {
                        return Signed::KeyRefused;
                    }
                    if said
                        .iter()
                        .any(|l| l.contains(UNTRUSTED.0) && l.contains(UNTRUSTED.1))
                    {
                        return Signed::Untrusted;
                    }
                    if said.iter().any(|l| REFUSED.iter().any(|r| l.contains(r))) {
                        return Signed::Refused;
                    }
                    if said.iter().any(|l| l.contains(REFUSED_AT_FIRST)) {
                        return Signed::RefusedAtFirst;
                    }
                    return Signed::Failed(reason(&said).unwrap_or_else(|| e.to_string()));
                }
            }
        }
    }

    /// The tunnel's IKE SA is established.
    pub async fn up(&mut self, name: &str) -> Result<bool> {
        let msg = ListSas {
            ike: name,
            noblock: true,
        };
        let sas = self
            .0
            .stream_request::<_, BTreeMap<String, IkeSa>>("list-sas", "list-sa", msg);
        pin_mut!(sas);
        while let Some(sa) = sas.try_next().await? {
            if sa
                .get(name)
                .is_some_and(|s| s.state == "ESTABLISHED" || s.state == "REKEYING")
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// End the tunnel (logging off at the gateway) and forget it.
    pub async fn end(&mut self, name: &str) {
        let msg = Terminate {
            ike: name,
            timeout: TERMINATE_MS,
        };
        let logs = self
            .0
            .stream_request::<_, Log>("terminate", "control-log", msg);
        pin_mut!(logs);
        while let Ok(Some(_)) = logs.try_next().await {}
        self.forget(name).await;
    }

    /// Unload what was loaded for it; fine if nothing was.
    pub async fn forget(&mut self, name: &str) {
        let _ = self.ask("unload-conn", Named { name }).await;
        for which in ["ike", "eap"] {
            let _ = self
                .ask(
                    "unload-shared",
                    Id {
                        id: shared_id(name, which),
                    },
                )
                .await;
        }
    }
}

/// How the user signs in, as charon's rounds: the password over EAP, the
/// certificate, or the certificate and then the password.
fn rounds<'a>(t: &Tunnel<'a>) -> BTreeMap<&'static str, Local<'a>> {
    let eap = Local {
        auth: "eap",
        eap_id: t.user,
        certs: Vec::new(),
    };
    let Some(c) = t.credential else {
        return BTreeMap::from([("local", eap)]);
    };
    let pubkey = Local {
        auth: "pubkey",
        eap_id: "",
        certs: vec![c.cert_pem.as_str()],
    };
    if t.user.is_empty() {
        BTreeMap::from([("local", pubkey)])
    } else {
        BTreeMap::from([("local-1", pubkey), ("local-2", eap)])
    }
}

/// The connection a look at `tunnel`'s gateway makes (a tunnel's name is
/// letters, digits and dashes: never this).
pub fn look_name(tunnel: &str) -> String {
    format!("look:{tunnel}")
}

/// The line of charon's log that says most about a failure: the last that
/// says something failed, else the last.
fn reason(said: &[String]) -> Option<String> {
    said.iter()
        .rev()
        .find(|l| {
            let l = l.to_lowercase();
            TELLING.iter().any(|t| l.contains(t))
        })
        .or_else(|| said.last())
        .cloned()
}

fn shared_id(name: &str, which: &str) -> String {
    format!("crake-{name}-{which}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn strongswans_words_for_a_refused_password() {
        let said = lines(&[
            "server requested EAP_MSCHAPV2 authentication (id 0xA9)",
            "EAP-MS-CHAPv2 failed with error ERROR_AUTHENTICATION_FAILURE: '(null)'",
            "EAP_MSCHAPV2 method failed",
            "sending packet: from 198.51.100.20[37591] to 198.51.100.10[4500] (80 bytes)",
        ]);
        assert!(said.iter().any(|l| REFUSED.iter().any(|r| l.contains(r))));
        // Refused before any EAP: not the password.
        let at_first = lines(&["received AUTHENTICATION_FAILED notify error"]);
        assert!(
            !at_first
                .iter()
                .any(|l| REFUSED.iter().any(|r| l.contains(r)))
        );
        assert!(at_first.iter().any(|l| l.contains(REFUSED_AT_FIRST)));
    }

    #[test]
    fn a_certificate_and_a_password_are_two_rounds() {
        let secret = Secret::from_fields("k", None).unwrap();
        let c = crate::ipsec::credential::Credential {
            cert_pem: "CERT".into(),
            key_pem: "KEY".into(),
            cert: crate::ipsec::certificate::parse(include_bytes!("testdata/alice.der")).unwrap(),
        };
        let mut t = Tunnel {
            name: "acme",
            gateway: "gw",
            secret: &secret,
            credential: None,
            user: "alice",
            if_id: 1,
            updown: String::new(),
        };
        let r = rounds(&t);
        assert_eq!(r.keys().copied().collect::<Vec<_>>(), ["local"]);
        assert_eq!((r["local"].auth, r["local"].eap_id), ("eap", "alice"));
        t.credential = Some(&c);
        let r = rounds(&t);
        assert_eq!(
            r.keys().copied().collect::<Vec<_>>(),
            ["local-1", "local-2"]
        );
        assert_eq!(
            (r["local-1"].auth, r["local-1"].certs.as_slice()),
            ("pubkey", ["CERT"].as_slice())
        );
        assert_eq!(r["local-2"].auth, "eap");
        t.user = "";
        let r = rounds(&t);
        assert_eq!(r.keys().copied().collect::<Vec<_>>(), ["local"]);
        assert_eq!(r["local"].auth, "pubkey");
        // The key goes to charon by load-key, never in the connection.
        let conn = serde_json::to_string(&r["local"].certs).unwrap();
        assert!(!conn.contains("KEY"));
    }

    #[test]
    fn strongswans_words_for_a_key_that_doesnt_match() {
        let said = lines(&[
            "tried 1 shared key for '198.51.100.20' - 'vpn.acme.example', but MAC mismatched",
            "generating INFORMATIONAL request 2 [ N(AUTH_FAILED) ]",
        ]);
        assert!(said.iter().any(|l| l.contains(KEY_MISMATCH)));
    }

    #[test]
    fn strongswans_words_for_a_certificate_it_cant_trust() {
        let said = lines(&[
            "received end entity cert \"CN=vpn.acme.example\"",
            "no trusted RSA public key found for 'vpn.acme.example'",
        ]);
        assert!(
            said.iter()
                .any(|l| l.contains(UNTRUSTED.0) && l.contains(UNTRUSTED.1))
        );
    }

    #[test]
    fn a_failure_is_told_by_its_telling_line() {
        let said = lines(&[
            "giving up after 5 retransmits",
            "sending packet: from a to b",
        ]);
        assert_eq!(reason(&said).unwrap(), "giving up after 5 retransmits");
        let said = lines(&["one", "two"]);
        assert_eq!(reason(&said).unwrap(), "two");
        assert_eq!(reason(&[]), None);
    }
}
