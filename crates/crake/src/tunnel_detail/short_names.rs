//! Tunnel detail's Short names: each short name you chose this tunnel's
//! full name for ("expert", typed on its own), with Forget.

use super::hosts::{host_text, icon, row_button, row_frame, used_text};
use super::*;

impl TunnelDetail {
    /// `expert` over "expert.corp.acme.internal · chosen at 13:21".
    pub(super) fn short_row(
        &self,
        i: usize,
        r: &HostRow,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let full = r.name.clone().unwrap_or_default();
        let when = r
            .learned
            .as_ref()
            .and_then(|(at, _)| jiff::Timestamp::from_second(i64::try_from(*at).ok()?).ok())
            .map(|ts| ts.to_zoned(jiff::tz::TimeZone::system()))
            .map(|z| {
                if z.date() == jiff::Zoned::now().date() {
                    format!("chosen at {}", z.strftime("%H:%M"))
                } else {
                    format!("chosen on {}", z.strftime("%-d %b"))
                }
            });
        let how = match when {
            Some(when) => format!("{full} \u{b7} {when}"),
            None => full,
        };
        let short = r.target.clone();
        let (danger, surface) = (t.danger, t.surface);
        row_frame(t)
            .child(icon("crake/mark-pinned.svg", t.ink))
            .child(host_text(i, r.target.clone(), how, t, cx))
            .child(used_text(String::new(), t))
            .child(
                div().flex().justify_end().child(
                    row_button(("forget", i), "Forget", t)
                        .hover(move |s| s.bg(surface).text_color(danger))
                        .keys()
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.send(Request::Forget(this.tunnel.clone(), short.clone()));
                        }))
                        .test_support(),
                ),
            )
    }
}
