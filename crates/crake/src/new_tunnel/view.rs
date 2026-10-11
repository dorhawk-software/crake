//! New tunnel's window: the frame, the steps bar, the body and Continue.

use super::*;

impl NewTunnel {
    pub(super) fn steps(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div().flex().px(px(32.)).border_b_1().border_color(t.line);
        for (i, label) in ["Server", "Sign-in", "Routing"].into_iter().enumerate() {
            let n = i + 1;
            let (cur, done) = (self.step == n, self.step > n);
            let ring = if cur || done { t.accent } else { t.line_strong };
            bar = bar.child(
                div()
                    .id(("step", i))
                    .flex_1()
                    .h(px(50.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mb(px(-1.))
                    .border_b_2()
                    .border_color(if cur {
                        t.accent
                    } else {
                        gpui_kit::transparent_black().into()
                    })
                    .text_color(if cur || done { t.ink } else { t.muted })
                    .font_weight(if cur {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(20.))
                            .rounded_full()
                            .border(px(1.5))
                            .border_color(ring)
                            .when(done, |d| d.bg(t.accent))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if done {
                                t.accent_ink
                            } else if cur {
                                t.accent
                            } else {
                                t.muted
                            })
                            .child(if done {
                                "\u{2713}".to_owned()
                            } else {
                                n.to_string()
                            }),
                    )
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Forward only past steps that are complete.
                        let ok = (this.step..n).all(|s| this.check(s, cx));
                        if n <= this.step || ok {
                            this.step = n;
                            this.problem = None;
                        }
                        cx.notify();
                    })),
            );
        }
        bar
    }
}

impl Render for NewTunnel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::current(cx);
        let title = match &self.editing {
            Some(name) => format!("Edit {}", self.model.read(cx).display_of(Some(name))),
            None => "New tunnel".to_owned(),
        };
        let body: AnyElement = match self.step {
            1 => self.server(&t, cx).into_any_element(),
            2 => self.sign_in(&t, cx).into_any_element(),
            _ => self.routing(&t, cx).into_any_element(),
        };
        let next_label = match (self.step, &self.editing) {
            (3, Some(_)) => "Save",
            (3, None) => "Add and connect",
            _ => "Continue",
        };
        let max_height = self.max_height;
        // The window is the card: as tall as this step needs.
        let fit = gpui_kit::canvas(
            move |bounds, window, _| {
                // Measured inside the card's 1 px border: add it back, or
                // each resize comes out 2 px short.
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
        let card = div()
            .id("new-tunnel")
            .track_focus(&self.focus)
            .relative()
            .w_full()
            // Its own height, never squeezed to the window's: the canvas
            // below measures it, and the window grows to it.
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
            .on_key_down(cx.listener(|this, e: &gpui_kit::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    if this.open_select.is_some() {
                        this.open_select = None;
                        cx.notify();
                    } else {
                        cx.emit(Close);
                    }
                }
            }))
            // Header: Cancel, the title, close.
            .child(
                div()
                    .flex()
                    .items_center()
                    .py(px(10.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(t.line)
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                        window.start_window_move()
                    })
                    .child(
                        div().flex_1().child(
                            div()
                                .id("cancel")
                                .h(px(36.))
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .rounded(px(6.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(t.surface))
                                .child("Cancel")
                                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(Close))),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(
                        div().flex_1().flex().justify_end().child(
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
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(Close))),
                        ),
                    ),
            )
            .child(self.steps(&t, cx))
            .child(
                div()
                    .id("body")
                    // Its natural height; scrolls only past the screen's
                    // (less the header, steps and footer).
                    .flex_none()
                    .max_h(px(max_height - 190.))
                    .overflow_y_scroll()
                    .pt(px(26.))
                    .px(px(32.))
                    .pb(px(8.))
                    .child(body)
                    .when_some(self.problem.clone(), |b, p| {
                        b.child(
                            div()
                                .mt(px(14.))
                                .text_size(px(13.))
                                .text_color(t.danger)
                                .child(p),
                        )
                    })
                    .test_support(),
            )
            // Footer: Back, "Step n of 3", Continue.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pt(px(16.))
                    .px(px(32.))
                    .pb(px(22.))
                    .when(self.step > 1, |f| {
                        f.child(
                            div()
                                .id("back")
                                .h(px(42.))
                                .px(px(16.))
                                .flex()
                                .items_center()
                                .border_1()
                                .border_color(t.line)
                                .rounded(px(6.))
                                .bg(t.panel)
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .child("Back")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.step -= 1;
                                    this.problem = None;
                                    cx.notify();
                                }))
                                .test_support(),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.muted)
                            .child(format!("Step {} of 3", self.step)),
                    )
                    .child(
                        div()
                            .id("next")
                            .h(px(42.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(t.accent)
                            .text_color(t.accent_ink)
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .child(next_label)
                            .on_click(cx.listener(|this, _, _, cx| this.next(cx)))
                            .test_support(),
                    ),
            )
            .child(fit);
        div().w_full().flex().flex_col().child(card)
    }
}
