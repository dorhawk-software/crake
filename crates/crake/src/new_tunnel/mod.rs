//! New tunnel (design/screens/new-tunnel.html): three steps, Server, Sign-in
//! and Routing, then "Add and connect". The same window edits a tunnel added
//! in the app (filled in, "Save").

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, Pixels, Stateful, Subscription,
    Task, TestSupportExt, Window, div, px, svg,
};

use crate::Request;
use crate::client::Tunnel;
use crate::detect::Found;
use crate::model::Model;
use crate::theme::{Theme, fonts};

mod view;

mod import;
// IPsec's key, domains and identity (ipsec-tunnel.html).
mod ipsec;
// How an IPsec gateway proves itself: its key or its certificate
// (ipsec-certificates.html).
mod ipsec_proof;

mod routing;

mod server;

mod sign_in;
// Moving between the steps, and what Add sends.
mod steps;

mod widgets;

#[cfg(test)]
mod tests;

/// Steps 0 to 3: the server, sign-in, routing, the summary.
const LAST_STEP: usize = 3;

/// "Asking the gateway" shows at least this long, so it doesn't flicker;
/// the question itself gets DETECT_TIMEOUT.
const ASKING_AT_LEAST: Duration = Duration::from_millis(900);
const DETECT_TIMEOUT: Duration = Duration::from_secs(12);

// What the steps share, and what the rest of the app uses.
pub use self::server::{beta, beta_tag};
use self::widgets::{card, checkbox, field, heading, label, radio};

pub const WIDTH: f32 = 720.;

pub const HEIGHT: f32 = 860.;

/// The protocols openconnect speaks (step 1): id, name, sub, code.
const PROTOCOLS: [(&str, &str, &str, &str); 10] = [
    ("auto", "Detect automatically", "", ""),
    (
        "anyconnect",
        "Cisco AnyConnect / ocserv",
        "Most corporate gateways",
        "anyconnect",
    ),
    ("gp", "Palo Alto GlobalProtect", "Portal and gateway", "gp"),
    (
        "pulse",
        "Ivanti Connect Secure (Pulse)",
        "Newer Pulse servers",
        "pulse",
    ),
    (
        "nc",
        "Juniper Network Connect",
        "Older Juniper servers",
        "nc",
    ),
    (
        "fortinet",
        "Fortinet SSL VPN",
        "FortiClient\u{2019}s \u{201c}SSL-VPN\u{201d}",
        "fortinet",
    ),
    ("f5", "F5 BIG-IP", "Access Policy Manager", "f5"),
    ("array", "Array Networks", "AG / vxAG", "array"),
    // Crake runs it itself (CLAUDE.md, 7i); OpenVPN 3's profiles are
    // taken over from its Profile field.
    (
        "openvpn",
        "OpenVPN",
        "A .ovpn profile from your IT team",
        "openvpn",
    ),
    // craked runs it with strongSwan (CLAUDE.md, 7k); shown after Fortinet
    // SSL VPN, the other way into a FortiGate (`server::protocol_list`).
    (
        "ipsec",
        "IPsec (IKEv2)",
        "FortiClient\u{2019}s \u{201c}IPsec VPN\u{201d}, and other IKEv2 gateways",
        "ipsec",
    ),
];

/// Protocols Crake has been used with against a real gateway. Every other
/// one says "Beta" until it has been (CLAUDE.md: Beta protocols).
pub const TRIED: [&str; 3] = ["anyconnect", "openvpn", "openvpn3"];

/// Sign-in methods (step 2): id, name, sub.
const METHODS: [(&str, &str, &str); 3] = [
    (
        "browser",
        "In your browser",
        "Single sign-on, passkeys and MFA. Crake opens your browser and waits.",
    ),
    (
        "password",
        "Username and password",
        "Crake asks for a code if the gateway wants one.",
    ),
    (
        "certificate",
        "Certificate",
        "From a smart card, your keyring or a file.",
    ),
];

/// Routing modes (step 3): id, name, sub, tag.
const MODES: [(&str, &str, &str, &str); 4] = [
    (
        "learn",
        "Learn as it goes",
        "Only hosts that don\u{2019}t answer directly.",
        "Recommended",
    ),
    (
        "ask",
        "Ask me first",
        "A notification for each new host.",
        "",
    ),
    (
        "list",
        "Only what I list",
        "Names and networks you add, nothing else.",
        "",
    ),
    (
        "full",
        "Everything",
        "Full tunnel. Crake won\u{2019}t learn anything.",
        "",
    ),
];

#[derive(Clone, Debug, PartialEq)]
enum Detect {
    Idle,
    /// Since when.
    Asking(std::time::Instant),
    Done(Found),
}

pub struct NewTunnel {
    model: Entity<Model>,
    requests: async_channel::Sender<Request>,
    /// Editing this tunnel (its name), or adding one.
    editing: Option<String>,
    step: usize,
    name: Entity<InputState>,
    gateway: Entity<InputState>,
    user: Entity<InputState>,
    list_input: Entity<InputState>,
    protocol: &'static str,
    detect: Detect,
    detecting: Option<Task<()>>,
    group: Option<String>,
    method: &'static str,
    /// Certificate from: "pkcs11", "keyring" or "file".
    cert_source: &'static str,
    /// The file chosen, and its bytes (kept in the keyring on Add).
    cert_file: Option<(String, Vec<u8>)>,
    keep_signed_in: bool,
    advanced: bool,
    /// "Choose the protocol yourself": the list opened after the gateway
    /// answered (design/proposals/critique-2026-10-15, 3).
    protocols_open: bool,
    client_version: bool,
    weak_ciphers: bool,
    mode: &'static str,
    list: Vec<String>,
    adding_list: bool,
    /// The gateway's suggestions (editing a tunnel that has connected).
    suggested: Vec<String>,
    send_suggested: bool,
    forget: String,
    open_select: Option<&'static str>,
    /// What's missing, said where Continue was pressed.
    problem: Option<String>,
    /// The tallest the window may get (the screen, less some room).
    max_height: f32,
    /// OpenVPN 3 profiles on this computer (loaded when the window opens),
    /// those it keeps locked, and the one chosen.
    profiles: Vec<String>,
    locked: Vec<String>,
    /// Which VPN engines are installed (None until looked; all in demo).
    engines: Option<crate::engines::Engines>,
    profile: Option<String>,
    /// How importing a .ovpn file went, under the Profile field.
    profile_note: Option<import::ProfileNote>,
    /// OpenVPN: the profile chosen (a file's, or a copy of OpenVPN 3's).
    ovpn: Option<import::Chosen>,
    /// A name taken from an imported file, set in the field at the next
    /// frame (which has the window).
    pending_name: Option<String>,
    /// IPsec's key, domains and the gateway's identity.
    ipsec: ipsec::Ipsec,
    /// Editing: the tunnel's protocol and gateway as they were.
    editing_protocol: String,
    kept_gateway: String,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

/// openvpn3's profiles on this computer (and those it keeps locked) and
/// the installed engines. Tests get fixed ones (three profiles, globex
/// locked, every engine), never this computer's.
fn look_around() -> (Vec<String>, Vec<String>, crate::engines::Engines) {
    if cfg!(test) {
        let all = crate::engines::Engines::ALL;
        let profiles = vec!["globex".into(), "acme".into(), "initech".into()];
        return (profiles, vec!["globex".into()], all);
    }
    let profiles = crate::ovpn3::profiles();
    let locked = profiles
        .iter()
        .filter(|p| crate::ovpn3::profile::locked(p) == Some(true))
        .cloned()
        .collect();
    (profiles, locked, crate::engines::Engines::find())
}

impl gpui_kit::EventEmitter<Close> for NewTunnel {}

fn static_of<const N: usize, T>(
    table: &[T; N],
    id: &str,
    key: impl Fn(&T) -> &'static str,
) -> &'static str {
    table
        .iter()
        .map(&key)
        .find(|k| *k == id)
        .unwrap_or(key(&table[0]))
}

impl NewTunnel {
    /// The name, gateway and username fields (journey tests).
    #[cfg(test)]
    pub(crate) fn fields_for_tests(
        &self,
    ) -> (Entity<InputState>, Entity<InputState>, Entity<InputState>) {
        (self.name.clone(), self.gateway.clone(), self.user.clone())
    }

    /// Import a .ovpn file as the file picker would give it, and the
    /// gateway it names then (the Linux journey: the picker is the
    /// platform's).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn import_for_tests(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.import_ovpn(path, cx);
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn imported_for_tests(&self) -> Option<String> {
        self.ovpn.as_ref().map(|o| o.remote.clone())
    }

    pub fn new(
        model: Entity<Model>,
        requests: async_channel::Sender<Request>,
        editing: Option<Tunnel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let own = editing.as_ref().map(|t| t.own.clone()).unwrap_or_default();
        let field = |k: &str| own.get(k).cloned().unwrap_or_default();
        let input = |value: String,
                     placeholder: &'static str,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder(placeholder);
                s.set_value(value, window, cx);
                s
            })
        };
        let name = input(field("display_name"), "Nordbank", window, cx);
        let gateway = input(field("gateway"), "vpn.example.com", window, cx);
        let user = input(field("user"), "your.name", window, cx);
        let mut ipsec = ipsec::Ipsec::new(
            &field("domains"),
            ipsec::Kept {
                key: field("protocol") == "ipsec" && field("gateway_proof") != "certificate",
                certificate: field("gateway_proof") == "certificate",
                ca: field("gateway_ca") == "kept",
            },
            window,
            cx,
        );
        let list_input = input(
            String::new(),
            "*.example.internal or 10.0.0.0/16",
            window,
            cx,
        );
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.subscribe_in(&gateway, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.gateway_changed(cx);
                }
            }),
            cx.subscribe_in(
                &list_input,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let item = input.read(cx).value().trim().to_owned();
                        if !item.is_empty() && !this.list.contains(&item) {
                            this.list.push(item);
                        }
                        input.update(cx, |f, cx| f.set_value("", window, cx));
                        this.adding_list = false;
                        cx.notify();
                    }
                },
            ),
            cx.observe_window_appearance(window, |_, _, cx| {
                Theme::current(cx).sync_components(cx);
                cx.notify();
            }),
        ];
        let focus = cx.focus_handle();
        name.update(cx, |f, cx| f.focus(window, cx));
        let forget = match field("forget_after_days").as_str() {
            "" => model.read(cx).setting("forget_after_days").to_owned(),
            other => other.to_owned(),
        };
        let suggested = editing
            .as_ref()
            .map(|t| t.split_include.clone())
            .unwrap_or_default();
        let certificate = field("certificate");
        // CRAKE_SHOW=newtunnel[:<step>[:<protocol>]] in demo mode
        // (development): filled in as new-tunnel.html, on that step.
        let shown = std::env::var("CRAKE_SHOW")
            .ok()
            .filter(|v| v.starts_with("newtunnel"));
        let demo = std::env::var_os("CRAKE_DEMO").is_some() && shown.is_some();
        if demo {
            name.update(cx, |f, cx| f.set_value("Nordbank", window, cx));
            gateway.update(cx, |f, cx| f.set_value("gw.nordbank.example", window, cx));
            user.update(cx, |f, cx| f.set_value("ondrej.simon", window, cx));
        }
        let step = shown
            .as_deref()
            .and_then(|v| v.strip_prefix("newtunnel:"))
            .and_then(|s| s.split(':').next()?.parse().ok())
            .unwrap_or(1)
            .min(LAST_STEP);
        let shown_protocol = shown
            .as_deref()
            .and_then(|v| v.split(':').nth(2))
            .filter(|_| demo)
            .map(|p| static_of(&PROTOCOLS, p, |p| p.0));
        // newtunnel:1:ipsec:certificate: the gateway proving itself with its
        // certificate, IT's authority picked (ipsec-certificates.html, 1);
        // the variant after the step and the protocol.
        // literal: CRAKE_SHOW's fourth part
        if demo && shown.as_deref().and_then(|v| v.split(':').nth(3)) == Some("certificate") {
            ipsec.certificate = true;
            ipsec.ca = Some(("nordbank-ca.pem".into(), String::new()));
            ipsec.domains.update(cx, |f, cx| {
                f.set_value("nordbank.internal nordbank.example", window, cx)
            });
        }
        // openvpn3's profiles and the installed engines, without holding the
        // window up.
        cx.spawn(async move |this, cx| {
            let (found, locked, engines) = cx
                .background_executor()
                .spawn(async { look_around() })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.profiles = found;
                this.locked = locked;
                // An OpenVPN 3 tunnel edited: its profile, taken over.
                if this.protocol == "openvpn"
                    && this.ovpn.is_none()
                    && let Some(profile) = this.profile.clone()
                {
                    this.take_over(profile, cx);
                }
                let engines = *this.engines.get_or_insert(engines);
                // A new tunnel where the browser can't sign in starts on the
                // password instead (Ubuntu 22.04's openconnect).
                if this.editing.is_none() && this.method == "browser" && !engines.browser {
                    this.method = "password";
                }
                cx.notify();
            });
        })
        .detach();
        Self {
            model,
            requests,
            editing: editing.as_ref().map(|t| t.name.clone()),
            step,
            name,
            gateway,
            user,
            list_input,
            protocol: if let Some(p) = shown_protocol {
                p
            } else if field("protocol") == "openvpn3" {
                // Moving to Crake's own OpenVPN (7i, decided 2026-10-18).
                "openvpn"
            } else if editing.is_some() {
                static_of(&PROTOCOLS, &field("protocol"), |p| p.0)
            } else {
                "auto"
            },
            detect: if demo {
                Detect::Done(Found::AnyConnect {
                    groups: vec!["Employees".into(), "Contractors".into()],
                    // new-tunnel.html: "valid until March 2027".
                    expires: "2027-03-31T12:00:00Z".parse().ok(),
                })
            } else {
                Detect::Idle
            },
            detecting: None,
            group: if demo {
                Some("Employees".into())
            } else {
                Some(field("group")).filter(|g| !g.is_empty())
            },
            method: static_of(&METHODS, &field("sign_in"), |m| m.0),
            cert_source: match certificate.as_str() {
                c if c.starts_with("pkcs11:") => "pkcs11",
                "keyring" => "keyring",
                "" => "file",
                _ => "file",
            },
            cert_file: None,
            keep_signed_in: field("keep_signed_in") != "false",
            advanced: false,
            protocols_open: false,
            client_version: field("client_version") == "true",
            weak_ciphers: field("weak_ciphers") == "true",
            mode: static_of(&MODES, &field("mode"), |m| m.0),
            list: field("list")
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_owned)
                .collect(),
            adding_list: false,
            suggested,
            send_suggested: false,
            forget,
            open_select: None,
            problem: None,
            max_height: crate::place::max_window_height(),
            profiles: Vec::new(),
            locked: Vec::new(),
            engines: demo.then_some(crate::engines::Engines::ALL),
            profile: (field("protocol") == "openvpn3")
                .then(|| field("gateway"))
                .filter(|g| !g.is_empty()),
            profile_note: (demo && shown_protocol == Some("openvpn")).then(|| {
                import::ProfileNote::Imported("From initech.ovpn. Crake keeps its own copy, which only its service can read, so you can delete the file.".into())
            }),
            ovpn: (demo && shown_protocol == Some("openvpn")).then(|| import::Chosen {
                text: String::new(),
                remote: "vpn.initech.example".into(),
                from_file: true,
            }),
            pending_name: (demo && shown_protocol == Some("openvpn")).then(|| "Initech".into()),
            editing_protocol: field("protocol"),
            kept_gateway: field("gateway"),
            ipsec,
            focus,
            _subscriptions: subscriptions,
        }
    }

    /// "Detect automatically": ask the gateway, a moment after typing stops.
    fn gateway_changed(&mut self, cx: &mut Context<Self>) {
        let gateway = self.gateway.read(cx).value().trim().to_owned();
        self.group = None;
        if gateway.is_empty() {
            self.detect = Detect::Idle;
            self.detecting = None;
            cx.notify();
            return;
        }
        self.detect = Detect::Asking(std::time::Instant::now());
        self.detecting = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ASKING_AT_LEAST).await;
            let found = cx
                .background_executor()
                .spawn(async move { crate::detect::gateway(&gateway, DETECT_TIMEOUT) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Found::AnyConnect { groups, .. } = &found
                    && this.group.is_none()
                {
                    this.group = groups.first().cloned();
                }
                this.detect = Detect::Done(found);
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn display_name(&self, cx: &Context<Self>) -> String {
        self.name.read(cx).value().trim().to_owned()
    }

    /// The protocol to use: chosen, or detected.
    fn protocol(&self) -> Option<&'static str> {
        match (self.protocol, &self.detect) {
            ("auto", Detect::Done(Found::AnyConnect { .. })) => Some("anyconnect"),
            ("auto", _) => None,
            (p, _) => Some(p),
        }
    }

    fn groups(&self) -> Vec<String> {
        match &self.detect {
            Detect::Done(Found::AnyConnect { groups, .. }) => groups.clone(),
            _ => self.group.iter().cloned().collect(),
        }
    }

    /// The engine `protocol` needs isn't installed: what to do about it.
    fn missing(&self, protocol: &str) -> Option<&'static str> {
        self.engines.and_then(|e| e.missing(protocol))
    }

    /// Why the browser can't sign in here, if it can't.
    fn no_browser(&self) -> Option<String> {
        self.engines.and_then(|e| e.no_browser())
    }
}
