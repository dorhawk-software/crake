//! openvpn's management interface (doc/management-notes.txt): what it says,
//! and the commands craked answers with.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

/// One line from openvpn, as far as craked acts on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// Waiting to be let go (`--management-hold`).
    Hold,
    /// It needs the user name and password; a static challenge asks for a
    /// code with them.
    NeedAuth { challenge: Option<Challenge> },
    /// The private key's password.
    NeedKeyPassword,
    /// Something else it needs (an HTTP proxy's password, ...).
    Need(String),
    /// What was sent was refused: `Auth` (with a dynamic challenge to
    /// answer, if the gateway sent one) or `Private Key`.
    Refused {
        what: String,
        challenge: Option<Crv1>,
    },
    /// The gateway's sign-in for next time.
    Token(String),
    /// Finish signing in on this page.
    WebAuth(String),
    /// The gateway asks for a code (`CR_TEXT`), answered with `cr-response`.
    CrText { echo: bool, text: String },
    /// Its state changed: CONNECTING, AUTH_PENDING, RECONNECTING, EXITING, ...
    State { name: String, detail: String },
    /// It's stopping because of this.
    Fatal(String),
    /// Anything else.
    Other,
}

/// A static challenge (`static-challenge` in the profile).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    pub echo: bool,
    /// The response goes after the password itself, not as SCRV1.
    pub concat: bool,
    pub text: String,
}

/// A dynamic challenge (CRV1): answered by signing in again with the user
/// name it names and `CRV1::<state>::<response>` as the password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crv1 {
    pub echo: bool,
    pub state: String,
    pub user: String,
    pub text: String,
}

/// Static challenge flags: bit 0 shows the answer, bit 1 concatenates it.
const SC_ECHO: u32 = 1;
const SC_CONCAT: u32 = 2;

pub fn parse(line: &str) -> Said {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.starts_with(">HOLD:") {
        return Said::Hold;
    }
    if let Some(rest) = line.strip_prefix(">PASSWORD:") {
        return password(rest);
    }
    if let Some(rest) = line.strip_prefix(">INFOMSG:") {
        if let Some(url) = rest.strip_prefix("WEB_AUTH:") {
            // WEB_AUTH:<flags>:<url>
            return Said::WebAuth(url.split_once(':').map_or(url, |(_, u)| u).to_owned());
        }
        if let Some(url) = rest.strip_prefix("OPEN_URL:") {
            return Said::WebAuth(url.to_owned());
        }
        if let Some(cr) = rest.strip_prefix("CR_TEXT:") {
            let (flags, text) = cr.split_once(':').unwrap_or(("", cr));
            return Said::CrText {
                echo: flags.split(',').any(|f| f == "E"),
                text: text.to_owned(),
            };
        }
        return Said::Other;
    }
    if let Some(rest) = line.strip_prefix(">STATE:") {
        // <time>,<state>,<detail>,...
        let mut parts = rest.split(',');
        let _time = parts.next();
        return Said::State {
            name: parts.next().unwrap_or("").to_owned(),
            detail: parts.next().unwrap_or("").to_owned(),
        };
    }
    if let Some(rest) = line.strip_prefix(">FATAL:") {
        return Said::Fatal(rest.to_owned());
    }
    Said::Other
}

fn password(rest: &str) -> Said {
    if let Some(token) = rest.strip_prefix("Auth-Token:") {
        return Said::Token(token.to_owned());
    }
    if let Some(what) = rest.strip_prefix("Verification Failed: '") {
        let (what, after) = what.split_once('\'').unwrap_or((what, ""));
        let challenge = after
            .split_once("['CRV1:")
            .and_then(|(_, c)| c.strip_suffix("']"))
            .and_then(crv1);
        return Said::Refused {
            what: what.to_owned(),
            challenge,
        };
    }
    if let Some(need) = rest.strip_prefix("Need '") {
        let (what, after) = need.split_once('\'').unwrap_or((need, ""));
        return match what {
            "Auth" => Said::NeedAuth {
                challenge: after
                    .split_once(" SC:")
                    .and_then(|(_, sc)| static_challenge(sc)),
            },
            "Private Key" => Said::NeedKeyPassword,
            other => Said::Need(other.to_owned()),
        };
    }
    Said::Other
}

/// `<flags>,<text>`
fn static_challenge(sc: &str) -> Option<Challenge> {
    let (flags, text) = sc.split_once(',')?;
    let flags: u32 = flags.parse().ok()?;
    Some(Challenge {
        echo: flags & SC_ECHO != 0,
        concat: flags & SC_CONCAT != 0,
        text: text.to_owned(),
    })
}

/// `<flags>:<state>:<user, base64>:<text>`
fn crv1(c: &str) -> Option<Crv1> {
    const PARTS: usize = 4;
    let mut parts = c.splitn(PARTS, ':');
    let flags = parts.next()?;
    let state = parts.next()?.to_owned();
    let user = String::from_utf8(BASE64.decode(parts.next()?).ok()?).ok()?;
    let text = parts.next().unwrap_or("").to_owned();
    Some(Crv1 {
        echo: flags.split(',').any(|f| f == "E"),
        state,
        user,
        text,
    })
}

/// A word for a command line, quoted as the interface wants it.
pub fn quote(s: &str) -> String {
    let mut q = String::from('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            q.push('\\');
        }
        q.push(c);
    }
    q.push('"');
    q
}

pub fn username(what: &str, user: &str) -> String {
    format!("username {} {}", quote(what), quote(user))
}

pub fn password_cmd(what: &str, password: &str) -> String {
    format!("password {} {}", quote(what), quote(password))
}

/// A static challenge's password: the password and the response together.
pub fn with_response(challenge: &Challenge, password: &str, response: &str) -> String {
    if challenge.concat {
        format!("{password}{response}")
    } else {
        format!(
            "SCRV1:{}:{}",
            BASE64.encode(password),
            BASE64.encode(response)
        )
    }
}

/// A dynamic challenge's answer, sent as the password.
pub fn crv1_password(c: &Crv1, response: &str) -> String {
    format!("CRV1::{}::{response}", c.state)
}

/// The answer to a `CR_TEXT` question.
pub fn cr_response(response: &str) -> String {
    format!("cr-response {}", BASE64.encode(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_browser_sign_in() {
        // As the user's real gateway said it (2026-10-18), the state cut.
        assert_eq!(
            parse(">INFOMSG:WEB_AUTH::https://vpn.initech.example/oauth2/start?state=x"),
            Said::WebAuth("https://vpn.initech.example/oauth2/start?state=x".into())
        );
        assert_eq!(
            parse(">INFOMSG:WEB_AUTH:proxy,hidden:https://a.example/b"),
            Said::WebAuth("https://a.example/b".into())
        );
        assert_eq!(
            parse(">STATE:1792296760,AUTH_PENDING,timeout 180,,,,,"),
            Said::State {
                name: "AUTH_PENDING".into(),
                detail: "timeout 180".into()
            }
        );
        assert_eq!(
            parse(">PASSWORD:Auth-Token:SESS_ID_AT_abc"),
            Said::Token("SESS_ID_AT_abc".into())
        );
    }

    #[test]
    fn passwords_and_challenges() {
        assert_eq!(
            parse(">PASSWORD:Need 'Auth' username/password"),
            Said::NeedAuth { challenge: None }
        );
        assert_eq!(
            parse(">PASSWORD:Need 'Auth' username/password SC:1,Enter your PIN"),
            Said::NeedAuth {
                challenge: Some(Challenge {
                    echo: true,
                    concat: false,
                    text: "Enter your PIN".into()
                })
            }
        );
        assert_eq!(
            parse(">PASSWORD:Need 'Private Key' password"),
            Said::NeedKeyPassword
        );
        assert_eq!(
            parse(">PASSWORD:Verification Failed: 'Auth'"),
            Said::Refused {
                what: "Auth".into(),
                challenge: None
            }
        );
        // "alice" in base64.
        let refused = parse(
            ">PASSWORD:Verification Failed: 'Auth' ['CRV1:R,E:Om01u7:YWxpY2U=:Enter the code: we texted you']",
        );
        let Said::Refused {
            challenge: Some(c), ..
        } = refused
        else {
            panic!("{refused:?}");
        };
        assert_eq!(c.user, "alice");
        assert!(c.echo);
        assert_eq!(c.text, "Enter the code: we texted you");
        assert_eq!(crv1_password(&c, "123456"), "CRV1::Om01u7::123456");
        assert_eq!(
            parse(">INFOMSG:CR_TEXT:R,E:Your code?"),
            Said::CrText {
                echo: true,
                text: "Your code?".into()
            }
        );
    }

    #[test]
    fn commands_are_quoted() {
        assert_eq!(
            username("Auth", r#"al"ice\"#),
            r#"username "Auth" "al\"ice\\""#
        );
        let sc = Challenge {
            echo: false,
            concat: false,
            text: String::new(),
        };
        assert_eq!(with_response(&sc, "pw", "123"), "SCRV1:cHc=:MTIz");
        assert_eq!(cr_response("123"), "cr-response MTIz");
    }
}
