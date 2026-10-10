//! Tunnel detail (design/screens/tunnel-detail.html): one tunnel's facts,
//! who decides what goes through it, today's traffic through it with the
//! hosts it learned, its hosts (learned, pinned, from the gateway) with Pin
//! and Forget, and "Add a host". Opened from Settings → Tunnels.

use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, PathBuilder, Stateful, Subscription,
    Window, canvas, div, point, px, svg,
};

use crate::Request;
use crate::client::Tunnel;
use crate::model::{HostRow, Model};
use crate::theme::{Theme, fonts};

pub const WIDTH: f32 = 900.;

/// How often the hosts and the chart are asked for again while open.
const REFRESH: Duration = Duration::from_secs(5);

/// The chart, as in the design's 840 × 60 drawing: the baseline, and the
/// time labels under it.
const CHART_H: f32 = 60.;
const BASELINE: f32 = 44.;

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
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for TunnelDetail {}

impl TunnelDetail {
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

impl Render for TunnelDetail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let model = self.model.read(cx);
        let Some(tunnel) = model.tunnels.iter().find(|x| x.name == self.tunnel).cloned() else {
            // Removed meanwhile.
            return div().id("tunnel-detail").into_any_element();
        };
        let rows = model.host_rows.get(&self.tunnel).cloned().unwrap_or_default();
        let traffic = today(model.tunnel_traffic.get(&self.tunnel));
        let ticks = learned_today(model, &self.tunnel);
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
                    })),
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
                    })),
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
                    .child(svg().path("crake/close.svg").size(px(14.)).text_color(t.ink))
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation()
                    })
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Close))),
            );

        let top = div()
            .pt(px(24.))
            .px(px(28.))
            .child(title_row(&tunnel, &t))
            .child(facts(&tunnel, &t))
            .child(policy(&tunnel, &t))
            .child(chart(&tunnel, &traffic, &ticks, &t));

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

impl TunnelDetail {
    /// The tabs, their rows, and "Add a host".
    fn hosts(
        &mut self,
        tunnel: &Tunnel,
        rows: &[HostRow],
        failed: Option<String>,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let display = tunnel.display.clone();
        let gateway = gateway_rows(tunnel);
        let mut learned: Vec<&HostRow> = rows
            .iter()
            .filter(|r| r.kind == "learned" || (r.kind == "pinned" && r.learned.is_some()))
            .collect();
        // Newest first.
        learned.sort_by_key(|r| std::cmp::Reverse(r.learned.as_ref().map_or(0, |l| l.0)));
        let pinned: Vec<&HostRow> = rows.iter().filter(|r| r.kind == "pinned").collect();
        let counts = [learned.len(), pinned.len(), gateway.len()];

        let mut bar = div()
            .flex()
            .gap(px(22.))
            .border_b_1()
            .border_color(t.line);
        for (i, (tab, label)) in [
            (Tab::Learned, "Learned"),
            (Tab::Pinned, "Pinned"),
            (Tab::Gateway, "From the gateway"),
        ]
        .into_iter()
        .enumerate()
        {
            let on = self.tab == tab;
            bar = bar.child(
                div()
                    .id(("tab", i))
                    .h(px(42.))
                    .mb(px(-1.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .border_b_2()
                    .border_color(if on { t.accent } else { gpui_kit::transparent_black().into() })
                    .text_color(if on { t.ink } else { t.muted })
                    .font_weight(if on { FontWeight::BOLD } else { FontWeight::SEMIBOLD })
                    .cursor_pointer()
                    .child(label)
                    .child(
                        div()
                            .font_family(fonts::MONO)
                            .text_size(px(12.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(t.muted)
                            .child(counts[i].to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    })),
            );
        }

        let shown: Vec<AnyElement> = match self.tab {
            Tab::Learned => learned
                .iter()
                .enumerate()
                .map(|(i, r)| self.host_row(i, r, t, cx).into_any_element())
                .collect(),
            Tab::Pinned => pinned
                .iter()
                .enumerate()
                .map(|(i, r)| self.host_row(i, r, t, cx).into_any_element())
                .collect(),
            Tab::Gateway => gateway
                .iter()
                .map(|(host, how, used)| {
                    row_frame(t)
                        .child(icon("crake/mark-gateway.svg", t.muted))
                        .child(host_text(host.clone(), how.clone(), t))
                        .child(used_text(used.clone(), t))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .text_size(px(13.))
                                .text_color(t.muted)
                                .child("Set by the gateway"),
                        )
                        .into_any_element()
                })
                .collect(),
        };
        let empty = shown.is_empty();

        // The form's hint: a name under its domain, an address in its network.
        let domain = tunnel
            .dns_domains
            .first()
            .cloned()
            .unwrap_or_else(|| "example.internal".into());
        let net = tunnel
            .learn
            .iter()
            .find(|l| l.contains('/'))
            .cloned()
            .or_else(|| (!tunnel.address.is_empty()).then(|| tunnel.address.clone()))
            .map(|a| {
                let o: Vec<&str> = a.split(['.', '/']).collect();
                format!("{}.{}.x.x", o.first().unwrap_or(&"10"), o.get(1).unwrap_or(&"0"))
            })
            .unwrap_or_else(|| "10.x.x.x".into());
        let hint = format!("name.{domain} or {net}");
        if self.hint != hint {
            self.add
                .update(cx, |f, cx| f.set_placeholder(hint.clone(), window, cx));
            self.hint = hint;
        }

        div()
            .pt(px(22.))
            .px(px(28.))
            .child(bar)
            .children(shown)
            .when(empty, |d| {
                d.child(
                    div()
                        .py(px(24.))
                        .text_color(t.muted)
                        .border_b_1()
                        .border_color(t.line)
                        .child("Nothing here."),
                )
            })
            .when(!tunnel.full_tunnel, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(10.))
                        .items_center()
                        .pt(px(16.))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("Add a host"))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(240.))
                                .h(px(40.))
                                .px(px(1.))
                                .flex()
                                .items_center()
                                .border_1()
                                .border_color(t.line_strong)
                                .rounded(px(6.))
                                .bg(t.panel)
                                .font_family(fonts::MONO)
                                .text_size(px(13.))
                                .child(Input::new(&self.add).appearance(false)),
                        )
                        .child(
                            div()
                                .id("send")
                                .h(px(40.))
                                .px(px(16.))
                                .flex()
                                .items_center()
                                .rounded(px(6.))
                                .bg(t.accent)
                                .text_color(t.accent_ink)
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .child(format!("Send through {display}"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.send_added(window, cx)
                                })),
                        ),
                )
            })
            .child(
                div()
                    .pb(px(28.))
                    .when_some(failed, |d, text| {
                        d.child(
                            div()
                                .pt(px(10.))
                                .text_size(px(13.))
                                .text_color(t.danger)
                                .child(text),
                        )
                    }),
            )
    }

    fn host_row(&self, i: usize, r: &HostRow, t: &Theme, cx: &mut Context<Self>) -> Div {
        let pinned = r.kind == "pinned";
        let (icon_path, color) = if pinned {
            ("crake/mark-pinned.svg", t.ink)
        } else {
            ("crake/mark-routed.svg", t.accent)
        };
        let host = r.name.clone().unwrap_or_else(|| r.target.clone());
        let how = if pinned {
            "Pinned \u{b7} kept until you forget it".to_owned()
        } else {
            learned_how(r)
        };
        let used = match r.used_ago {
            Some(s) if s < 90 => "Used now".to_owned(),
            Some(s) if s < 3600 => format!("{} min ago", s / 60),
            Some(s) if s < 86_400 => format!("{} h ago", s / 3600),
            Some(s) => match s / 86_400 {
                1 => "Yesterday".to_owned(),
                d => format!("{d} days ago"),
            },
            None => "\u{2014}".to_owned(),
        };
        let (target, pin_target) = (r.target.clone(), r.target.clone());
        row_frame(t)
            .child(icon(icon_path, color))
            .child(host_text(host, how, t))
            .child(used_text(used, t))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .justify_end()
                    .child(
                        row_button(("pin", i), if pinned { "Unpin" } else { "Pin" }, t).on_click(
                            cx.listener(move |this, _, _, _| {
                                let tunnel = this.tunnel.clone();
                                this.send(if pinned {
                                    Request::Unpin(tunnel, pin_target.clone())
                                } else {
                                    Request::Pin(tunnel, vec![pin_target.clone()])
                                });
                            }),
                        ),
                    )
                    .child(row_button(("forget", i), "Forget", t).on_click(cx.listener(
                        move |this, _, _, _| {
                            this.send(Request::Forget(this.tunnel.clone(), target.clone()));
                        },
                    ))),
            )
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

/// "ACME", the Managed tag, and the state.
fn title_row(tunnel: &Tunnel, t: &Theme) -> impl IntoElement {
    let (dot, state) = match tunnel.state.as_str() {
        "connected" => (t.accent, "Connected"),
        "connecting" => (t.accent, "Connecting\u{2026}"),
        "trouble" => (t.danger, "Couldn\u{2019}t stay connected"),
        _ => (t.line_strong, "Off"),
    };
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(28.))
                .line_height(px(36.))
                .font_weight(FontWeight::BOLD)
                .child(tunnel.display.clone()),
        )
        .when(tunnel.managed, |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(4.))
                    .bg(t.surface)
                    .text_color(t.muted)
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(svg().path("crake/lock.svg").size(px(12.)).text_color(t.muted))
                    .child("Managed"),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(13.))
                .text_color(t.muted)
                .child(div().size(px(8.)).rounded_full().bg(dot))
                .child(state),
        )
}

/// Connected for, your address, protocol, gateway.
fn facts(tunnel: &Tunnel, t: &Theme) -> impl IntoElement {
    let up = tunnel.state == "connected";
    let since = if up && tunnel.since > 0 {
        let secs = jiff::Timestamp::now().as_second().max(0) as u64;
        duration(secs.saturating_sub(tunnel.since))
    } else {
        "\u{2014}".to_owned()
    };
    let address = if up && !tunnel.address.is_empty() {
        tunnel.address.clone()
    } else {
        "\u{2014}".to_owned()
    };
    let protocol = match (protocol_name(&tunnel.protocol), tunnel.transport.as_str()) {
        (p, "") => p.to_owned(),
        (p, how) if up => format!("{p} \u{b7} {how}"),
        (p, _) => p.to_owned(),
    };
    let openvpn3 = tunnel.protocol == "openvpn3";
    let cell = |first: bool| {
        div()
            .flex_1()
            .min_w(px(180.))
            .py(px(12.))
            .px(px(14.))
            .when(!first, |d| d.border_l_1().border_color(t.line))
    };
    let label = |text: &'static str| {
        div()
            .text_size(px(12.))
            .line_height(px(17.))
            .text_color(t.muted)
            .child(text)
    };
    let strong = |text: String| {
        div()
            .mt(px(2.))
            .text_size(px(15.))
            .line_height(px(22.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(text)
    };
    let mono = |text: String| {
        div()
            .mt(px(2.))
            .font_family(fonts::MONO)
            .text_size(px(14.))
            .line_height(px(22.))
            .child(text)
    };
    div()
        .mt(px(18.))
        .flex()
        .border_1()
        .border_color(t.line)
        .rounded(px(8.))
        .overflow_hidden()
        .child(cell(true).child(label("Connected for")).child(strong(since)))
        .child(cell(false).child(label("Your address")).child(mono(address)))
        .child(cell(false).child(label("Protocol")).child(strong(protocol)))
        .child(
            cell(false)
                .child(label(if openvpn3 { "Profile" } else { "Gateway" }))
                .child(mono(tunnel.gateway.clone())),
        )
}

/// Who decides what goes through, in plain words.
fn policy(tunnel: &Tunnel, t: &Theme) -> impl IntoElement {
    let display = &tunnel.display;
    let mono = |text: &str| {
        div()
            .font_family(fonts::MONO)
            .text_size(px(13.))
            .child(text.to_owned())
    };
    // The bold line: words and monospaced patterns.
    let mut line = div().flex().flex_wrap().font_weight(FontWeight::BOLD);
    let mode = tunnel.own.get("mode").map(String::as_str).unwrap_or("learn");
    let listed: Vec<String> = if tunnel.managed {
        tunnel.learn.clone()
    } else {
        tunnel
            .own
            .get("list")
            .map(|l| l.lines().map(str::trim).filter(|x| !x.is_empty()).map(str::to_owned).collect())
            .unwrap_or_default()
    };
    let words = |line: Div, text: String| line.child(text.replace(' ', "\u{a0}"));
    let patterns = |mut line: Div, items: &[String]| {
        for (i, item) in items.iter().enumerate() {
            line = line.child(mono(item));
            let sep = match items.len() - i {
                1 => ".",
                2 => "\u{a0}and\u{a0}",
                _ => ",\u{a0}",
            };
            line = line.child(sep);
        }
        line
    };
    let sub = if tunnel.full_tunnel {
        let who = if tunnel.managed {
            format!("{} sends everything through {display}.", organisation(tunnel))
        } else {
            format!("Everything goes through {display}.")
        };
        line = words(line, who);
        format!("Except your local network. Crake doesn\u{2019}t learn anything for {display}.")
    } else if tunnel.managed && !listed.is_empty() {
        line = words(line, format!("{} allows learning for ", organisation(tunnel)));
        line = patterns(line, &listed);
        "Anything else stays direct, even if it doesn\u{2019}t answer. You can pin and forget hosts, but not widen the rule.".to_owned()
    } else if mode == "list" && !listed.is_empty() {
        line = words(line, format!("Only these go through {display}: "));
        line = patterns(line, &listed);
        "Anything else stays direct, even if it doesn\u{2019}t answer.".to_owned()
    } else if mode == "ask" {
        line = words(line, format!("Crake asks before sending a host through {display}."));
        "A notification for each new host. You can pin, forget and add hosts.".to_owned()
    } else {
        line = words(line, format!("Crake learns what needs {display}."));
        format!(
            "A host goes through {display} when it only answers through {display}, or when only {display}\u{2019}s DNS knows its name. You can pin, forget and add hosts."
        )
    };
    div()
        .mt(px(14.))
        .flex()
        .gap(px(12.))
        .items_start()
        .py(px(14.))
        .px(px(16.))
        .rounded(px(8.))
        .bg(t.surface)
        .child(
            svg()
                .path("crake/shield.svg")
                .size(px(20.))
                .flex_none()
                .mt(px(1.))
                .text_color(t.ink),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(line)
                .child(div().text_size(px(13.)).text_color(t.muted).child(sub)),
        )
}

/// "Today through ACME": traffic through it (grey bars) and the hosts it
/// learned (ticks), midnight to now.
fn chart(tunnel: &Tunnel, traffic: &[(f32, u64)], ticks: &[f32], t: &Theme) -> impl IntoElement {
    let now = now_minutes();
    let total: u64 = traffic.iter().map(|x| x.1).sum();
    let hosts = match tunnel.hosts {
        1 => "1 host".to_owned(),
        n => format!("{n} hosts"),
    };
    // About one bar per 7 px of the design's 836.
    let bin = (now / (836. / 7.)).max(1.);
    let mut bins: Vec<(f32, u64)> = Vec::new();
    for (m, bytes) in traffic {
        let start = (m / bin).floor() * bin;
        match bins.last_mut() {
            Some((s, sum)) if *s == start => *sum += bytes,
            _ => bins.push((start, *bytes)),
        }
    }
    let max = bins.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32;
    let (bars, accent, panel, base) = (t.bars, t.accent, t.panel, t.line_strong);
    let (ticks, paint_bins) = (ticks.to_vec(), bins.clone());
    // A whole hour about halfway, as in the design's "06:00".
    let middle = ((now / 60. / 2.).floor() * 60.).max(0.);
    let mono = |text: String| {
        div()
            .absolute()
            .top(px(BASELINE + 3.))
            .whitespace_nowrap()
            .font_family(fonts::MONO)
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(t.muted)
            .child(text)
    };
    div()
        .mt(px(22.))
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::BOLD)
                        .child(format!("Today through {}", tunnel.display)),
                )
                .child(
                    div()
                        .font_family(fonts::MONO)
                        .text_size(px(12.))
                        .text_color(t.muted)
                        .child(format!("{hosts} \u{b7} {}", crate::model::human_bytes(total))),
                ),
        )
        .child(
            div()
                .relative()
                .mt(px(8.))
                .h(px(CHART_H))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let w = f32::from(bounds.size.width);
                            let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
                            let baseline = top + BASELINE;
                            let x_of = |m: f32| left + 2. + (w - 4.) * m / now;
                            let scale = window.scale_factor();
                            for (start, bytes) in &paint_bins {
                                if *bytes == 0 {
                                    continue;
                                }
                                let h = 2. + 16. * (*bytes as f32 / max).sqrt();
                                if let Some(p) =
                                    crate::chart::bar(x_of(start + bin / 2.), 3., baseline, h, scale)
                                {
                                    window.paint_path(p, bars);
                                }
                            }
                            // Ticks keep clear of the bars (DESIGN.md, Charts).
                            for m in &ticks {
                                for (dx, tip) in [(0., 10.), (4.5, 18.)] {
                                    if let Some(p) = crate::chart::rounded_bar(
                                        x_of(*m) + dx,
                                        top + tip - 1.5,
                                        baseline,
                                        6.,
                                    ) {
                                        window.paint_path(p, panel);
                                    }
                                }
                            }
                            let mut line = PathBuilder::stroke(px(1.));
                            line.move_to(point(px(left), px(baseline + 0.5)));
                            line.line_to(point(px(left + w), px(baseline + 0.5)));
                            if let Ok(p) = line.build() {
                                window.paint_path(p, base);
                            }
                            for m in &ticks {
                                for (dx, tip) in [(0., 10.), (4.5, 18.)] {
                                    if let Some(p) = crate::chart::rounded_bar(
                                        x_of(*m) + dx,
                                        top + tip,
                                        baseline,
                                        3.,
                                    ) {
                                        window.paint_path(p, accent);
                                    }
                                }
                            }
                        },
                    )
                    .size_full(),
                )
                .child(mono("00:00".into()).left_0())
                .when(middle >= 120. && now - middle >= 120., |d| {
                    // Centred under its time.
                    d.child(
                        div()
                            .absolute()
                            .left(gpui_kit::relative(middle / now))
                            .top_0()
                            .w(px(34.))
                            .h_full()
                            .child(
                                mono(format!("{:02}:00", (middle / 60.) as u32))
                                    .left(px(-17.)),
                            ),
                    )
                })
                .child(mono("now".into()).right_0()),
        )
}

fn row_frame(t: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(11.))
        .border_b_1()
        .border_color(t.line)
}

fn icon(path: &'static str, color: gpui_kit::Rgba) -> impl IntoElement {
    svg().path(path).size(px(18.)).flex_none().text_color(color)
}

fn host_text(host: String, how: String, t: &Theme) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .font_family(fonts::MONO)
                .text_size(px(14.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(host),
        )
        .child(div().text_size(px(13.)).text_color(t.muted).child(how))
}

fn used_text(used: String, t: &Theme) -> impl IntoElement {
    div()
        .w(px(120.))
        .flex_none()
        .text_size(px(13.))
        .text_color(t.muted)
        .child(used)
}

fn row_button(id: impl Into<gpui_kit::ElementId>, label: &'static str, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(34.))
        .px(px(12.))
        .flex()
        .items_center()
        .border_1()
        .border_color(t.line)
        .rounded(px(6.))
        .bg(t.panel)
        .text_color(t.ink)
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .child(label)
}

/// "Learned at 13:21 · no answer directly".
fn learned_how(r: &HostRow) -> String {
    let Some((at, how)) = &r.learned else {
        return "Learned earlier".to_owned();
    };
    let why = match how.as_str() {
        "probe" => "no answer directly",
        "dns" | "inside" => "name only exists inside",
        "split" => "public name, private address",
        "you" => "added by you",
        _ => "",
    };
    let when = jiff::Timestamp::from_second(*at as i64)
        .ok()
        .map(|ts| ts.to_zoned(jiff::tz::TimeZone::system()))
        .map(|z| {
            if z.date() == jiff::Zoned::now().date() {
                format!("Learned at {}", z.strftime("%H:%M"))
            } else {
                format!("Learned on {}", z.strftime("%-d %b"))
            }
        })
        .unwrap_or_else(|| "Learned earlier".to_owned());
    if why.is_empty() {
        when
    } else {
        format!("{when} \u{b7} {why}")
    }
}

/// What the gateway set: its DNS servers and the networks it pushed.
fn gateway_rows(tunnel: &Tunnel) -> Vec<(String, String, String)> {
    let up = tunnel.state == "connected";
    let display = &tunnel.display;
    tunnel
        .dns
        .iter()
        .map(|d| {
            (
                d.clone(),
                format!("{display}\u{2019}s DNS server"),
                if up { "Used now" } else { "\u{2014}" }.to_owned(),
            )
        })
        .chain(tunnel.split_include.iter().map(|n| {
            (
                n.clone(),
                format!("Pushed when {display} connects"),
                "\u{2014}".to_owned(),
            )
        }))
        .collect()
}

fn organisation(tunnel: &Tunnel) -> String {
    if tunnel.organisation.is_empty() {
        "IT".to_owned()
    } else {
        tunnel.organisation.clone()
    }
}

pub fn protocol_name(protocol: &str) -> &str {
    match protocol {
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
}

/// "4 h 53 min", "12 min", "2 days 3 h".
fn duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{} min", m.max(1)),
        (0, _) => format!("{h} h {m} min"),
        (1, _) => format!("1 day {h} h"),
        _ => format!("{d} days {h} h"),
    }
}

/// Today's minutes of a tunnel's traffic: (minutes since midnight, bytes).
fn today(minutes: Option<&Vec<(u64, u64)>>) -> Vec<(f32, u64)> {
    let midnight = jiff::Zoned::now()
        .start_of_day()
        .map(|z| z.timestamp().as_second() as u64 / 60)
        .unwrap_or(0);
    minutes
        .map(|v| {
            v.iter()
                .filter(|(m, _)| *m >= midnight)
                .map(|(m, b)| ((m - midnight) as f32, *b))
                .collect()
        })
        .unwrap_or_default()
}

/// When the tunnel learned hosts today (minutes since midnight).
fn learned_today(model: &Model, tunnel: &str) -> Vec<f32> {
    model
        .today_events()
        .into_iter()
        .filter(|(_, e)| {
            e.kind == crake_proto::LogKind::Routed && e.tunnel.as_deref() == Some(tunnel)
        })
        .map(|(m, _)| m)
        .collect()
}

/// Minutes since local midnight, now.
fn now_minutes() -> f32 {
    let now = jiff::Zoned::now();
    (now.hour() as f32 * 60. + now.minute() as f32 + now.second() as f32 / 60.).max(1.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_like_the_design() {
        assert_eq!(duration(4 * 3600 + 53 * 60 + 10), "4 h 53 min");
        assert_eq!(duration(20), "1 min");
        assert_eq!(duration(12 * 60), "12 min");
        assert_eq!(duration(86_400 + 3 * 3600), "1 day 3 h");
    }

    #[test]
    fn learned_hosts_say_when_and_why() {
        let now = jiff::Timestamp::now().as_second() as u64;
        let r = HostRow {
            target: "10.62.0.18".into(),
            kind: "learned".into(),
            learned: Some((now, "probe".into())),
            ..Default::default()
        };
        assert!(learned_how(&r).starts_with("Learned at "));
        assert!(learned_how(&r).ends_with("\u{b7} no answer directly"));
        let old = HostRow {
            learned: None,
            ..r
        };
        assert_eq!(learned_how(&old), "Learned earlier");
    }
}
