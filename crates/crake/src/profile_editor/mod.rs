//! Editing an OpenVPN 3 profile's text, for experts
//! (design/proposals/edit-ovpn-profile.html, A). Opened from tunnel
//! detail's Profile fact: the text as openvpn3 holds it, line numbers, the
//! inline certificates and keys folded into a line each and kept as they
//! are (`text`), changed lines tinted; Save gives openvpn3 the new text
//! under the same name and reconnects the tunnel when it's on. Three
//! departures from the proposal, all GPUI's editor's: a folded block is a
//! muted line of text, not a chip; a changed line's tint covers its text,
//! not the whole line; and the line numbers sit closer to the text.

#[cfg(test)]
mod tests;
pub mod text;

use std::sync::Arc;

use gpui_kit::component::input::{
    Editor, EditorState, InputEvent, RangeDecoration, RangeDecorationCollection,
    RangeDecorationStyle, TextDecoration, TextDecorationCollection,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    Entity, FocusHandle, FontWeight, HighlightStyle, Subscription, TestSupportExt, Window, div, px,
};

use crate::Request;
use crate::ovpn3::profile::Fetched;
use crate::theme::{Theme, fonts};

pub const WIDTH: f32 = 820.;
pub const HEIGHT: f32 = 640.;
/// The editor's text size (px), as the proposal draws it.
const CODE_SIZE: f32 = 13.;
const SIDE: f32 = 20.;
/// Save, until something changed.
const UNAVAILABLE: f32 = 0.45;

/// Where a profile's text comes from and goes to: openvpn3, or a test's
/// stand-in.
pub trait Store: Send + Sync {
    fn fetch(&self, profile: &str) -> Result<Fetched, String>;
    fn replace(&self, profile: &str, text: &str) -> Result<(), String>;
}

/// This computer's openvpn3.
pub struct Openvpn3;

impl Store for Openvpn3 {
    fn fetch(&self, profile: &str) -> Result<Fetched, String> {
        crate::ovpn3::profile::fetch(profile)
    }
    fn replace(&self, profile: &str, text: &str) -> Result<(), String> {
        crate::ovpn3::profile::replace(profile, text)
    }
}

/// CRAKE_DEMO: the proposal's profile, and a save that works.
pub struct Demo;

// literal: the proposal's sample profile (design/proposals/edit-ovpn-profile.html)
impl Store for Demo {
    fn fetch(&self, _: &str) -> Result<Fetched, String> {
        let block = |tag: &str, lines: usize| {
            let body: String = (0..lines)
                .map(|_| "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB\n")
                .collect();
            format!("<{tag}>\n{body}</{tag}>\n")
        };
        Ok(Fetched::Text(format!(
            "client\ndev tun\nproto udp\nremote vpn.initech.example 1194\nresolv-retry infinite\nnobind\nremote-cert-tls server\nroute 10.20.0.0 255.255.0.0\ndhcp-option DNS 10.8.0.1\ndhcp-option DOMAIN initech.internal\n{}{}{}{}verb 3\n",
            block("ca", 31),
            block("cert", 27),
            block("key", 28),
            block("tls-crypt", 21),
        )))
    }
    fn replace(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Loading,
    Editing,
    Saving,
    Locked,
    /// Couldn't be read (gone, or openvpn3 said why).
    Unreadable(String),
}

pub struct ProfileEditor {
    tunnel: String,
    display: String,
    profile: String,
    editor: Entity<EditorState>,
    original: Option<text::Folded>,
    stage: Stage,
    /// openvpn3's refusal, or a folded line that can't be put back.
    note: Option<String>,
    changed: bool,
    tint: Option<RangeDecorationCollection>,
    muted: Option<TextDecorationCollection>,
    store: Arc<dyn Store>,
    requests: async_channel::Sender<Request>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for ProfileEditor {}

impl ProfileEditor {
    pub fn new(
        tunnel: String,
        display: String,
        profile: String,
        store: Arc<dyn Store>,
        requests: async_channel::Sender<Request>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(true)
                .folding(false)
        });
        let subscriptions = vec![
            cx.subscribe_in(&editor, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.edited(cx);
                }
            }),
            cx.observe_window_appearance(window, |_, _, cx| {
                Theme::current(cx).sync_components(cx);
                cx.notify();
            }),
        ];
        let (fetching, named) = (Arc::clone(&store), profile.clone());
        cx.spawn_in(window, async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move { fetching.fetch(&named) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| this.fetched(fetched, window, cx));
        })
        .detach();
        Self {
            tunnel,
            display,
            profile,
            editor,
            original: None,
            stage: Stage::Loading,
            note: None,
            changed: false,
            tint: None,
            muted: None,
            store,
            requests,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// For the journey (Linux's).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn stage_for_tests(&self) -> &Stage {
        &self.stage
    }

    fn fetched(
        &mut self,
        fetched: Result<Fetched, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stage = match fetched {
            Ok(Fetched::Text(t)) => {
                let folded = text::fold(&t);
                let shown = folded.shown.clone();
                self.original = Some(folded);
                self.editor
                    .update(cx, |e, cx| e.set_value(shown, window, cx));
                self.mute_folded(cx);
                Stage::Editing
            }
            Ok(Fetched::Locked) => Stage::Locked,
            Ok(Fetched::Missing) => Stage::Unreadable(format!(
                "OpenVPN 3 has no profile called {} any more.",
                self.profile
            )),
            Err(e) => Stage::Unreadable(format!("Couldn\u{2019}t read it from OpenVPN 3: {e}")),
        };
        cx.notify();
    }

    /// The folded lines in the muted colour, so they read as placeholders.
    fn mute_folded(&mut self, cx: &mut Context<Self>) {
        let Some(original) = &self.original else {
            return;
        };
        let t = Theme::current(cx);
        let value = self.editor.read(cx).value().to_string();
        let mut ranges = Vec::new();
        for block in &original.blocks {
            if let Some(at) = value.find(&block.shown) {
                ranges.push(TextDecoration::new(
                    at..at + block.shown.len(),
                    HighlightStyle {
                        color: Some(t.muted.into()),
                        ..Default::default()
                    },
                ));
            }
        }
        let muted = self
            .editor
            .update(cx, |e, cx| e.create_decorations_collection(ranges, cx));
        self.muted = Some(muted);
    }

    /// The text changed: tint what differs from openvpn3's, and Save is there.
    fn edited(&mut self, cx: &mut Context<Self>) {
        let Some(original) = &self.original else {
            return;
        };
        let value = self.editor.read(cx).value().to_string();
        self.changed = value != original.shown;
        self.note = None;
        let t = Theme::current(cx);
        let tint: Vec<RangeDecoration> = text::changed_lines(&original.shown, &value)
            .into_iter()
            .map(|r| {
                RangeDecoration::new(r)
                    .with_style(RangeDecorationStyle::Fill)
                    .with_color(t.accent_soft.into())
            })
            .collect();
        match &self.tint {
            Some(c) => c.set(tint, cx),
            None => {
                let c = self
                    .editor
                    .update(cx, |e, cx| e.create_range_decorations_collection(tint, cx));
                self.tint = Some(c);
            }
        }
        cx.notify();
    }

    /// Save and reconnect: the folded blocks back in, openvpn3 given the
    /// text; refused, nothing changed and it says why.
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(original) = &self.original else {
            return;
        };
        if !self.changed || self.stage != Stage::Editing {
            return;
        }
        let value = self.editor.read(cx).value().to_string();
        let whole = match text::unfold(&value, &original.blocks) {
            Ok(whole) => whole,
            Err(why) => {
                self.note = Some(why);
                cx.notify();
                return;
            }
        };
        self.stage = Stage::Saving;
        self.note = None;
        cx.notify();
        let (store, profile) = (Arc::clone(&self.store), self.profile.clone());
        cx.spawn_in(window, async move |this, cx| {
            let saved = cx
                .background_executor()
                .spawn(async move { store.replace(&profile, &whole) })
                .await;
            let _ = this.update(cx, |this, cx| match saved {
                Ok(()) => {
                    let _ = this
                        .requests
                        .send_blocking(Request::Reconnect(this.tunnel.clone()));
                    cx.emit(Close);
                }
                Err(why) => {
                    this.stage = Stage::Editing;
                    this.note = Some(why);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn footer(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let (note, bad) = match (&self.note, &self.stage) {
            (Some(n), _) => (n.clone(), true),
            (None, Stage::Saving) => ("Giving OpenVPN 3 the new text\u{2026}".to_owned(), false),
            _ => (
                format!(
                    "Saving gives OpenVPN 3 the new text under the same name and reconnects {}.",
                    self.display
                ),
                false,
            ),
        };
        let can_save = self.changed && self.stage == Stage::Editing;
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(SIDE))
            .py(px(12.))
            .border_t_1()
            .border_color(t.line)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .text_color(if bad { t.danger } else { t.muted })
                    .child(note),
            )
            .child(
                button("cancel", "Cancel", false, t)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))
                    .test_support(),
            )
            .child(
                button("save", "Save and reconnect", true, t)
                    .when(!can_save, |b| b.opacity(UNAVAILABLE).cursor_default())
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))
                    .test_support(),
            )
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    t: &Theme,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .h(px(34.))
        .px(px(14.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .border_1()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(13.))
        .cursor_pointer()
        .when(primary, |b| {
            b.bg(t.accent)
                .border_color(t.accent)
                .text_color(t.accent_ink)
        })
        .when(!primary, |b| b.border_color(t.line).text_color(t.ink))
        .child(label)
}

impl Render for ProfileEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let system = crate::chrome::system_buttons(cx);
        let header = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pt(px(14.))
            .pb(px(12.))
            .pl(px(if system {
                crate::chrome::BUTTONS_WIDTH
            } else {
                SIDE
            }))
            .pr(px(12.))
            .border_b_1()
            .border_color(t.line)
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::BOLD)
                    .child(format!("{}\u{2019}s profile", self.display)),
            )
            .child(
                div()
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(format!("{} \u{b7} in OpenVPN 3", self.profile)),
            )
            .child(div().flex_1())
            .when(!system, |h| {
                h.child(
                    crate::chrome::close_button(&t)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))
                        .test_support(),
                )
            });
        let say = |text: String| div().flex_1().p(px(SIDE)).text_color(t.muted).child(text);
        let body = match &self.stage {
            Stage::Loading => say("Reading it from OpenVPN 3\u{2026}".to_owned()).into_any_element(),
            Stage::Locked => say(format!(
                "{}\u{2019}s profile is locked in OpenVPN 3: its text can\u{2019}t be shown or changed here.",
                self.display
            ))
            .into_any_element(),
            Stage::Unreadable(why) => say(why.clone()).into_any_element(),
            Stage::Editing | Stage::Saving => div()
                .flex_1()
                .min_h_0()
                .pt(px(10.))
                .pl(px(8.))
                .font_family(fonts::MONO)
                .text_size(px(CODE_SIZE))
                .child(
                    Editor::new(&self.editor)
                        .appearance(false)
                        .readonly(self.stage == Stage::Saving)
                        .h_full(),
                )
                .into_any_element(),
        };
        div()
            .id("profile-editor")
            .track_focus(&self.focus)
            .size_full()
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
            .on_key_down(cx.listener(|_, e: &gpui_kit::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    cx.emit(Close);
                }
            }))
            .child(header)
            .child(body)
            .when(matches!(self.stage, Stage::Editing | Stage::Saving), |d| {
                d.child(self.footer(&t, cx))
            })
    }
}
