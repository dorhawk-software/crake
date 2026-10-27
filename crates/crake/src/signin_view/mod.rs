//! The sign-in window (design/screens/sign-in.html): the steps as they
//! happen, the gateway's questions in its own words, and what went wrong
//! when it couldn't connect.

use std::time::Duration;

/// How long "Copied" shows after Copy for helpdesk.
const COPIED_FOR: Duration = Duration::from_secs(3);

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    Context, Div, Entity, FontWeight, Stateful, Subscription, TestSupportExt, Window, div, px, svg,
};

use crate::Request;
use crate::model::Model;
use crate::signin::{Ask, Event, Prompt, Reply, Spec};
use crate::theme::{Theme, fonts};

mod explain;
mod question;
mod widgets;

// Against a fake openconnect, a shell script.
#[cfg(all(test, target_os = "linux"))]
mod tests;

/// "Connected" shows this long before the window closes.
const CONNECTED_SHOWN: Duration = Duration::from_millis(900);

use self::explain::explain;
use self::widgets::{Look, button, buttons, checkbox, link, lower_first, para, steps, title};
use crate::focus::Keys as _;

pub const WIDTH: f32 = 340.;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stage {
    Reaching,
    SigningIn,
    /// OpenVPN: finishing on the page opened in the browser (its address,
    /// and the browser's name).
    Browser {
        url: String,
        browser: String,
    },
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
            Stage::SigningIn | Stage::Browser { .. } | Stage::Asking(_) => 1,
            Stage::Connecting => 2,
            Stage::Up => 3, // literal: the fourth of the steps shown
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
    /// The trust question's "Don't connect", where the keyboard starts when
    /// the gateway's certificate changed.
    dont_connect: gpui_kit::FocusHandle,
    remember: bool,
    keep_certificate: bool,
    model: Entity<Model>,
    /// The session went to the app or craked: closing ends nothing.
    handed_over: bool,
    /// Copy for helpdesk was used, and when ("Copied" shows for a while).
    copied: Option<std::time::Instant>,
    _subscriptions: Vec<Subscription>,
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
    // OpenVPN 3 profiles run as openvpn3 sessions; OpenVPN's and IPsec's
    // sign-in is craked's questions; the rest is openconnect.
    let replies = match spec.protocol.as_str() {
        "openvpn3" => {
            crate::ovpn3::start(spec.tunnel.clone(), spec.gateway.clone(), events, requests)
        }
        p if crate::signin::craked_asks(p) => {
            let asks = cx.default_global::<crate::openvpn::Asks>();
            crate::openvpn::start(spec.tunnel.clone(), events, requests, asks)
        }
        _ => crate::signin::start(spec, events),
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
        let card_scroll = crate::controls::scroll_handle("sign-in-card", window, cx);
        let room = room(window, cx);
        let t = Theme::current(cx);
        let display = self.spec.display.clone();
        // What the card holds, all of it, however much of it shows: the
        // window is measured from it (`fit_window`).
        let mut card = div().relative().flex().flex_col().flex_shrink_0();

        card = match self.stage.clone() {
            Stage::Asking(prompt) => card.child(self.question(&prompt, &t, window, cx)),
            // "When it can't connect": what happened, the steps up to where it
            // stopped, Copy for helpdesk and Try again.
            Stage::Failed { reason, at } => {
                let why = explain(&reason, at, &self.spec);
                card.child(title(why.title))
                    .child(para(why.sentence, t.ink))
                    .child(steps(at, Some((at, why.row)), None, &self.spec, &t))
                    // openconnect's (or openvpn3's) own words, for whoever helps.
                    .child(
                        div()
                            .mt(px(12.))
                            .text_size(px(13.))
                            .text_color(t.muted)
                            .child(format!("{} said: {reason}", self.spec.program())),
                    )
                    // Beside the words it copies: the buttons' row has room
                    // for two (design/proposals/signin-error-buttons.html, A).
                    .child(
                        div()
                            .mt(px(6.))
                            .flex()
                            .items_center()
                            .child(
                                link("helpdesk", "Copy for helpdesk", &t)
                                    .keys()
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.copy_for_helpdesk(cx)),
                                    )
                                    .test_support(),
                            )
                            .when(self.copied.is_some_and(|c| c.elapsed() < COPIED_FOR), |d| {
                                d.child(crate::controls::copied(&t))
                            }),
                    )
                    .child(
                        buttons()
                            .child(div().flex_1())
                            .child(
                                button("close", "Close", Look::Text, &t)
                                    .keys()
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
                                    .test_support(),
                            )
                            .child(if reason.starts_with(crake_proto::trouble::KEY_MISMATCH) {
                                // A key the gateway turns down only IT can fix:
                                // trying again can't help, changing it can
                                // (design/proposals/ipsec-tunnel.html).
                                button("change-key", "Change the key\u{2026}", Look::Primary, &t)
                                    .keys()
                                    .on_click(cx.listener(|this, _, _, cx| this.change_key(cx)))
                                    .test_support()
                            } else {
                                button("retry", "Try again", Look::Primary, &t)
                                    .keys()
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.retry(window, cx)),
                                    )
                                    .test_support()
                            }),
                    )
            }
            // design/screens/sign-in.html, "Single sign-on".
            Stage::Browser { url, browser } => card
                .child(title(format!("Sign in to {display}")))
                .child(para(
                    "Finish in the browser tab Crake opened. This window updates by itself.".into(),
                    t.muted,
                ))
                .child(steps(
                    1,
                    None,
                    Some(format!("Waiting for you in {browser}")),
                    &self.spec,
                    &t,
                ))
                .child(
                    buttons()
                        .child(
                            button("open-again", "Open the page again", Look::Outline, &t)
                                .keys()
                                .on_click(move |_, _, cx| cx.open_url(&url))
                                .test_support(),
                        )
                        .child(div().flex_1())
                        .child(
                            button("cancel", "Cancel", Look::Text, &t)
                                .keys()
                                .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
                                .test_support(),
                        ),
                ),
            stage => card
                .child(title(format!("Sign in to {display}")))
                .child(para("This window updates by itself.".into(), t.muted))
                .child(steps(stage.step(), None, None, &self.spec, &t))
                .child(
                    buttons().child(div().flex_1()).child(
                        button("cancel", "Cancel", Look::Text, &t)
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
                            .test_support(),
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
            .on_key_down(crate::focus::tab_moves)
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" || crate::focus::closes_window(event) {
                    this.cancel(cx);
                }
            }))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(crate::controls::over(
                "card",
                // Never past its screen's bottom: a gateway's long list of
                // groups, or a long error, scrolls inside the card instead
                // (Cancel and Try again had gone below a 768 px screen,
                // 2026-10-19).
                // The card of sign-in.html: 12 px corners, a hairline, 22 px
                // inside.
                div()
                    .id("sign-in-card")
                    .relative()
                    .w(px(WIDTH))
                    .flex()
                    .flex_col()
                    .pt(px(CARD_TOP))
                    .px(px(22.))
                    .pb(px(CARD_BOTTOM))
                    .bg(t.panel)
                    .map(|card| crate::chrome::card_edge(card, t.line))
                    .text_color(t.ink)
                    .font_family(fonts::UI)
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .max_h(px(room))
                    .overflow_y_scroll()
                    .track_scroll(&card_scroll)
                    .child(card.child(fit_window(room)))
                    .test_support(),
                &card_scroll,
                window,
                cx,
            ))
    }
}

/// Least room below the window's top, and what's kept clear above the
/// screen's bottom edge (a bottom bar's place).
const ROOM_LEAST: f32 = 240.;
const SCREEN_BOTTOM: f32 = 56.;

/// What's kept clear above a Mac screen's visible bottom (the Dock is
/// already outside it).
#[cfg(target_os = "macos")]
const MAC_SCREEN_BOTTOM: f32 = 16.;

/// The card's tallest: from where the window is down to its screen's bottom.
fn room(window: &Window, cx: &gpui_kit::App) -> f32 {
    let top = f32::from(window.bounds().origin.y);
    // macOS: GPUI measures a window from the top of the screen it's on,
    // not from the main one's, so the main screen's height gave a window
    // on a taller second screen too little room and its card scrolled
    // (2026-10-26). That screen's visible part, menu bar and Dock left out.
    #[cfg(target_os = "macos")]
    if let Some(display) = window.display(cx) {
        let visible = display.visible_bounds();
        let bottom = f32::from(visible.origin.y + visible.size.height);
        return (bottom - top - MAC_SCREEN_BOTTOM).max(ROOM_LEAST);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
    crate::place::monitors()
        .first()
        .map_or_else(crate::place::max_window_height, |m| {
            (m.y + m.h - top - SCREEN_BOTTOM).max(ROOM_LEAST)
        })
}

/// The card's padding above and below what it holds.
const CARD_TOP: f32 = 22.;
const CARD_BOTTOM: f32 = 18.;

/// The window follows the card's height (it grows and shrinks downward),
/// measured from what the card holds, which never shrinks to the window.
fn fit_window(room: f32) -> impl IntoElement {
    gpui_kit::canvas(
        move |bounds, window, _| {
            let height = wanted_height(f32::from(bounds.size.height), room);
            let want = gpui_kit::size(px(WIDTH), px(height));
            let have = window.viewport_size();
            if (f32::from(have.height) - f32::from(want.height)).abs() > crate::windows::FIT_SLACK {
                window.on_next_frame(move |window, _| window.resize(want));
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// The window's height for a card holding `content` that tall: all of it
/// with the card's padding, up to `room`, and the card's hairlines
/// (`chrome::CARD_BORDERS`). The card itself shrinks to the window, so its
/// own height was the window's, not what it needs (a window that opened
/// short only crept up and stopped short on a Mac, Cancel cut in half,
/// 2026-10-26); and what GPUI says it could scroll counted the padding
/// once more, as the card's measuring canvas spans it: 38 points too
/// tall, an empty band under the card (a Mac, 0.15.4, 2026-10-27).
pub(crate) fn wanted_height(content: f32, room: f32) -> f32 {
    (content + CARD_TOP + CARD_BOTTOM).min(room) + crate::chrome::CARD_BORDERS
}

impl SignIn {
    /// Where sign-in is (journey tests).
    #[cfg(test)]
    pub(crate) fn stage(&self) -> &Stage {
        &self.stage
    }

    /// Whose sign-in it is (the journeys, Linux's and Windows').
    #[cfg(all(test, any(target_os = "linux", windows)))]
    pub(crate) fn tunnel(&self) -> &str {
        &self.spec.tunnel
    }

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
        cx.on_release(|this: &mut Self, cx| this.ended(cx)).detach();
        let subscriptions = vec![
            cx.subscribe_in(&field, window, |this, _, event: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.answer(cx);
                }
            }),
            // The tunnel coming up (or not) after sign-in.
            cx.observe(&model, |this, model, cx| {
                // OpenVPN, IPsec: craked signs in and connects; it's up
                // (or not) whatever the window shows meanwhile.
                let openvpn = crate::signin::craked_asks(&this.spec.protocol)
                    && matches!(
                        this.stage,
                        Stage::Reaching | Stage::SigningIn | Stage::Browser { .. }
                    );
                if this.stage != Stage::Connecting && !openvpn {
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
                        // Up: craked's state takes over from "signing in"
                        // now, not when the window goes (OpenVPN's and
                        // IPsec's sign-in ends here; switched off before
                        // then, craked's "off" was drawn as connecting).
                        if !this.handed_over {
                            this.handed_over = true;
                            this.signing(false, true, cx);
                        }
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(CONNECTED_SHOWN).await;
                            let _ = this.update(cx, |_, cx| cx.emit(Close));
                        })
                        .detach();
                    }
                    Some("trouble") if openvpn => {
                        let why = model
                            .read(cx)
                            .tunnels
                            .iter()
                            .find(|t| t.name == this.spec.tunnel)
                            .map(|t| t.why.clone())
                            .unwrap_or_default();
                        let at = this.stage.step();
                        this.stage = Stage::Failed { reason: why, at };
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
            copied: None,
            requests,
            replies,
            stage: Stage::Reaching,
            field,
            focus: cx.focus_handle(),
            dont_connect: cx.focus_handle(),
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
            Event::Browser(url) => {
                self.stage = Stage::Browser {
                    url,
                    browser: crate::sys::browser_name(),
                };
            }
            Event::Prompt(prompt) => {
                let secret = matches!(prompt.ask, Ask::Secret | Ask::CertificatePassword { .. });
                self.field.update(cx, |f, cx| {
                    f.set_value("", window, cx);
                    f.set_masked(secret, window, cx);
                });
                // A certificate that changed since it was trusted: the
                // keyboard starts on "Don't connect", for a second look.
                if matches!(
                    prompt.ask,
                    Ask::TrustServer {
                        trusted_on: Some(_),
                        ..
                    }
                ) {
                    window.focus(&self.dont_connect, cx);
                } else {
                    self.field.update(cx, |f, cx| f.focus(window, cx));
                }
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
            Event::Attached(path, fields) => {
                self.handed_over = true;
                // (Still "signing in" until the app has the session: then
                // nothing else takes it over meanwhile.)
                // The session stays the app's; its control goes with it.
                let _ = self.requests.send_blocking(Request::Attach(
                    self.spec.tunnel.clone(),
                    path,
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
        // "Trust it" on openconnect's question: remembered for this gateway
        // (`signin::trust`); craked keeps IPsec's and OpenVPN's itself.
        if let Stage::Asking(prompt) = &self.stage
            && let Ask::TrustServer {
                fingerprint: Some(pin),
                ..
            } = &prompt.ask
            && text == "yes"
            && self.spec.program() == "openconnect"
        {
            let trusted = crate::prefs::Trusted {
                pin: pin.clone(),
                on: jiff::Zoned::now().date().to_string(),
            };
            self.spec.trusted = Some(trusted.clone());
            let gateway = self.spec.gateway.clone();
            cx.update_global::<crate::prefs::Prefs, _>(|prefs, _| {
                prefs.trusted.insert(gateway, trusted);
                prefs.save();
            });
        }
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
        // OpenVPN, IPsec: craked runs the sign-in; switched on again, it
        // asks.
        if crate::signin::craked_asks(&self.spec.protocol) {
            let _ = self
                .requests
                .send_blocking(Request::Switch(self.spec.tunnel.clone(), true));
        }
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
        // Said, then gone: it copied (it used to copy and say nothing).
        self.copied = Some(std::time::Instant::now());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        let _ = self.replies.send_blocking(Reply::Cancel);
        // Not connecting from now on, whenever the view itself goes
        // (whoever still holds it: the journey kept Globex "connecting").
        self.ended(cx);
        cx.emit(Close);
    }

    /// The window's sign-in is over: the tunnel no longer shows as
    /// connecting, unless its session was handed over.
    fn ended(&self, cx: &mut gpui_kit::App) {
        if self.handed_over {
            return;
        }
        let tunnel = self.spec.tunnel.clone();
        self.model.update(cx, |m, cx| {
            if m.signing_in.contains(&tunnel) {
                m.signing_in(&tunnel, false, false);
                cx.notify();
            }
        });
    }

    /// The tunnel's Edit, where its key is changed; this window goes.
    fn change_key(&mut self, cx: &mut Context<Self>) {
        let _ = self
            .requests
            .send_blocking(Request::NewTunnel(Some(self.spec.tunnel.clone())));
        self.cancel(cx);
    }
}
