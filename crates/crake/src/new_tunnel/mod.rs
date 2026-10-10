//! New tunnel (design/screens/new-tunnel.html): three steps, Server, Sign-in
//! and Routing, then "Add and connect". The same window edits a tunnel added
//! in the app (filled in, "Save").

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, Stateful, Subscription, Task,
    Window, div, px, svg,
};

use crate::Request;
use crate::client::Tunnel;
use crate::detect::Found;
use crate::model::Model;
use crate::theme::{Theme, fonts};

mod routing;
mod server;
mod sign_in;
mod widgets;

// What the steps share, and what the rest of the app uses.
pub use self::server::{beta, beta_tag};
use self::sign_in::default_browser;
use self::widgets::{card, checkbox, field, heading, label, radio};

pub const WIDTH: f32 = 720.;

pub const HEIGHT: f32 = 860.;

/// The protocols openconnect speaks (step 1): id, name, sub, code.
const PROTOCOLS: [(&str, &str, &str, &str); 9] = [
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
    ("fortinet", "Fortinet SSL VPN", "FortiGate", "fortinet"),
    ("f5", "F5 BIG-IP", "Access Policy Manager", "f5"),
    ("array", "Array Networks", "AG / vxAG", "array"),
    (
        "openvpn3",
        "OpenVPN 3 profile",
        "One of your openvpn3 profiles on this computer",
        "openvpn3",
    ),
];

/// Protocols Crake has been used with against a real gateway. Every other
/// one says "Beta" until it has been (CLAUDE.md: Beta protocols).
pub const TRIED: [&str; 2] = ["anyconnect", "openvpn3"];

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
    Asking,
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
    /// The default browser's name, when known.
    browser: Option<String>,
    /// The tallest the window may get (the screen, less some room).
    max_height: f32,
    /// OpenVPN 3 profiles on this computer (loaded when the window opens),
    /// and the one chosen.
    profiles: Vec<String>,
    /// Which VPN engines are installed (None until looked; all in demo).
    engines: Option<crate::engines::Engines>,
    profile: Option<String>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

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

impl Render for NewTunnel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let title = match &self.editing {
            Some(name) => format!("Edit {}", self.model.read(cx).display_of(Some(name))),
            None => "New tunnel".to_owned(),
        };
        let body: AnyElement = match self.step {
            1 => self.server(&t, cx).into_any_element(),
            2 => self.sign_in(&t, cx).into_any_element(),
            _ => self.routing(&t, cx).into_any_element(),
        };
        let next_label = match (self.step, &self.editing) {
            (3, Some(_)) => "Save",
            (3, None) => "Add and connect",
            _ => "Continue",
        };
        let max_height = self.max_height;
        // The window is the card: as tall as this step needs.
        let fit = gpui_kit::canvas(
            move |bounds, window, _| {
                // Measured inside the card's 1 px border: add it back, or
                // each resize comes out 2 px short.
                let want = (f32::from(bounds.size.height) + 2.).min(max_height);
                if (f32::from(window.viewport_size().height) - want).abs() > 0.5 {
                    window.on_next_frame(move |window, _| {
                        window.resize(gpui_kit::size(px(WIDTH), px(want)));
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let card = div()
            .id("new-tunnel")
            .track_focus(&self.focus)
            .relative()
            .w_full()
            // Its own height, never squeezed to the window's: the canvas
            // below measures it, and the window grows to it.
            .flex_none()
            .flex()
            .flex_col()
            .bg(t.panel)
            .border_1()
            .border_color(t.line)
            .rounded(px(12.))
            .overflow_hidden()
            .text_color(t.ink)
            .font_family(fonts::UI)
            .text_size(px(14.))
            .line_height(px(20.))
            .on_key_down(cx.listener(|this, e: &gpui_kit::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    if this.open_select.is_some() {
                        this.open_select = None;
                        cx.notify();
                    } else {
                        cx.emit(Close);
                    }
                }
            }))
            // Header: Cancel, the title, close.
            .child(
                div()
                    .flex()
                    .items_center()
                    .py(px(10.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(t.line)
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                        window.start_window_move()
                    })
                    .child(
                        div().flex_1().child(
                            div()
                                .id("cancel")
                                .h(px(36.))
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .rounded(px(6.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(t.surface))
                                .child("Cancel")
                                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(Close))),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(
                        div().flex_1().flex().justify_end().child(
                            div()
                                .id("close")
                                .size(px(32.))
                                .rounded_full()
                                .bg(t.surface)
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(
                                    svg()
                                        .path("crake/close.svg")
                                        .size(px(14.))
                                        .text_color(t.ink),
                                )
                                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(Close))),
                        ),
                    ),
            )
            .child(self.steps(&t, cx))
            .child(
                div()
                    .id("body")
                    // Its natural height; scrolls only past the screen's
                    // (less the header, steps and footer).
                    .flex_none()
                    .max_h(px(max_height - 190.))
                    .overflow_y_scroll()
                    .pt(px(26.))
                    .px(px(32.))
                    .pb(px(8.))
                    .child(body)
                    .when_some(self.problem.clone(), |b, p| {
                        b.child(
                            div()
                                .mt(px(14.))
                                .text_size(px(13.))
                                .text_color(t.danger)
                                .child(p),
                        )
                    }),
            )
            // Footer: Back, "Step n of 3", Continue.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pt(px(16.))
                    .px(px(32.))
                    .pb(px(22.))
                    .when(self.step > 1, |f| {
                        f.child(
                            div()
                                .id("back")
                                .h(px(42.))
                                .px(px(16.))
                                .flex()
                                .items_center()
                                .border_1()
                                .border_color(t.line)
                                .rounded(px(6.))
                                .bg(t.panel)
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child("Back")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.step -= 1;
                                    this.problem = None;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.muted)
                            .child(format!("Step {} of 3", self.step)),
                    )
                    .child(
                        div()
                            .id("next")
                            .h(px(42.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(t.accent)
                            .text_color(t.accent_ink)
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .child(next_label)
                            .on_click(cx.listener(|this, _, _, cx| this.next(cx))),
                    ),
            )
            .child(fit);
        div().w_full().flex().flex_col().child(card)
    }
}

impl NewTunnel {
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
            .unwrap_or(1);
        let shown_protocol = shown
            .as_deref()
            .and_then(|v| v.split(':').nth(2))
            .filter(|_| demo)
            .map(|p| static_of(&PROTOCOLS, p, |p| p.0));
        // openvpn3's profiles and the installed engines, without holding the
        // window up.
        cx.spawn(async move |this, cx| {
            let (found, engines) = cx
                .background_executor()
                .spawn(async { (crate::ovpn3::profiles(), crate::engines::Engines::find()) })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.profiles = found;
                this.engines.get_or_insert(engines);
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
            } else if editing.is_some() {
                static_of(&PROTOCOLS, &field("protocol"), |p| p.0)
            } else {
                "auto"
            },
            detect: if demo {
                Detect::Done(Found::AnyConnect {
                    groups: vec!["Employees".into(), "Contractors".into()],
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
            browser: default_browser(),
            max_height: crate::place::monitors()
                .first()
                .map_or(1000., |m| m.h - 120.),
            profiles: Vec::new(),
            engines: demo.then_some(crate::engines::Engines {
                openconnect: true,
                openvpn3: true,
            }),
            profile: (field("protocol") == "openvpn3")
                .then(|| field("gateway"))
                .filter(|g| !g.is_empty()),
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
        self.detect = Detect::Asking;
        self.detecting = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(900))
                .await;
            let found = cx
                .background_executor()
                .spawn(async move { crate::detect::gateway(&gateway, Duration::from_secs(12)) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Found::AnyConnect { groups } = &found
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
            Detect::Done(Found::AnyConnect { groups }) => groups.clone(),
            _ => self.group.iter().cloned().collect(),
        }
    }

    /// The engine `protocol` needs isn't installed: what to do about it.
    fn missing(&self, protocol: &str) -> Option<&'static str> {
        self.engines.and_then(|e| e.missing(protocol))
    }

    /// Can step `step` be left? Says what's missing if not.
    fn check(&mut self, step: usize, cx: &Context<Self>) -> bool {
        let problem = match step {
            1 if self.display_name(cx).is_empty() => Some("Give it a name."),
            1 if self.missing(self.protocol).is_some() => self.missing(self.protocol),
            1 if self.protocol == "openvpn3" && self.profile.is_none() => {
                Some("Which OpenVPN 3 profile?")
            }
            1 if self.protocol != "openvpn3" && self.gateway.read(cx).value().trim().is_empty() => {
                Some("Which gateway? Its name or address, like vpn.example.com.")
            }
            1 if self.protocol().is_none() => match self.detect {
                Detect::Asking => Some("Still asking the gateway. A moment."),
                _ => Some("Pick the protocol: the gateway didn\u{2019}t say."),
            },
            2 if self.method == "certificate"
                && self.cert_source == "file"
                && self.cert_file.is_none()
                && self.editing.is_none() =>
            {
                Some("Choose the certificate file.")
            }
            _ => None,
        };
        self.problem = problem.map(str::to_owned);
        problem.is_none()
    }

    fn fields(&self, cx: &Context<Self>) -> HashMap<String, String> {
        let mut f = HashMap::new();
        f.insert("display_name".into(), self.display_name(cx));
        // An OpenVPN 3 tunnel's "gateway" is its profile.
        let gateway = if self.protocol == "openvpn3" {
            self.profile.clone().unwrap_or_default()
        } else {
            self.gateway.read(cx).value().trim().to_owned()
        };
        f.insert("gateway".into(), gateway);
        f.insert(
            "protocol".into(),
            self.protocol().unwrap_or("anyconnect").into(),
        );
        let group = match self.protocol {
            "openvpn3" => String::new(),
            _ => self.group.clone().unwrap_or_default(),
        };
        f.insert("group".into(), group);
        f.insert("sign_in".into(), self.method.into());
        f.insert("user".into(), self.user.read(cx).value().trim().to_owned());
        let certificate = match (self.method, self.cert_source) {
            ("certificate", "pkcs11") => "pkcs11:".to_owned(),
            // A chosen file is kept in the keyring (7e).
            ("certificate", _) => "keyring".to_owned(),
            _ => String::new(),
        };
        f.insert("certificate".into(), certificate);
        f.insert("mode".into(), self.mode.into());
        f.insert("list".into(), self.list.join("\n"));
        f.insert("forget_after_days".into(), self.forget.clone());
        f.insert("weak_ciphers".into(), self.weak_ciphers.to_string());
        f.insert("client_version".into(), self.client_version.to_string());
        f.insert("keep_signed_in".into(), self.keep_signed_in.to_string());
        f
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        if !self.check(self.step, cx) {
            cx.notify();
            return;
        }
        if self.step < 3 {
            self.step += 1;
            cx.notify();
            return;
        }
        let fields = self.fields(cx);
        let certificate = self.cert_file.as_ref().map(|(_, b)| b.clone());
        match &self.editing {
            Some(name) => {
                let _ = self.requests.send_blocking(Request::UpdateTunnel(
                    name.clone(),
                    fields,
                    certificate,
                ));
                if self.send_suggested {
                    let _ = self
                        .requests
                        .send_blocking(Request::Pin(name.clone(), self.suggested.clone()));
                }
            }
            None => {
                let _ = self
                    .requests
                    .send_blocking(Request::AddTunnel(fields, certificate));
            }
        }
        cx.emit(Close);
    }

    fn choose_certificate(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let read = std::fs::read(&path);
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(bytes) => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        this.cert_file = Some((name, bytes));
                        this.problem = None;
                    }
                    Err(e) => this.problem = Some(format!("Couldn\u{2019}t read that file: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn steps(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div().flex().px(px(32.)).border_b_1().border_color(t.line);
        for (i, label) in ["Server", "Sign-in", "Routing"].into_iter().enumerate() {
            let n = i + 1;
            let (cur, done) = (self.step == n, self.step > n);
            let ring = if cur || done { t.accent } else { t.line_strong };
            bar = bar.child(
                div()
                    .id(("step", i))
                    .flex_1()
                    .h(px(50.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mb(px(-1.))
                    .border_b_2()
                    .border_color(if cur {
                        t.accent
                    } else {
                        gpui_kit::transparent_black().into()
                    })
                    .text_color(if cur || done { t.ink } else { t.muted })
                    .font_weight(if cur {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(20.))
                            .rounded_full()
                            .border(px(1.5))
                            .border_color(ring)
                            .when(done, |d| d.bg(t.accent))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if done {
                                t.accent_ink
                            } else if cur {
                                t.accent
                            } else {
                                t.muted
                            })
                            .child(if done {
                                "\u{2713}".to_owned()
                            } else {
                                n.to_string()
                            }),
                    )
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Forward only past steps that are complete.
                        let ok = (this.step..n).all(|s| this.check(s, cx));
                        if n <= this.step || ok {
                            this.step = n;
                            this.problem = None;
                        }
                        cx.notify();
                    })),
            );
        }
        bar
    }
}
