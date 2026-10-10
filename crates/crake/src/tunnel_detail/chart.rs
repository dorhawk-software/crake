//! Tunnel detail: “Today through X”, traffic and the hosts learned today.

use super::*;

/// "Today through ACME": traffic through it (grey bars) and the hosts it
/// learned (ticks), midnight to now.
pub(super) fn chart(
    tunnel: &Tunnel,
    traffic: &[(f32, u64)],
    ticks: &[f32],
    t: &Theme,
) -> impl IntoElement {
    let now = now_minutes();
    let total: u64 = traffic.iter().map(|x| x.1).sum();
    let hosts = match tunnel.hosts {
        1 => "1 host".to_owned(),
        n => format!("{n} hosts"),
    };
    // About one bar per 7 px of the design's 836.
    let bin = (now / (836. / 7.)).max(1.);
    let mut bins: Vec<(f32, u64)> = Vec::new();
    for (m, bytes) in traffic {
        let start = (m / bin).floor() * bin;
        match bins.last_mut() {
            Some((s, sum)) if *s == start => *sum += bytes,
            _ => bins.push((start, *bytes)),
        }
    }
    let max = bins.iter().map(|b| b.1).max().unwrap_or(0).max(1) as f32;
    let (bars, accent, panel, base) = (t.bars, t.accent, t.panel, t.line_strong);
    let (ticks, paint_bins) = (ticks.to_vec(), bins.clone());
    // A whole hour about halfway, as in the design's "06:00".
    let middle = ((now / 60. / 2.).floor() * 60.).max(0.);
    let mono = |text: String| {
        div()
            .absolute()
            .top(px(BASELINE + 3.))
            .whitespace_nowrap()
            .font_family(fonts::MONO)
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(t.muted)
            .child(text)
    };
    div()
        .mt(px(22.))
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::BOLD)
                        .child(format!("Today through {}", tunnel.display)),
                )
                .child(
                    div()
                        .font_family(fonts::MONO)
                        .text_size(px(12.))
                        .text_color(t.muted)
                        .child(format!(
                            "{hosts} \u{b7} {}",
                            crate::model::human_bytes(total)
                        )),
                ),
        )
        .child(
            div()
                .relative()
                .mt(px(8.))
                .h(px(CHART_H))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let w = f32::from(bounds.size.width);
                            let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
                            let baseline = top + BASELINE;
                            let x_of = |m: f32| left + 2. + (w - 4.) * m / now;
                            let scale = window.scale_factor();
                            for (start, bytes) in &paint_bins {
                                if *bytes == 0 {
                                    continue;
                                }
                                let h = 2. + 16. * (*bytes as f32 / max).sqrt();
                                if let Some(p) = crate::chart::bar(
                                    x_of(start + bin / 2.),
                                    3.,
                                    baseline,
                                    h,
                                    scale,
                                ) {
                                    window.paint_path(p, bars);
                                }
                            }
                            // Ticks keep clear of the bars (DESIGN.md, Charts).
                            for m in &ticks {
                                for (dx, tip) in [(0., 10.), (4.5, 18.)] {
                                    if let Some(p) = crate::chart::rounded_bar(
                                        x_of(*m) + dx,
                                        top + tip - 1.5,
                                        baseline,
                                        6.,
                                    ) {
                                        window.paint_path(p, panel);
                                    }
                                }
                            }
                            let mut line = PathBuilder::stroke(px(1.));
                            line.move_to(point(px(left), px(baseline + 0.5)));
                            line.line_to(point(px(left + w), px(baseline + 0.5)));
                            if let Ok(p) = line.build() {
                                window.paint_path(p, base);
                            }
                            for m in &ticks {
                                for (dx, tip) in [(0., 10.), (4.5, 18.)] {
                                    if let Some(p) = crate::chart::rounded_bar(
                                        x_of(*m) + dx,
                                        top + tip,
                                        baseline,
                                        3.,
                                    ) {
                                        window.paint_path(p, accent);
                                    }
                                }
                            }
                        },
                    )
                    .size_full(),
                )
                .child(mono("00:00".into()).left_0())
                .when(middle >= 120. && now - middle >= 120., |d| {
                    // Centred under its time.
                    d.child(
                        div()
                            .absolute()
                            .left(gpui_kit::relative(middle / now))
                            .top_0()
                            .w(px(34.))
                            .h_full()
                            .child(mono(format!("{:02}:00", (middle / 60.) as u32)).left(px(-17.))),
                    )
                })
                .child(mono("now".into()).right_0()),
        )
}

/// Today's minutes of a tunnel's traffic: (minutes since midnight, bytes).
pub(super) fn today(minutes: Option<&Vec<(u64, u64)>>) -> Vec<(f32, u64)> {
    let midnight = jiff::Zoned::now()
        .start_of_day()
        .map(|z| z.timestamp().as_second() as u64 / 60)
        .unwrap_or(0);
    minutes
        .map(|v| {
            v.iter()
                .filter(|(m, _)| *m >= midnight)
                .map(|(m, b)| ((m - midnight) as f32, *b))
                .collect()
        })
        .unwrap_or_default()
}

/// When the tunnel learned hosts today (minutes since midnight).
pub(super) fn learned_today(model: &Model, tunnel: &str) -> Vec<f32> {
    model
        .today_events()
        .into_iter()
        .filter(|(_, e)| {
            e.kind == crake_proto::LogKind::Routed && e.tunnel.as_deref() == Some(tunnel)
        })
        .map(|(m, _)| m)
        .collect()
}

/// Minutes since local midnight, now.
pub(super) fn now_minutes() -> f32 {
    let now = jiff::Zoned::now();
    (now.hour() as f32 * 60. + now.minute() as f32 + now.second() as f32 / 60.).max(1.)
}
