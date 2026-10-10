//! The full log (design/screens/log.html): every decision as a sentence with
//! its reason, by day, newest first; filters, search, the day's chart,
//! Copy for helpdesk and Export.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, ClipboardItem, Context, Div, Entity, FocusHandle, FontWeight, PathBuilder, Pixels,
    Stateful, Subscription, TestSupportExt, Window, canvas, div, point, px, svg,
};

use crake_proto::{LogEntry, LogKind};

use crate::model::{Day, Model, Row, Show};
use crate::theme::{Theme, fonts};

mod day_chart;
mod rows;

#[cfg(test)]
mod tests;

use self::rows::rows;

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
                            }))
                            .test_support(),
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
            .child(
                div()
                    .id("days")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(body)
                    .test_support(),
            )
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
                .on_click(cx.listener(|this, _, _, cx| this.copy_for_helpdesk(cx)))
                .test_support(),
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
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))
                    .test_support(),
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
                    }))
                    .test_support(),
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
                }))
                .test_support(),
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
                        }))
                        .test_support(),
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
