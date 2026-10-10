//! The full log (design/screens/log.html): every decision as a sentence with
//! its reason, by day, newest first; filters, search, the day's chart,
//! Copy for helpdesk and Export.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, ClipboardItem, Context, Div, Entity, FocusHandle, FontWeight, PathBuilder, Pixels,
    Stateful, Subscription, Window, canvas, div, point, px, svg,
};

use crake_proto::{LogEntry, LogKind};

use crate::model::{Day, Model, Row, Show};
use crate::theme::{Theme, fonts};

pub const WIDTH: f32 = 980.;
/// The day chart's drawing (log.html: 940 × 84, baseline at 62).
const CHART_HEIGHT: f32 = 84.;
const BASELINE: f32 = 62.;
/// The closest the day chart zooms in, in minutes.
const MIN_SPAN: f32 = 15.;

pub struct Log {
    model: Entity<Model>,
    search: Entity<InputState>,
    show: Show,
    /// A tunnel's name, or all.
    tunnel: Option<String>,
    picking_tunnel: bool,
    earlier_days: bool,
    copied: bool,
    /// The day chart's zoom: (from, to) in minutes since midnight.
    view: Option<(f32, f32)>,
    /// Where the pointer is on the day chart, in minutes since midnight.
    hover: Option<f32>,
    /// A drag along the chart: where it started (x) and what was shown then.
    drag: Option<(f32, (f32, f32))>,
    /// The chart's left edge and width (window pixels), from its last paint.
    chart_geometry: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Asks the app to close the window.
pub struct Close;

impl gpui_kit::EventEmitter<Close> for Log {}

impl Log {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Host, address or tunnel"));
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.observe_window_appearance(window, |_, _, cx| {
                Theme::current(cx).sync_components(cx);
                cx.notify();
            }),
        ];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            model,
            search,
            show: Show::Everything,
            tunnel: None,
            picking_tunnel: false,
            earlier_days: false,
            copied: false,
            view: std::env::var("CRAKE_SHOW").ok().and_then(|v| {
                let (a, b) = v.strip_prefix("log@")?.split_once('-')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            }),
            hover: None,
            drag: None,
            chart_geometry: Default::default(),
            focus,
            _subscriptions: subscriptions,
        }
    }

    fn days(&self, cx: &Context<Self>) -> Vec<Day> {
        let search = self.search.read(cx).value().to_string();
        self.model
            .read(cx)
            .log_days(self.show, self.tunnel.as_deref(), &search)
    }

    /// The rows shown, as plain text for whoever helps.
    fn as_text(&self, cx: &Context<Self>) -> String {
        let now = jiff::Zoned::now().strftime("%Y-%m-%d %H:%M %Z").to_string();
        let mut s = format!("Crake log, {now}\n");
        let hide = cx.global::<crate::prefs::Prefs>().hide_hosts;
        let say = |t: &str| {
            if hide {
                crate::model::scrub(t)
            } else {
                t.to_owned()
            }
        };
        for day in self.days(cx) {
            s += &format!("\n{}\n", day.title);
            for r in day.rows {
                s += &format!("  {}  {}  [{}]\n", r.time, say(&r.text), r.badge);
                if !r.why.is_empty() {
                    s += &format!("         {}\n", say(&r.why));
                }
            }
        }
        s
    }

    fn copy_for_helpdesk(&mut self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.as_text(cx)));
        self.copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Export…: the rows shown, as a text file wherever the user puts it.
    fn export(&mut self, cx: &mut Context<Self>) {
        let text = self.as_text(cx);
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let dir = [home.join("Documents"), home.clone()]
            .into_iter()
            .find(|d| d.is_dir())
            .unwrap_or(home);
        let name = format!("crake-log-{}.txt", jiff::Zoned::now().strftime("%Y-%m-%d"));
        let chosen = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |_, _| {
            if let Ok(Ok(Some(path))) = chosen.await {
                let _ = std::fs::write(path, text);
            }
        })
        .detach();
    }
}

impl Render for Log {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let model = self.model.read(cx).clone();
        let days = self.days(cx);

        let mut body = div().flex().flex_col();
        // Today always heads the list, with its chart, even when nothing matches.
        let mut days = days.into_iter().peekable();
        let today_rows = if days.peek().is_some_and(|d| d.title == "Today") {
            days.next().map(|d| d.rows).unwrap_or_default()
        } else {
            Vec::new()
        };
        body = body
            .child(
                div()
                    .pt(px(18.))
                    .px(px(20.))
                    .pb(px(4.))
                    .child(self.today_head(&model, &t, cx))
                    .child(self.day_chart(&model, &t, cx)),
            )
            .child(rows(today_rows, "Nothing matches today.", &t));

        let mut rest: Vec<Day> = days.collect();
        let more = rest.len() > 1;
        if !self.earlier_days {
            rest.truncate(1);
        }
        let count = rest.len();
        for (i, day) in rest.into_iter().enumerate() {
            body = body
                .child(
                    div()
                        .pt(px(22.))
                        .px(px(20.))
                        .pb(px(6.))
                        .text_size(px(18.))
                        .font_weight(FontWeight::BOLD)
                        .child(day.title),
                )
                .child(rows(day.rows, "", &t));
            if i + 1 == count && more && !self.earlier_days {
                body = body.child(
                    div().px(px(20.)).child(
                        outline_button("earlier", "Show earlier days", 38., &t)
                            .mt(px(8.))
                            .px(px(14.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.earlier_days = true;
                                cx.notify();
                            })),
                    ),
                );
            }
        }
        body = body.child(div().h(px(20.)));

        div()
            .id("log")
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
            .on_key_down(cx.listener(|this, e: &gpui_kit::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    if this.picking_tunnel {
                        this.picking_tunnel = false;
                        cx.notify();
                    } else {
                        cx.emit(Close);
                    }
                }
            }))
            .child(self.header(&t, cx))
            .child(self.filters(&model, &t, cx))
            .child(div().id("days").flex_1().overflow_y_scroll().child(body))
    }
}

impl Log {
    fn header(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("header")
            .flex()
            .items_center()
            .gap(px(8.))
            .py(px(10.))
            .pl(px(20.))
            .pr(px(12.))
            .border_b_1()
            .border_color(t.line)
            // No title bar: the window moves by its header.
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::BOLD)
                    .child("Log"),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(kept_for(self.model.read(cx).setting("log_days"))),
            )
            .child(div().flex_1())
            .child(
                outline_button(
                    "helpdesk",
                    if self.copied {
                        "Copied"
                    } else {
                        "Copy for helpdesk"
                    },
                    36.,
                    t,
                )
                .px(px(12.))
                .on_click(cx.listener(|this, _, _, cx| this.copy_for_helpdesk(cx))),
            )
            .child(
                outline_button("export", "Export\u{2026}", 36., t)
                    .px(px(12.))
                    .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
            )
            .child(
                div()
                    .id("close")
                    // The header moves the window; its buttons keep their clicks.
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation()
                    })
                    .size(px(32.))
                    .ml(px(4.))
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
            )
    }

    fn filters(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut kinds = div().flex();
        let chosen = Show::ALL.iter().position(|s| *s == self.show);
        for (i, show) in Show::ALL.into_iter().enumerate() {
            kinds = kinds.child(
                crate::controls::segment(("show", i), i, Show::ALL.len(), chosen, 38., t)
                    .px(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(show.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show = show;
                        cx.notify();
                    })),
            );
        }

        let chosen = self
            .tunnel
            .as_ref()
            .and_then(|n| model.tunnels.iter().find(|x| &x.name == n))
            .map(|x| x.display.clone())
            .unwrap_or_else(|| "All tunnels".into());
        let mut picker = div().relative().child(
            div()
                .id("tunnel")
                .h(px(40.))
                .px(px(10.))
                .flex()
                .items_center()
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
                .on_click(cx.listener(|this, _, _, cx| {
                    this.picking_tunnel = !this.picking_tunnel;
                    cx.notify();
                })),
        );
        if self.picking_tunnel {
            let mut options: Vec<(Option<String>, String)> = vec![(None, "All tunnels".into())];
            options.extend(
                model
                    .tunnels
                    .iter()
                    .map(|x| (Some(x.name.clone()), x.display.clone())),
            );
            let mut menu = div()
                .absolute()
                .top(px(44.))
                .left_0()
                .min_w(px(160.))
                .py(px(4.))
                .bg(t.panel)
                .border_1()
                .border_color(t.line_strong)
                .rounded(px(6.));
            for (i, (name, label)) in options.into_iter().enumerate() {
                let on = self.tunnel == name;
                menu = menu.child(
                    div()
                        .id(("tunnel-option", i))
                        .px(px(10.))
                        .py(px(6.))
                        .cursor_pointer()
                        .when(on, |b| b.bg(t.accent_soft))
                        .hover(|s| s.bg(t.surface))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tunnel = name.clone();
                            this.picking_tunnel = false;
                            cx.notify();
                        })),
                );
            }
            picker = picker.child(gpui_kit::deferred(menu.occlude()).with_priority(1));
        }

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.))
            .py(px(14.))
            .px(px(20.))
            .border_b_1()
            .border_color(t.line)
            .child(
                div()
                    .relative()
                    .w(px(300.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .pl(px(34.))
                    .pr(px(12.))
                    .border_1()
                    .border_color(t.line_strong)
                    .rounded(px(6.))
                    .bg(t.panel)
                    .child(
                        svg()
                            .path("crake/search.svg")
                            .size(px(16.))
                            .text_color(t.muted)
                            .absolute()
                            .left(px(11.))
                            .top(px(11.)),
                    )
                    .child(Input::new(&self.search).appearance(false)),
            )
            .child(kinds)
            .child(picker)
    }
}

/// The header's note on retention (Settings, "Keep the log for").
fn kept_for(days: &str) -> String {
    match days {
        "never" => "Not kept: only this session".to_owned(),
        "1" => "Kept for a day on this machine".to_owned(),
        d => format!("Kept for {d} days on this machine"),
    }
}

/// A day's rows: time, mark, the sentence and its reason, the tunnel.
fn rows(rows: Vec<Row>, empty: &'static str, t: &Theme) -> Div {
    let mut list = div().px(px(20.));
    if rows.is_empty() && !empty.is_empty() {
        return list.child(
            div()
                .py(px(20.))
                .border_t_1()
                .border_color(t.line)
                .text_color(t.muted)
                .child(empty),
        );
    }
    for r in rows {
        list = list.child(row(r, t));
    }
    list
}

fn row(r: Row, t: &Theme) -> AnyElement {
    let (mark, color) = match r.kind {
        LogKind::Routed => ("crake/mark-routed.svg", t.accent),
        LogKind::LeftAlone => ("crake/mark-left-alone.svg", t.muted),
        LogKind::Undone => ("crake/mark-undone.svg", t.muted),
        LogKind::System => ("crake/mark-system.svg", t.muted),
        LogKind::Asked => ("crake/help.svg", t.accent),
    };
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .py(px(11.))
        .border_t_1()
        .border_color(t.line)
        .child(
            div()
                .w(px(52.))
                .flex_none()
                .pt(px(1.))
                .font_family(fonts::MONO)
                .text_size(px(13.))
                .text_color(t.muted)
                .child(r.time),
        )
        .child(
            div()
                .w(px(20.))
                .flex_none()
                .mt(px(1.))
                .child(svg().path(mark).size(px(18.)).text_color(color)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(r.text)
                .child(div().text_size(px(13.)).text_color(t.muted).child(r.why)),
        )
        .child(
            div()
                .flex_none()
                .px(px(8.))
                .py(px(2.))
                .rounded(px(4.))
                .bg(t.surface)
                .text_color(t.muted)
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(r.badge),
        )
        .into_any_element()
}

fn outline_button(id: &'static str, label: &'static str, height: f32, t: &Theme) -> Stateful<Div> {
    div()
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
        .id(id)
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(t.line)
        .rounded(px(6.))
        .bg(t.panel)
        .text_color(t.ink)
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.bg(t.surface))
        .child(label)
}

impl Log {
    /// The minutes shown: (from, to) since midnight; the whole day when not zoomed.
    fn span(&self) -> (f32, f32) {
        self.view.unwrap_or((0., now_minutes()))
    }

    /// Zoom by `factor` around minute `at`, keeping it under the pointer.
    fn zoom(&mut self, at: f32, factor: f32) {
        let now = now_minutes();
        let (a, b) = self.span();
        let span = ((b - a) * factor).clamp(MIN_SPAN.min(now), now);
        let from = (at - (at - a) * span / (b - a)).clamp(0., (now - span).max(0.));
        self.view = if span >= now - 0.5 {
            None
        } else {
            Some((from, from + span))
        };
    }

    /// "Today", its summary, and when zoomed what's shown with Whole day.
    fn today_head(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let (a, b) = self.span();
        let mut head = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::BOLD)
                    .child("Today"),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(model.today_summary()),
            )
            .child(div().flex_1());
        if self.view.is_some() {
            let bin = bin_minutes(b - a, self.chart_geometry.get().1.max(900.));
            let bars = if bin as i32 == 1 {
                "1-minute bars".to_owned()
            } else {
                format!("{bin}-minute bars")
            };
            head = head
                .child(
                    div()
                        .font_family(fonts::MONO)
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format!("{}\u{2013}{} \u{b7} {bars}", hm(a), hm(b))),
                )
                .child(
                    outline_button("whole-day", "Whole day", 30., t)
                        .px(px(10.))
                        .text_size(px(13.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.view = None;
                            cx.notify();
                        })),
                );
        }
        head
    }

    /// Today (DESIGN.md, Charts): grey bars are all traffic, finer as it
    /// zooms; two-stroke ticks the moments Crake routed a host (merged when
    /// close, with a count); hollow circles hosts it left alone. Scroll zooms,
    /// drag moves, double-click goes back; pointing reads it.
    fn day_chart(&self, model: &Model, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let (a, b) = self.span();
        let span = (b - a).max(1.);
        let width = self.chart_geometry.get().1.max(900.);
        let x_frac = |m: f32| (m - a) / span;
        let bin = bin_minutes(span, width);

        // Traffic in bins of `bin` minutes: (start minute, bytes).
        let mut bins: Vec<(f32, u64)> = Vec::new();
        for (m, bytes) in model.today_traffic() {
            if m < a - bin || m > b {
                continue;
            }
            let start = (m / bin).floor() * bin;
            match bins.last_mut() {
                Some((s, sum)) if *s == start => *sum += bytes,
                _ => bins.push((start, bytes)),
            }
        }
        let max = bins.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32;

        // Marks less than 8 px apart share one, with a count.
        let mut marks: Vec<(f32, bool, Vec<LogEntry>)> = Vec::new();
        for (m, e) in model.today_events() {
            if m < a || m > b {
                continue;
            }
            let routed = e.kind == LogKind::Routed;
            match marks.iter_mut().rev().find(|x| x.1 == routed) {
                Some(x) if (m - x.0) / span * width < 8. => {
                    x.0 = m;
                    x.2.push(e);
                }
                _ => marks.push((m, routed, vec![e])),
            }
        }

        // What the pointer is on: a mark close by, else that bin.
        let pointed = self.hover.map(|h| {
            let near = marks
                .iter()
                .filter(|x| ((x.0 - h) / span * width).abs() <= 6.)
                .min_by(|p, q| (p.0 - h).abs().total_cmp(&(q.0 - h).abs()));
            match near {
                Some((m, routed, entries)) => {
                    let refs: Vec<&LogEntry> = entries.iter().collect();
                    let top = match (*routed, entries.len()) {
                        (true, n) if n > 1 => 6.,
                        (true, _) => 24. - 1.5,
                        (false, _) => BASELINE - 8. - 4.,
                    };
                    (*m, None, hm(*m), crate::model::mark_text(&refs), top)
                }
                None => {
                    let start = (h / bin).floor() * bin;
                    let bytes = bins.iter().find(|x| x.0 == start).map(|x| x.1).unwrap_or(0);
                    let time = if bin > 1. {
                        format!("{}\u{2013}{}", hm(start), hm(start + bin))
                    } else {
                        hm(start)
                    };
                    let text = match (bytes, bin > 1.) {
                        (0, _) => "No traffic".to_owned(),
                        (b, true) => format!(
                            "{} of traffic in {} minutes",
                            crate::model::human_bytes(b),
                            bin
                        ),
                        (b, false) => format!("{} of traffic", crate::model::human_bytes(b)),
                    };
                    let top = if bytes > 0 {
                        BASELINE - (2. + 21. * (bytes as f32 / max).sqrt())
                    } else {
                        BASELINE + 3.
                    };
                    (start + bin / 2., Some(start), time, text, top)
                }
            }
        });

        let (accent, muted, panel, base, grey) =
            (t.accent, t.muted, t.panel, t.line_strong, t.bars);
        let paint_bins = bins.clone();
        let paint_marks: Vec<(f32, bool)> = marks.iter().map(|m| (m.0, m.1)).collect();
        let hover = pointed.as_ref().map(|p| (p.0, p.1, p.4));
        let geometry = self.chart_geometry.clone();
        let mono = |s: String| {
            div()
                .font_family(fonts::MONO)
                .text_size(px(11.))
                .text_color(t.muted)
                .child(s)
        };

        let mut counts = div().absolute().top_0().left_0().size_full();
        for (m, routed, entries) in &marks {
            if *routed && entries.len() > 1 {
                counts = counts.child(
                    div()
                        .absolute()
                        .top(px(4.))
                        .left(gpui_kit::relative(x_frac(*m).clamp(0., 1.)))
                        .ml(px(-8.))
                        .w(px(20.))
                        .flex()
                        .justify_center()
                        .font_family(fonts::MONO)
                        .text_size(px(11.))
                        .line_height(px(12.))
                        .text_color(t.accent)
                        .child(entries.len().to_string()),
                );
            }
        }

        // Under the chart: round times, or one line about what's pointed at.
        let now = now_minutes();
        let label = |m: f32| {
            if (m - now).abs() < 0.5 {
                "now".to_owned()
            } else {
                hm(m)
            }
        };
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
            None => {
                let step = [360., 120., 60., 30., 15., 10., 5., 1.]
                    .into_iter()
                    .find(|s| span / s >= 2.)
                    .unwrap_or(1.);
                let mid = ((a + b) / 2. / step).round() * step;
                div()
                    .relative()
                    .w_full()
                    .h(px(16.))
                    .child(div().absolute().left_0().child(mono(label(a))))
                    .child(
                        div()
                            .absolute()
                            .left(gpui_kit::relative(x_frac(mid)))
                            .ml(px(-20.))
                            .w(px(40.))
                            .flex()
                            .justify_center()
                            .child(mono(hm(mid))),
                    )
                    .child(div().absolute().right_0().child(mono(label(b))))
            }
        };

        div()
            .mt(px(10.))
            .child(
                div()
                    .id("day-chart")
                    .relative()
                    .w_full()
                    .h(px(CHART_HEIGHT - 20.))
                    .cursor_grab()
                    .on_scroll_wheel(cx.listener(|this, e: &gpui_kit::ScrollWheelEvent, _, cx| {
                        let (left, width) = this.chart_geometry.get();
                        if width <= 4. {
                            return;
                        }
                        let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
                        if dy == 0. {
                            return;
                        }
                        let (a, b) = this.span();
                        let at =
                            a + (b - a) * ((f32::from(e.position.x) - left) / width).clamp(0., 1.);
                        this.zoom(at, if dy > 0. { 0.8 } else { 1.25 });
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, e: &gpui_kit::MouseDownEvent, _, cx| {
                            if e.click_count >= 2 {
                                this.view = None;
                                this.drag = None;
                            } else {
                                this.drag = Some((f32::from(e.position.x), this.span()));
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, _, _, _| this.drag = None),
                    )
                    .on_mouse_move(cx.listener(|this, e: &gpui_kit::MouseMoveEvent, _, cx| {
                        let (left, width) = this.chart_geometry.get();
                        if width <= 4. {
                            return;
                        }
                        let x = f32::from(e.position.x);
                        if let (Some((x0, (a0, b0))), true) = (this.drag, e.dragging()) {
                            let span = b0 - a0;
                            let now = now_minutes();
                            let from =
                                (a0 - span * (x - x0) / width).clamp(0., (now - span).max(0.));
                            if this.view.is_some() {
                                this.view = Some((from, from + span));
                            }
                        }
                        let (a, b) = this.span();
                        this.hover = Some(a + (b - a) * ((x - left) / width).clamp(0., 1.));
                        cx.notify();
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
                                let baseline = top + BASELINE;
                                let x_of = |m: f32| left + 2. + (w - 4.) * (m - a) / span;
                                let scale = window.scale_factor();
                                // Bars about half their spacing wide, 2 to 8 px.
                                let bw = (bin / span * w * 0.55).clamp(2., 8.);
                                for (start, bytes) in &paint_bins {
                                    if *bytes == 0 {
                                        continue;
                                    }
                                    let h = 2. + 21. * (*bytes as f32 / max).sqrt();
                                    let x = x_of(start + bin / 2.);
                                    if x < left || x > left + w {
                                        continue;
                                    }
                                    if let Some(p) = crate::chart::bar(x, bw, baseline, h, scale) {
                                        let on = hover.is_some_and(|(_, s, _)| s == Some(*start));
                                        window.paint_path(p, if on { muted } else { grey });
                                    }
                                }
                                // Ticks keep clear of the bars (DESIGN.md, Charts).
                                for (m, routed) in &paint_marks {
                                    if *routed {
                                        let x = x_of(*m);
                                        for (dx, tip) in [(0., 24.), (4.5, 33.)] {
                                            if let Some(p) =
                                                crate::chart::rounded_bar(x + dx, top + tip - 1.5, baseline, 6.)
                                            {
                                                window.paint_path(p, panel);
                                            }
                                        }
                                    }
                                }
                                // The pointer's minute: a dotted hairline that stops above
                                // whatever stands there.
                                if let Some((m, _, stop)) = hover {
                                    let x = x_of(m);
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
                                let mut line = PathBuilder::stroke(px(1.));
                                line.move_to(point(px(left), px(baseline + 0.5)));
                                line.line_to(point(px(left + w), px(baseline + 0.5)));
                                if let Ok(p) = line.build() {
                                    window.paint_path(p, base);
                                }
                                for (m, routed) in &paint_marks {
                                    let x = x_of(*m);
                                    if *routed {
                                        for (dx, tip) in [(0., 24.), (4.5, 33.)] {
                                            if let Some(p) =
                                                crate::chart::rounded_bar(x + dx, top + tip, baseline, 3.)
                                            {
                                                window.paint_path(p, accent);
                                            }
                                        }
                                    } else {
                                        for (fill, stroke) in
                                            [(Some(panel), None), (None, Some(1.6))]
                                        {
                                            if let Some(p) = ring(x, baseline - 8., 4., stroke) {
                                                window.paint_path(p, fill.unwrap_or(muted));
                                            }
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
                    .mt(px(2.))
                    .flex()
                    .items_baseline()
                    .child(under),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child("Scroll to zoom \u{b7} drag to move"),
            )
    }
}

/// Minutes since local midnight, now.
fn now_minutes() -> f32 {
    let now = jiff::Zoned::now();
    (now.hour() as f32 * 60. + now.minute() as f32 + now.second() as f32 / 60.).max(1.)
}

/// "09:14".
fn hm(minutes: f32) -> String {
    let m = minutes.max(0.).round() as u32;
    format!("{:02}:{:02}", (m / 60).min(23), m % 60)
}

/// The finest bar size that keeps bars about 6 px apart.
fn bin_minutes(span: f32, width: f32) -> f32 {
    [1., 2., 5., 10., 15., 30.]
        .into_iter()
        .find(|n| n / span * width >= 6.)
        .unwrap_or(30.)
}

fn ring(cx: f32, cy: f32, r: f32, stroke: Option<f32>) -> Option<gpui_kit::Path<Pixels>> {
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
