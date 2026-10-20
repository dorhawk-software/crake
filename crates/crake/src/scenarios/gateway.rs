//! The kinds of test gateway: how each signs in, what New tunnel is told
//! of it, what IT's files say, and what each system can reach.

use super::platform;

/// The kinds of gateway every scenario runs against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gateway {
    /// ocserv, signed in with the app's own sign-in (trust, password).
    AnyConnect,
    /// openvpn, its username and password asked through craked.
    OpenVpn,
    /// strongSwan (IKEv2), the gateway's key and the user's password
    /// (Linux: no engine elsewhere yet, and IKE is UDP, which Cloudflare
    /// doesn't carry).
    #[cfg(target_os = "linux")]
    Ipsec,
    /// AnyConnect again, signed in with a certificate (carol's, a PKCS#12
    /// file with a password) instead of a password.
    AnyConnectCertificate,
    /// AnyConnect behind a front that admits only clients calling
    /// themselves AnyConnect (7d; scripts/e2e/picky.py; through Cloudflare
    /// where its hostname is, crake-ci-picky).
    AnyConnectOnly,
    /// AnyConnect where signing in chooses a group (erin's, Engineering;
    /// through Cloudflare where its hostname is, crake-ci-groups).
    AnyConnectGroups,
    /// OpenVPN again, signed in as dave: his password, then a code from his
    /// app, asked as a challenge (CR_TEXT; scripts/e2e/sso.py).
    OpenVpnCode,
    /// A second company on the AnyConnect gateway (its user bob): its own
    /// network, DNS and domain (scripts/e2e/companies.sh). Not a kind the
    /// scenarios run against: the tunnel beside theirs.
    Globex,
}

impl Gateway {
    /// The tunnel's name and how the app shows it.
    pub(super) fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::AnyConnect | Self::AnyConnectCertificate => ("acme", "ACME"),
            Self::AnyConnectOnly | Self::AnyConnectGroups => ("acme", "ACME"),
            Self::OpenVpn | Self::OpenVpnCode => ("initech", "Initech"),
            #[cfg(target_os = "linux")]
            Self::Ipsec => ("nordbank", "Nordbank"),
            Self::Globex => ("globex", "Globex"),
        }
    }

    /// craked keeps its sign-in while it's off (a cookie, a token): IPsec's
    /// password is the app's to keep, in the keyring.
    pub(super) fn keeps_sign_in(self) -> bool {
        match self {
            Self::AnyConnect
            | Self::AnyConnectCertificate
            | Self::OpenVpn
            | Self::OpenVpnCode
            | Self::Globex => true,
            Self::AnyConnectOnly | Self::AnyConnectGroups => true,
            #[cfg(target_os = "linux")]
            Self::Ipsec => false,
        }
    }

    /// Why this system can't reach this kind of gateway, if it can't.
    pub(super) fn missing(self) -> Option<&'static str> {
        match self {
            Self::AnyConnectOnly if platform::anyconnect_only_gateway().is_none() => Some(
                "the AnyConnect-only front isn't reachable here (a Cloudflare route for \
                 crake-ci-picky.dorhawk.software)",
            ),
            Self::AnyConnectGroups if platform::anyconnect_groups_gateway().is_none() => Some(
                "the gateway with groups isn't reachable here (a Cloudflare route for \
                 crake-ci-groups.dorhawk.software)",
            ),
            _ => None,
        }
    }

    /// The user an AnyConnect gateway signs in with the app's own sign-in.
    pub(super) fn anyconnect_user(self) -> Option<&'static str> {
        match self {
            Self::AnyConnect => Some("alice"),
            Self::AnyConnectCertificate => Some(""),
            Self::AnyConnectOnly => Some("alice"),
            Self::AnyConnectGroups => Some("erin"),
            Self::Globex => Some("bob"),
            _ => None,
        }
    }

    /// Another kind, for a second tunnel beside this one.
    pub(super) fn other(self) -> Self {
        match self {
            Self::AnyConnect | Self::AnyConnectCertificate => Self::OpenVpn,
            Self::AnyConnectOnly | Self::AnyConnectGroups => Self::OpenVpn,
            Self::OpenVpn | Self::OpenVpnCode => Self::AnyConnect,
            #[cfg(target_os = "linux")]
            Self::Ipsec => Self::AnyConnect,
            Self::Globex => Self::AnyConnect,
        }
    }

    /// IT's files for it (managed.d), with `learn` as its scope.
    pub(super) fn managed_files(self, learn: &str) -> Vec<(&'static str, String)> {
        match self {
            Self::AnyConnect => vec![(
                "acme.toml",
                format!(
                    "display_name = \"ACME\"\nprotocol = \"anyconnect\"\ngateway = \"{}\"\n\
                     sign_in = \"password\"\nuser = \"alice\"\n{learn}\n",
                    platform::anyconnect_gateway()
                ),
            )],
            Self::OpenVpn => vec![
                ("initech.ovpn", platform::openvpn_profile()),
                (
                    "initech.toml",
                    format!(
                        "display_name = \"Initech\"\nprotocol = \"openvpn\"\n\
                         profile = \"initech.ovpn\"\n{learn}\n"
                    ),
                ),
            ],
            #[cfg(target_os = "linux")]
            Self::Ipsec => panic!("IPsec in IT's files isn't built yet (CLAUDE.md, 7k)"),
            Self::AnyConnectCertificate | Self::OpenVpnCode | Self::Globex => {
                panic!("{self:?} is never IT's in these tests")
            }
            Self::AnyConnectOnly | Self::AnyConnectGroups => {
                panic!("{self:?} is never IT's in these tests")
            }
        }
    }

    /// New tunnel's fields for it.
    pub(super) fn fields(self) -> Vec<(&'static str, String)> {
        let (_, label) = self.names();
        let mut fields = vec![("display_name", label.to_owned())];
        match self {
            Self::AnyConnect => fields.extend([
                ("gateway", platform::anyconnect_gateway()),
                ("protocol", "anyconnect".into()),
                ("sign_in", "password".into()),
                ("user", "alice".into()),
            ]),
            Self::AnyConnectCertificate => fields.extend([
                ("gateway", platform::anyconnect_gateway()),
                ("protocol", "anyconnect".into()),
                ("sign_in", "certificate".into()),
                ("certificate", platform::user_certificate()),
            ]),
            Self::AnyConnectGroups => fields.extend([
                (
                    "gateway",
                    platform::anyconnect_groups_gateway().unwrap_or_default(),
                ),
                ("protocol", "anyconnect".into()),
                ("sign_in", "password".into()),
                ("user", "erin".into()),
            ]),
            Self::AnyConnectOnly => fields.extend([
                (
                    "gateway",
                    platform::anyconnect_only_gateway().unwrap_or_default(),
                ),
                ("protocol", "anyconnect".into()),
                ("sign_in", "password".into()),
                ("user", "alice".into()),
            ]),
            Self::Globex => fields.extend([
                ("gateway", platform::anyconnect_gateway()),
                ("protocol", "anyconnect".into()),
                ("sign_in", "password".into()),
                ("user", "bob".into()),
            ]),
            Self::OpenVpn | Self::OpenVpnCode => fields.extend([
                ("protocol", "openvpn".into()),
                ("profile", platform::openvpn_profile()),
            ]),
            #[cfg(target_os = "linux")]
            Self::Ipsec => fields.extend([
                ("protocol", "ipsec".into()),
                ("gateway", platform::IPSEC_GATEWAY.into()),
                ("gateway_key", "hunter2hunter2".into()),
                ("gateway_id", "vpn.acme.example".into()),
                ("sign_in", "password".into()),
                ("user", "alice".into()),
                ("domains", "acme.internal".into()),
                ("mode", "learn".into()),
            ]),
        }
        fields
    }
}
