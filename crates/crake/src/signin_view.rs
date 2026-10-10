//! The sign-in window (design/screens/sign-in.html): the steps as they
//! happen, the gateway's questions in its own words, and what went wrong
//! when it couldn't connect.

use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Entity, FontWeight, Stateful, Subscription, Window, div, px, svg};

use crate::Request;
use crate::model::Model;
use crate::signin::{Ask, Event, Prompt, Reply, Spec};
use crate::theme::{Theme, fonts};

pub const WIDTH: f32 = 340.;

#[derive(Clone, Debug, PartialEq)]
enum Stage {
    Reaching,
    SigningIn,
    Asking(Prompt),
    /// Signed in; waiting for craked to bring the tunnel up.
    Connecting,
    Up,
    /// It stopped at this step (see `steps`), for this reason.
    Failed {
        reason: String,
        at: usize,
    },
}

impl Stage {
    /// The step it's at: reaching the gateway, signing in, tunnel up.
    fn step(&self) -> usize {
        match self {
            Stage::Reaching => 0,
            Stage::SigningIn | Stage::Asking(_) => 1,
            Stage::Connecting => 2,
            Stage::Up => 3,
            Stage::Failed { at, .. } => *at,
        }
    }
}

pub struct SignIn {
    spec: Spec,
    requests: async_channel::Sender<Request>,
    replies: async_channel::Sender<Reply>,
    stage: Stage,
    field: Entity<InputState>,
    /// The window's own focus, for Esc.
    focus: gpui_kit::FocusHandle,
    remember: bool,
    keep_certificate: bool,
    model: Entity<Model>,
    /// The session went to the app or craked: closing ends nothing.
    handed_over: bool,
    _subscriptions: Vec<Subscription>,
}

impl SignIn {
    pub fn new(
        spec: Spec,
        model: Entity<Model>,
        requests: async_channel::Sender<Request>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx).masked(true));
        let replies = start(spec.clone(), requests.clone(), window, cx);
        // Connecting, as far as the tray and panel go, from now on; until
        // this window goes, unless the session was handed over first.
        let tunnel = spec.tunnel.clone();
        model.update(cx, |m, cx| {
            m.signing_in(&tunnel, true, false);
            cx.notify();
        });
        let ended = model.clone();
        cx.on_release(move |this: &mut Self, cx| {
            if this.handed_over {
                return;
            }
            ended.update(cx, |m, cx| {
                if m.signing_in.contains(&tunnel) {
                    m.signing_in(&tunnel, false, false);
                    cx.notify();
                }
            });
        })
        .detach();
        let subscriptions = vec![
            cx.subscribe_in(&field, window, |this, _, event: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.answer(cx);
                }
            }),
            // The tunnel coming up (or not) after sign-in.
            cx.observe(&model, |this, model, cx| {
                if this.stage != Stage::Connecting {
                    return;
                }
                let state = model
                    .read(cx)
                    .tunnels
                    .iter()
                    .find(|t| t.name == this.spec.tunnel)
                    .map(|t| t.state.clone());
                match state.as_deref() {
                    Some("connected") => {
                        this.stage = Stage::Up;
                        cx.spawn(async move |this, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(900))
                                .await;
                            let _ = this.update(cx, |_, cx| cx.emit(Close));
                        })
                        .detach();
                    }
                    Some("trouble") => {
                        let display = &this.spec.display;
                        this.stage = Stage::Failed {
                            reason: format!("{display} accepted the sign-in, but the tunnel didn\u{2019}t stay up."),
                            at: 2,
                        };
                    }
                    _ => {}
                }
                cx.notify();
            }),
            cx.observe_window_appearance(window, |_, _, cx| {
                Theme::current(cx).sync_components(cx);
                cx.notify();
            }),
        ];
        Self {
            spec,
            model,
            handed_over: false,
            requests,
            replies,
            stage: Stage::Reaching,
            field,
            focus: cx.focus_handle(),
            remember: true,
            keep_certificate: true,
            _subscriptions: subscriptions,
        }
    }

    fn on_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Reached => {
                if self.stage == Stage::Reaching {
                    self.stage = Stage::SigningIn;
                }
            }
            Event::Prompt(prompt) => {
                let secret = matches!(prompt.ask, Ask::Secret | Ask::CertificatePassword { .. });
                self.field.update(cx, |f, cx| {
                    f.set_value("", window, cx);
                    f.set_masked(secret, window, cx);
                });
                self.field.update(cx, |f, cx| f.focus(window, cx));
                self.stage = Stage::Asking(prompt);
            }
            Event::Done(session) => {
                self.stage = Stage::Connecting;
                self.handed_over = true;
                self.signing(false, true, cx);
                // Let in only as AnyConnect: remembered, and said openly.
                if let Some(ua) = &session.user_agent
                    && self.spec.user_agent.as_ref() != Some(ua)
                {
                    let _ = self
                        .requests
                        .send_blocking(Request::ActsAs(self.spec.gateway.clone(), ua.clone()));
                }
                let _ = self.requests.send_blocking(Request::ConnectWithSession(
                    self.spec.tunnel.clone(),
                    session,
                ));
            }
            Event::Attached(fields) => {
                self.handed_over = true;
                // (Still "signing in" until the app has the session: then
                // nothing else takes it over meanwhile.)
                // The session stays the app's; its control goes with it.
                let _ = self.requests.send_blocking(Request::Attach(
                    self.spec.tunnel.clone(),
                    fields,
                    self.replies.clone(),
                ));
                return cx.emit(Close);
            }
            Event::Failed(reason) => {
                if reason == "Cancelled." {
                    return cx.emit(Close);
                }
                self.signing(false, false, cx);
                let at = self.stage.step().min(2);
                self.stage = Stage::Failed { reason, at };
                window.focus(&self.focus, cx);
            }
        }
        cx.notify();
    }

    fn answer(&mut self, cx: &mut Context<Self>) {
        let text = self.field.read(cx).value().to_string();
        self.reply(text, cx);
    }

    fn reply(&mut self, text: String, cx: &mut Context<Self>) {
        let _ = self.replies.send_blocking(Reply::Answer {
            text,
            remember: self.remember,
            keep_certificate: self.keep_certificate,
        });
        self.stage = Stage::SigningIn;
        cx.notify();
    }

    fn signing(&self, on: bool, handed_over: bool, cx: &mut Context<Self>) {
        let tunnel = self.spec.tunnel.clone();
        self.model.update(cx, |m, cx| {
            m.signing_in(&tunnel, on, handed_over);
            cx.notify();
        });
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.handed_over = false;
        self.signing(true, false, cx);
        self.replies = start(self.spec.clone(), self.requests.clone(), window, cx);
        self.stage = Stage::Reaching;
        cx.notify();
    }

    /// What happened, in plain text for whoever helps.
    fn copy_for_helpdesk(&mut self, cx: &mut Context<Self>) {
        let reason = match &self.stage {
            Stage::Failed { reason, .. } => reason.clone(),
            _ => return,
        };
        let now = jiff::Zoned::now().strftime("%Y-%m-%d %H:%M %Z").to_string();
        let text = format!(
            "Crake, {now}\nCouldn\u{2019}t sign in to {} ({}, {}).\n{reason}\n",
            self.spec.display, self.spec.gateway, self.spec.protocol
        );
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        let _ = self.replies.send_blocking(Reply::Cancel);
        cx.emit(Close);
    }
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for SignIn {}

/// Runs a sign-in and feeds its events to the view.
fn start(
    spec: Spec,
    requests: async_channel::Sender<Request>,
    window: &mut Window,
    cx: &mut Context<SignIn>,
) -> async_channel::Sender<Reply> {
    let (events, incoming) = async_channel::unbounded();
    // OpenVPN 3 profiles run as openvpn3 sessions; the rest is openconnect.
    let replies = if spec.protocol == "openvpn3" {
        crate::ovpn3::start(spec.tunnel.clone(), spec.gateway.clone(), events, requests)
    } else {
        crate::signin::start(spec, events)
    };
    cx.spawn_in(window, async move |this, cx| {
        while let Ok(event) = incoming.recv().await {
            if this
                .update_in(cx, |this, window, cx| this.on_event(event, window, cx))
                .is_err()
            {
                return;
            }
        }
    })
    .detach();
    replies
}

impl Render for SignIn {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let display = self.spec.display.clone();
        // The card of sign-in.html: 12 px corners, a hairline, 22 px inside.
        let mut card = div()
            .relative()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .pt(px(22.))
            .px(px(22.))
            .pb(px(18.))
            .bg(t.panel)
            .border_1()
            .border_color(t.line)
            .rounded(px(12.))
            .text_color(t.ink)
            .font_family(fonts::UI)
            .text_size(px(14.))
            .line_height(px(20.));

        card = match self.stage.clone() {
            Stage::Asking(prompt) => card.child(self.question(&prompt, &t, window, cx)),
            // "When it can't connect": what happened, the steps up to where it
            // stopped, Copy for helpdesk and Try again.
            Stage::Failed { reason, at } => {
                let why = explain(&reason, at, &self.spec);
                card.child(title(why.title))
                    .child(para(why.sentence, t.ink))
                    .child(steps(at, Some((at, why.row)), &self.spec, &t))
                    // openconnect's (or openvpn3's) own words, for whoever helps.
                    .child(
                        div()
                            .mt(px(12.))
                            .text_size(px(13.))
                            .text_color(t.muted)
                            .child(format!("{} said: {reason}", self.spec.program())),
                    )
                    .child(
                        buttons()
                            .child(
                                button("helpdesk", "Copy for helpdesk", Look::Outline, &t)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.copy_for_helpdesk(cx)),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                button("close", "Close", Look::Text, &t)
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                            )
                            .child(button("retry", "Try again", Look::Primary, &t).on_click(
                                cx.listener(|this, _, window, cx| this.retry(window, cx)),
                            )),
                    )
            }
            stage => card
                .child(title(format!("Sign in to {display}")))
                .child(para("This window updates by itself.".into(), t.muted))
                .child(steps(stage.step(), None, &self.spec, &t))
                .child(
                    buttons().child(div().flex_1()).child(
                        button("cancel", "Cancel", Look::Text, &t)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    ),
                ),
        };
        // No title bar: the card moves by dragging it anywhere but its
        // controls; Esc closes it too.
        div()
            .id("sign-in")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.cancel(cx);
                }
            }))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(card.child(fit_window()))
    }
}

/// The window follows the card's height (it grows and shrinks downward).
fn fit_window() -> impl IntoElement {
    gpui_kit::canvas(
        |bounds, window, _| {
            // Measured inside the card's 1 px border: add it back.
            let want = gpui_kit::size(px(WIDTH), bounds.size.height + px(2.));
            let have = window.viewport_size();
            if (f32::from(have.height) - f32::from(want.height)).abs() > 0.5 {
                window.on_next_frame(move |window, _| window.resize(want));
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

impl SignIn {
    fn question(&self, prompt: &Prompt, t: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        use gpui_kit::Focusable;
        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let display = &self.spec.display;
        let mut body = div().flex().flex_col();
        match &prompt.ask {
            Ask::TrustServer { fingerprint } => {
                body = body
                    .child(title(format!("Crake can\u{2019}t vouch for {display}\u{2019}s gateway")))
                    .child(para(
                        "Its certificate isn\u{2019}t signed by an authority this computer knows. Trust it only if your IT says this is the right one."
                            .into(),
                        t.muted,
                    ))
                    .when_some(fingerprint.clone(), |b, f| {
                        b.child(div().mt(px(12.)).font_family(fonts::MONO).text_size(px(12.)).text_color(t.muted).child(f))
                    })
                    .child(
                        buttons().child(div().flex_1())
                            .child(button("no", "Don\u{2019}t connect", Look::Text, t).on_click(cx.listener(|this, _, _, cx| {
                                this.reply("no".into(), cx)
                            })))
                            .child(button("yes", "Trust it", Look::Primary, t).on_click(cx.listener(|this, _, _, cx| {
                                this.reply("yes".into(), cx)
                            }))),
                    );
                return body;
            }
            Ask::Choice(options) => {
                body = body
                    .child(title(format!("{display} asks: {}", prompt.label)))
                    .when_some(prompt.words.clone(), |b, w| {
                        b.child(para(
                            format!("The gateway\u{2019}s own words: \u{201c}{w}\u{201d}"),
                            t.muted,
                        ))
                    });
                let mut list = div().flex().flex_col().gap(px(8.)).mt(px(16.));
                for (i, option) in options.iter().enumerate() {
                    let o = option.clone();
                    list = list.child(
                        button(("choice", i), "", Look::Outline, t)
                            .h(px(40.))
                            .child(option.clone())
                            .on_click(cx.listener(move |this, _, _, cx| this.reply(o.clone(), cx))),
                    );
                }
                return body.child(list).child(
                    buttons().child(div().flex_1()).child(
                        button("cancel", "Cancel", Look::Text, t)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    ),
                );
            }
            Ask::CertificatePassword { retry, .. } => {
                body = body
                    .child(title("Unlock your certificate".into()))
                    .child(para(
                        if *retry {
                            "The password in your keyring didn\u{2019}t open it. Type it again."
                                .into()
                        } else {
                            format!(
                                "{display} signs you in with a certificate. Its password opens it."
                            )
                        },
                        t.muted,
                    ));
            }
            Ask::Secret | Ask::Text => {
                body = body
                    .child(title(format!(
                        "{display} asks for {}",
                        lower_first(&prompt.label)
                    )))
                    .when_some(prompt.words.clone(), |b, w| {
                        b.child(para(
                            format!("The gateway\u{2019}s own words: \u{201c}{w}\u{201d}"),
                            t.muted,
                        ))
                    });
            }
        }
        body = body
            .child(
                div()
                    .mt(px(16.))
                    .mb(px(8.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(prompt.label.clone()),
            )
            .child(
                div()
                    .h(px(44.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    // As in the design: 1 px line, 2 px teal while typing in it.
                    .when(focused, |b| b.border_2().border_color(t.accent))
                    .when(!focused, |b| b.border_1().border_color(t.line_strong))
                    .rounded(px(6.))
                    .child(Input::new(&self.field).appearance(false)),
            );
        if let Ask::CertificatePassword { from_file, .. } = &prompt.ask {
            body = body.child(
                checkbox(
                    "remember",
                    self.remember,
                    "Remember the password in your keyring",
                    t,
                )
                .mt(px(14.))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.remember = !this.remember;
                    cx.notify();
                })),
            );
            if let Some(path) = from_file {
                let home = std::env::var("HOME").unwrap_or_default();
                let shown = path.display().to_string().replacen(&home, "~", 1);
                body = body.child(
                    checkbox("keep", self.keep_certificate, "Keep a copy of the certificate in your keyring", t)
                        .mt(px(8.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.keep_certificate = !this.keep_certificate;
                            cx.notify();
                        })),
                )
                .when(self.keep_certificate, |b| {
                    b.child(
                        div()
                            .ml(px(28.))
                            .mt(px(2.))
                            .text_size(px(13.))
                            .text_color(t.muted)
                            .child("Crake keeps its own encrypted copy, so this file can then be deleted:")
                            .child(div().font_family(fonts::MONO).text_size(px(12.)).child(shown)),
                    )
                });
            }
        }
        body.child(
            buttons()
                .child(div().flex_1())
                .child(
                    button("cancel", "Cancel", Look::Text, t)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                )
                .child(
                    button("continue", "Continue", Look::Primary, t)
                        .on_click(cx.listener(|this, _, _, cx| this.answer(cx))),
                ),
        )
    }
}

fn title(text: String) -> Div {
    div()
        .text_size(px(19.))
        .line_height(px(25.))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

fn para(text: String, color: gpui_kit::Rgba) -> Div {
    div().mt(px(6.)).text_color(color).child(text)
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_lowercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Reached the gateway, signed in, tunnel up: done (two-stroke tick), now
/// (dots, bold), later (empty circle); or failed there (a red cross, bold).
/// The gateway's name is a machine fact: mono.
fn steps(at: usize, failed: Option<(usize, String)>, spec: &Spec, t: &Theme) -> Div {
    let failed_row = failed.as_ref().map(|(_, row)| row.clone());
    let failed = failed.map(|(at, _)| at);
    let gateway = || {
        div()
            .font_family(fonts::MONO)
            .text_size(px(13.))
            .child(spec.gateway.clone())
    };
    let rows: [(Div, Div); 3] = [
        (
            div().flex().gap(px(4.)).child("Reached").child(gateway()),
            div().flex().gap(px(4.)).child("Reaching").child(gateway()),
        ),
        (div().child("Signed in"), div().child("Signing in")),
        (
            div().child("Tunnel up"),
            div().child("Bringing the tunnel up"),
        ),
    ];
    let mut list = div()
        .mt(px(18.))
        .border_1()
        .border_color(t.line)
        .rounded(px(8.));
    for (i, (done, doing)) in rows.into_iter().enumerate() {
        if failed.is_some_and(|f| i > f) {
            break;
        }
        let (mark, text, bold, color) = if failed == Some(i) {
            (
                "crake/cross.svg",
                div().child(failed_row.clone().unwrap_or_default()),
                true,
                t.danger,
            )
        } else if i < at {
            ("crake/mark-routed.svg", done, false, t.accent)
        } else if i == at {
            ("crake/dots.svg", doing, true, t.accent)
        } else {
            ("crake/circle.svg", done, false, t.line_strong)
        };
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(11.))
                .px(px(12.))
                .when(i > 0, |r| r.border_t_1().border_color(t.line))
                .when(i > at && failed.is_none(), |r| r.text_color(t.muted))
                .when(bold, |r| r.font_weight(FontWeight::BOLD))
                .child(svg().path(mark).size(px(18.)).flex_none().text_color(color))
                .child(text),
        );
    }
    list
}

fn buttons() -> Div {
    div().flex().items_center().gap(px(8.)).mt(px(18.))
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    /// Teal, the action that moves on.
    Primary,
    /// Hairline outline, a secondary action.
    Outline,
    /// Text only (Cancel).
    Text,
}

fn button(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    look: Look,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(40.))
        .px(px(if look == Look::Primary { 18. } else { 14. }))
        .rounded(px(6.))
        .font_weight(if look == Look::Primary {
            FontWeight::BOLD
        } else {
            FontWeight::SEMIBOLD
        })
        .cursor_pointer()
        .when(look == Look::Primary, |b| {
            b.bg(t.accent).text_color(t.accent_ink)
        })
        .when(look == Look::Outline, |b| {
            b.border_1()
                .border_color(t.line)
                .bg(t.panel)
                .text_color(t.ink)
                .hover(|s| s.bg(t.surface))
        })
        .when(look == Look::Text, |b| {
            b.text_color(t.ink).hover(|s| s.bg(t.surface))
        })
        .child(label)
}

fn checkbox(id: &'static str, on: bool, label: &'static str, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.))
        .cursor_pointer()
        .child(
            div()
                .size(px(18.))
                .flex_none()
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .when(on, |b| b.bg(t.accent))
                .when(!on, |b| b.border_1().border_color(t.line_strong))
                .when(on, |b| {
                    b.child(
                        svg()
                            .path("crake/check.svg")
                            .size(px(14.))
                            .text_color(t.accent_ink),
                    )
                }),
        )
        .child(label)
}

/// A failure in Crake's words (DESIGN.md, copy voice): the title, one
/// sentence with what it means, and the failed step's own short line.
struct Why {
    title: String,
    sentence: String,
    row: String,
}

/// OpenVPN 3: openvpn3 reaches the server; what fails before that is
/// openvpn3 itself or the profile, not the network.
fn explain_openvpn3(r: &str, at: usize, spec: &Spec) -> Why {
    let (display, profile) = (&spec.display, &spec.gateway);
    if r.contains("no profile called") || r.contains("look up the profile") {
        return Why {
            title: format!("Couldn\u{2019}t start {display}"),
            sentence: format!("OpenVPN 3 has no profile called {profile} for you. It may have been removed or renamed."),
            row: format!("No profile {profile}"),
        };
    }
    if r.contains("reach openvpn 3") || r.contains("start a session") || r.contains("wasn\u{2019}t ready") {
        return Why {
            title: format!("Couldn\u{2019}t start {display}"),
            sentence: "OpenVPN 3 didn\u{2019}t start the session. Its own words are below.".into(),
            row: "OpenVPN 3 didn\u{2019}t start it".into(),
        };
    }
    if r.contains("accept the sign-in") {
        return Why {
            title: format!("Couldn\u{2019}t sign in to {display}"),
            sentence: format!("{display} didn\u{2019}t accept the sign-in. Ask {display}\u{2019}s IT whether your account is allowed in."),
            row: "Sign-in not accepted".into(),
        };
    }
    Why {
        title: format!("{display} didn\u{2019}t come up"),
        sentence: format!("OpenVPN 3 couldn\u{2019}t connect {display}. Its own words are below."),
        row: if at >= 2 { "Tunnel didn\u{2019}t come up".into() } else { "Connection failed".into() },
    }
}

fn explain(raw: &str, at: usize, spec: &Spec) -> Why {
    let (display, gateway) = (&spec.display, &spec.gateway);
    let r = raw.to_ascii_lowercase();
    if spec.protocol == "openvpn3" {
        return explain_openvpn3(&r, at, spec);
    }
    let unreachable = at == 0
        || [
            "connect to",
            "timed out",
            "refused",
            "resolve",
            "no route",
            "unreachable",
            "https connection",
        ]
        .iter()
        .any(|w| r.contains(w));
    if unreachable {
        return Why {
            title: format!("Couldn\u{2019}t reach {display}"),
            sentence: "The gateway didn\u{2019}t answer. It\u{2019}s either down or blocked on this network.".into(),
            row: format!("No answer from {gateway}"),
        };
    }
    if at >= 2 {
        return Why {
            title: format!("{display} didn\u{2019}t stay up"),
            sentence: format!(
                "{display} accepted the sign-in, but the tunnel didn\u{2019}t come up."
            ),
            row: "Tunnel didn\u{2019}t come up".into(),
        };
    }
    if ["certificate", "pkcs", "pass phrase", "decrypt"]
        .iter()
        .any(|w| r.contains(w))
    {
        return Why {
            title: format!("Couldn\u{2019}t sign in to {display}"),
            sentence: "The certificate didn\u{2019}t open. Its password may be wrong, or the file damaged.".into(),
            row: "Certificate didn\u{2019}t open".into(),
        };
    }
    Why {
        title: format!("Couldn\u{2019}t sign in to {display}"),
        sentence: format!(
            "{display}\u{2019}s gateway didn\u{2019}t accept the sign-in. Check what you typed, or ask {display}\u{2019}s IT whether your account is allowed in."
        ),
        row: "Sign-in not accepted".into(),
    }
}
