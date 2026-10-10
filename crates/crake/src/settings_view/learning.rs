//! Settings, Learning: how long to wait, when two tunnels answer, forget
//! after, the never list.

use super::*;

impl Settings {
    pub(super) fn learning(
        &self,
        model: &Model,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = window;
        // Wait for a direct answer: 1, 2, 3 or 5 s.
        let wait = model.setting("probe_wait_secs").to_owned();
        let values = ["1", "2", "3", "5"];
        let chosen = values.iter().position(|v| *v == wait);
        let mut waits = div().flex().mt(px(10.));
        for (i, n) in values.into_iter().enumerate() {
            waits = waits.child(
                crate::controls::segment(("wait", i), i, values.len(), chosen, 36., t)
                    .min_w(px(56.))
                    .px(px(12.))
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .child(format!("{n} s"))
                    .on_click(cx.listener(move |this, _, _, _| {
                        this.set("probe_wait_secs", n.to_owned());
                    })),
            );
        }

        let both = select(
            self,
            "both",
            170.,
            &[
                ("ask", "Ask me"),
                ("faster", "Use the faster one"),
                ("first", "Use the first in order"),
            ],
            model.setting("both"),
            t,
            cx,
            |this, value, _| this.set("both", value),
        );
        let forget = select(
            self,
            "forget",
            92.,
            &[
                ("1", "1 day"),
                ("7", "7 days"),
                ("30", "30 days"),
                ("never", "Never"),
            ],
            model.setting("forget_after_days"),
            t,
            cx,
            |this, value, _| this.set("forget_after_days", value),
        );

        // Never send through a tunnel: chips, "+ Add".
        let never = model.setting_list("never");
        let mut chips = div().flex().flex_wrap().gap(px(6.)).mt(px(10.));
        for (i, item) in never.iter().enumerate() {
            let rest: Vec<String> = never
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, v)| v.clone())
                .collect();
            chips = chips.child(
                div()
                    .h(px(30.))
                    .pl(px(10.))
                    .pr(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(4.))
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .child(item.clone())
                    .child(
                        div()
                            .id(("remove", i))
                            .size(px(22.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(t.surface))
                            .child(
                                svg()
                                    .path("crake/close.svg")
                                    .size(px(11.))
                                    .text_color(t.muted),
                            )
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.set("never", rest.join("\n"));
                            })),
                    ),
            );
        }
        if self.adding_never {
            chips = chips.child(
                div()
                    .w(px(240.))
                    .h(px(30.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(t.accent)
                    .rounded(px(4.))
                    .font_family(fonts::MONO)
                    .text_size(px(13.))
                    .child(Input::new(&self.never_input).appearance(false)),
            );
        } else {
            chips = chips.child(
                div()
                    .id("add-never")
                    .h(px(30.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_dashed()
                    .border_color(t.line_strong)
                    .rounded(px(4.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child("+ Add")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.adding_never = true;
                        this.never_input.update(cx, |f, cx| f.focus(window, cx));
                        cx.notify();
                    })),
            );
        }

        div()
            .child(
                group(t)
                    .mt(px(22.))
                    .child(
                        div()
                            .py(px(14.))
                            .px(px(16.))
                            .child(self.title("Wait for a direct answer", t, cx))
                            .child(sub(
                                "Shorter feels faster. Longer avoids routing slow but public servers.",
                                t,
                            ))
                            .child(div().flex().child(waits)),
                    )
                    .child(row(self.title("When two tunnels answer", t, cx), None, both, true, t))
                    .child(row(self.title("Forget hosts unused for", t, cx), None, forget, true, t)),
            )
            .child(
                group(t)
                    .mt(px(16.))
                    .py(px(14.))
                    .px(px(16.))
                    .child(self.title("Never send through a tunnel", t, cx))
                    .child(sub(
                        "Personal banking, home devices. Crake leaves these alone even if they don\u{2019}t answer.",
                        t,
                    ))
                    .child(chips),
            )
    }
}
