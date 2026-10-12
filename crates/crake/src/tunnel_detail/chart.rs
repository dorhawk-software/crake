//! Tunnel detail: “Today through X”, traffic and the hosts learned today.

use super::*;

impl TunnelDetail {
    /// "Today through ACME": traffic through it (grey bars) and its
    /// decisions (ticks, circles), midnight to now: the panel's timeline
    /// (`crate::timeline`) at a day's span.
    pub(super) fn chart(
        &self,
        tunnel: &Tunnel,
        marks: Vec<crate::model::Mark>,
        traffic: &[(f32, u64)],
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let now = now_minutes();
        let total: u64 = traffic.iter().map(|x| x.1).sum();
        let hosts = match tunnel.hosts {
            1 => "1 host".to_owned(),
            n => format!("{n} hosts"),
        };
        // About one bar per PX_PER_BAR of the design's width.
        let bin = (now / (DESIGN_WIDTH / PX_PER_BAR)).max(1.);
        let mut bins: Vec<(f32, u64)> = Vec::new();
        for (m, bytes) in traffic {
            let start = (m / bin).floor() * bin;
            match bins.last_mut() {
                Some((s, sum)) if *s == start => *sum += bytes,
                _ => bins.push((start, *bytes)),
            }
        }
        let bars = bins
            .into_iter()
            .map(|(start, bytes)| ((now - start - bin / 2.).max(0.), bytes))
            .collect();
        // A whole hour about halfway, as in the design's "06:00".
        let middle = ((now / HOUR / 2.).floor() * HOUR).max(0.);
        let mut axis = vec![(0., "00:00".to_owned())];
        if middle >= AXIS_CLEAR && now - middle >= AXIS_CLEAR {
            axis.push((middle / now, format!("{:02}:00", (middle / HOUR) as u32)));
        }
        axis.push((1., "now".to_owned()));
        let display = tunnel.display.clone();
        let timeline = crate::timeline::Timeline {
            span: now,
            bars,
            bin,
            grown: Vec::new(),
            marks,
            words: Box::new(move |b| format!("{} through {display}", crate::model::human_bytes(b))),
            nothing: format!("Nothing through {}", tunnel.display),
            axis,
            shape: crate::timeline::TODAY,
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
            .child(timeline.element(&self.chart_hover, t, cx, |d| &mut d.chart_hover))
    }
}

/// Today's minutes of a tunnel's traffic: (minutes since midnight, bytes).
pub(super) fn today(minutes: Option<&Vec<(u64, u64)>>) -> Vec<(f32, u64)> {
    let midnight = crate::units::midnight_minute();
    minutes
        .map(|v| {
            v.iter()
                .filter(|(m, _)| *m >= midnight)
                .map(|(m, b)| ((m - midnight) as f32, *b))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) use crate::units::minutes_today as now_minutes;

/// The design's chart width (tunnel-detail.html), and a bar per this many
/// pixels of it.
pub(super) const DESIGN_WIDTH: f32 = 836.;
const PX_PER_BAR: f32 = 7.;
/// The axis's middle label: a whole hour, at least two hours from each end.
const HOUR: f32 = crate::units::MINUTES_PER_HOUR_F;
const AXIS_CLEAR: f32 = 2. * HOUR;
