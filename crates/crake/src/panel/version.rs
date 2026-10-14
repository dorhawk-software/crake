//! A newer Crake (design/proposals/new-version.html): "updated, restart"
//! when a package updated craked, or "X is out" when asked and found.

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
        let notice = versions::notice(model.service_version.as_deref(), newest, &prefs.dismissed)?;
        let (text, actions): (String, Vec<(&'static str, Request)>) = match notice {
            Notice::Restart(v) => (
                format!("Crake was updated to {v}. Restart it to finish."),
                vec![("Restart", Request::Restart)],
            ),
            Notice::Out(v) => (
                format!("Crake {v} is out. You have {}.", versions::this()),
                vec![
                    (
                        "Download",
                        Request::OpenUrl(format!("{}{v}", versions::RELEASES)),
                    ),
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
            note = note.child(
                div()
                    .id(label)
                    .flex_none()
                    .text_color(if label == "Not now" {
                        t.muted
                    } else {
                        t.accent
                    })
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, _| {
                        this.send(request.clone());
                    }))
                    .test_support(),
            );
        }
        Some(note)
    }
}
