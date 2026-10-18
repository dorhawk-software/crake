//! Signing in to an OpenVPN gateway, as openvpn's management interface
//! asks for it: what craked answers by itself (the gateway's token from
//! last time, the profile's own credentials), what it asks the user, and
//! the commands it sends. No I/O here: the runner (`super`) does what it
//! says.

use crate::openvpn::mgmt::{self, Challenge, Crv1, Said};
use crate::openvpn::{AskKind, Token};

/// What the runner does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Do {
    /// A command for openvpn.
    Send(String),
    /// A question for the user: kind, the gateway's words, a page to
    /// open, and whether the last answer was refused.
    Ask(AskKind, String, Option<String>, bool),
    /// Keep the gateway's sign-in for next time.
    Keep(Token),
    /// The kept sign-in was refused: forget it.
    Forget,
    /// Signed in: nothing more to ask.
    Done,
    /// openvpn is stopping because of this.
    Fatal(String),
}

/// What was sent last for `Auth`, to know what a refusal means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sent {
    Nothing,
    Token,
    Credentials,
    User,
}

/// What the user's next answer is for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Collecting {
    Auth {
        challenge: Option<Challenge>,
        user: Option<String>,
        password: Option<String>,
    },
    KeyPassword,
    CrText,
    Crv1(Crv1),
    Browser,
}

pub struct SignIn {
    token: Option<Token>,
    credentials: Option<(String, Option<String>)>,
    sent: Sent,
    tried_token: bool,
    tried_credentials: bool,
    /// A dynamic challenge to answer at the next request.
    crv1: Option<Crv1>,
    /// The user's last answer was refused.
    refused: bool,
    key_refused: bool,
    collecting: Option<Collecting>,
    /// The user name the last sign-in went with (a token is for it).
    user: String,
}

impl SignIn {
    pub fn new(token: Option<Token>, credentials: Option<(String, Option<String>)>) -> Self {
        Self {
            token,
            credentials,
            sent: Sent::Nothing,
            tried_token: false,
            tried_credentials: false,
            crv1: None,
            refused: false,
            key_refused: false,
            collecting: None,
            user: String::new(),
        }
    }

    pub fn said(&mut self, said: Said) -> Vec<Do> {
        match said {
            Said::Hold => vec![Do::Send("hold release".into())],
            Said::NeedAuth { challenge } => self.need_auth(challenge),
            Said::NeedKeyPassword => {
                self.collecting = Some(Collecting::KeyPassword);
                let retry = std::mem::take(&mut self.key_refused);
                vec![Do::Ask(AskKind::KeyPassword, String::new(), None, retry)]
            }
            Said::Refused { what, challenge } if what == "Auth" => self.refused_auth(challenge),
            Said::Refused { .. } => {
                self.key_refused = true;
                Vec::new()
            }
            Said::Token(token) => {
                let token = Token {
                    user: self.user.clone(),
                    token,
                };
                self.token = Some(token.clone());
                vec![Do::Keep(token)]
            }
            Said::WebAuth(url) if url.starts_with("https://") || url.starts_with("http://") => {
                self.collecting = Some(Collecting::Browser);
                vec![Do::Ask(AskKind::Browser, String::new(), Some(url), false)]
            }
            Said::CrText { echo, text } => {
                self.collecting = Some(Collecting::CrText);
                vec![Do::Ask(AskKind::Code { echo }, text, None, false)]
            }
            // Past signing in: the gateway is sending its settings.
            Said::State { name, .. }
                if matches!(name.as_str(), "GET_CONFIG" | "ASSIGN_IP" | "CONNECTED") =>
            {
                self.collecting = None;
                vec![Do::Done]
            }
            Said::Fatal(why) => vec![Do::Fatal(why)],
            _ => Vec::new(),
        }
    }

    fn need_auth(&mut self, challenge: Option<Challenge>) -> Vec<Do> {
        if let Some(c) = self.crv1.take() {
            let ask = Do::Ask(AskKind::Code { echo: c.echo }, c.text.clone(), None, false);
            self.collecting = Some(Collecting::Crv1(c));
            return vec![ask];
        }
        if let Some(t) = self.token.clone().filter(|_| !self.tried_token) {
            self.tried_token = true;
            return self.send_auth(Sent::Token, &t.user, &t.token);
        }
        // The profile's own, once; refused, the user is asked for both.
        let (user, password) = match &self.credentials {
            Some((u, p)) if !self.tried_credentials => (Some(u.clone()), p.clone()),
            _ => (None, None),
        };
        self.tried_credentials = true;
        if let (Some(u), Some(p), None) = (&user, &password, &challenge) {
            let (u, p) = (u.clone(), p.clone());
            return self.send_auth(Sent::Credentials, &u, &p);
        }
        self.collecting = Some(Collecting::Auth {
            challenge,
            user: user.filter(|u| !u.is_empty()),
            password,
        });
        self.next_auth()
    }

    fn refused_auth(&mut self, challenge: Option<Crv1>) -> Vec<Do> {
        if challenge.is_some() {
            self.crv1 = challenge;
            return Vec::new();
        }
        match self.sent {
            Sent::Token => {
                self.token = None;
                vec![Do::Forget]
            }
            // The profile's placeholder wasn't it: the user is asked.
            Sent::Credentials => Vec::new(),
            Sent::User | Sent::Nothing => {
                self.refused = true;
                Vec::new()
            }
        }
    }

    /// The next thing to ask for `Auth`, or the answer once complete.
    fn next_auth(&mut self) -> Vec<Do> {
        let Some(Collecting::Auth {
            challenge,
            user,
            password,
        }) = self.collecting.clone()
        else {
            return Vec::new();
        };
        let retry = std::mem::take(&mut self.refused);
        match (user, password, challenge) {
            (None, _, _) => vec![Do::Ask(AskKind::User, String::new(), None, retry)],
            (Some(_), None, _) => vec![Do::Ask(AskKind::Password, String::new(), None, retry)],
            (Some(_), Some(_), Some(c)) => {
                vec![Do::Ask(AskKind::Code { echo: c.echo }, c.text, None, retry)]
            }
            (Some(u), Some(p), None) => {
                self.collecting = None;
                self.send_auth(Sent::User, &u, &p)
            }
        }
    }

    fn send_auth(&mut self, sent: Sent, user: &str, password: &str) -> Vec<Do> {
        self.sent = sent;
        self.user = user.to_owned();
        vec![
            Do::Send(mgmt::username("Auth", user)),
            Do::Send(mgmt::password_cmd("Auth", password)),
        ]
    }

    /// The user answered the last question.
    pub fn answered(&mut self, text: String) -> Vec<Do> {
        match self.collecting.take() {
            Some(Collecting::Auth {
                challenge,
                user,
                password,
            }) => match (user, password, challenge) {
                (None, password, challenge) => {
                    self.collecting = Some(Collecting::Auth {
                        challenge,
                        user: Some(text),
                        password,
                    });
                    self.next_auth()
                }
                (Some(user), None, challenge) => {
                    self.collecting = Some(Collecting::Auth {
                        challenge,
                        user: Some(user),
                        password: Some(text),
                    });
                    self.next_auth()
                }
                (Some(user), Some(password), Some(c)) => {
                    let p = mgmt::with_response(&c, &password, &text);
                    self.send_auth(Sent::User, &user, &p)
                }
                (Some(user), Some(password), None) => self.send_auth(Sent::User, &user, &password),
            },
            Some(Collecting::KeyPassword) => {
                vec![Do::Send(mgmt::password_cmd("Private Key", &text))]
            }
            Some(Collecting::CrText) => vec![Do::Send(mgmt::cr_response(&text))],
            Some(Collecting::Crv1(c)) => {
                let p = mgmt::crv1_password(&c, &text);
                self.send_auth(Sent::User, &c.user.clone(), &p)
            }
            Some(Collecting::Browser) => {
                // Nothing to answer: the page signs in.
                self.collecting = Some(Collecting::Browser);
                Vec::new()
            }
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openvpn::mgmt::parse;

    fn said(s: &mut SignIn, line: &str) -> Vec<Do> {
        s.said(parse(line))
    }

    const NEED_AUTH: &str = ">PASSWORD:Need 'Auth' username/password";

    /// The user's real gateway: placeholder credentials, then the page,
    /// then its token.
    #[test]
    fn a_browser_sign_in_with_the_profiles_placeholder() {
        let creds = Some(("sso-placeholder".into(), Some("sso-placeholder".into())));
        let mut s = SignIn::new(None, creds);
        assert_eq!(
            said(&mut s, ">HOLD:Waiting for hold release:0"),
            [Do::Send("hold release".into())]
        );
        assert_eq!(
            said(&mut s, NEED_AUTH),
            [
                Do::Send(r#"username "Auth" "sso-placeholder""#.into()),
                Do::Send(r#"password "Auth" "sso-placeholder""#.into())
            ]
        );
        assert_eq!(
            said(
                &mut s,
                ">INFOMSG:WEB_AUTH::https://vpn.initech.example/oauth2/start?state=x"
            ),
            [Do::Ask(
                AskKind::Browser,
                String::new(),
                Some("https://vpn.initech.example/oauth2/start?state=x".into()),
                false
            )]
        );
        assert_eq!(said(&mut s, ">STATE:1,GET_CONFIG,,,,,,"), [Do::Done]);
        assert_eq!(
            said(&mut s, ">PASSWORD:Auth-Token:SESS_ID_AT_1"),
            [Do::Keep(Token {
                user: "sso-placeholder".into(),
                token: "SESS_ID_AT_1".into()
            })]
        );
    }

    #[test]
    fn a_kept_token_goes_first_and_is_forgotten_when_refused() {
        let token = Token {
            user: "alice".into(),
            token: "SESS_ID_AT_old".into(),
        };
        let mut s = SignIn::new(Some(token), None);
        assert_eq!(
            said(&mut s, NEED_AUTH)[1],
            Do::Send(r#"password "Auth" "SESS_ID_AT_old""#.into())
        );
        assert_eq!(
            said(&mut s, ">PASSWORD:Verification Failed: 'Auth'"),
            [Do::Forget]
        );
        // Then the user, and their password; not marked refused.
        assert_eq!(
            said(&mut s, NEED_AUTH),
            [Do::Ask(AskKind::User, String::new(), None, false)]
        );
        assert_eq!(
            s.answered("alice".into()),
            [Do::Ask(AskKind::Password, String::new(), None, false)]
        );
        assert_eq!(
            s.answered("hunter2".into()),
            [
                Do::Send(r#"username "Auth" "alice""#.into()),
                Do::Send(r#"password "Auth" "hunter2""#.into())
            ]
        );
        // A wrong password: asked again, saying so.
        said(&mut s, ">PASSWORD:Verification Failed: 'Auth'");
        assert_eq!(
            said(&mut s, NEED_AUTH),
            [Do::Ask(AskKind::User, String::new(), None, true)]
        );
    }

    #[test]
    fn challenges() {
        // Static: the code after the password, as SCRV1.
        let mut s = SignIn::new(None, Some(("bob".into(), None)));
        assert_eq!(
            said(
                &mut s,
                ">PASSWORD:Need 'Auth' username/password SC:0,Your PIN"
            ),
            [Do::Ask(AskKind::Password, String::new(), None, false)]
        );
        assert_eq!(
            s.answered("pw".into()),
            [Do::Ask(
                AskKind::Code { echo: false },
                "Your PIN".into(),
                None,
                false
            )]
        );
        assert_eq!(
            s.answered("123".into())[1],
            Do::Send(r#"password "Auth" "SCRV1:cHc=:MTIz""#.into())
        );
        // Dynamic: refused with a challenge, answered at the next request.
        assert!(
            said(
                &mut s,
                ">PASSWORD:Verification Failed: 'Auth' ['CRV1:R:st8:Ym9i:Code we sent']"
            )
            .is_empty()
        );
        assert_eq!(
            said(&mut s, NEED_AUTH),
            [Do::Ask(
                AskKind::Code { echo: false },
                "Code we sent".into(),
                None,
                false
            )]
        );
        assert_eq!(
            s.answered("999".into()),
            [
                Do::Send(r#"username "Auth" "bob""#.into()),
                Do::Send(r#"password "Auth" "CRV1::st8::999""#.into())
            ]
        );
        // CR_TEXT: answered with cr-response.
        said(&mut s, ">INFOMSG:CR_TEXT:R,E:Code?");
        assert_eq!(
            s.answered("42".into()),
            [Do::Send("cr-response NDI=".into())]
        );
    }

    #[test]
    fn a_private_keys_password() {
        let mut s = SignIn::new(None, None);
        assert_eq!(
            said(&mut s, ">PASSWORD:Need 'Private Key' password"),
            [Do::Ask(AskKind::KeyPassword, String::new(), None, false)]
        );
        assert_eq!(
            s.answered("k".into()),
            [Do::Send(r#"password "Private Key" "k""#.into())]
        );
        said(&mut s, ">PASSWORD:Verification Failed: 'Private Key'");
        assert_eq!(
            said(&mut s, ">PASSWORD:Need 'Private Key' password"),
            [Do::Ask(AskKind::KeyPassword, String::new(), None, true)]
        );
    }
}
