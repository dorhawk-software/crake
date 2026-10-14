//! The detail under the graph, in two tabs: "Right now" (the open
//! connections, busiest first) and "Where things go" (every rule Crake
//! has: learned, pinned, sent direct with Undo, never, and everything
//! else).

use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{Div, FontWeight, Rgba, div, px};

use crate::model::Model;
use crate::theme::{Theme, fonts};

use super::graph::rate;

/// The most rows a tab lists (the rest are counted under them).
pub const MOST_ROWS: usize = 100;
const MINUTE: u64 = 60;
/// Column widths: the way, a column of words, a column of numbers.
const WAY_COL: f32 = 160.;
const WHY_COL: f32 = 340.;
const NUMBER_COL: f32 = 90.;
const HOUR: u64 = 60 * MINUTE;

/// A row: destination (and the address under a name), the way, and the
/// two right-hand columns.
pub struct Row {
    pub dest: String,
    pub under: Option<String>,
    /// None: direct.
    pub tunnel: Option<String>,
    pub goes: String,
    pub cols: Vec<String>,
}

/// "Right now": each open connection, busiest first.
pub fn right_now(model: &Model, now: Instant) -> Vec<Row> {
    let Some(rates) = model.live.rates.as_ref() else {
        return Vec::new();
    };
    let mut conns: Vec<_> = rates.connections.iter().collect();
    conns.sort_by(|a, b| b.1.total_cmp(&a.1));
    conns
        .into_iter()
        .map(|(c, r)| Row {
            dest: format!(
                "{}:{}",
                c.name.clone().unwrap_or_else(|| c.dst.to_string()),
                c.dport
            ),
            under: None,
            tunnel: c.tunnel.clone(),
            goes: way_label(model, c.tunnel.as_deref()),
            cols: vec![
                rate(*r),
                model.live.open_for(c, now).map(since).unwrap_or_default(),
            ],
        })
        .collect()
}

/// "Where things go": what each tunnel sends (learned, pinned), what was
/// sent direct with Undo, the never list, and everything else.
pub fn where_things_go(model: &Model) -> Vec<Row> {
    let mut rows = Vec::new();
    for tunnel in &model.tunnels {
        for r in model.host_rows.get(&tunnel.name).into_iter().flatten() {
            let (way, why) = match r.kind.as_str() {
                "direct" => (None, "You sent it direct with Undo".to_owned()),
                "pinned" => (Some(tunnel.name.clone()), "Pinned".to_owned()),
                _ => (
                    Some(tunnel.name.clone()),
                    crate::tunnel_detail::learned_how(r),
                ),
            };
            rows.push(Row {
                dest: r.name.clone().unwrap_or_else(|| r.target.clone()),
                under: r.name.as_ref().map(|_| r.target.clone()),
                goes: match &way {
                    Some(_) => format!("through {}", tunnel.display),
                    None => "direct".into(),
                },
                tunnel: way,
                cols: vec![why],
            });
        }
    }
    for never in model.setting_list("never") {
        rows.push(Row {
            dest: never,
            under: None,
            tunnel: None,
            goes: "direct".into(),
            cols: vec!["Never through a tunnel (Settings \u{2192} Learning)".into()],
        });
    }
    let full = model
        .tunnels
        .iter()
        .find(|t| t.full_tunnel && t.state == "connected");
    rows.push(Row {
        dest: "Everything else".into(),
        under: None,
        tunnel: full.map(|t| t.name.clone()),
        goes: full.map_or("direct".into(), |t| format!("through {}", t.display)),
        cols: vec![match full {
            Some(t) => format!("{} is a full tunnel", t.display),
            None => "Crake sends a host through a tunnel only when it needs one".into(),
        }],
    });
    rows
}

/// A way's name in a row: the tunnel's, or "direct".
fn way_label(model: &Model, tunnel: Option<&str>) -> String {
    match tunnel {
        Some(name) => model
            .tunnels
            .iter()
            .find(|t| t.name == name)
            .map_or_else(|| name.to_owned(), |t| t.display.clone()),
        None => "direct".into(),
    }
}

/// How long a connection has been open: "12 s", "4 min", "1 h".
pub fn since(d: Duration) -> String {
    let s = d.as_secs();
    if s < MINUTE {
        format!("{s} s")
    } else if s < HOUR {
        format!("{} min", s / MINUTE)
    } else {
        format!("{} h", s / HOUR)
    }
}

/// The table: a header line, then the rows (up to MOST_ROWS, then how
/// many more).
pub fn table(heads: &[&'static str], rows: &[Row], t: &Theme) -> Div {
    let cell = |w: f32| div().w(px(w)).flex_none().min_w_0();
    let right_cols = heads.len().saturating_sub(2);
    let head = heads.iter().enumerate().fold(
        div()
            .flex()
            .py(px(6.))
            .border_b_1()
            .border_color(t.line)
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.muted),
        |row, (i, h)| match i {
            0 => row.child(div().flex_1().min_w_0().child(*h)),
            1 => row.child(cell(WAY_COL).child(*h)),
            _ => row.child(
                right(
                    cell(if right_cols == 1 { WHY_COL } else { NUMBER_COL }),
                    right_cols,
                )
                .child(*h),
            ),
        },
    );
    let mut body = div().flex().flex_col().child(head);
    for r in rows.iter().take(MOST_ROWS) {
        let dot = if r.tunnel.is_some() {
            t.accent
        } else {
            t.muted
        };
        let mut line = div()
            .flex()
            .items_center()
            .py(px(7.))
            .border_b_1()
            .border_color(t.line)
            .text_size(px(13.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_family(fonts::MONO)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(r.dest.clone()),
                    )
                    .when_some(r.under.clone(), |d, under| {
                        d.child(div().text_size(px(12.)).text_color(t.muted).child(under))
                    }),
            )
            .child(cell(WAY_COL).child(way(&r.goes, dot)));
        for c in &r.cols {
            line = line.child(
                right(
                    cell(if right_cols == 1 { WHY_COL } else { NUMBER_COL }),
                    right_cols,
                )
                .text_color(t.muted)
                .when(right_cols > 1, |d| {
                    d.font_family(fonts::MONO).text_size(px(12.))
                })
                .child(c.clone()),
            );
        }
        body = body.child(line);
    }
    if rows.len() > MOST_ROWS {
        body = body.child(
            div()
                .py(px(7.))
                .text_size(px(13.))
                .text_color(t.muted)
                .child(format!("and {} more", rows.len() - MOST_ROWS)),
        );
    }
    body
}

/// Numbers sit right; a single wide column of words stays left.
fn right(d: Div, right_cols: usize) -> Div {
    if right_cols > 1 {
        d.flex().justify_end()
    } else {
        d.pl(px(10.))
    }
}

/// A way: a dot in its colour and its name.
fn way(label: &str, color: Rgba) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(div().size(px(8.)).rounded_full().bg(color))
        .child(label.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn how_long_in_words() {
        assert_eq!(since(Duration::from_secs(12)), "12 s");
        assert_eq!(since(Duration::from_secs(4 * 60 + 5)), "4 min");
        assert_eq!(since(Duration::from_secs(3 * 3600)), "3 h");
    }

    #[test]
    fn every_rule_and_then_everything_else() {
        let mut m = crate::model::demo();
        m.host_rows.insert(
            "acme".into(),
            vec![
                crate::model::HostRow {
                    target: "10.40.1.10".into(),
                    name: Some("git.acme.internal".into()),
                    kind: "learned".into(),
                    learned: Some((0, "probe".into())),
                    used_ago: None,
                },
                crate::model::HostRow {
                    target: "10.40.8.0/24".into(),
                    kind: "pinned".into(),
                    ..Default::default()
                },
                crate::model::HostRow {
                    target: "10.40.1.11".into(),
                    kind: "direct".into(),
                    ..Default::default()
                },
            ],
        );
        m.settings.insert("never".into(), "192.168.0.0/16".into());
        let rows = where_things_go(&m);
        let acme = &m.tunnels.iter().find(|t| t.name == "acme").unwrap().display;
        assert_eq!(rows[0].dest, "git.acme.internal");
        assert_eq!(rows[0].under.as_deref(), Some("10.40.1.10"));
        assert_eq!(rows[0].goes, format!("through {acme}"));
        assert_eq!(rows[1].cols, ["Pinned"]);
        assert_eq!(rows[2].goes, "direct");
        assert!(
            rows.iter()
                .any(|r| r.dest == "192.168.0.0/16" && r.goes == "direct")
        );
        let last = rows.last().unwrap();
        assert_eq!(last.dest, "Everything else");
    }
}
