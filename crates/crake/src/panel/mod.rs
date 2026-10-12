//! The main surface (design/screens/tray-panel.html): what Crake is doing,
//! the last hour, what it did with Undo, the tunnels with their switches,
//! and Copy for helpdesk. Opened from the tray; closes when it loses focus.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, ClipboardItem, Context, Div, Entity, FontWeight, MouseButton, Stateful,
    Subscription, TestSupportExt, Window, canvas, div, px, svg,
};

use crate::Request;
use crate::model::{Action, Decision, Model, tunnel_state};
use crate::theme::{Theme, fonts};

mod chart;
mod decisions;
mod tunnels;

#[cfg(test)]
mod tests;

pub const WIDTH: f32 = 452.;

/// The panel's rows, as tall as in design/screens/tray-panel.html: the
/// header, the chart, each section's heading, a decision, a tunnel, the
/// footer.
const HEADER: f32 = 62.;
const CHART: f32 = 128.;
const SECTION: f32 = 44.;
const DECISION: f32 = 66.;
const TUNNEL: f32 = 62.;
const FOOTER: f32 = 72.;
/// Motion (CLAUDE.md, 7c): the panel opens rising RISE px over OPENS ms; a
/// switch slides on a critically damped spring (damping = 2 * sqrt of the
/// stiffness), settling in about 150 ms.
const OPENS: u64 = 180;
const RISE: f32 = 8.;
const SPRING_STIFFNESS: f32 = 1600.;
const SPRING_DAMPING: f32 = 80.;
const MS_PER_SEC: f32 = 1000.;

/// Transparent room around the panel (only its rounded corners need any).
pub const MARGIN: f32 = 0.;

/// Decisions shown in the panel; the full log has the rest.
pub(crate) const DECISIONS: usize = 3;

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
    let t = (since.elapsed().as_secs_f32() * MS_PER_SEC / slow - delay as f32) / ms as f32;
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
    /// Where the pointer is on the chart (`crate::timeline`).
    chart_hover: crate::timeline::Hover,
    /// The "?" card after "What Crake did" is open.
    help_open: bool,
    _subscriptions: Vec<Subscription>,
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
            && at.elapsed() < crate::model::FAILURE_SHOWN
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
        let open = phase(self.motion.opened, 0, OPENS, window);
        let rise = if self.upward { RISE } else { -RISE };
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
    let spring = SpringAnimation::new(SpringConfig::new(
        SPRING_STIFFNESS / (slow * slow),
        SPRING_DAMPING / slow,
        1.,
    ))
    .to(on);
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
            if (f32::from(have.height) - f32::from(want.height)).abs() > crate::windows::FIT_SLACK {
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
            chart_hover: crate::timeline::Hover {
                at: std::env::var("CRAKE_SHOW")
                    .ok()
                    .and_then(|v| v.strip_prefix("hover:")?.parse().ok()),
                ..Default::default()
            },
            help_open: false,
            _subscriptions: subscriptions,
        }
    }

    /// The panel's height for the current content, so the window fits it.
    pub fn height(model: &Model) -> f32 {
        let decisions = model.decisions(DECISIONS).len().max(1) as f32;
        let tunnels = model.tunnels.len().max(1) as f32;
        HEADER + CHART + SECTION + decisions * DECISION + SECTION + tunnels * TUNNEL + FOOTER
    }

    fn send(&self, r: Request) {
        self.requests.send_blocking(r).ok();
    }

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
                    .on_click(cx.listener(|this, _, _, _| this.send(Request::Settings)))
                    .test_support(),
            )
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
                    .on_click(cx.listener(|this, _, _, _| this.send(Request::Log)))
                    .test_support(),
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
                }))
                .test_support(),
            )
    }
}
