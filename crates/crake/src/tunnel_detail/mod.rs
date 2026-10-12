//! Tunnel detail (design/screens/tunnel-detail.html): one tunnel's facts,
//! who decides what goes through it, today's traffic through it with the
//! hosts it learned, its hosts (learned, pinned, from the gateway) with Pin
//! and Forget, and "Add a host". Opened from Settings → Tunnels.

use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, Stateful, Subscription,
    TestSupportExt, Window, canvas, div, px, svg,
};

use crate::Request;
use crate::client::Tunnel;
use crate::model::{HostRow, Model};
use crate::theme::{Theme, fonts};

mod chart;
mod facts;
mod hosts;

#[cfg(test)]
mod tests;

use self::chart::{now_minutes, today};
use self::facts::{facts, policy, title_row};

pub const WIDTH: f32 = 900.;

/// How often the hosts and the chart are asked for again while open.
const REFRESH: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Learned,
    Pinned,
    Gateway,
}

pub struct TunnelDetail {
    tunnel: String,
    model: Entity<Model>,
    requests: async_channel::Sender<Request>,
    tab: Tab,
    add: Entity<InputState>,
    /// The add field's placeholder, as last set.
    hint: String,
    /// The tallest the window may be (the screen's height, less a margin).
    max_height: f32,
    focus: FocusHandle,
    /// Where the pointer is on the chart (`crate::timeline`).
    chart_hover: crate::timeline::Hover,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for TunnelDetail {}

impl Render for TunnelDetail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let model = self.model.read(cx);
        let Some(tunnel) = model
            .tunnels
            .iter()
            .find(|x| x.name == self.tunnel)
            .cloned()
        else {
            // Removed meanwhile.
            return div().id("tunnel-detail").into_any_element();
        };
        let rows = model
            .host_rows
            .get(&self.tunnel)
            .cloned()
            .unwrap_or_default();
        let traffic = today(model.tunnel_traffic.get(&self.tunnel));
        // Decisions about 12 px apart or closer share a tick (the design's
        // 836 px for the day so far).
        let span = now_minutes();
        let marks = model.marks_within(span, Some(&self.tunnel), span * 12. / 836.);
        let failed = model
            .failed
            .as_ref()
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(8))
            .map(|(text, _)| text.clone());
        let on = matches!(tunnel.state.as_str(), "connected" | "connecting");
        let max_height = self.max_height;

        // The window is the card: as tall as it needs, at most the screen.
        let fit = canvas(
            move |bounds, window, _| {
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

        let header = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .pt(px(10.))
            .pb(px(10.))
            .pl(px(10.))
            .pr(px(12.))
            .border_b_1()
            .border_color(t.line)
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(
                header_button("back", &t)
                    .pl(px(6.))
                    .pr(px(10.))
                    .gap(px(4.))
                    .child(
                        svg()
                            .path("crake/chevron-left.svg")
                            .size(px(18.))
                            .text_color(t.ink),
                    )
                    .child("Tunnels")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let _ = this.requests.send_blocking(Request::Settings);
                        cx.emit(Close);
                    }))
                    .test_support(),
            )
            .child(div().flex_1())
            .child(
                header_button("switch", &t)
                    .px(px(14.))
                    .border_1()
                    .border_color(t.line)
                    .bg(t.panel)
                    .child(if on { "Disconnect" } else { "Connect" })
                    .on_click(cx.listener(move |this, _, _, _| {
                        let _ = this
                            .requests
                            .send_blocking(Request::Switch(this.tunnel.clone(), !on));
                    }))
                    .test_support(),
            )
            .child(
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
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))
                    .test_support(),
            );

        let top = div()
            .pt(px(24.))
            .px(px(28.))
            .child(title_row(&tunnel, &t))
            .child(facts(&tunnel, &t))
            .child(policy(&tunnel, &t))
            .child(self.chart(&tunnel, marks, &traffic, &t, cx));

        let body = div()
            .id("body")
            .flex_none()
            .max_h(px(max_height - 60.))
            .overflow_y_scroll()
            .child(top)
            .child(self.hosts(&tunnel, &rows, failed, &t, window, cx));

        let card = div()
            .id("tunnel-detail")
            .track_focus(&self.focus)
            .relative()
            .w_full()
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
            .on_key_down(cx.listener(|_, e: &gpui_kit::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    cx.emit(Close);
                }
            }))
            .child(header)
            .child(body)
            .child(fit);
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(card)
            .into_any_element()
    }
}

fn header_button(id: &'static str, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(36.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .text_color(t.ink)
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
}

impl TunnelDetail {
    /// "Add a host"'s field (journey tests).
    #[cfg(all(test, any(target_os = "linux", windows)))]
    pub(crate) fn add_field(&self) -> Entity<InputState> {
        self.add.clone()
    }

    pub fn new(
        tunnel: String,
        model: Entity<Model>,
        requests: async_channel::Sender<Request>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let add = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.subscribe_in(&add, window, |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.send_added(window, cx);
                }
            }),
            cx.observe_window_appearance(window, |_, _, cx| {
                Theme::current(cx).sync_components(cx);
                cx.notify();
            }),
        ];
        // Its hosts and traffic now, and again every few seconds while open.
        let name = tunnel.clone();
        let ask = requests.clone();
        cx.spawn(async move |this, cx| {
            loop {
                if ask.send(Request::Detail(name.clone())).await.is_err() {
                    return;
                }
                cx.background_executor().timer(REFRESH).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
        })
        .detach();
        let max_height = crate::place::monitors()
            .first()
            .map_or(1000., |m| m.h - 120.);
        Self {
            tunnel,
            model,
            requests,
            tab: Tab::Learned,
            add,
            hint: String::new(),
            max_height,
            focus: cx.focus_handle(),
            chart_hover: crate::timeline::Hover::default(),
            _subscriptions: subscriptions,
        }
    }

    fn send(&self, request: Request) {
        let _ = self.requests.send_blocking(request);
        // What changed shows at once, not at the next refresh.
        let _ = self
            .requests
            .send_blocking(Request::Detail(self.tunnel.clone()));
    }

    /// "Send through X": a name or an address goes through, a network is
    /// pinned.
    fn send_added(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.add.read(cx).value().trim().to_owned();
        if text.is_empty() {
            return;
        }
        let request = if text.contains('/') {
            Request::Pin(self.tunnel.clone(), vec![text])
        } else {
            Request::Send(self.tunnel.clone(), vec![text])
        };
        self.send(request);
        self.add.update(cx, |f, cx| f.set_value("", window, cx));
    }
}
