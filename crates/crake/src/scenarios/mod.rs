//! The scenario suite: what Crake does with a tunnel that's up, one
//! scenario per test, written once and run on every system against the
//! same gateways (the testing-gaps review, 2026-10-19: one long test hid
//! everything after its first failure, and each system had its own copy).
//!
//! Each test starts from nothing, as a Spring context does: craked fresh
//! (Linux and macOS: in the test, as root, with a folder of its own;
//! Windows: the installed service, its test tunnels removed first), the
//! tunnel added as New tunnel adds it, signed in as the app signs in, and
//! connected. Then one scenario, then everything removed again.
//!
//! The gateways are scripts/e2e's: ACME's internal network (its DNS at
//! 10.40.0.2, git 10.40.1.10, wiki 10.40.1.11, portal.acme.example
//! 10.40.1.14) behind ocserv (AnyConnect), openvpn and, on Linux,
//! strongSwan (IPsec). Linux reaches them in the e2e containers (no
//! network, a socket pair as the wire, the system's own resolved;
//! scripts/e2e-test.sh), macOS and Windows through Cloudflare
//! (scripts/ci-through-cloudflare.sh). What differs per system lives in
//! `platform.rs`: where an address goes, the tunnel's device, how names
//! are asked, what's left behind. A scenario a system can't run says so
//! (`not_here`) instead of passing quietly.
//!
//! Skipped unless CRAKE_E2E=1.

use std::collections::HashMap;
use std::time::Duration;

use crate::client::Command;
use crate::test_app::{App, enabled, sign_in_as};

mod checks;
mod hosts;
mod managed;
mod names;
mod platform;
mod settings;
mod several;
mod sign_in;

pub(crate) use platform::Craked;

/// How long a tunnel, and a host through it, get.
const WAIT: Duration = Duration::from_secs(60);
/// The test tunnels, gone before and after.
const TUNNELS: [&str; 4] = ["acme", "initech", "nordbank", "globex"];

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
    /// A second company on the AnyConnect gateway (its user bob): its own
    /// network, DNS and domain (scripts/e2e/companies.sh). Not a kind the
    /// scenarios run against: the tunnel beside theirs.
    Globex,
}

impl Gateway {
    /// The tunnel's name and how the app shows it.
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::AnyConnect => ("acme", "ACME"),
            Self::OpenVpn => ("initech", "Initech"),
            #[cfg(target_os = "linux")]
            Self::Ipsec => ("nordbank", "Nordbank"),
            Self::Globex => ("globex", "Globex"),
        }
    }

    /// craked keeps its sign-in while it's off (a cookie, a token): IPsec's
    /// password is the app's to keep, in the keyring.
    fn keeps_sign_in(self) -> bool {
        match self {
            Self::AnyConnect | Self::OpenVpn | Self::Globex => true,
            #[cfg(target_os = "linux")]
            Self::Ipsec => false,
        }
    }

    /// The user an AnyConnect gateway signs in with the app's own sign-in.
    fn anyconnect_user(self) -> Option<&'static str> {
        match self {
            Self::AnyConnect => Some("alice"),
            Self::Globex => Some("bob"),
            _ => None,
        }
    }

    /// Another kind, for a second tunnel beside this one.
    fn other(self) -> Self {
        match self {
            Self::AnyConnect => Self::OpenVpn,
            Self::OpenVpn => Self::AnyConnect,
            #[cfg(target_os = "linux")]
            Self::Ipsec => Self::AnyConnect,
            Self::Globex => Self::AnyConnect,
        }
    }

    /// IT's files for it (managed.d), with `learn` as its scope.
    fn managed_files(self, learn: &str) -> Vec<(&'static str, String)> {
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
            Self::Globex => panic!("Globex is never IT's in these tests"),
        }
    }

    /// New tunnel's fields for it.
    fn fields(self) -> Vec<(&'static str, String)> {
        let (_, label) = self.names();
        let mut fields = vec![("display_name", label.to_owned())];
        match self {
            Self::AnyConnect => fields.extend([
                ("gateway", platform::anyconnect_gateway()),
                ("protocol", "anyconnect".into()),
                ("user", "alice".into()),
            ]),
            Self::Globex => fields.extend([
                ("gateway", platform::anyconnect_gateway()),
                ("protocol", "anyconnect".into()),
                ("user", "bob".into()),
            ]),
            Self::OpenVpn => fields.extend([
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

/// A tunnel that's up, and everything around it.
pub(crate) struct Up {
    pub crake: Craked,
    pub app: App,
    pub gateway: Gateway,
    /// The tunnel's name ("acme") and how it's shown ("ACME").
    pub name: &'static str,
    pub label: &'static str,
    /// The device it runs on (crake-acme, utun5).
    pub device: String,
}

impl Up {
    async fn start(gateway: Gateway) -> Self {
        Self::start_from(gateway, None).await
    }

    /// The tunnel from IT's files (`learn` its scope), not added in the app.
    async fn start_managed(gateway: Gateway, learn: &str) -> Self {
        Self::start_from(gateway, Some(learn)).await
    }

    async fn start_from(gateway: Gateway, managed: Option<&str>) -> Self {
        let (name, label) = gateway.names();
        let files = managed
            .map(|l| gateway.managed_files(l))
            .unwrap_or_default();
        let (crake, mut app) = Craked::start_with(&files).await;
        app.within(WAIT, "craked reached", |m| {
            m.reachable == Some(true) && !m.settings.is_empty()
        })
        .await;
        remove_tunnels(&mut app).await;
        // Windows' service keeps its settings from test to test.
        for (key, value) in DEFAULTS {
            set(&mut app, key, value).await;
        }
        let before = platform::answers("10.40.1.10").await;
        assert!(!before, "10.40.1.10 answered without a tunnel");
        if managed.is_some() {
            app.until("IT's tunnel", |m| {
                m.tunnels.iter().any(|t| t.name == name && t.managed)
            })
            .await;
        } else {
            add(&mut app, gateway).await;
        }
        sign_in_and_connect(&mut app, gateway, &["hunter2"]).await;
        let device = crake.device(name);
        Self {
            crake,
            app,
            gateway,
            name,
            label,
            device,
        }
    }

    /// Removed and added again, off and with no sign-in kept.
    pub async fn add_again(&mut self) {
        let name = self.name;
        self.app.send(Command::RemoveTunnel(name.into()));
        self.app
            .until("removed", |m| !m.tunnels.iter().any(|t| t.name == name))
            .await;
        add(&mut self.app, self.gateway).await;
    }

    /// Switched off, as the panel's switch does.
    pub async fn off(&mut self) {
        let name = self.name;
        self.app.send(Command::Disconnect(name.into()));
        self.app
            .within(WAIT, "off", |m| state(m, name) == "off")
            .await;
    }

    async fn stop(mut self) {
        if self.crake.stopped {
            return;
        }
        remove_tunnels(&mut self.app).await;
        self.crake.stop().await;
    }
}

/// Settings the scenarios start from: Settings' defaults where a
/// scenario changes them.
const DEFAULTS: [(&str, &str); 5] = [
    ("never", ""),
    ("ask_unknown", "true"),
    ("keep_private", "true"),
    // Tunnels tried in the order they were added; when two answer, ask.
    ("order", ""),
    ("both", "ask"),
];

/// A setting changed as Settings changes it, once craked has it.
pub(crate) async fn set(app: &mut App, key: &'static str, value: &'static str) {
    app.send(Command::SetSetting(key.into(), value.into()));
    app.until(key, |m| {
        m.settings.get(key).map(String::as_str) == Some(value)
    })
    .await;
}

/// The tunnel added as New tunnel adds it.
pub(crate) async fn add(app: &mut App, gateway: Gateway) {
    let (name, _) = gateway.names();
    let fields: HashMap<String, String> = gateway
        .fields()
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    app.send(Command::AddTunnel(fields));
    app.until("added", |m| m.tunnels.iter().any(|t| t.name == name))
        .await;
}

/// Switched on and signed in, giving `passwords` in turn (a wrong one,
/// then the right one: it's asked again, saying so), until connected.
pub(crate) async fn sign_in_and_connect(app: &mut App, gateway: Gateway, passwords: &[&str]) {
    sign_in(app, gateway, passwords).await;
    let (name, _) = gateway.names();
    app.within(WAIT, "connected", |m| state(m, name) == "connected")
        .await;
}

/// Switched on and signed in, as `sign_in_and_connect`, without waiting
/// for what comes of it.
pub(crate) async fn sign_in(app: &mut App, gateway: Gateway, passwords: &[&str]) {
    let (name, label) = gateway.names();
    if let Some(user) = gateway.anyconnect_user() {
        // ocserv refuses every password after a wrong one in the same
        // sign-in (Cisco's gateways let you try again): the window says it
        // was refused and asks again; cancelled, a new sign-in works.
        let gw = platform::anyconnect_gateway();
        let mut session = None;
        for (n, password) in passwords.iter().enumerate() {
            match sign_in_as(&gw, user, &[password]).await {
                Ok((s, _)) => {
                    session = Some(s);
                    break;
                }
                Err((asked, said)) => {
                    assert!(n + 1 < passwords.len(), "{password} refused: {said:?}");
                    assert_eq!(asked.last(), Some(&"password"), "asked again: {asked:?}");
                    assert_eq!(said.as_deref(), Some("Login failed."), "the refusal said");
                }
            }
        }
        let session = session.expect("signed in");
        app.send(Command::ConnectWithSession(name.into(), session));
    } else {
        app.send(Command::Connect(name.into()));
        for (n, password) in passwords.iter().enumerate() {
            // IPsec's username comes with the tunnel; OpenVPN's profile has
            // none, so it's asked with each password.
            let mut asks = vec![("password", *password)];
            if gateway == Gateway::OpenVpn {
                asks.insert(0, ("user", "alice"));
            }
            for (i, (kind, text)) in asks.into_iter().enumerate() {
                let ask = app.asked(kind).await;
                assert_eq!(ask["kind"], kind, "{label} asked {ask:?}");
                if i == 0 {
                    let again = (n > 0).to_string();
                    assert_eq!(ask["retry"], again, "{label}: refused said, {ask:?}");
                }
                app.send(Command::Answer(
                    name.into(),
                    ask["id"].parse().unwrap(),
                    text.to_owned(),
                ));
            }
        }
    }
}

fn state(m: &crate::model::Model, tunnel: &str) -> String {
    m.tunnels
        .iter()
        .find(|t| t.name == tunnel)
        .map(|t| t.state.clone())
        .unwrap_or_default()
}

/// No test tunnel left (Windows' service keeps them from run to run).
async fn remove_tunnels(app: &mut App) {
    for old in TUNNELS {
        // IT's can't be removed: they go with their files.
        if app
            .model
            .tunnels
            .iter()
            .any(|t| t.name == old && !t.managed)
        {
            app.send(Command::RemoveTunnel(old.into()));
            app.until("an old one gone", |m| {
                !m.tunnels.iter().any(|t| t.name == old)
            })
            .await;
        }
    }
}

/// One scenario against a tunnel that's up through `gateway`.
async fn run(gateway: Gateway, scenario: impl AsyncFnOnce(&mut Up)) {
    if !enabled() {
        return;
    }
    let mut up = Up::start(gateway).await;
    scenario(&mut up).await;
    up.stop().await;
}

/// One scenario against IT's tunnel through `gateway`, its scope `learn`.
async fn run_managed(gateway: Gateway, learn: &str, scenario: impl AsyncFnOnce(&mut Up)) {
    if !enabled() {
        return;
    }
    let mut up = Up::start_managed(gateway, learn).await;
    scenario(&mut up).await;
    up.stop().await;
}

/// A scenario this system can't run, said in the test's output.
#[cfg(not(target_os = "linux"))]
pub(crate) fn not_here(why: &str) {
    println!("not on {}: {why}", std::env::consts::OS);
}

/// Every scenario, as a test of its own, against one kind of gateway.
macro_rules! scenarios {
    ($kind:ident, $gateway:expr, [$($module:ident::$scenario:ident),* $(,)?]) => {
        mod $kind {
            $(
                #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
                async fn $scenario() {
                    super::run($gateway, super::$module::$scenario).await;
                }
            )*
        }
    };
}

macro_rules! every_scenario {
    ($kind:ident, $gateway:expr) => {
        scenarios!(
            $kind,
            $gateway,
            [
                checks::nothing_is_routed_up_front,
                checks::a_host_is_learned_by_probing,
                checks::a_tunnel_name_is_routed_before_its_first_packet,
                checks::the_internet_stays_direct,
                checks::public_names_resolve,
                checks::a_name_public_dns_doesnt_know_is_rescued,
                checks::a_name_asked_over_tcp_is_routed,
                checks::a_report_says_where_things_go,
                checks::switching_off_leaves_nothing,
                checks::names_resolve_after_switching_off,
                checks::names_are_kept_private_while_off,
                checks::stopping_craked_leaves_nothing,
                names::a_public_name_with_a_private_address_goes_through,
                names::a_tunnel_dns_that_stops_answering_is_stood_in_for,
                hosts::undo_sends_a_host_direct_and_redo_brings_it_back,
                hosts::a_host_added_by_hand_goes_through,
                hosts::a_name_added_by_hand_goes_through,
                hosts::a_pinned_network_goes_through_until_unpinned,
                hosts::forgetting_a_host_sends_it_direct,
                hosts::forget_all_keeps_the_pins,
                hosts::the_never_list_keeps_a_host_direct,
                hosts::tunnel_detail_lists_what_was_learned,
                settings::with_the_rescue_off_an_unknown_name_stays_unknown,
                settings::names_not_kept_private_go_to_public_dns,
                sign_in::a_wrong_password_is_asked_again,
                sign_in::switching_on_again_uses_what_was_kept,
                several::a_second_tunnel_that_clashes_is_refused,
                several::two_companies_each_through_their_own,
                several::a_host_both_answer_for_can_be_moved,
                several::one_address_goes_through_one_tunnel,
            ]
        );
    };
}

every_scenario!(anyconnect, super::Gateway::AnyConnect);
every_scenario!(openvpn, super::Gateway::OpenVpn);
#[cfg(target_os = "linux")]
every_scenario!(ipsec, super::Gateway::Ipsec);

/// The scenarios of IT's tunnels (managed.d), each a test of its own,
/// against the kinds of gateway IT's files can name (not IPsec yet).
macro_rules! it_scenarios {
    ($kind:ident, $gateway:expr) => {
        mod $kind {
            use super::managed;
            #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
            async fn it_s_tunnel_keeps_to_its_scope() {
                super::run_managed(
                    $gateway,
                    managed::SCOPE,
                    managed::it_s_tunnel_keeps_to_its_scope,
                )
                .await;
            }
            #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
            async fn it_s_tunnel_cant_be_changed_or_removed() {
                super::run_managed(
                    $gateway,
                    managed::SCOPE,
                    managed::it_s_tunnel_cant_be_changed_or_removed,
                )
                .await;
            }
        }
    };
}

it_scenarios!(it_anyconnect, super::Gateway::AnyConnect);
it_scenarios!(it_openvpn, super::Gateway::OpenVpn);
