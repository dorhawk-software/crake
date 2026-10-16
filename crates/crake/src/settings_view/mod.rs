//! Settings (design/screens/settings.html): General, Learning, Names,
//! Privacy & log, Managed by IT, About. craked's settings go over D-Bus;
//! the app's own (appearance, notifications, the bird, start at login,
//! helpdesk copies) live in `prefs`.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, Pixels, Stateful, Subscription,
    TestSupportExt, Window, div, px, svg,
};

use crate::Request;
use crate::model::Model;
use crate::prefs::Prefs;
use crate::theme::{Theme, fonts};

mod about;
mod general;
mod learning;
mod managed;
mod names;
mod privacy;
mod tray_row;
mod tunnels;
mod widgets;

#[cfg(test)]
mod tests;

// What the sections share.
use self::about::{about, bird};
use self::managed::managed;
use self::widgets::{group, outline, row, row_below, select, small_button, sub, toggle};
use crate::focus::Keys as _;

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

/// Above the sidebar's heading (settings.html), where no window buttons sit.
const NAV_TOP: f32 = 16.;

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
            .filter(|(_, at)| at.elapsed() < crate::model::FAILURE_SHOWN)
            .map(|(text, _)| text.clone());

        let system = crate::chrome::system_buttons(cx);
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
            .on_key_down(crate::focus::tab_moves)
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
                        // With macOS's own buttons (`chrome`) it stays, empty:
                        // the page keeps its place.
                        div()
                            .h(px(32.))
                            .flex()
                            .justify_end()
                            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            })
                            .when(!system, |strip| {
                                strip.child(
                                    crate::chrome::close_button(&t)
                                        .keys()
                                        .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))
                                        .test_support(),
                                )
                            }),
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
                                            .keys()
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
        "Tell me when there\u{2019}s a new version" => {
            "Once a day Crake asks dorhawk.software which version is the newest. It sends nothing about you, this computer or your tunnels; the site sees an address asking, as with any web page.\n\nWhen a newer Crake is out, the panel says so, with its download page.\n\nOff: nothing is asked; check the website yourself."
        }
        "Start when you log in" => {
            "When you log in to your computer, Crake starts by itself: the bird is there, ready. It doesn't connect anything on its own unless you also switch on the setting below it."
        }
        "Switch on the tunnels you left on" => {
            "At login, Crake switches back on the VPNs that were on when you last used it.\n\nExample: ACME was on when you shut down yesterday, so it comes back on today. You may be asked to sign in again.\n\nOff: Crake starts with every VPN off, and you switch on what you need."
        }
        tray_row::TITLE => tray_row::HELP.as_str(),
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

    /// A setting's name, with a "?" that explains it in plain words and an
    /// example (hover or click it).
    fn title(&self, label: &'static str, t: &Theme, cx: &Context<Self>) -> Div {
        let row = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(label)
            .when(label == tray_row::TITLE, |r| {
                r.child(tray_row::title_badge())
            });
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
                // A click lands after the hover that opened it: it keeps
                // it open (moving away closes it), instead of closing it.
                .keys()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.hint = Some(label);
                    cx.notify();
                }))
                .test_support(),
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
            for (i, para) in text.split("\n\n").enumerate() {
                card = card.child(div().child(para.to_owned()));
                // The bird's looks, after what it's for.
                if i == 0 && label == tray_row::TITLE {
                    card = card.child(tray_row::legend(t));
                }
            }
            q = q.child(gpui_kit::deferred(card.occlude()).with_priority(2));
        }
        row.child(q)
    }

    fn nav(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        // macOS draws its own window buttons over the sidebar's top
        // (`chrome`): the heading starts below them.
        let top = if crate::chrome::system_buttons(cx) {
            crate::chrome::HEADER_HEIGHT
        } else {
            NAV_TOP
        };
        let mut nav = div()
            .id("nav")
            .w(px(222.))
            .flex_none()
            .h_full()
            .pt(px(top))
            .px(px(12.))
            .bg(t.surface)
            // GPUI clips to a rectangle, not the card's rounded corners: the
            // nav rounds its own (the card's 12 px less its 1 px border).
            .rounded_l(px(11.))
            .border_r_1()
            .border_color(t.line)
            .child(
                div()
                    .id("nav-title")
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                        window.start_window_move()
                    })
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pt(px(4.))
                    .px(px(10.))
                    .pb(px(16.))
                    .child(bird(px(22.), "crake/bird.svg", "crake/call.svg", t))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::BOLD)
                            .child("Settings"),
                    )
                    .test_support(),
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
                    .when(!current, |b| {
                        b.font_weight(FontWeight::MEDIUM)
                            .hover(|s| s.bg(t.nav_hover))
                    })
                    .cursor_pointer()
                    .child(svg().path(icon).size(px(18.)).text_color(t.ink))
                    .child(label)
                    .keys()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.section = section;
                        this.open_select = None;
                        this.adding_never = false;
                        this.confirm_clear = false;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }
        nav
    }
}
