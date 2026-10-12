//! Settings, General: appearance, notifications, the bird, start at login.

use super::*;

/// A row that doesn't apply yet.
const DIMMED: f32 = 0.45;

impl Settings {
    pub(super) fn general(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = cx.global::<Prefs>().clone();
        let starts = crate::autostart::enabled();
        let notify = select(
            self,
            "notify",
            px(221.),
            &[
                ("none", "Say nothing"),
                ("quiet", "Quiet notification"),
                ("crex", "Notification and a \u{201c}crex crex\u{201d}"),
            ],
            &prefs.notify,
            t,
            cx,
            |_, value, cx| Self::pref(cx, |p| p.notify = value),
        );
        let appearance = select(
            self,
            "appearance",
            px(153.),
            &[
                ("system", "Follow the system"),
                ("light", "Light"),
                ("dark", "Dark"),
            ],
            &prefs.appearance,
            t,
            cx,
            |_, value, cx| {
                Self::pref(cx, |p| p.appearance = value);
                Theme::current(cx).sync_components(cx);
                cx.refresh_windows();
            },
        );
        div()
            .child(
                group(t)
                    .mt(px(22.))
                    .child(row(
                        self.title("Start when you log in", t, cx),
                        Some("The bird is there when you need it."),
                        toggle("startup", starts, t)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                crate::autostart::set(!starts);
                                cx.notify();
                            }))
                            .test_support(),
                        false,
                        t,
                    ))
                    // Only means something when Crake starts at login.
                    .child(
                        div()
                            .pl(px(24.))
                            .border_t_1()
                            .border_color(t.line)
                            .when(!starts, |r| r.opacity(DIMMED))
                            .child(row(
                                self.title("Switch on the tunnels you left on", t, cx),
                                Some("Some may ask you to sign in."),
                                toggle("reconnect", starts && prefs.reconnect, t).when(
                                    starts,
                                    |sw| {
                                        sw.on_click(cx.listener(move |_, _, _, cx| {
                                            Self::pref(cx, |p| p.reconnect = !prefs.reconnect);
                                        }))
                                    },
                                ),
                                false,
                                t,
                            )),
                    )
                    .child(row(
                        self.title("Show the bird in the top bar", t, cx),
                        Some("Without it, open Crake from your apps."),
                        toggle("bird", prefs.show_bird, t)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let on = !prefs.show_bird;
                                Self::pref(cx, |p| p.show_bird = on);
                                this.requests.send_blocking(Request::ShowBird(on)).ok();
                            }))
                            .test_support(),
                        true,
                        t,
                    ))
                    .child(row(
                        self.title("When Crake routes a host", t, cx),
                        Some("Undo stays in the panel either way."),
                        notify,
                        true,
                        t,
                    )),
            )
            .child(group(t).mt(px(16.)).child(row(
                self.title("Appearance", t, cx),
                None,
                appearance,
                false,
                t,
            )))
    }
}
