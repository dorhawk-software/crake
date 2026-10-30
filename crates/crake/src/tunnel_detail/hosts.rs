//! Tunnel detail: the hosts in their tabs (learned, pinned, from the
//! gateway, and short names once there are any) with Pin and Forget, and
//! Add a host.

use super::*;
use crate::units::{SECS_PER_DAY as DAY, SECS_PER_HOUR as HOUR, SECS_PER_MINUTE as MINUTE};

/// A host's name in a row (px).
const HOST_SIZE: f32 = 14.;
/// Used within this many seconds: "now".
const JUST_NOW: u64 = 90;

impl TunnelDetail {
    /// The tabs, their rows, and "Add a host".
    pub(super) fn hosts(
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
        // Kept until forgotten: pinned hosts, or short names in their tab.
        let kept_kind = if self.tab == Tab::Short {
            "short"
        } else {
            "pinned"
        };
        let pinned: Vec<&HostRow> = rows.iter().filter(|r| r.kind == kept_kind).collect();
        let counts = tab_counts(rows, gateway.len());
        let short = counts[TABS - 1] > 0 || self.tab == Tab::Short;

        let mut bar = div().flex().gap(px(22.)).border_b_1().border_color(t.line);
        for (i, (tab, label)) in [
            (Tab::Learned, "Learned"),
            (Tab::Pinned, "Pinned"),
            (Tab::Gateway, "From the gateway"),
            (Tab::Short, "Short names"),
        ]
        .into_iter()
        .enumerate()
        .filter(|(_, (tab, _))| *tab != Tab::Short || short)
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
                    .border_color(if on {
                        t.accent
                    } else {
                        gpui_kit::transparent_black().into()
                    })
                    .text_color(if on { t.ink } else { t.muted })
                    .font_weight(if on {
                        FontWeight::BOLD
                    } else {
                        FontWeight::SEMIBOLD
                    })
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
                    .keys()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        let (bar, confirm) = self.forget_all(bar, rows, &display, t, cx);

        let (list, empty) = self.rows_list(&learned, &pinned, &gateway, t, window, cx);
        let forget = self.model.read(cx).setting("forget_after_days").to_owned();
        // How long the tab's hosts stay: said once, over the list, the same
        // height on every tab so the list doesn't move between them.
        let kept = div()
            .py(px(10.))
            .text_size(px(13.))
            .text_color(t.muted)
            .border_b_1()
            .border_color(t.line)
            .child(kept_line(self.tab, &display, &forget));
        let nothing = if self.tab == Tab::Learned {
            format!("Nothing learned through {display} yet.")
        } else {
            "Nothing here.".to_owned()
        };

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
                format!(
                    "{}.{}.x.x",
                    o.first().unwrap_or(&"10"),
                    o.get(1).unwrap_or(&"0")
                )
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
            .children(confirm)
            .child(kept)
            .child(list)
            .when(empty, |d| {
                d.child(
                    div()
                        .py(px(24.))
                        .text_color(t.muted)
                        .border_b_1()
                        .border_color(t.line)
                        .child(nothing),
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
                                .keys()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.send_added(window, cx)),
                                )
                                .test_support(),
                        ),
                )
            })
            .child(div().pb(px(28.)).when_some(failed, |d, text| {
                d.child(
                    div()
                        .pt(px(10.))
                        .text_size(px(13.))
                        .text_color(t.danger)
                        .child(text),
                )
            }))
    }

    /// A tab's rows: only those in view built, spacers for the rest
    /// (hundreds of hosts took 20 ms or more a frame, every frame, until
    /// 2026-10-27); and whether the tab has none.
    fn rows_list(
        &self,
        learned: &[&HostRow],
        pinned: &[&HostRow],
        gateway: &[(String, String, String)],
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Div, bool) {
        let count = match self.tab {
            Tab::Learned => learned.len(),
            Tab::Pinned | Tab::Short => pinned.len(),
            Tab::Gateway => gateway.len(),
        };
        let body = crate::controls::scroll_handle(super::BODY, window, cx);
        // Opened on a host: scroll its row into view, a row's height below
        // the top, once the rows and where they start are known.
        if let (true, Some(top), Some(host)) = (
            self.scroll_to_host.get(),
            self.rows_top.get(),
            self.shown_host.as_deref(),
        ) {
            let list = if self.tab == Tab::Pinned {
                pinned
            } else {
                learned
            };
            if let Some(i) = list.iter().position(|r| r.target == host) {
                let to = top + i as f32 * ROW_HEIGHT - ROW_HEIGHT;
                body.set_offset(gpui_kit::point(px(0.), px(-to.max(0.))));
                self.scroll_to_host.set(false);
            }
        }
        let range = self
            .rows_top
            .get()
            .map_or(0..count.min(FIRST_DRAWN), |top| {
                rows_in_view(
                    count,
                    -f32::from(body.offset().y) - top,
                    f32::from(body.bounds().size.height),
                )
            });
        let (first, after) = (range.start, count - range.end);
        let shown: Vec<AnyElement> = match self.tab {
            Tab::Learned => learned[range.clone()]
                .iter()
                .zip(range.clone())
                .map(|(r, i)| self.host_row(i, r, t, cx).into_any_element())
                .collect(),
            Tab::Pinned => pinned[range.clone()]
                .iter()
                .zip(range.clone())
                .map(|(r, i)| self.host_row(i, r, t, cx).into_any_element())
                .collect(),
            Tab::Short => pinned[range.clone()]
                .iter()
                .zip(range.clone())
                .map(|(r, i)| self.short_row(i, r, t, cx).into_any_element())
                .collect(),
            Tab::Gateway => gateway[range.clone()]
                .iter()
                .zip(range.clone())
                .map(|((host, how, used), i)| {
                    row_frame(t)
                        .child(icon("crake/mark-gateway.svg", t.muted))
                        .child(host_text(i, host.clone(), how.clone(), t, cx))
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

        // Where the rows start, for the next frame's; drawn again at once
        // when the rows in view aren't the ones built.
        let (top, built) = (self.rows_top.clone(), range);
        let measure = canvas(
            move |bounds, window, _| {
                let at = f32::from(bounds.top())
                    - f32::from(body.bounds().top())
                    - f32::from(body.offset().y);
                top.set(Some(at));
                let wanted = rows_in_view(
                    count,
                    -f32::from(body.offset().y) - at,
                    f32::from(body.bounds().size.height),
                );
                if wanted != built {
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let list = div()
            .relative()
            .child(div().h(px(first as f32 * ROW_HEIGHT)))
            .children(shown)
            .child(div().h(px(after as f32 * ROW_HEIGHT)))
            .child(measure);
        (list, empty)
    }

    pub(super) fn host_row(&self, i: usize, r: &HostRow, t: &Theme, cx: &mut Context<Self>) -> Div {
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
            Some(s) if s < JUST_NOW => "Used now".to_owned(),
            Some(s) if s < HOUR => format!("{} min ago", s / MINUTE),
            Some(s) if s < DAY => format!("{} h ago", s / HOUR),
            Some(s) => match s / DAY {
                1 => "Yesterday".to_owned(),
                d => format!("{d} days ago"),
            },
            None => String::new(),
        };
        let (target, pin_target) = (r.target.clone(), r.target.clone());
        let (danger, surface) = (t.danger, t.surface);
        let shown = self.shown_host.as_deref() == Some(r.target.as_str());
        row_frame(t)
            .when(shown, |row| row.bg(t.nav_hover))
            .child(icon(icon_path, color))
            .child(host_text(i, host, how, t, cx))
            .child(used_text(used, t))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .justify_end()
                    .child(
                        row_button(("pin", i), if pinned { "Unpin" } else { "Pin" }, t)
                            .hover(move |s| s.bg(surface))
                            .keys()
                            .on_click(cx.listener(move |this, _, _, _| {
                                let tunnel = this.tunnel.clone();
                                this.send(if pinned {
                                    Request::Unpin(tunnel, pin_target.clone())
                                } else {
                                    Request::Pin(tunnel, vec![pin_target.clone()])
                                });
                            }))
                            .test_support(),
                    )
                    .child(
                        row_button(("forget", i), "Forget", t)
                            .hover(move |s| s.bg(surface).text_color(danger))
                            .keys()
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.send(Request::Forget(this.tunnel.clone(), target.clone()));
                            }))
                            .test_support(),
                    ),
            )
    }
}

/// Learned, Pinned, From the gateway, Short names.
const TABS: usize = 4;

/// A host row's height: its two lines, 11 px above and below, its border.
pub(super) const ROW_HEIGHT: f32 = 63.;
/// Rows built beyond those in view, each way (a fast scroll's next frame
/// still finds rows there).
const OVERSCAN: usize = 20;
/// Rows built before the list was first drawn: more than a screen's.
const FIRST_DRAWN: usize = 40;

/// Of `count` rows, the ones to build when the view starts `view_top`
/// below the first row and is `view_height` tall.
pub(super) fn rows_in_view(
    count: usize,
    view_top: f32,
    view_height: f32,
) -> std::ops::Range<usize> {
    let first = (view_top / ROW_HEIGHT).floor().max(0.) as usize;
    let last = ((view_top + view_height) / ROW_HEIGHT).ceil().max(0.) as usize;
    first.saturating_sub(OVERSCAN).min(count)..last.saturating_add(OVERSCAN).min(count)
}

/// Each tab's count: learned hosts, pinned ones, the gateway's. A learned
/// host since pinned shows in Learned too, but counts once, under Pinned
/// (design/proposals/critique-2026-10-15, 5).
/// The line over a tab's list: how long its hosts are kept
/// (`forget_after_days`, Settings → Learning: a number of days or "never").
pub(super) fn kept_line(tab: Tab, display: &str, forget: &str) -> String {
    match tab {
        Tab::Learned => match forget.parse::<u32>() {
            Ok(1) => "Crake forgets a learned host after a day unused.".into(),
            Ok(days) => format!("Crake forgets a learned host after {days} days unused."),
            Err(_) => "Crake keeps a learned host until you forget it.".into(),
        },
        Tab::Pinned => "Pinned hosts are kept until you forget them.".into(),
        Tab::Gateway => format!("{display}\u{2019}s gateway sent these when it connected."),
        Tab::Short => "Short names are kept until you forget them.".into(),
    }
}

pub(super) fn tab_counts(rows: &[HostRow], gateway: usize) -> [usize; TABS] {
    let of = |kind: &str| rows.iter().filter(|r| r.kind == kind).count();
    [of("learned"), of("pinned"), gateway, of("short")]
}

pub(super) fn row_frame(t: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(11.))
        .border_b_1()
        .border_color(t.line)
}

pub(super) fn icon(path: &'static str, color: gpui_kit::Rgba) -> impl IntoElement {
    svg().path(path).size(px(18.)).flex_none().text_color(color)
}

/// A row's host (its whole name on hover, copied on a click:
/// `full_name`) and how it got there. `i`: the row's place in its tab.
pub(super) fn host_text(
    i: usize,
    host: String,
    how: String,
    t: &Theme,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .child(crate::full_name::host_name(
            ("host-name", i),
            host,
            HOST_SIZE,
            t,
            cx,
        ))
        // One line, so every row is ROW_HEIGHT.
        .child(
            div()
                .text_size(px(13.))
                .text_color(t.muted)
                .truncate()
                .child(how),
        )
}

/// When the host was last used, right-aligned to one edge; a pinned host
/// shows nothing here (its line says it's kept).
pub(super) fn used_text(used: String, t: &Theme) -> impl IntoElement {
    div()
        .w(px(120.))
        .flex_none()
        .flex()
        .justify_end()
        .text_size(px(13.))
        .text_color(t.muted)
        .child(used)
}

impl TunnelDetail {
    /// "Forget all…" (design/proposals/forget-all.html): on the Learned
    /// tab, when something was learned or sent direct with Undo; asked in
    /// place first. The tab row with the button, and the question if asked.
    fn forget_all(
        &mut self,
        bar: Div,
        rows: &[HostRow],
        display: &str,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> (Div, Option<AnyElement>) {
        let forgettable = rows.iter().filter(|r| r.kind == "learned").count();
        let undone = rows.iter().filter(|r| r.kind == "direct").count();
        let can = self.tab == Tab::Learned && forgettable + undone > 0;
        if !can {
            self.confirm_forget_all = false;
        }
        let bar = bar.items_center().when(can, |b| {
            b.child(div().flex_1()).child(
                row_button("forget-all", "Forget all\u{2026}", t)
                    .px(px(12.))
                    .border_1()
                    .border_color(t.line)
                    .h(px(30.))
                    .text_size(px(13.))
                    .keys()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_forget_all = true;
                        cx.notify();
                    }))
                    .test_support(),
            )
        });
        let confirm = self
            .confirm_forget_all
            .then(|| self.forget_all_confirm(display, forgettable, undone, t, cx));
        (bar, confirm)
    }

    /// "Forget the 4 hosts Crake learned for ACME?", asked in place, with
    /// Cancel and Forget all.
    fn forget_all_confirm(
        &self,
        display: &str,
        learned: usize,
        undone: usize,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hosts = if learned == 1 { "host" } else { "hosts" };
        let mut detail =
            "They go direct again, and Crake learns them anew as you use them.".to_owned();
        if undone > 0 {
            detail += &format!(" The {undone} you sent direct with Undo may be learned again.");
        }
        detail += " Pinned hosts stay.";
        div()
            .mt(px(10.))
            .mb(px(4.))
            .px(px(14.))
            .py(px(12.))
            .rounded(px(8.))
            .bg(t.surface)
            .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                "Forget the {learned} {hosts} Crake learned for {display}?"
            )))
            .child(
                div()
                    .mt(px(2.))
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child(detail),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .justify_end()
                    .gap(px(10.))
                    .child(
                        div()
                            .id("forget-all-cancel")
                            .h(px(32.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .child("Cancel")
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_forget_all = false;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(
                        div()
                            .id("forget-all-confirm")
                            .h(px(32.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(t.danger)
                            .rounded(px(6.))
                            .text_color(t.danger)
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .child("Forget all")
                            .keys()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_forget_all = false;
                                this.send(Request::ForgetLearned(this.tunnel.clone()));
                                cx.notify();
                            }))
                            .test_support(),
                    ),
            )
            .into_any_element()
    }
}

/// A host's action: quiet, no border, so the host leads; still there at
/// rest for a screen share. Its caller fills it when pointed at (Forget
/// turns red too; design/proposals/critique-2026-10-15, 5).
/// Pin, Unpin, Forget: quiet bordered buttons, as the screen draws them
/// (tunnel-detail.html; bare bold text had weighed as much as the host).
pub(super) fn row_button(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    t: &Theme,
) -> Stateful<Div> {
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
pub(crate) fn learned_how(r: &HostRow) -> String {
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
pub(super) fn gateway_rows(tunnel: &Tunnel) -> Vec<(String, String, String)> {
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
