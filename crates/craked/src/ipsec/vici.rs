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

/// How long sign-in may take, in milliseconds, before charon gives up
/// waiting (the gateway, then the user's EAP, then the tunnel).
const INITIATE_MS: u32 = 30_000;
/// How long ending it may take, in milliseconds.
const TERMINATE_MS: u32 = 5_000;
/// What charon's log says when the gateway turned the sign-in down: its
/// EAP method failing ("EAP-MS-CHAPv2 failed with error
/// ERROR_AUTHENTICATION_FAILURE", "EAP_MSCHAPV2 method failed"), the
/// gateway's notify, or an EAP failure it sent.
const REFUSED: [&str; 4] = [
    "AUTHENTICATION_FAILURE",
    "AUTHENTICATION_FAILED",
    "method failed",
    "received EAP_FAILURE",
];
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
    local: Local<'a>,
    remote: Remote<'a>,
    children: BTreeMap<&'a str, Child<'a>>,
}

#[derive(Serialize)]
struct Local<'a> {
    auth: &'a str,
    eap_id: &'a str,
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
        let key = t.secret.key.as_deref();
        // A certificate is checked for a name: the identity IT gave, else
        // the gateway as typed (strongSwan's "%any" would take any trusted
        // certificate for any name).
        let gateway_id = match (&t.secret.gateway_id, key) {
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
            local: Local {
                auth: "eap",
                eap_id: t.user,
            },
            remote: Remote {
                auth: if key.is_some() { "psk" } else { "pubkey" },
                id: gateway_id,
                cacerts: t.secret.ca.as_deref().into_iter().collect(),
            },
            children: BTreeMap::from([(t.name, child)]),
        };
        self.ask("load-conn", BTreeMap::from([(t.name, conn)]))
            .await?;
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
