//! Nothing shows tray icons (`crate::no_tray`): when Crake was started by
//! hand, its panel opens near the top of the screen with this note
//! (design/proposals/no-tray-after.html, E). Neutral, not amber: the tray is
//! optional and Crake works fully without it; no teal either: a bordered
//! action and a muted "Not now", which puts it off until Crake starts again.

use super::*;

impl Panel {
    pub(super) fn no_tray_note(
        &self,
        model: &Model,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let show = cx.global::<crate::prefs::Prefs>().show_bird;
        let remedy = model
            .no_tray
            .as_ref()
            .filter(|_| show && !model.no_tray_quiet)?;
        let action = crate::no_tray_view::remedy(remedy, &self.requests, t);
        // The command takes the whole line; "Not now" goes under it.
        let wide = remedy.command().is_some();
        let later = div()
            .id("no-tray-later")
            .flex_none()
            .text_color(t.muted)
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .child("Not now")
            .keys()
            .on_click(cx.listener(|this, _, _, cx| {
                this.send(Request::NoTray(crate::tray_host::Event::Quiet));
                cx.notify();
            }))
            .test_support();
        let actions = if wide {
            div()
                .mt(px(8.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(action)
                .child(later)
        } else {
            div()
                .mt(px(8.))
                .flex()
                .items_center()
                .gap(px(14.))
                .children(action)
                .child(later)
        };
        Some(
            div()
                .id("no-tray")
                .mx(px(18.))
                .mb(px(10.))
                .px(px(12.))
                .py(px(10.))
                .rounded(px(8.))
                .bg(t.surface)
                .text_size(px(13.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(crate::no_tray::TITLE),
                )
                .child(div().mt(px(3.)).text_color(t.muted).child(remedy.note()))
                .child(actions)
                .test_support(),
        )
    }
}
