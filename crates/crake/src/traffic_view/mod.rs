//! Traffic (design/proposals/traffic-window.html, decided with the user
//! 2026-10-14): what goes where, live. The graph is the point: this
//! computer, the ways out (each tunnel that's up, and direct), the busiest
//! destinations of each, and packets ticking along every path; under it
//! the last five minutes, then the tables as the detail. Opened from the
//! panel's footer. craked is asked once a second while it's open (D-Bus
//! `Live`), and each tunnel's hosts every few seconds; nothing is asked
//! once it's closed.

use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{
    Context, Div, Entity, FocusHandle, FontWeight, Stateful, Subscription, TestSupportExt, Window,
    canvas, div, px,
};

use crate::Request;
use crate::model::Model;
use crate::theme::{Theme, fonts};

mod graph;
mod strip;
mod tables;

#[cfg(test)]
mod tests;

use self::graph::{Flows, HOST_X, ME_H, ME_W, ME_X, WAY_H, WAY_W, WAY_X, rate};
use crate::focus::Keys as _;

pub const WIDTH: f32 = 920.;
/// The window's height, at most the screen's.
pub const HEIGHT: f32 = 860.;
/// craked is asked this often while the window is open.
const POLL: Duration = Duration::from_secs(1);
/// Each tunnel's hosts ("Where things go"), every this many looks.
const HOSTS_EVERY: u32 = 5;
/// Where a destination's rate ends (right-aligned).
const RATE_RIGHT: f32 = 900.;
const RATE_W: f32 = 64.;
/// The two-stroke tick before a learned destination, and its gap.
const TICK_W: f32 = 14.;
const TICK_GAP: f32 = 4.;
/// The window's inset: the title row and its columns start where every
/// other window's do (design/proposals/audit-2026-10-17.html, 10).
const SIDE: f32 = 20.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Now,
    Rules,
}

pub struct Traffic {
    model: Entity<Model>,
    tab: Tab,
    flows: Flows,
    /// The first paths are there when the window opens, not drawn in.
    settled: bool,
    max_height: f32,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for Traffic {}

impl Traffic {
    pub fn new(
        model: Entity<Model>,
        requests: async_channel::Sender<Request>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.observe_window_appearance(window, |_, _, cx| {
                Theme::current(cx).sync_components(cx);
                cx.notify();
            }),
        ];
        // The counters every second, and each tunnel's hosts every few,
        // for as long as the window is there.
        let tunnels = model.clone();
        cx.spawn(async move |this, cx| {
            let mut n = 0u32;
            loop {
                if requests.send(Request::Live).await.is_err() {
                    return;
                }
                if n.is_multiple_of(HOSTS_EVERY) {
                    let names: Vec<String> = tunnels.read_with(cx, |m, _| {
                        m.tunnels.iter().map(|t| t.name.clone()).collect()
                    });
                    for name in names {
                        let _ = requests.send(Request::Detail(name)).await;
                    }
                }
                n += 1;
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |_, _| {}).is_err() {
                    return;
                }
            }
        })
        .detach();
        Self {
            model,
            tab: Tab::Now,
            flows: Flows::default(),
            settled: false,
            max_height: crate::place::max_window_height_or(HEIGHT),
            // Focused as it opens: the keyboard reaches it (Tab, Escape).
            focus: {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                focus
            },
            _subscriptions: subscriptions,
        }
    }

    /// The graph: boxes and names over the paths and their dots.
    fn graph(&mut self, model: &Model, t: &Theme, window: &mut Window) -> Stateful<Div> {
        let ways = graph::ways(model);
        let l = graph::layout(&ways);
        // Every path one grey: a way is told by its name, a decision by the
        // tick (design/proposals/audit-2026-10-17.html, 1).
        let paths = graph::paths(&ways, &l, t.muted);
        let moving = crate::motion::enabled();
        let now = Instant::now();
        self.flows.step(&paths, now, moving);
        if !self.settled && model.live.rates.is_some() {
            self.flows.settle(now);
            self.settled = true;
        }
        let painted = self.flows.painted(paths, now);
        if moving && (self.flows.any_moving() || painted.iter().any(|p| p.drawn < 1.)) {
            window.request_animation_frame();
        }
        let (line, accent) = (t.line, t.accent);
        let mut g = div()
            .id("graph")
            .relative()
            .w(px(WIDTH))
            .h(px(l.height))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        graph::paint(&painted, line, accent, bounds.origin, !moving, window);
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(ME_X - ME_W / 2.))
                    .top(px(l.me - ME_H / 2.))
                    .w(px(ME_W))
                    .h(px(ME_H))
                    .rounded(px(8.))
                    .bg(t.surface)
                    .border_1()
                    .border_color(t.line)
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BOLD)
                    .child("This computer"),
            );
        for (way, (cy, hosts)) in ways.iter().zip(&l.ways) {
            // Every way the same: teal is for decisions, not for which way
            // (design/proposals/critique-2026-10-15, 2).
            let color = t.line_strong;
            g = g.child(
                div()
                    .id(gpui_kit::ElementId::Name(
                        format!("way:{}", way.label).into(),
                    ))
                    .absolute()
                    .left(px(WAY_X - WAY_W / 2.))
                    .top(px(cy - WAY_H / 2.))
                    .w(px(WAY_W))
                    .h(px(WAY_H))
                    .rounded(px(8.))
                    .bg(t.panel)
                    .border_1()
                    .border_color(color)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .line_height(px(18.))
                            .child(way.label.clone()),
                    )
                    .child(
                        div()
                            .font_family(fonts::MONO)
                            .text_size(px(11.5))
                            .line_height(px(15.))
                            .text_color(t.muted)
                            .child(rate(way.rate)),
                    )
                    .test_support(),
            );
            for (dest, hy) in way.hosts.iter().zip(hosts) {
                if dest.learned {
                    g = g.child(
                        div()
                            .id(gpui_kit::ElementId::Name(
                                format!("tick:{}:{}", way.label, dest.label).into(),
                            ))
                            .absolute()
                            .left(px(HOST_X - TICK_W - TICK_GAP))
                            .top(px(hy - TICK_W / 2.))
                            .size(px(TICK_W))
                            .child(
                                gpui_kit::svg()
                                    .path("crake/mark-routed.svg")
                                    .size(px(TICK_W))
                                    .text_color(t.accent),
                            )
                            .test_support(),
                    );
                }
                g = g
                    .child(
                        div()
                            .id(gpui_kit::ElementId::Name(
                                format!("dest:{}:{}", way.label, dest.label).into(),
                            ))
                            .absolute()
                            .left(px(HOST_X))
                            .top(px(hy - 9.))
                            .w(px(RATE_RIGHT - RATE_W - HOST_X - 8.))
                            .h(px(18.))
                            .font_family(fonts::MONO)
                            .text_size(px(12.5))
                            .text_color(if dest.more { t.muted } else { t.ink })
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(dest.label.clone())
                            .test_support(),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(RATE_RIGHT - RATE_W))
                            .top(px(hy - 9.))
                            .w(px(RATE_W))
                            .h(px(18.))
                            .flex()
                            .justify_end()
                            .font_family(fonts::MONO)
                            .text_size(px(11.5))
                            .text_color(t.muted)
                            .child(rate(dest.rate)),
                    );
            }
        }
        g
    }

    fn strip(&self, model: &Model, t: &Theme) -> Div {
        let bars = strip::bars(&model.live.history, Instant::now());
        let ticks =
            strip::ticks(&model.marks_within(strip::SPAN_MINUTES, None, strip::MERGE_MINUTES));
        let colours = strip::Colours {
            tunnels: t.muted,
            direct: t.bars,
            baseline: t.line_strong,
            accent: t.accent,
            undone: t.muted,
            panel: t.panel,
        };
        let legend = |color, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(div().size(px(10.)).rounded(px(2.)).bg(color))
                .child(label)
        };
        div()
            .px(px(SIDE))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Last 5 minutes")
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(t.muted)
                            .child(legend(t.muted, "through tunnels"))
                            .child(legend(t.bars, "direct"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        gpui_kit::svg()
                                            .path("crake/mark-routed.svg")
                                            .size(px(12.))
                                            .text_color(t.accent),
                                    )
                                    .child("routed"),
                            ),
                    ),
            )
            .child(
                div().mt(px(6.)).h(px(strip::HEIGHT)).w_full().child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            strip::paint(&bars, &ticks, bounds, colours, window);
                        },
                    )
                    .size_full(),
                ),
            )
            .child(
                div()
                    .mt(px(3.))
                    .flex()
                    .justify_between()
                    .font_family(fonts::MONO)
                    .text_size(px(11.))
                    .text_color(t.muted)
                    .child("5 min ago")
                    .child("now"),
            )
    }

    fn tabs(
        &self,
        now: &[tables::Row],
        rules: &[tables::Row],
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let tab = |id: &'static str, label: &'static str, n: usize, on: bool| {
            div()
                .id(id)
                .pb(px(8.))
                .flex()
                .items_baseline()
                .gap(px(4.))
                .cursor_pointer()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if on { t.ink } else { t.muted })
                .when(on, |d| d.border_b_2().border_color(t.accent))
                .child(label)
                .child(
                    div()
                        .font_weight(FontWeight::NORMAL)
                        .text_size(px(12.))
                        .text_color(t.muted)
                        .child(n.to_string()),
                )
        };
        div()
            .mx(px(SIDE))
            .mt(px(18.))
            .flex()
            .gap(px(22.))
            .border_b_1()
            .border_color(t.line)
            .child(
                tab("tab-now", "Right now", now.len(), self.tab == Tab::Now)
                    .keys()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tab = Tab::Now;
                        cx.notify();
                    }))
                    .test_support(),
            )
            .child(
                tab(
                    "tab-rules",
                    "Where things go",
                    rules.len(),
                    self.tab == Tab::Rules,
                )
                .keys()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tab = Tab::Rules;
                    cx.notify();
                }))
                .test_support(),
            )
    }

    fn detail(
        &self,
        model: &Model,
        now: &[tables::Row],
        rules: &[tables::Row],
        t: &Theme,
        cx: &gpui_kit::App,
    ) -> Div {
        let body = div().px(px(SIDE)).pb(px(20.));
        match self.tab {
            Tab::Now => match model.live.lists_connections() {
                Some(false) => body.child(note(
                    "Crake couldn\u{2019}t list the connections. The ways out above are live.",
                    t,
                )),
                None => body.child(note("Looking\u{2026}", t)),
                Some(true) if now.is_empty() => body.child(note("No connections open.", t)),
                Some(true) => body.child(tables::table(
                    &["Destination", "Way", "Now", "Since"],
                    now,
                    t,
                    cx,
                )),
            },
            Tab::Rules => body.child(tables::table(&["Destination", "Way", "Why"], rules, t, cx)),
        }
    }
}

/// A line where a table would be.
fn note(text: &'static str, t: &Theme) -> Div {
    div()
        .py(px(14.))
        .text_size(px(13.))
        .text_color(t.muted)
        .child(text)
}

impl Render for Traffic {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let model = self.model.read(cx).clone();
        let now_rows = tables::right_now(&model, Instant::now());
        let rule_rows = tables::where_things_go(&model);
        let total = model
            .live
            .rates
            .as_ref()
            .map(|r| r.direct + r.tunnels.values().sum::<f64>());
        let system = crate::chrome::system_buttons(cx);
        let header = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pt(px(14.))
            .pb(px(10.))
            .pl(px(if system {
                crate::chrome::BUTTONS_WIDTH
            } else {
                SIDE
            }))
            .pr(px(12.))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(
                div()
                    .text_size(px(22.))
                    .font_weight(FontWeight::BOLD)
                    .child("Traffic"),
            )
            .child(div().text_size(px(13.)).text_color(t.muted).child("Live"))
            .child(div().flex_1())
            .when_some(total, |h, total| {
                h.child(
                    div()
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format!("{} in all", rate(total))),
                )
            })
            .when(!system, |h| h.child(close_button(&t, cx)));
        let graph = self.graph(&model, &t, window);
        let body = div()
            .id("body")
            .flex_none()
            .max_h(px(self.max_height - 60.))
            .overflow_y_scroll()
            .child(graph)
            .child(self.strip(&model, &t))
            .child(self.tabs(&now_rows, &rule_rows, &t, cx))
            .child(self.detail(&model, &now_rows, &rule_rows, &t, cx));
        div()
            .id("traffic")
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
            .on_key_down(crate::focus::tab_moves)
            .on_key_down(cx.listener(|_, e: &gpui_kit::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    cx.emit(Close);
                }
            }))
            .child(header)
            .child(body)
    }
}

fn close_button(t: &Theme, cx: &mut Context<Traffic>) -> impl IntoElement {
    crate::chrome::close_button(t)
        .keys()
        .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))
        .test_support()
}
