//! Settings, Managed by IT: what an organisation set up, and where.

use super::*;

pub(super) fn managed(model: &Model, t: &Theme) -> impl IntoElement {
    let managed: Vec<_> = model.tunnels.iter().filter(|x| x.managed).collect();
    let mono = |s: &str| {
        div()
            .font_family(fonts::MONO)
            .text_size(px(13.))
            .child(s.to_owned())
    };
    let mut list = group(t).mt(px(22.));
    if managed.is_empty() {
        list = list.child(
            div()
                .py(px(14.))
                .px(px(16.))
                .text_color(t.muted)
                .child("Your organisation hasn\u{2019}t set up any tunnels here."),
        );
    }
    for (i, tunnel) in managed.into_iter().enumerate() {
        // "Learning allowed for *.acme.internal and 10.40.0.0/16."
        let what: AnyElement = if tunnel.full_tunnel {
            div()
                .child("Full tunnel required. Crake sends everything through and learns nothing.")
                .into_any_element()
        } else if tunnel.learn.is_empty() {
            div()
                .child("Crake may learn anything that needs this tunnel.")
                .into_any_element()
        } else {
            let mut line = div()
                .flex()
                .flex_wrap()
                .items_baseline()
                .gap(px(4.))
                .child("Learning allowed for");
            let n = tunnel.learn.len();
            for (j, item) in tunnel.learn.iter().enumerate() {
                // Commas and the period sit right after the name.
                let tail = match n - j {
                    1 => ".",
                    2 => "",
                    _ => ",",
                };
                line = line.child(div().flex().items_baseline().child(mono(item)).child(tail));
                if n - j == 2 {
                    line = line.child("and");
                }
            }
            line.into_any_element()
        };
        list = list.child(
            div()
                .py(px(14.))
                .px(px(16.))
                .when(i > 0, |b| b.border_t_1().border_color(t.line))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::BOLD)
                                .child(tunnel.display.clone()),
                        )
                        .child(
                            div()
                                .px(px(7.))
                                .py(px(1.))
                                .rounded(px(4.))
                                .bg(t.surface)
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(t.muted)
                                .child("Managed"),
                        ),
                )
                .child(div().mt(px(4.)).child(what))
                .child(
                    div()
                        .mt(px(4.))
                        .font_family(fonts::MONO)
                        .text_size(px(12.))
                        .text_color(t.muted)
                        .child(tunnel.managed_file.clone()),
                ),
        );
    }
    div().child(list).child(
        div()
            .mt(px(16.))
            .text_color(t.muted)
            .child("You can switch managed tunnels on and off, but not change them. Your organisation gets nothing from Crake itself."),
    )
}
