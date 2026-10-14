//! Tunnel detail: the hosts in their tabs (learned, pinned, from the
//! gateway) with Pin and Forget, and Add a host.

use super::*;
use crate::units::{SECS_PER_DAY as DAY, SECS_PER_HOUR as HOUR, SECS_PER_MINUTE as MINUTE};

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
        let pinned: Vec<&HostRow> = rows.iter().filter(|r| r.kind == "pinned").collect();
        let counts = [learned.len(), pinned.len(), gateway.len()];

        let mut bar = div().flex().gap(px(22.)).border_b_1().border_color(t.line);
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
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        let (bar, confirm) = self.forget_all(bar, rows, &display, t, cx);

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
                        row_button(("pin", i), if pinned { "Unpin" } else { "Pin" }, t)
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
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.send(Request::Forget(this.tunnel.clone(), target.clone()));
                            }))
                            .test_support(),
                    ),
            )
    }
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

pub(super) fn host_text(host: String, how: String, t: &Theme) -> impl IntoElement {
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

pub(super) fn used_text(used: String, t: &Theme) -> impl IntoElement {
    div()
        .w(px(120.))
        .flex_none()
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
                    .h(px(30.))
                    .text_size(px(13.))
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
pub(super) fn learned_how(r: &HostRow) -> String {
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
