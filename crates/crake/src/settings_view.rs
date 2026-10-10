//! Settings (design/screens/settings.html): General, Learning, Names,
//! Privacy & log, Managed by IT, About. craked's settings go over D-Bus;
//! the app's own (appearance, notifications, the bird, start at login,
//! helpdesk copies) live in `prefs`.

use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, Stateful, Subscription, Window, div,
    px, svg,
};

use crate::Request;
use crate::model::Model;
use crate::prefs::Prefs;
use crate::theme::{Theme, fonts};

pub const WIDTH: f32 = 1040.;
pub const HEIGHT: f32 = 820.;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Tunnels,
    General,
    Learning,
    Names,
    Privacy,
    Managed,
    About,
}

impl Section {
    const ALL: [Section; 7] = [
        Self::Tunnels,
        Self::General,
        Self::Learning,
        Self::Names,
        Self::Privacy,
        Self::Managed,
        Self::About,
    ];

    /// Title, lede and icon, as in settings.html.
    fn info(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Tunnels => (
                "Tunnels",
                "The VPNs Crake can use, in the order it tries them.",
                "crake/section-tunnels.svg",
            ),
            Self::General => (
                "General",
                "How Crake starts and how it tells you things.",
                "crake/section-general.svg",
            ),
            Self::Learning => (
                "Learning",
                "How Crake decides what needs a tunnel.",
                "crake/section-learning.svg",
            ),
            Self::Names => (
                "Names",
                "Which DNS answers which names.",
                "crake/section-names.svg",
            ),
            Self::Privacy => (
                "Privacy & log",
                "What Crake remembers, and for how long.",
                "crake/section-privacy.svg",
            ),
            Self::Managed => (
                "Managed by IT",
                "Tunnels your organisation set up for you.",
                "crake/section-managed.svg",
            ),
            Self::About => (
                "About",
                // Shown with the Latin name linked (see `render`).
                "Named after the corncrake, Crex crex.",
                "crake/section-about.svg",
            ),
        }
    }
}

/// A row being dragged in "Try tunnels in this order".
#[derive(Clone)]
struct Dragged {
    from: usize,
    label: String,
    t: Theme,
}

impl Render for Dragged {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = &self.t;
        div()
            .h(px(40.))
            .w(px(300.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(10.))
            .bg(t.panel)
            .border_1()
            .border_color(t.line_strong)
            .rounded(px(6.))
            .font_family(fonts::UI)
            .text_size(px(14.))
            .text_color(t.ink)
            .font_weight(FontWeight::SEMIBOLD)
            .child(
                svg()
                    .path("crake/grip.svg")
                    .size(px(16.))
                    .text_color(t.muted),
            )
            .child(self.label.clone())
    }
}

pub struct Settings {
    model: Entity<Model>,
    requests: async_channel::Sender<Request>,
    section: Section,
    /// The select whose options are showing.
    open_select: Option<&'static str>,
    /// "+ Add" was pressed on the never list.
    adding_never: bool,
    never_input: Entity<InputState>,
    /// "Clear log…" was pressed once: ask before clearing.
    confirm_clear: bool,
    /// "Remove…" was pressed on this tunnel: ask before removing.
    removing: Option<String>,
    /// The setting whose plain-words explanation is showing (its label).
    hint: Option<&'static str>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for Settings {}

impl Settings {
    pub fn new(
        model: Entity<Model>,
        requests: async_channel::Sender<Request>,
        section: Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let never_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Host, network or *.name"));
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.observe_global::<Prefs>(|_, cx| cx.notify()),
            cx.subscribe_in(
                &never_input,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let item = input.read(cx).value().trim().to_owned();
                        if !item.is_empty() {
                            let mut list = this.model.read(cx).setting_list("never");
                            if !list.contains(&item) {
                                list.push(item);
                                this.set("never", list.join("\n"));
                            }
                        }
                        input.update(cx, |f, cx| f.set_value("", window, cx));
                        this.adding_never = false;
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
        window.focus(&focus, cx);
        Self {
            model,
            requests,
            section,
            open_select: None,
            adding_never: false,
            never_input,
            confirm_clear: false,
            removing: None,
            // CRAKE_HINT=<setting> (development): that explanation shows.
            hint: std::env::var("CRAKE_HINT")
                .ok()
                .map(|h| &*Box::leak(h.into_boxed_str())),
            focus,
            _subscriptions: subscriptions,
        }
    }

    fn set(&self, key: &str, value: String) {
        self.requests
            .send_blocking(Request::Setting(key.to_owned(), value))
            .ok();
    }

    /// Change one of the app's own preferences, and keep it.
    fn pref(cx: &mut Context<Self>, change: impl FnOnce(&mut Prefs)) {
        let prefs = cx.global_mut::<Prefs>();
        change(prefs);
        prefs.save();
    }
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let model = self.model.read(cx).clone();
        let (title, lede, _) = self.section.info();
        let body: AnyElement = match self.section {
            Section::Tunnels => self.tunnels(&model, &t, cx).into_any_element(),
            Section::General => self.general(&t, cx).into_any_element(),
            Section::Learning => self.learning(&model, &t, window, cx).into_any_element(),
            Section::Names => self.names(&model, &t, cx).into_any_element(),
            Section::Privacy => self.privacy(&model, &t, cx).into_any_element(),
            Section::Managed => managed(&model, &t).into_any_element(),
            Section::About => about(&t, cx).into_any_element(),
        };
        let failed = model
            .failed
            .as_ref()
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(8))
            .map(|(text, _)| text.clone());

        div()
            .id("settings")
            .track_focus(&self.focus)
            .size_full()
            .flex()
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
                    if this.open_select.is_some() || this.adding_never || this.confirm_clear {
                        this.open_select = None;
                        this.adding_never = false;
                        this.confirm_clear = false;
                        cx.notify();
                    } else {
                        cx.emit(Close);
                    }
                }
            }))
            .child(self.nav(&t, cx))
            .child(
                div()
                    .id("main")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .pt(px(14.))
                    .pr(px(16.))
                    .pb(px(32.))
                    .pl(px(36.))
                    .child(
                        // No title bar: the window moves by this top strip
                        // and the heading (not by its controls: a drag
                        // started there would swallow their clicks).
                        div()
                            .flex()
                            .justify_end()
                            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            })
                            .child(
                                div()
                                    .id("close")
                                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
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
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Close))),
                            ),
                    )
                    .child(
                        div()
                            .max_w(px(660.))
                            .pr(px(20.))
                            .child(
                                div()
                                    .text_size(px(26.))
                                    .line_height(px(38.))
                                    .font_weight(FontWeight::BOLD)
                                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                                        window.start_window_move()
                                    })
                                    .child(title),
                            )
                            .child(if self.section == Section::About {
                                // The corncrake's Latin name is its call.
                                div()
                                    .mt(px(4.))
                                    .flex()
                                    .text_color(t.muted)
                                    .child("Named after the corncrake,\u{a0}")
                                    .child(
                                        div()
                                            .id("crex")
                                            .italic()
                                            .underline()
                                            .text_color(t.accent)
                                            .cursor_pointer()
                                            .child("Crex crex")
                                            .on_click(|_, _, cx| {
                                                cx.open_url(
                                                    "https://en.wikipedia.org/wiki/Corn_crake",
                                                )
                                            }),
                                    )
                                    .child(".")
                            } else {
                                div().mt(px(4.)).text_color(t.muted).child(lede)
                            })
                            .child(body)
                            .when_some(failed, |c, text| {
                                c.child(
                                    div()
                                        .mt(px(14.))
                                        .text_size(px(13.))
                                        .text_color(t.danger)
                                        .child(text),
                                )
                            }),
                    ),
            )
    }
}

/// What a setting means, in plain words with an example: shown from the
/// "?" next to its name. Paragraphs are separated by a blank line.
fn help(label: &str) -> Option<&'static str> {
    Some(match label {
        "Start when you log in" => {
            "When you log in to your computer, Crake starts by itself: the bird is there, ready. It doesn't connect anything on its own unless you also switch on the setting below it."
        }
        "Switch on the tunnels you left on" => {
            "At login, Crake switches back on the VPNs that were on when you last used it.\n\nExample: ACME was on when you shut down yesterday, so it comes back on today. You may be asked to sign in again.\n\nOff: Crake starts with every VPN off, and you switch on what you need."
        }
        "Show the bird in the top bar" => {
            "The little bird next to the clock is how you open Crake and see what it's doing at a glance.\n\nHide it and Crake keeps working; start Crake again from your apps to open it."
        }
        "When Crake routes a host" => {
            "Each time Crake decides something needs a VPN, it can tell you.\n\nExample: you open wiki.acme.internal. Crake sends it through ACME and says \u{201c}Sent wiki.acme.internal through ACME\u{201d}, with Undo."
        }
        "Wait for a direct answer" => {
            "When you open something, Crake first tries it the normal way, without a VPN. If nothing answers in this time, it tries your VPNs.\n\nExample: with 3 s, a company server that stays silent for 3 seconds gets tried through ACME.\n\nShorter: things open sooner. Longer: slow public sites are less likely to end up in a VPN by mistake."
        }
        "Try tunnels in this order" => {
            "With several VPNs on, Crake asks them one after another, from the top. Drag a VPN to move it.\n\nExample: put your work VPN first if most of what you open is for work."
        }
        "When two tunnels answer" => {
            "Sometimes two VPNs can both reach the same thing.\n\nAsk me: Crake uses the first in the list and asks whether you'd rather use the other.\nFaster: whichever answered first.\nFirst in order: always the one higher in the list."
        }
        "Forget hosts unused for" => {
            "Crake remembers what needs a VPN. If you don't use something for this long, Crake forgets it, and next time tries it the normal way first.\n\nExample: with 7 days, a server you used for one project is forgotten a week later."
        }
        "Never send through a tunnel" => {
            "Things that always go the normal way, never through a VPN, even if they don't answer.\n\nExamples: your bank, *.mybank.cz (the * means \u{201c}anything ending in\u{201d}), or your home network, 192.168.1.0/24."
        }
        "Ask a tunnel\u{2019}s DNS for these names" => {
            "DNS is the internet's phone book: it turns a name like wiki.acme.internal into an address.\n\nYour VPNs bring their own phone book for company names. Names ending like these are looked up in that VPN's book."
        }
        "Keep internal names private" => {
            "When a VPN is off, your internet provider doesn't get to see your company's internal names.\n\nExample: with ACME off, looking up git.acme.internal simply fails at once instead of asking your provider."
        }
        "Ask every tunnel when a name is unknown" => {
            "If the normal phone book says a name doesn't exist, Crake asks your VPNs' phone books too. That's how it finds company names nobody told it about.\n\nIt also asks a company's own phone book about the company's public names, in case there's an inside address. Example: portal.acme.example works from inside ACME only."
        }
        "Keep the log for" => {
            "Crake notes every decision: what went through which VPN, and why. This is how long it keeps those notes. They stay on this computer."
        }
        "Hide host names in helpdesk copies" => {
            "\u{201c}Copy for helpdesk\u{201d} copies a summary you can send to your IT helpdesk. With this on, server names become \u{201c}a host\u{201d}: you share which VPN and how fast, not where you went."
        }
        _ => return None,
    })
}

impl Settings {
    /// A setting's name, with a "?" that explains it in plain words and an
    /// example (hover or click it).
    fn title(&self, label: &'static str, t: &Theme, cx: &Context<Self>) -> Div {
        let row = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(label);
        let Some(text) = help(label) else {
            return row;
        };
        let open = self.hint == Some(label);
        let mut q = div().relative().flex_none().child(
            div()
                .id(label)
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .cursor_pointer()
                .child(
                    svg()
                        .path("crake/help.svg")
                        .size(px(15.))
                        .text_color(if open { t.accent } else { t.muted }),
                )
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if *hovered {
                        this.hint = Some(label);
                    } else if this.hint == Some(label) {
                        this.hint = None;
                    }
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.hint = if this.hint == Some(label) {
                        None
                    } else {
                        Some(label)
                    };
                    cx.notify();
                })),
        );
        if open {
            let mut card = div()
                .absolute()
                .top(px(24.))
                .left(px(-10.))
                .w(px(340.))
                .py(px(10.))
                .px(px(12.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .bg(t.panel)
                .border_1()
                .border_color(t.line_strong)
                .rounded(px(8.))
                .font_weight(FontWeight::NORMAL)
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(t.ink);
            for para in text.split("\n\n") {
                card = card.child(div().child(para.to_owned()));
            }
            q = q.child(gpui_kit::deferred(card.occlude()).with_priority(2));
        }
        row.child(q)
    }

    fn nav(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = div()
            .id("nav")
            .w(px(222.))
            .flex_none()
            .h_full()
            .pt(px(16.))
            .px(px(12.))
            .bg(t.surface)
            // GPUI clips to a rectangle, not the card's rounded corners: the
            // nav rounds its own (the card's 12 px less its 1 px border).
            .rounded_l(px(11.))
            .border_r_1()
            .border_color(t.line)
            .child(
                div()
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                        window.start_window_move()
                    })
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pt(px(4.))
                    .px(px(10.))
                    .pb(px(16.))
                    .child(bird(22., "crake/bird.svg", "crake/call.svg", t))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::BOLD)
                            .child("Settings"),
                    ),
            );
        for (i, section) in Section::ALL.into_iter().enumerate() {
            let (label, _, icon) = section.info();
            let current = self.section == section;
            nav = nav.child(
                div()
                    .id(("section", i))
                    .w_full()
                    .h(px(40.))
                    .px(px(10.))
                    .mb(px(2.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .rounded(px(6.))
                    .when(current, |b| b.bg(t.panel).font_weight(FontWeight::BOLD))
                    .when(!current, |b| b.font_weight(FontWeight::MEDIUM))
                    .cursor_pointer()
                    .child(svg().path(icon).size(px(18.)).text_color(t.ink))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.section = section;
                        this.open_select = None;
                        this.adding_never = false;
                        this.confirm_clear = false;
                        cx.notify();
                    })),
            );
        }
        nav
    }

    /// Settings → Tunnels (design/proposals/settings-tunnels.html): every
    /// tunnel in the order Crake tries them, then the full tunnels that take
    /// everything else; Edit and Remove… for those added here.
    fn tunnels(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let tried: Vec<crate::client::Tunnel> = model.try_order().into_iter().cloned().collect();
        let full: Vec<crate::client::Tunnel> = model
            .tunnels
            .iter()
            .filter(|x| x.full_tunnel)
            .cloned()
            .collect();
        let names: Vec<String> = tried.iter().map(|x| x.name.clone()).collect();
        let mut list = group(t).mt(px(22.));
        if tried.is_empty() && full.is_empty() {
            list = list.child(
                div()
                    .py(px(14.))
                    .px(px(16.))
                    .text_color(t.muted)
                    .child("No tunnels yet. Add the first one below."),
            );
        }
        for (i, tunnel) in tried.iter().enumerate() {
            let names = names.clone();
            let hover = t.accent_soft;
            list = list.child(
                self.tunnel_row(tunnel, Some(i), t, cx)
                    .when(i > 0, |r| r.border_t_1().border_color(t.line))
                    .on_drag(
                        Dragged {
                            from: i,
                            label: tunnel.display.clone(),
                            t: *t,
                        },
                        |d, _, _, cx| cx.new(|_| d.clone()),
                    )
                    .drag_over::<Dragged>(move |s, _, _, _| s.bg(hover))
                    .on_drop(cx.listener(move |this, d: &Dragged, _, _| {
                        let mut names = names.clone();
                        let moved = names.remove(d.from);
                        names.insert(i, moved);
                        this.set("order", names.join("\n"));
                    })),
            );
            list = list.children(self.confirm_remove(tunnel, t, cx));
        }
        if !full.is_empty() {
            list = list.child(
                div()
                    .pt(px(10.))
                    .pb(px(6.))
                    .px(px(16.))
                    .when(!tried.is_empty(), |d| d.border_t_1().border_color(t.line))
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.muted)
                    .child("Everything else goes through:"),
            );
            for tunnel in &full {
                list = list.child(self.tunnel_row(tunnel, None, t, cx));
                list = list.children(self.confirm_remove(tunnel, t, cx));
            }
        }
        div()
            .child(list)
            .child(
                div().flex().child(
                    div()
                        .id("new-tunnel")
                        .mt(px(16.))
                        .h(px(38.))
                        .px(px(14.))
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .bg(t.accent)
                        .text_color(t.accent_ink)
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .child("+ New tunnel")
                        .on_click(cx.listener(|this, _, _, _| {
                            this.requests.send_blocking(Request::NewTunnel(None)).ok();
                        })),
                ),
            )
            .when(tried.len() > 1, |d| {
                d.child(div().mt(px(10.)).child(sub("Drag to change the order.", t)))
            })
    }

    /// One tunnel: grip and number (when it's tried in order), name, gateway
    /// and protocol, and who may change it.
    fn tunnel_row(
        &self,
        tunnel: &crate::client::Tunnel,
        position: Option<usize>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let protocol = match tunnel.protocol.as_str() {
            "anyconnect" => "AnyConnect",
            "gp" => "GlobalProtect",
            "pulse" => "Ivanti Connect Secure",
            "nc" => "Network Connect",
            "fortinet" => "Fortinet SSL VPN",
            "f5" => "F5 BIG-IP",
            "array" => "Array Networks",
            "openvpn3" => "OpenVPN 3",
            other => other,
        }
        .to_owned();
        let meta = if tunnel.gateway.is_empty() {
            protocol
        } else {
            format!("{} \u{b7} {protocol}", tunnel.gateway)
        };
        let right: AnyElement = match tunnel.source.as_str() {
            "app" => {
                let (edit, remove) = (tunnel.name.clone(), tunnel.name.clone());
                div()
                    .flex()
                    .gap(px(8.))
                    .flex_none()
                    .child(
                        small_button(
                            ("edit", position.unwrap_or(99)),
                            "Edit",
                            None,
                            t.line,
                            t.ink,
                            t,
                        )
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.requests
                                .send_blocking(Request::NewTunnel(Some(edit.clone())))
                                .ok();
                        })),
                    )
                    .child(
                        small_button(
                            ("remove", position.unwrap_or(99)),
                            "Remove\u{2026}",
                            Some("crake/trash.svg"),
                            t.danger,
                            t.danger,
                            t,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.removing = Some(remove.clone());
                            cx.notify();
                        })),
                    )
                    .into_any_element()
            }
            "managed" => div()
                .flex_none()
                .text_size(px(13.))
                .text_color(t.muted)
                .child(format!("Set up by {}", tunnel.organisation))
                .into_any_element(),
            _ => div()
                .flex_none()
                .text_size(px(13.))
                .text_color(t.muted)
                .child("From the config file")
                .into_any_element(),
        };
        div()
            .id(gpui_kit::ElementId::Name(format!("tunnel-{}", tunnel.name).into()))
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(12.))
            .px(px(16.))
            .when(position.is_some(), |r| r.cursor_grab())
            .child(match position {
                Some(_) => svg()
                    .path("crake/grip.svg")
                    .size(px(16.))
                    .flex_none()
                    .text_color(t.muted)
                    .into_any_element(),
                None => div().w(px(16.)).flex_none().into_any_element(),
            })
            .child(
                div()
                    .w(px(10.))
                    .flex_none()
                    .font_family(fonts::MONO)
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child(position.map(|i| (i + 1).to_string()).unwrap_or_default()),
            )
            .child(
                // Its name and gateway open tunnel detail.
                div()
                    .id(gpui_kit::ElementId::Name(format!("open-{}", tunnel.name).into()))
                    .flex_1()
                    .min_w_0()
                    .cursor_pointer()
                    .on_click({
                        let name = tunnel.name.clone();
                        cx.listener(move |this, _, _, _| {
                            this.requests
                                .send_blocking(Request::OpenDetail(name.clone()))
                                .ok();
                        })
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(tunnel.display.clone()),
                            )
                            .when(tunnel.managed, |d| {
                                d.child(
                                    div()
                                        .px(px(7.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .bg(t.surface)
                                        .text_size(px(12.))
                                        .line_height(px(16.))
                                        .text_color(t.muted)
                                        .child("Managed"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .mt(px(1.))
                            .truncate()
                            .font_family(fonts::MONO)
                            .text_size(px(12.))
                            .text_color(t.muted)
                            .child(meta),
                    )
                    .when(position.is_none(), |d| {
                        d.child(sub(
                            "A full tunnel: while it\u{2019}s on, whatever the tunnels above don\u{2019}t take goes through it.",
                            t,
                        ))
                    }),
            )
            .child(right)
            // Opens tunnel detail too: says the row leads somewhere.
            .child(
                div()
                    .id(gpui_kit::ElementId::Name(format!("chevron-{}", tunnel.name).into()))
                    .size(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.surface))
                    .child(
                        svg()
                            .path("crake/chevron-right.svg")
                            .size(px(16.))
                            .text_color(t.muted),
                    )
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation()
                    })
                    .on_click({
                        let name = tunnel.name.clone();
                        cx.listener(move |this, _, _, _| {
                            this.requests
                                .send_blocking(Request::OpenDetail(name.clone()))
                                .ok();
                        })
                    }),
            )
    }

    /// "Remove Home lab?" under its row, after Remove… was pressed.
    fn confirm_remove(
        &self,
        tunnel: &crate::client::Tunnel,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        (self.removing.as_deref() == Some(tunnel.name.as_str())).then(|| {
            let name = tunnel.name.clone();
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .px(px(16.))
                .border_t_1()
                .border_color(t.line)
                .bg(t.surface)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .child(format!("Remove {}?", tunnel.display)),
                        )
                        .child(sub(
                            "What Crake learned for it and its saved sign-in go too.",
                            t,
                        )),
                )
                .child(
                    small_button("keep-tunnel", "Keep it", None, t.line, t.ink, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.removing = None;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    small_button(
                        "remove-tunnel",
                        "Remove",
                        Some("crake/trash.svg"),
                        t.danger,
                        t.panel,
                        t,
                    )
                    .bg(t.danger)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.requests
                            .send_blocking(Request::RemoveTunnel(name.clone()))
                            .ok();
                        this.removing = None;
                        cx.notify();
                    })),
                )
        })
    }

    fn general(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = cx.global::<Prefs>().clone();
        let starts = crate::autostart::enabled();
        let notify = select(
            self,
            "notify",
            221.,
            &[
                ("none", "Say nothing"),
                ("quiet", "Quiet notification"),
                ("crex", "Notification and a \u{201c}crex crex\u{201d}"),
            ],
            &prefs.notify,
            t,
            cx,
            |_, value, cx| Self::pref(cx, |p| p.notify = value),
        );
        let appearance = select(
            self,
            "appearance",
            153.,
            &[
                ("system", "Follow the system"),
                ("light", "Light"),
                ("dark", "Dark"),
            ],
            &prefs.appearance,
            t,
            cx,
            |_, value, cx| {
                Self::pref(cx, |p| p.appearance = value);
                Theme::current(cx).sync_components(cx);
                cx.refresh_windows();
            },
        );
        div()
            .child(
                group(t)
                    .mt(px(22.))
                    .child(row(
                        self.title("Start when you log in", t, cx),
                        Some("The bird is there when you need it."),
                        toggle("startup", starts, t).on_click(cx.listener(move |_, _, _, cx| {
                            crate::autostart::set(!starts);
                            cx.notify();
                        })),
                        false,
                        t,
                    ))
                    // Only means something when Crake starts at login.
                    .child(
                        div()
                            .pl(px(24.))
                            .border_t_1()
                            .border_color(t.line)
                            .when(!starts, |r| r.opacity(0.45))
                            .child(row(
                                self.title("Switch on the tunnels you left on", t, cx),
                                Some("Some may ask you to sign in."),
                                toggle("reconnect", starts && prefs.reconnect, t).when(
                                    starts,
                                    |sw| {
                                        sw.on_click(cx.listener(move |_, _, _, cx| {
                                            Self::pref(cx, |p| p.reconnect = !prefs.reconnect);
                                        }))
                                    },
                                ),
                                false,
                                t,
                            )),
                    )
                    .child(row(
                        self.title("Show the bird in the top bar", t, cx),
                        Some("Without it, open Crake from your apps."),
                        toggle("bird", prefs.show_bird, t).on_click(cx.listener(
                            move |this, _, _, cx| {
                                let on = !prefs.show_bird;
                                Self::pref(cx, |p| p.show_bird = on);
                                this.requests.send_blocking(Request::ShowBird(on)).ok();
                            },
                        )),
                        true,
                        t,
                    ))
                    .child(row(
                        self.title("When Crake routes a host", t, cx),
                        Some("Undo stays in the panel either way."),
                        notify,
                        true,
                        t,
                    )),
            )
            .child(group(t).mt(px(16.)).child(row(
                self.title("Appearance", t, cx),
                None,
                appearance,
                false,
                t,
            )))
    }

    fn learning(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = window;
        // Wait for a direct answer: 1, 2, 3 or 5 s.
        let wait = model.setting("probe_wait_secs").to_owned();
        let values = ["1", "2", "3", "5"];
        let chosen = values.iter().position(|v| *v == wait);
        let mut waits = div().flex().mt(px(10.));
        for (i, n) in values.into_iter().enumerate() {
            waits = waits.child(
                crate::controls::segment(("wait", i), i, values.len(), chosen, 36., t)
                    .min_w(px(56.))
                    .px(px(12.))
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .child(format!("{n} s"))
                    .on_click(cx.listener(move |this, _, _, _| {
                        this.set("probe_wait_secs", n.to_owned());
                    })),
            );
        }

        let both = select(
            self,
            "both",
            170.,
            &[
                ("ask", "Ask me"),
                ("faster", "Use the faster one"),
                ("first", "Use the first in order"),
            ],
            model.setting("both"),
            t,
            cx,
            |this, value, _| this.set("both", value),
        );
        let forget = select(
            self,
            "forget",
            92.,
            &[
                ("1", "1 day"),
                ("7", "7 days"),
                ("30", "30 days"),
                ("never", "Never"),
            ],
            model.setting("forget_after_days"),
            t,
            cx,
            |this, value, _| this.set("forget_after_days", value),
        );

        // Never send through a tunnel: chips, "+ Add".
        let never = model.setting_list("never");
        let mut chips = div().flex().flex_wrap().gap(px(6.)).mt(px(10.));
        for (i, item) in never.iter().enumerate() {
            let rest: Vec<String> = never
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, v)| v.clone())
                .collect();
            chips = chips.child(
                div()
                    .h(px(30.))
                    .pl(px(10.))
                    .pr(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(4.))
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .child(item.clone())
                    .child(
                        div()
                            .id(("remove", i))
                            .size(px(22.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(t.surface))
                            .child(
                                svg()
                                    .path("crake/close.svg")
                                    .size(px(11.))
                                    .text_color(t.muted),
                            )
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.set("never", rest.join("\n"));
                            })),
                    ),
            );
        }
        if self.adding_never {
            chips = chips.child(
                div()
                    .w(px(240.))
                    .h(px(30.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(t.accent)
                    .rounded(px(4.))
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .child(Input::new(&self.never_input).appearance(false)),
            );
        } else {
            chips = chips.child(
                div()
                    .id("add-never")
                    .h(px(30.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_dashed()
                    .border_color(t.line_strong)
                    .rounded(px(4.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child("+ Add")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.adding_never = true;
                        this.never_input.update(cx, |f, cx| f.focus(window, cx));
                        cx.notify();
                    })),
            );
        }

        div()
            .child(
                group(t)
                    .mt(px(22.))
                    .child(
                        div()
                            .py(px(14.))
                            .px(px(16.))
                            .child(self.title("Wait for a direct answer", t, cx))
                            .child(sub(
                                "Shorter feels faster. Longer avoids routing slow but public servers.",
                                t,
                            ))
                            .child(div().flex().child(waits)),
                    )
                    .child(row(self.title("When two tunnels answer", t, cx), None, both, true, t))
                    .child(row(self.title("Forget hosts unused for", t, cx), None, forget, true, t)),
            )
            .child(
                group(t)
                    .mt(px(16.))
                    .py(px(14.))
                    .px(px(16.))
                    .child(self.title("Never send through a tunnel", t, cx))
                    .child(sub(
                        "Personal banking, home devices. Crake leaves these alone even if they don\u{2019}t answer.",
                        t,
                    ))
                    .child(chips),
            )
    }

    fn names(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut chips = div().flex().flex_wrap().gap(px(6.)).mt(px(10.));
        let mut any = false;
        for tunnel in &model.tunnels {
            for d in &tunnel.dns_domains {
                any = true;
                chips = chips.child(
                    div()
                        .h(px(28.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .bg(t.surface)
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .child(format!("*.{d} \u{2192} {}", tunnel.display)),
                );
            }
        }
        if !any {
            chips = chips.child(
                div()
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child("None yet. Gateways push them when they connect."),
            );
        }
        let private = model.setting("keep_private") == "true";
        let ask = model.setting("ask_unknown") == "true";
        group(t)
            .mt(px(22.))
            .child(
                div()
                    .py(px(14.))
                    .px(px(16.))
                    .child(self.title("Ask a tunnel\u{2019}s DNS for these names", t, cx))
                    .child(sub("Pushed by your gateways when they connect.", t))
                    .child(chips),
            )
            .child(row(
                self.title("Keep internal names private", t, cx),
                Some("Names like *.acme.internal never go to your internet provider\u{2019}s DNS."),
                toggle("private", private, t).on_click(cx.listener(move |this, _, _, _| {
                    this.set("keep_private", (!private).to_string());
                })),
                true,
                t,
            ))
            .child(row(
                self.title("Ask every tunnel when a name is unknown", t, cx),
                Some("How Crake finds internal names nobody told it about."),
                toggle("ask-unknown", ask, t).on_click(cx.listener(move |this, _, _, _| {
                    this.set("ask_unknown", (!ask).to_string());
                })),
                true,
                t,
            ))
    }

    fn privacy(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let hide = cx.global::<Prefs>().hide_hosts;
        let keep = select(
            self,
            "keep",
            142.,
            &[
                ("1", "1 day"),
                ("7", "7 days"),
                ("30", "30 days"),
                ("never", "Don\u{2019}t keep a log"),
            ],
            model.setting("log_days"),
            t,
            cx,
            |this, value, _| this.set("log_days", value),
        );
        let clear: AnyElement = if self.confirm_clear {
            div()
                .flex()
                .gap(px(8.))
                .child(
                    outline("cancel-clear", "Keep it", t.line_strong, t.ink, None).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.confirm_clear = false;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    outline(
                        "clear",
                        "Clear log",
                        t.danger,
                        t.panel,
                        Some("crake/broom.svg"),
                    )
                    .bg(t.danger)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.requests.send_blocking(Request::ClearLog).ok();
                        this.confirm_clear = false;
                        cx.notify();
                    })),
                )
                .into_any_element()
        } else {
            outline(
                "clear-log",
                "Clear log\u{2026}",
                t.danger,
                t.danger,
                Some("crake/broom.svg"),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.confirm_clear = true;
                cx.notify();
            }))
            .into_any_element()
        };
        div()
            .child(
                div()
                    .mt(px(22.))
                    .flex()
                    .gap(px(12.))
                    .items_start()
                    .py(px(14.))
                    .px(px(16.))
                    .rounded(px(8.))
                    .bg(t.accent_soft)
                    .child(
                        svg()
                            .path("crake/section-privacy.svg")
                            .size(px(20.))
                            .flex_none()
                            .mt(px(1.))
                            .text_color(t.accent),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Everything Crake decides stays on this machine."),
                            )
                            .child(sub(
                                "It sends nothing to anyone, including us. Your tunnels\u{2019} gateways see what passes through them, as with any VPN.",
                                t,
                            )),
                    ),
            )
            .child(
                group(t)
                    .mt(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .py(px(14.))
                            .px(px(16.))
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.title("Keep the log for", t, cx))
                                    .child(
                                        div()
                                            .font_family(fonts::MONO)
                                            .text_size(px(12.))
                                            .text_color(t.muted)
                                            .child("~/.local/share/crake/log"),
                                    ),
                            )
                            .child(keep),
                    )
                    .child(row(self.title("Hide host names in helpdesk copies", t, cx),
                        Some("\u{201c}Copy for helpdesk\u{201d} replaces names with tunnel and timing only."),
                        toggle("hide-hosts", hide, t).on_click(cx.listener(move |_, _, _, cx| {
                            Self::pref(cx, |p| p.hide_hosts = !hide);
                        })),
                        true,
                        t,
                    ))
                    .child(row(self.title("Clear the log", t, cx),
                        Some("Learned hosts stay. Only the history goes."),
                        clear,
                        true,
                        t,
                    )),
            )
    }
}

/// A bordered group of rows.
fn group(t: &Theme) -> Div {
    div().border_1().border_color(t.line).rounded(px(8.))
}

fn sub(text: &str, t: &Theme) -> impl IntoElement {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(t.muted)
        .child(text.to_owned())
}

/// A setting: its label and explanation on the left, the control on the right.
fn row(
    label: impl IntoElement,
    explain: Option<&str>,
    control: impl IntoElement,
    divided: bool,
    t: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .py(px(14.))
        .px(px(16.))
        .when(divided, |r| r.border_t_1().border_color(t.line))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(label))
                .when_some(explain, |c, e| c.child(sub(e, t))),
        )
        .child(div().flex_none().child(control))
}

fn toggle(id: &'static str, on: bool, t: &Theme) -> Stateful<Div> {
    crate::panel::switch(id, on, t)
}

/// A button with a coloured outline.
fn outline(
    id: &'static str,
    label: &'static str,
    border: gpui_kit::Rgba,
    ink: gpui_kit::Rgba,
    icon: Option<&'static str>,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(38.))
        .px(px(14.))
        .flex()
        .items_center()
        .gap(px(8.))
        .when_some(icon, |b, path| {
            b.child(svg().path(path).size(px(16.)).text_color(ink))
        })
        .border_1()
        .border_color(border)
        .rounded(px(6.))
        .text_color(ink)
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .child(label)
}

/// A 32 px button with an outline (Edit, Remove…), maybe with an icon.
fn small_button(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    icon: Option<&'static str>,
    border: gpui_kit::Rgba,
    ink: gpui_kit::Rgba,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(32.))
        .px(px(12.))
        .flex()
        .items_center()
        .gap(px(6.))
        .border_1()
        .border_color(border)
        .rounded(px(6.))
        .bg(t.panel)
        .text_color(ink)
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
        .when_some(icon, |b, path| {
            b.child(svg().path(path).size(px(14.)).text_color(ink))
        })
        .child(label)
}

/// A drop-down like the design's `<select>`: 38 px, its options below.
#[allow(clippy::too_many_arguments)] // one control's whole description
fn select(
    this: &Settings,
    id: &'static str,
    width: f32,
    options: &[(&'static str, &'static str)],
    current: &str,
    t: &Theme,
    cx: &mut Context<Settings>,
    pick: impl Fn(&mut Settings, String, &mut Context<Settings>) + Clone + 'static,
) -> Div {
    let chosen = options
        .iter()
        .find(|(v, _)| *v == current)
        .map_or(current.to_owned(), |(_, l)| (*l).to_owned());
    let open = this.open_select == Some(id);
    let mut picker = div().relative().child(
        div()
            .id(id)
            .w(px(width))
            .h(px(38.))
            .pl(px(14.))
            .pr(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .border_1()
            .border_color(t.line_strong)
            .rounded(px(6.))
            .bg(t.panel)
            .cursor_pointer()
            .child(chosen)
            .child(
                svg()
                    .path("crake/chevron-down.svg")
                    .size(px(14.))
                    .text_color(t.ink),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_select = if open { None } else { Some(id) };
                cx.notify();
            })),
    );
    if open {
        let mut menu = div()
            .absolute()
            .top(px(42.))
            .right_0()
            .min_w(px(180.))
            .py(px(4.))
            .bg(t.panel)
            .border_1()
            .border_color(t.line_strong)
            .rounded(px(6.));
        for (i, (value, label)) in options.iter().enumerate() {
            let on = *value == current;
            let pick = pick.clone();
            let value = (*value).to_owned();
            menu = menu.child(
                div()
                    .id((id, i))
                    .px(px(10.))
                    .py(px(6.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .when(on, |b| b.bg(t.accent_soft))
                    .hover(|s| s.bg(t.surface))
                    .child(*label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_select = None;
                        pick(this, value.clone(), cx);
                        cx.notify();
                    })),
            );
        }
        picker = picker.child(gpui_kit::deferred(menu.occlude()).with_priority(1));
    }
    picker
}

/// The line bird with its teal call marks.
fn bird(size: f32, body: &'static str, call: &'static str, t: &Theme) -> impl IntoElement {
    div()
        .relative()
        .size(px(size))
        .flex_none()
        .child(svg().path(body).size(px(size)).text_color(t.ink).absolute())
        .child(
            svg()
                .path(call)
                .size(px(size))
                .text_color(t.accent)
                .absolute(),
        )
}

fn managed(model: &Model, t: &Theme) -> impl IntoElement {
    let managed: Vec<_> = model.tunnels.iter().filter(|x| x.managed).collect();
    let mono = |s: &str| {
        div()
            .font_family(fonts::MONO)
            .text_size(px(13.))
            .child(s.to_owned())
    };
    let mut list = group(t).mt(px(22.));
    if managed.is_empty() {
        list = list.child(
            div()
                .py(px(14.))
                .px(px(16.))
                .text_color(t.muted)
                .child("Your organisation hasn\u{2019}t set up any tunnels here."),
        );
    }
    for (i, tunnel) in managed.into_iter().enumerate() {
        // "Learning allowed for *.acme.internal and 10.40.0.0/16."
        let what: AnyElement = if tunnel.full_tunnel {
            div()
                .child("Full tunnel required. Crake sends everything through and learns nothing.")
                .into_any_element()
        } else if tunnel.learn.is_empty() {
            div()
                .child("Crake may learn anything that needs this tunnel.")
                .into_any_element()
        } else {
            let mut line = div()
                .flex()
                .flex_wrap()
                .items_baseline()
                .gap(px(4.))
                .child("Learning allowed for");
            let n = tunnel.learn.len();
            for (j, item) in tunnel.learn.iter().enumerate() {
                // Commas and the period sit right after the name.
                let tail = match n - j {
                    1 => ".",
                    2 => "",
                    _ => ",",
                };
                line = line.child(div().flex().items_baseline().child(mono(item)).child(tail));
                if n - j == 2 {
                    line = line.child("and");
                }
            }
            line.into_any_element()
        };
        list = list.child(
            div()
                .py(px(14.))
                .px(px(16.))
                .when(i > 0, |b| b.border_t_1().border_color(t.line))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::BOLD)
                                .child(tunnel.display.clone()),
                        )
                        .child(
                            div()
                                .px(px(7.))
                                .py(px(1.))
                                .rounded(px(4.))
                                .bg(t.surface)
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(t.muted)
                                .child("Managed"),
                        ),
                )
                .child(div().mt(px(4.)).child(what))
                .child(
                    div()
                        .mt(px(4.))
                        .font_family(fonts::MONO)
                        .text_size(px(12.))
                        .text_color(t.muted)
                        .child(tunnel.managed_file.clone()),
                ),
        );
    }
    div().child(list).child(
        div()
            .mt(px(16.))
            .text_color(t.muted)
            .child("You can switch managed tunnels on and off, but not change them. Your organisation gets nothing from Crake itself."),
    )
}

fn about(t: &Theme, cx: &mut Context<Settings>) -> impl IntoElement {
    let link = |id: &'static str, label: &'static str, url: &'static str| {
        div()
            .id(id)
            .text_color(t.accent)
            .font_weight(FontWeight::SEMIBOLD)
            .underline()
            .cursor_pointer()
            .child(label)
            .on_click(move |_, _, cx| cx.open_url(url))
    };
    let _ = cx;
    div()
        .child(
            div()
                .mt(px(22.))
                .flex()
                .gap(px(16.))
                .items_center()
                .child(bird(56., "crake/bird-thin.svg", "crake/call-thin.svg", t))
                .child(
                    div()
                        .child(
                            div()
                                .text_size(px(20.))
                                .line_height(px(28.))
                                .font_weight(FontWeight::BOLD)
                                .child(format!("Crake {}", env!("CARGO_PKG_VERSION"))),
                        )
                        .child(
                            div()
                                .text_color(t.muted)
                                .child("A VPN client that routes only what needs routing. Built on openconnect."),
                        )
                        .child(
                            div()
                                .text_color(t.muted)
                                .child("Because a VPN shouldn\u{2019}t be a chore."),
                        ),
                ),
        )
        .child(
            div()
                .mt(px(18.))
                .flex()
                .flex_wrap()
                .gap(px(10.))
                .child(link("github", "Source on GitHub", "https://github.com/dorhawk-software"))
                .child(div().text_color(t.muted).child("\u{b7}"))
                .child(link("site", "dorhawk.software", "https://dorhawk.software/")),
        )
}
