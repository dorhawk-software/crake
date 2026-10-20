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
use crate::test_app::{App, enabled, gateway, sign_in};

mod checks;
mod hosts;
mod platform;

pub(crate) use platform::Craked;

/// How long a tunnel, and a host through it, get.
const WAIT: Duration = Duration::from_secs(60);
/// The test tunnels, gone before and after.
const TUNNELS: [&str; 3] = ["acme", "initech", "nordbank"];

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
}

impl Gateway {
    /// The tunnel's name and how the app shows it.
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::AnyConnect => ("acme", "ACME"),
            Self::OpenVpn => ("initech", "Initech"),
            #[cfg(target_os = "linux")]
            Self::Ipsec => ("nordbank", "Nordbank"),
        }
    }

    /// New tunnel's fields for it.
    fn fields(self) -> Vec<(&'static str, String)> {
        let (_, label) = self.names();
        let mut fields = vec![("display_name", label.to_owned())];
        match self {
            Self::AnyConnect => fields.extend([
                ("gateway", gateway()),
                ("protocol", "anyconnect".into()),
                ("user", "alice".into()),
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
    /// The tunnel's name ("acme") and how it's shown ("ACME").
    pub name: &'static str,
    pub label: &'static str,
    /// The device it runs on (crake-acme, utun5).
    pub device: String,
}

impl Up {
    async fn start(gateway: Gateway) -> Self {
        let (name, label) = gateway.names();
        let (crake, mut app) = Craked::start().await;
        app.within(WAIT, "craked reached", |m| {
            m.reachable == Some(true) && !m.settings.is_empty()
        })
        .await;
        remove_tunnels(&mut app).await;
        // Windows' service keeps its settings from test to test.
        app.send(Command::SetSetting("never".into(), String::new()));
        app.until("no never list", |m| {
            m.settings.get("never").is_some_and(String::is_empty)
        })
        .await;
        let before = platform::answers("10.40.1.10").await;
        assert!(!before, "10.40.1.10 answered without a tunnel");

        let fields: HashMap<String, String> = gateway
            .fields()
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        app.send(Command::AddTunnel(fields));
        app.until("added", |m| m.tunnels.iter().any(|t| t.name == name))
            .await;
        if gateway == Gateway::AnyConnect {
            let session = sign_in().await;
            app.send(Command::ConnectWithSession(name.into(), session));
        } else {
            app.send(Command::Connect(name.into()));
            // IPsec's username comes with the tunnel; OpenVPN's profile has none.
            let asks: &[_] = if gateway == Gateway::OpenVpn {
                &[("user", "alice"), ("password", "hunter2")]
            } else {
                &[("password", "hunter2")]
            };
            for &(kind, text) in asks {
                let ask = app.asked(kind).await;
                assert_eq!(ask["kind"], kind, "{label} asked {ask:?}");
                app.send(Command::Answer(
                    name.into(),
                    ask["id"].parse().unwrap(),
                    text.to_owned(),
                ));
            }
        }
        app.within(WAIT, "connected", |m| state(m, name) == "connected")
            .await;
        let device = crake.device(name);
        Self {
            crake,
            app,
            name,
            label,
            device,
        }
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
        if app.model.tunnels.iter().any(|t| t.name == old) {
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

/// A scenario this system can't run, said in the test's output (today
/// only Windows has one).
#[cfg(windows)]
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
                hosts::undo_sends_a_host_direct_and_redo_brings_it_back,
                hosts::a_host_added_by_hand_goes_through,
                hosts::a_name_added_by_hand_goes_through,
                hosts::a_pinned_network_goes_through_until_unpinned,
                hosts::forgetting_a_host_sends_it_direct,
                hosts::forget_all_keeps_the_pins,
                hosts::the_never_list_keeps_a_host_direct,
                hosts::tunnel_detail_lists_what_was_learned,
            ]
        );
    };
}

every_scenario!(anyconnect, super::Gateway::AnyConnect);
every_scenario!(openvpn, super::Gateway::OpenVpn);
#[cfg(target_os = "linux")]
every_scenario!(ipsec, super::Gateway::Ipsec);
