//! The main surface (design/screens/tray-panel.html): what Crake is doing,
//! the last hour, what it did with Undo, the tunnels with their switches,
//! and Copy for helpdesk. Opened from the tray; closes when it loses focus.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, ClipboardItem, Context, Div, Entity, FontWeight, MouseButton, PathBuilder, Pixels,
    Stateful, Subscription, Window, canvas, div, point, px, svg,
};

use crate::Request;
use crate::model::{Action, Decision, Model, tunnel_state};
use crate::theme::{Theme, fonts};

pub const WIDTH: f32 = 452.;
/// The chart's width inside the panel (its border and 18 px sides off).
const CHART_WIDTH: f32 = WIDTH - 2. - 36.;
/// Transparent room around the panel (only its rounded corners need any).
pub const MARGIN: f32 = 0.;
/// Decisions shown in the panel; the full log has the rest.
const DECISIONS: usize = 3;

/// What moves in the panel (design/proposals/animations.html). It outlives a
/// refit (the window reopened at its measured height), so nothing replays.
pub struct Motion {
    opened: Instant,
    /// The newest decision when the panel opened: only later ones arrive.
    seen: Option<jiff::Timestamp>,
    /// When each later one was first shown.
    arrived: RefCell<HashMap<String, Instant>>,
}

impl Motion {
    pub fn new(model: &Model) -> Rc<Self> {
        let seen = model
            .log
            .iter()
            .filter(|e| e.kind != crake_proto::LogKind::System)
            .filter_map(|e| e.ts.parse().ok())
            .max();
        Rc::new(Self {
            opened: Instant::now(),
            seen,
            arrived: Default::default(),
        })
    }

    /// When a decision arrived, if it came after the panel opened.
    fn arrival(&self, ts: &str) -> Option<Instant> {
        let at: jiff::Timestamp = ts.parse().ok()?;
        (Some(at) > self.seen).then(|| {
            *self
                .arrived
                .borrow_mut()
                .entry(ts.to_owned())
                .or_insert_with(Instant::now)
        })
    }
}

/// How far along (0 to 1, eased) a motion `ms` long is that starts `delay` ms
/// after `since`. Asks for the next frame until it has landed.
fn phase(since: Instant, delay: u64, ms: u64, window: &mut Window) -> f32 {
    if !crate::motion::enabled() {
        return 1.;
    }
    let slow = crate::motion::slowed() as f32;
    let t = (since.elapsed().as_secs_f32() * 1000. / slow - delay as f32) / ms as f32;
    if t < 1. {
        window.request_animation_frame();
    }
    crate::motion::ease_out(t)
}

pub struct Panel {
    model: Entity<Model>,
    motion: Rc<Motion>,
    requests: async_channel::Sender<Request>,
    /// It has been in front once (only then does losing focus close it).
    been_active: bool,
    /// It opened upward from a bottom panel: its bottom edge stays put.
    upward: bool,
    /// Where the pointer is on the chart, in minutes ago.
    hover: Option<f32>,
    /// The chart's left edge and width (window pixels), from its last paint.
    chart_geometry: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    _subscriptions: Vec<Subscription>,
}

impl Panel {
    pub fn new(
        model: Entity<Model>,
        requests: async_channel::Sender<Request>,
        motion: Rc<Motion>,
        upward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.observe_window_activation(window, |this: &mut Self, window, cx| {
                crate::debug(&format!(
                    "panel focus: active={} (been active: {})",
                    window.is_window_active(),
                    this.been_active
                ));
                if window.is_window_active() {
                    this.been_active = true;
                } else if this.been_active {
                    window.remove_window();
                    crate::panel_blurred(window.window_handle(), cx);
                }
            }),
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
            cx.observe_global::<crate::prefs::Prefs>(|_, cx| cx.notify()),
        ];
        Self {
            model,
            motion,
            requests,
            been_active: window.is_window_active(),
            upward,
            // CRAKE_SHOW=hover:<minutes ago> (development): as if pointing there.
            hover: std::env::var("CRAKE_SHOW")
                .ok()
                .and_then(|v| v.strip_prefix("hover:")?.parse().ok()),
            chart_geometry: Default::default(),
            _subscriptions: subscriptions,
        }
    }

    /// The panel's height for the current content, so the window fits it.
    pub fn height(model: &Model) -> f32 {
        let decisions = model.decisions(DECISIONS).len().max(1) as f32;
        let tunnels = model.tunnels.len().max(1) as f32;
        62. + 128. + 44. + decisions * 66. + 44. + tunnels * 62. + 72.
    }

    fn send(&self, r: Request) {
        self.requests.send_blocking(r).ok();
    }
}

impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        // A snapshot, so the view can be built with `cx` free for its handlers.
        let model = &self.model.read(cx).clone();
        let reachable = model.reachable == Some(true);

        let mut panel = div()
            .relative()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .bg(t.panel)
            .border_1()
            .border_color(t.line)
            // The hairline is the edge (DESIGN.md: no offset shadows). A drawn
            // shadow bands through XWayland's transparency.
            .rounded(px(10.))
            .text_color(t.ink)
            .font_family(fonts::UI)
            .text_size(px(14.))
            .line_height(px(20.))
            .child(self.header(model, &t, cx));

        if reachable {
            panel = panel
                .child(self.chart(model, &t, window, cx))
                .child(self.decisions(model, &t, window, cx))
                .child(self.tunnels(model, &t, cx))
                .child(self.footer(model, &t, cx));
        } else {
            panel = panel.child(not_running(model, &t));
        }
        if let Some((text, at)) = &model.failed
            && at.elapsed() < Duration::from_secs(8)
        {
            panel = panel.child(
                div()
                    .px(px(18.))
                    .pb(px(12.))
                    .text_size(px(13.))
                    .text_color(t.danger)
                    .child(text.clone()),
            );
        }
        // It rises from a bottom bar (drops from a top one) and fades in.
        let open = phase(self.motion.opened, 0, 180, window);
        let rise = if self.upward { 8. } else { -8. };
        let panel = panel.top(px(rise * (1. - open))).opacity(open);
        // The window is the panel; it follows the panel's height.
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .p(px(MARGIN))
            .child(panel.child(fit_window(self.upward)))
    }
}

impl Panel {
    fn header(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let listening = model.status() == "Listening";
        div()
            .id("header")
            .flex()
            .items_center()
            .gap(px(10.))
            .pt(px(14.))
            .pb(px(12.))
            .pl(px(18.))
            .pr(px(14.))
            // The panel has no title bar: drag it by its header.
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(
                div()
                    .relative()
                    .size(px(24.))
                    .flex_none()
                    .child(
                        svg()
                            .path("crake/bird.svg")
                            .size(px(24.))
                            .text_color(t.ink)
                            .absolute(),
                    )
                    .child(
                        svg()
                            .path("crake/call.svg")
                            .size(px(24.))
                            .text_color(t.accent)
                            .absolute(),
                    ),
            )
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::BOLD)
                    .child("Crake"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .ml(px(6.))
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(div().size(px(8.)).rounded_full().bg(if listening {
                        t.accent
                    } else {
                        t.line_strong
                    }))
                    .child(model.status()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("settings")
                    .size(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.surface))
                    .child(
                        svg()
                            .path("crake/sliders.svg")
                            .size(px(20.))
                            .text_color(t.muted),
                    )
                    // Stop the header's window drag from taking the click.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, _| this.send(Request::Settings))),
            )
    }

    /// "Last hour" (DESIGN.md, Charts): grey bars are how much traffic went
    /// direct per minute; teal two-stroke ticks the moments Crake routed a
    /// host; hollow circles hosts it tried and left alone. Pointing at it
    /// reads it: a dotted hairline at the minute, one line about it under
    /// the chart.
    fn chart(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let marks = model.marks();
        let now_ts = jiff::Timestamp::now();
        let now_minute = now_ts.as_second() as u64 / 60;
        let bars: Vec<(f32, u64)> = model
            .traffic
            .iter()
            .filter(|(m, _)| now_minute.saturating_sub(*m) <= 60)
            .map(|(m, bytes)| ((now_minute - m) as f32, *bytes))
            .collect();
        let max = bars.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32;

        // What the pointer is on: a mark close by, else that minute's bar.
        let pointed = self.hover.map(|h| {
            match marks
                .iter()
                .filter(|m| (m.ago - h).abs() <= 0.75)
                .min_by(|a, b| (a.ago - h).abs().total_cmp(&(b.ago - h).abs()))
            {
                Some(m) => {
                    // The hairline stops above what stands there (in the
                    // chart's 72 px drawing): the count, the tick and its gap,
                    // or the circle.
                    let top = match (m.routed, m.count) {
                        (true, n) if n > 1 => 12.,
                        (true, _) => 30. - 1.5,
                        (false, _) => 72. - 10. - 4.5,
                    };
                    (m.ago, None, m.time.clone(), m.text.clone(), top)
                }
                None => {
                    let ago = h.round();
                    let minute = now_minute.saturating_sub(ago as u64);
                    let time = jiff::Timestamp::from_second(minute as i64 * 60)
                        .map(|t| {
                            t.to_zoned(jiff::tz::TimeZone::system())
                                .strftime("%H:%M")
                                .to_string()
                        })
                        .unwrap_or_default();
                    let bytes = model
                        .traffic
                        .iter()
                        .find(|(m, _)| *m == minute)
                        .map(|(_, b)| *b);
                    // Above the bar, or down to the baseline when there is none.
                    let top = match bytes {
                        Some(b) if b > 0 => 72. - (3. + 22. * (b as f32 / max).sqrt()),
                        _ => 72. + 3.,
                    };
                    let text = match model.traffic.iter().find(|(m, _)| *m == minute) {
                        Some((_, 0)) => "Nothing went direct".to_owned(),
                        Some((_, b)) => format!("{} went direct", crate::model::human_bytes(*b)),
                        None => "Crake wasn\u{2019}t counting yet".to_owned(),
                    };
                    (ago, Some(ago), time, text, top)
                }
            }
        });
        let hover_x = pointed.as_ref().map(|p| (p.0, p.4));
        let hover_bar = pointed.as_ref().and_then(|p| p.1);

        let (accent, muted, panel, base, grey) =
            (t.accent, t.muted, t.panel, t.line_strong, t.bars);
        // A new tick grows from the baseline, one stroke after the other.
        let paint_marks: Vec<(f32, bool, [f32; 2])> = marks
            .iter()
            .map(|m| {
                let grown = match self.motion.arrival(&m.ts) {
                    Some(at) => [phase(at, 0, 260, window), phase(at, 90, 260, window)],
                    None => [1., 1.],
                };
                (m.ago, m.routed, grown)
            })
            .collect();
        let now = jiff::Zoned::now();
        let label = |mins: i64| {
            now.checked_sub(jiff::SignedDuration::from_mins(mins))
                .map(|z| z.strftime("%H:%M").to_string())
                .unwrap_or_default()
        };
        let mono = |s: String| {
            div()
                .font_family(fonts::MONO)
                .text_size(px(11.))
                .text_color(t.muted)
                .child(s)
        };
        let geometry = self.chart_geometry.clone();

        // Counts above merged ticks, in mono 11 px accent.
        let mut counts = div().absolute().top_0().left_0().size_full();
        for m in marks.iter().filter(|m| m.routed && m.count > 1) {
            let x = 2. + (CHART_WIDTH - 4.) * (60. - m.ago) / 60. + 2.5;
            counts = counts.child(
                div()
                    .absolute()
                    .top(px(12.))
                    .left(px(x - 10.))
                    .w(px(20.))
                    .flex()
                    .justify_center()
                    .font_family(fonts::MONO)
                    .text_size(px(11.))
                    .line_height(px(12.))
                    .text_color(t.accent)
                    .child(m.count.to_string()),
            );
        }

        let under = match &pointed {
            Some((_, _, time, text, _)) => div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .min_w_0()
                .child(mono(time.clone()))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .text_color(t.ink)
                        .child(text.clone()),
                ),
            None => div()
                .flex()
                .justify_between()
                .w_full()
                .child(mono(label(60)))
                .child(mono(label(30)))
                .child(mono("now".into())),
        };

        div()
            .px(px(18.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Last hour"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .text_color(t.muted)
                            .child(div().size(px(10.)).mr(px(5.)).bg(t.bars))
                            .child("direct")
                            .child(div().ml(px(8.)).w(px(3.)).h(px(11.)).bg(t.accent))
                            .child(div().ml(px(2.)).w(px(3.)).h(px(8.)).bg(t.accent))
                            .child(div().ml(px(5.)).child("routed")),
                    ),
            )
            .child(
                div()
                    .id("chart")
                    .relative()
                    .w_full()
                    .h(px(73.))
                    .mt(px(8.))
                    .on_mouse_move(cx.listener(|this, e: &gpui_kit::MouseMoveEvent, _, cx| {
                        let (left, width) = this.chart_geometry.get();
                        if width <= 4. {
                            return;
                        }
                        let x = f32::from(e.position.x) - left;
                        let ago = (60. * (1. - (x - 2.) / (width - 4.))).clamp(0., 60.);
                        if this.hover != Some(ago) {
                            this.hover = Some(ago);
                            cx.notify();
                        }
                    }))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !*hovered {
                            this.hover = None;
                            cx.notify();
                        }
                    }))
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                geometry
                                    .set((f32::from(bounds.left()), f32::from(bounds.size.width)));
                            },
                            move |bounds, _, window, _| {
                                let w = f32::from(bounds.size.width);
                                let (left, top) =
                                    (f32::from(bounds.left()), f32::from(bounds.top()));
                                // The design's 416 x 72 drawing, at the panel's width.
                                let baseline = top + 72.;
                                let x_of = |ago: f32| left + 2. + (w - 4.) * (60. - ago) / 60.;
                                for (ago, bytes) in &bars {
                                    let x = x_of(*ago);
                                    if *bytes == 0 {
                                        continue;
                                    }
                                    let h = 3. + 22. * (*bytes as f32 / max).sqrt();
                                    if let Some(p) = crate::chart::bar(
                                        x,
                                        3.6,
                                        baseline,
                                        h,
                                        window.scale_factor(),
                                    ) {
                                        let on = hover_bar.is_some_and(|b| (b - ago).abs() < 0.01);
                                        window.paint_path(p, if on { muted } else { grey });
                                    }
                                }
                                // The hairline never draws over a bar, a tick or a circle:
                                // it ends 3 px above whatever stands at that minute.
                                if let Some((ago, stop)) = hover_x {
                                    let x = x_of(ago);
                                    let end = top + stop - 3.;
                                    if end - (top + 2.) >= 4. {
                                        let mut line = PathBuilder::stroke(px(1.))
                                            .dash_array(&[px(2.), px(3.)]);
                                        line.move_to(point(px(x), px(top + 2.)));
                                        line.line_to(point(px(x), px(end)));
                                        if let Ok(p) = line.build() {
                                            window.paint_path(p, base);
                                        }
                                    }
                                }
                                // A tick keeps clear of the bars: a gap in the panel's
                                // colour around it, drawn before the baseline so the
                                // baseline stays whole (DESIGN.md, Charts).
                                // A stroke grown `g` of the way up from the baseline.
                                let stroke_top = |top: f32, g: f32| {
                                    (baseline - (72. - top) * g).min(baseline - 1.6)
                                };
                                for (ago, routed, grown) in &paint_marks {
                                    if !*routed {
                                        continue;
                                    }
                                    let x = x_of(*ago);
                                    for ((dx, top), g) in
                                        [(0., 30.), (5., 40.)].into_iter().zip(grown)
                                    {
                                        if *g <= 0. {
                                            continue;
                                        }
                                        if let Some(p) =
                                            tick_gap(x + dx, stroke_top(top, *g), baseline, 3.2)
                                        {
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
                                for (ago, routed, grown) in &paint_marks {
                                    let x = x_of(*ago);
                                    if *routed {
                                        for ((dx, top), g) in
                                            [(0., 30.), (5., 40.)].into_iter().zip(grown)
                                        {
                                            if *g <= 0. {
                                                continue;
                                            }
                                            if let Some(p) =
                                                tick_bar(x + dx, stroke_top(top, *g), baseline, 3.2)
                                            {
                                                window.paint_path(p, accent);
                                            }
                                        }
                                    } else {
                                        if let Some(p) = circle(x, baseline - 10., 4.5, None) {
                                            window.paint_path(p, panel);
                                        }
                                        if let Some(p) = circle(x, baseline - 10., 4.5, Some(1.6)) {
                                            window.paint_path(p, muted);
                                        }
                                    }
                                }
                            },
                        )
                        .size_full(),
                    )
                    .child(counts),
            )
            .child(
                div()
                    .h(px(18.))
                    .mt(px(4.))
                    .flex()
                    .items_baseline()
                    .child(under),
            )
    }

    fn decisions(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = model.decisions(DECISIONS);
        let mut list = section("What Crake did", t).mt(px(14.));
        if rows.is_empty() {
            list = list.child(
                div()
                    .px(px(18.))
                    .pb(px(12.))
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child("Nothing yet. Crake routes nothing until something needs a tunnel."),
            );
        }
        for (i, d) in rows.into_iter().enumerate() {
            // A decision made while the panel is open slides in from above.
            let arrived = self.motion.arrival(&d.entry.ts);
            let row = self.decision(i, d, t, cx);
            list = list.child(match arrived {
                Some(at) => {
                    let p = phase(at, 0, 220, window);
                    div()
                        .relative()
                        .top(px(-6. * (1. - p)))
                        .opacity(p)
                        .child(row)
                        .into_any_element()
                }
                None => row,
            });
        }
        list
    }

    fn decision(&self, i: usize, d: Decision, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        use crake_proto::LogKind;
        let (mark, color) = match d.entry.kind {
            LogKind::Routed if d.entry.message.starts_with("Pinned") => {
                ("crake/mark-pinned.svg", t.ink)
            }
            LogKind::Routed => ("crake/mark-routed.svg", t.accent),
            LogKind::Undone => ("crake/mark-undone.svg", t.muted),
            LogKind::Asked => ("crake/help.svg", t.accent),
            _ => ("crake/mark-left-alone.svg", t.muted),
        };
        let mut right = div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(4.))
            .flex_none()
            .child(
                div()
                    .font_family(fonts::MONO)
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child(d.time.clone()),
            );
        if let (Some(action), Some(tunnel)) = (d.action.clone(), d.entry.tunnel.clone()) {
            let hosts = d.entry.hosts.clone();
            let label = match action {
                Action::Undo => "Undo".to_owned(),
                Action::Redo => "Redo".to_owned(),
                Action::Send => format!(
                    "Send through {}",
                    self.model.read(cx).display_of(Some(&tunnel))
                ),
            };
            right = right.child(
                button(("act", i), label, t)
                    .px(px(10.))
                    .h(px(30.))
                    .on_click(cx.listener(move |this, _, _, _| {
                        let r = match action {
                            Action::Undo => Request::Undo(tunnel.clone(), hosts.clone()),
                            Action::Redo => Request::Redo(tunnel.clone(), hosts.clone()),
                            Action::Send => Request::Send(tunnel.clone(), hosts.clone()),
                        };
                        this.send(r);
                    })),
            );
        }
        div()
            .flex()
            .items_start()
            .gap(px(10.))
            .pt(px(8.))
            .pb(px(10.))
            .px(px(18.))
            .child(
                svg()
                    .path(mark)
                    .size(px(18.))
                    .flex_none()
                    .mt(px(2.))
                    .text_color(color),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_size(px(14.)).child(d.entry.message.clone()))
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(12.5))
                            .line_height(px(17.))
                            .text_color(t.muted)
                            .child(d.entry.reason.clone().unwrap_or_default()),
                    ),
            )
            .child(right)
            .into_any_element()
    }

    fn tunnels(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = section("Tunnels", t).mt(px(6.));
        if model.tunnels.is_empty() {
            list = list.child(
                div()
                    .px(px(18.))
                    .pb(px(12.))
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child("No tunnels yet."),
            );
        }
        for (i, tunnel) in model.tunnels.iter().enumerate() {
            let on = matches!(tunnel.state.as_str(), "connected" | "connecting");
            let name = tunnel.name.clone();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(9.))
                    .px(px(18.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if on { t.ink } else { t.muted })
                                            .child(tunnel.display.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(
                                                if tunnel.state == "trouble"
                                                    || (tunnel.state == "connected"
                                                        && tunnel.dns_down)
                                                {
                                                    t.danger
                                                } else {
                                                    t.muted
                                                },
                                            )
                                            .child(tunnel_state(tunnel)),
                                    )
                                    .when(tunnel.managed, |row| {
                                        row.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.))
                                                .px(px(7.))
                                                .py(px(1.))
                                                .rounded(px(4.))
                                                .bg(t.surface)
                                                .text_size(px(12.))
                                                .line_height(px(16.))
                                                .text_color(t.muted)
                                                .child(
                                                    svg()
                                                        .path("crake/lock.svg")
                                                        .size(px(12.))
                                                        .text_color(t.muted),
                                                )
                                                .child("Managed"),
                                        )
                                    }),
                            )
                            .child({
                                // Who decides, and what's allowed (DESIGN.md:
                                // policy is visible). A full tunnel says it in ink.
                                let (icon, text, color) = if tunnel.managed {
                                    let icon = if tunnel.full_tunnel {
                                        "crake/block.svg"
                                    } else {
                                        "crake/shield.svg"
                                    };
                                    let color = if tunnel.full_tunnel { t.ink } else { t.muted };
                                    (icon, tunnel.policy.clone(), color)
                                } else {
                                    (
                                        "crake/person.svg",
                                        "Your profile · learning on".to_owned(),
                                        t.muted,
                                    )
                                };
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .mt(px(2.))
                                    .text_size(px(13.))
                                    .text_color(color)
                                    .child(
                                        svg()
                                            .path(icon)
                                            .size(px(14.))
                                            .flex_none()
                                            .text_color(color),
                                    )
                                    .child(div().min_w_0().truncate().child(text))
                            }),
                    )
                    .child(switch(("switch", i), on, t).on_click(cx.listener(
                        move |this, _, _, _| {
                            this.send(Request::Switch(name.clone(), !on));
                        },
                    ))),
            );
            // Said openly, under the tunnel, until the user has read it
            // (design/proposals/anyconnect-notice.html).
            if cx
                .global::<crate::prefs::Prefs>()
                .acting_as(&tunnel.name, &tunnel.gateway)
                .is_some()
            {
                let key = format!("anyconnect:{}", tunnel.name);
                let d = &tunnel.display;
                list = list.child(
                    div()
                        .mx(px(18.))
                        .mb(px(8.))
                        .px(px(12.))
                        .py(px(10.))
                        .rounded(px(8.))
                        .bg(t.surface)
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .text_size(px(13.))
                        .child(div().flex_1().min_w_0().child(format!(
                            "Crake introduces itself to {d} as Cisco AnyConnect, because {d}\u{2019}s gateway only lets AnyConnect in."
                        )))
                        .child(
                            div()
                                .id(("got-it", i))
                                .flex_none()
                                .text_color(t.accent)
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child("Got it")
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.send(Request::Dismiss(key.clone()));
                                })),
                        ),
                );
            }
        }
        list
    }

    fn footer(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let copied = model
            .copied_at
            .is_some_and(|at| at.elapsed() < Duration::from_secs(2));
        let model = self.model.clone();
        div()
            .flex()
            .gap(px(8.))
            .pt(px(12.))
            .pb(px(14.))
            .px(px(14.))
            .mt(px(6.))
            .border_t_1()
            .border_color(t.line)
            // As in tray-panel.html: Full log, then Copy for helpdesk.
            .child(
                button("full-log", "Full log", t)
                    .flex_1()
                    .h(px(40.))
                    .on_click(cx.listener(|this, _, _, _| this.send(Request::Log))),
            )
            .child(
                button(
                    "helpdesk",
                    if copied {
                        "Copied"
                    } else {
                        "Copy for helpdesk"
                    },
                    t,
                )
                .flex_1()
                .h(px(40.))
                .on_click(cx.listener(move |_, _, _, cx| {
                    let hide = cx.global::<crate::prefs::Prefs>().hide_hosts;
                    let text = model.read(cx).helpdesk(hide);
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    model.update(cx, |m, cx| {
                        m.copied_at = Some(std::time::Instant::now());
                        cx.notify();
                    });
                    let model = model.clone();
                    cx.spawn(async move |_, cx| {
                        cx.background_executor().timer(Duration::from_secs(2)).await;
                        model.update(cx, |_, cx| cx.notify());
                    })
                    .detach();
                })),
            )
    }
}

fn not_running(model: &Model, t: &Theme) -> impl IntoElement {
    let (title, detail) = if model.reachable.is_none() {
        ("Looking for Crake's service…", "")
    } else {
        (
            "Crake's service isn't running.",
            "Nothing is routed. This panel fills in by itself once it's back.",
        )
    };
    div()
        .px(px(18.))
        .pt(px(8.))
        .pb(px(22.))
        .border_t_1()
        .border_color(t.line)
        .child(
            div()
                .pt(px(12.))
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .mt(px(4.))
                .text_size(px(13.))
                .text_color(t.muted)
                .child(detail),
        )
}

/// A titled group under a hairline.
fn section(title: &'static str, t: &Theme) -> Div {
    div().border_t_1().border_color(t.line).child(
        div()
            .pt(px(12.))
            .pb(px(4.))
            .px(px(18.))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(title),
    )
}

fn button(
    id: impl Into<gpui_kit::ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    t: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(t.line)
        .rounded(px(6.))
        .bg(t.panel)
        .text_color(t.ink)
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.bg(t.surface))
        .child(label.into())
}

/// The platform's rounded pill: teal when on. When it changes, the knob
/// slides and the track turns, about 150 ms, no overshoot.
pub(crate) fn switch(id: impl Into<gpui_kit::ElementId>, on: bool, t: &Theme) -> Stateful<Div> {
    use gpui_kit::{AnimationExt, SpringAnimation, SpringConfig};
    // Critically damped (damping = 2 * sqrt(stiffness)): settles in ~150 ms.
    let slow = crate::motion::slowed() as f32;
    let spring =
        SpringAnimation::new(SpringConfig::new(1600. / (slow * slow), 80. / slow, 1.)).to(on);
    let (off_color, on_color) = (t.track, t.accent);
    div()
        .id(id)
        .flex_none()
        .w(px(46.))
        .h(px(26.))
        .cursor_pointer()
        .child(div().relative().size_full().rounded_full().with_spring(
            "knob",
            spring,
            move |track, phase| {
                let p = phase.clamp(0. ..=1.);
                track.bg(p.interpolate(off_color, on_color)).child(
                    div()
                        .absolute()
                        .top(px(3.))
                        .left(px(3. + 20. * p.0))
                        .size(px(20.))
                        .rounded_full()
                        .bg(gpui_kit::rgb(0xFFFFFF))
                        .shadow_sm(),
                )
            },
        ))
}

/// Makes the window as tall as the panel, measured after layout. Opening
/// downward the window simply grows or shrinks; opening upward its bottom must
/// stay put, so the app reopens it at the measured height.
fn fit_window(upward: bool) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            // Measured inside the panel's 1 px border: add it back, or the
            // window cuts off the bottom hairline.
            let height = f32::from(bounds.size.height) + 2.;
            let want = gpui_kit::size(px(WIDTH + 2. * MARGIN), px(height + 2. * MARGIN));
            let have = window.viewport_size();
            if (f32::from(have.height) - f32::from(want.height)).abs() > 0.5 {
                window.on_next_frame(move |window, cx| {
                    if upward {
                        crate::refit_panel(height, cx);
                    } else {
                        window.resize(want);
                    }
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// One stroke of the two-stroke tick in a chart: flat bottom on the
/// baseline, rounded top (design/DESIGN.md, "The signature").
fn tick_bar(x: f32, top: f32, base: f32, w: f32) -> Option<gpui_kit::Path<Pixels>> {
    let r = w / 2.;
    let mut p = PathBuilder::fill();
    p.move_to(point(px(x - r), px(base)));
    p.line_to(point(px(x - r), px(top + r)));
    p.arc_to(
        point(px(r), px(r)),
        px(0.),
        false,
        true,
        point(px(x + r), px(top + r)),
    );
    p.line_to(point(px(x + r), px(base)));
    p.close();
    p.build().ok()
}

/// The 1.5 px gap around one stroke of a tick: the same shape, widened by
/// the gap on each side, painted in the panel's colour before the tick.
fn tick_gap(x: f32, top: f32, base: f32, w: f32) -> Option<gpui_kit::Path<Pixels>> {
    const GAP: f32 = 1.5;
    tick_bar(x, top - GAP, base, w + 2. * GAP)
}

/// A circle, filled or (with a width) stroked.
fn circle(cx: f32, cy: f32, r: f32, stroke: Option<f32>) -> Option<gpui_kit::Path<Pixels>> {
    let mut p = match stroke {
        Some(w) => PathBuilder::stroke(px(w)),
        None => PathBuilder::fill(),
    };
    let radii = point(px(r), px(r));
    p.move_to(point(px(cx - r), px(cy)));
    p.arc_to(radii, px(0.), false, true, point(px(cx + r), px(cy)));
    p.arc_to(radii, px(0.), false, true, point(px(cx - r), px(cy)));
    p.close();
    p.build().ok()
}
