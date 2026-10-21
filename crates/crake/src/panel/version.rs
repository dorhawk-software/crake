//! A newer Crake (design/proposals/new-version.html): "updated, restart"
//! when a package updated craked, or "X is out" when asked and found; and
//! once updated, What's new (`whats_new`).

use super::*;
use crate::versions::{self, Notice};

impl Panel {
    pub(super) fn version_notice(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let prefs = cx.global::<crate::prefs::Prefs>();
        let newest = model.newest.as_deref().filter(|_| prefs.check_versions);
        let notice = versions::notice(model.service_version.as_deref(), newest, &prefs.dismissed);
        // One version notice at a time: Restart, then "X is out", then
        // What's new (design/proposals/whats-new.html), which waits.
        let (text, actions): (String, Vec<(&'static str, Request)>) = match notice {
            None => (
                crate::whats_new::notice(prefs)?,
                vec![
                    ("What's new", Request::WhatsNew),
                    ("Not now", Request::SeenVersion),
                ],
            ),
            Some(Notice::Restart(v)) => (
                format!("Crake was updated to {v}. Restart it to finish."),
                vec![("Restart", Request::Restart)],
            ),
            Some(Notice::Out(v)) => (
                format!("Crake {v} is out. You have {}.", versions::this()),
                vec![
                    ("Download", Request::OpenUrl(versions::download(&v))),
                    ("Not now", Request::Dismiss(versions::dismiss_key(&v))),
                ],
            ),
        };
        let mut note = div()
            .id("version")
            .mx(px(18.))
            .mb(px(10.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(8.))
            .bg(t.surface)
            .flex()
            .items_center()
            .gap(px(12.))
            .text_size(px(13.))
            .child(div().flex_1().min_w_0().child(text));
        for (label, request) in actions {
            // A notice's shape (DESIGN.md): a bordered action, and a muted
            // "Not now" (they were teal text, against the colour rule).
            let dismiss = label == "Not now";
            note = note.child(
                div()
                    .id(label)
                    .flex_none()
                    .when(!dismiss, |d| {
                        d.px(px(12.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .bg(t.panel)
                            .border_1()
                            .border_color(t.line)
                            .text_color(t.ink)
                            .hover(|s| s.border_color(t.muted))
                    })
                    .when(dismiss, |d| d.text_color(t.muted))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child(label)
                    .keys()
                    .on_click(cx.listener(move |this, _, _, _| {
                        this.send(request.clone());
                    }))
                    .test_support(),
            );
        }
        Some(note)
    }
}
