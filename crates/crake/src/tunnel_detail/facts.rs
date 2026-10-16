//! Tunnel detail: the name and state, the facts, and who decides.

use super::*;

/// "ACME", the Managed tag, and the state.
pub(super) fn title_row(tunnel: &Tunnel, t: &Theme) -> impl IntoElement {
    let (dot, state) = match tunnel.state.as_str() {
        "connected" if tunnel.dns_down => {
            (t.danger, "Connected \u{b7} its DNS doesn\u{2019}t answer")
        }
        "connected" => (t.accent, "Connected"),
        "connecting" => (t.accent, "Connecting\u{2026}"),
        "trouble" => (t.danger, crate::model::trouble_words(tunnel)),
        _ => (t.line_strong, "Off"),
    };
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(28.))
                .line_height(px(36.))
                .font_weight(FontWeight::BOLD)
                .child(tunnel.display.clone()),
        )
        .when(tunnel.managed, |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(4.))
                    .bg(t.surface)
                    .text_color(t.muted)
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(
                        svg()
                            .path("crake/lock.svg")
                            .size(px(12.))
                            .text_color(t.muted),
                    )
                    .child("Managed"),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(13.))
                .text_color(t.muted)
                .child(div().size(px(8.)).rounded_full().bg(dot))
                .child(state),
        )
}

/// Connected for, your address, protocol, gateway.
/// Connected for, address, protocol, and the gateway (an OpenVPN 3
/// tunnel's profile, with "Edit…" unless openvpn3 locked it:
/// design/proposals/edit-ovpn-profile.html, A). A long gateway or profile
/// name is cut with "…" and shown whole on hover (`full_name`).
pub(super) fn facts(
    tunnel: &Tunnel,
    profile_locked: Option<bool>,
    t: &Theme,
    cx: &mut Context<TunnelDetail>,
) -> impl IntoElement {
    let up = tunnel.state == "connected";
    let since = if up && tunnel.since > 0 {
        let secs = jiff::Timestamp::now().as_second().max(0) as u64;
        duration(secs.saturating_sub(tunnel.since))
    } else {
        "\u{2014}".to_owned()
    };
    let address = if up && !tunnel.address.is_empty() {
        tunnel.address.clone()
    } else {
        "\u{2014}".to_owned()
    };
    let protocol = match (protocol_name(&tunnel.protocol), tunnel.transport.as_str()) {
        (p, "") => p.to_owned(),
        (p, how) if up => format!("{p} \u{b7} {how}"),
        (p, _) => p.to_owned(),
    };
    let openvpn3 = tunnel.protocol == "openvpn3";
    let cell = |first: bool| {
        div()
            .flex_1()
            .min_w(px(180.))
            .py(px(12.))
            .px(px(14.))
            .when(!first, |d| d.border_l_1().border_color(t.line))
    };
    let label = |text: &'static str| {
        div()
            .text_size(px(12.))
            .line_height(px(17.))
            .text_color(t.muted)
            .child(text)
    };
    let strong = |text: String| {
        div()
            .mt(px(2.))
            .text_size(px(15.))
            .line_height(px(22.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(text)
    };
    let mono = |text: String| {
        div()
            .mt(px(2.))
            .font_family(fonts::MONO)
            .text_size(px(14.))
            .line_height(px(22.))
            .child(text)
    };
    div()
        .mt(px(18.))
        .flex()
        .border_1()
        .border_color(t.line)
        .rounded(px(8.))
        .overflow_hidden()
        .child(
            cell(true)
                .child(label("Connected for"))
                .child(strong(since)),
        )
        .child(
            cell(false)
                .child(label("Your address"))
                .child(mono(address)),
        )
        .child(cell(false).child(label("Protocol")).child(strong(protocol)))
        .child(
            cell(false)
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(label(if openvpn3 { "Profile" } else { "Gateway" }))
                        .child(div().flex_1())
                        .when(openvpn3 && profile_locked == Some(true), |row| {
                            row.child(label("Locked"))
                        })
                        .when(openvpn3 && profile_locked != Some(true), |row| {
                            let name = tunnel.name.clone();
                            row.child(
                                div()
                                    .id("edit-profile")
                                    .text_size(px(12.))
                                    .line_height(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.accent)
                                    .cursor_pointer()
                                    .child("Edit\u{2026}")
                                    .keys()
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.ask(Request::EditProfile(name.clone()));
                                    }))
                                    .test_support(),
                            )
                        }),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .line_height(px(22.))
                        .child(crate::full_name::host_name(
                            "gateway-name",
                            tunnel.gateway.clone(),
                            GATEWAY_SIZE,
                            t,
                            cx,
                        )),
                ),
        )
}

/// The gateway's or profile's name (px), as the other monospace facts.
const GATEWAY_SIZE: f32 = 14.;

/// Who decides what goes through, in plain words.
pub(super) fn policy(tunnel: &Tunnel, t: &Theme) -> impl IntoElement {
    let display = &tunnel.display;
    let mono = |text: &str| {
        div()
            .font_family(fonts::MONO)
            .text_size(px(13.))
            .child(text.to_owned())
    };
    // The bold line: words and monospaced patterns.
    let mut line = div().flex().flex_wrap().font_weight(FontWeight::BOLD);
    let mode = tunnel
        .own
        .get("mode")
        .map(String::as_str)
        .unwrap_or("learn");
    let listed: Vec<String> = if tunnel.managed {
        tunnel.learn.clone()
    } else {
        tunnel
            .own
            .get("list")
            .map(|l| {
                l.lines()
                    .map(str::trim)
                    .filter(|x| !x.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let words = |line: Div, text: String| line.child(text.replace(' ', "\u{a0}"));
    let patterns = |mut line: Div, items: &[String]| {
        for (i, item) in items.iter().enumerate() {
            line = line.child(mono(item));
            let sep = match items.len() - i {
                1 => ".",
                2 => "\u{a0}and\u{a0}",
                _ => ",\u{a0}",
            };
            line = line.child(sep);
        }
        line
    };
    let sub = if tunnel.full_tunnel {
        let who = if tunnel.managed {
            format!(
                "{} sends everything through {display}.",
                organisation(tunnel)
            )
        } else {
            format!("Everything goes through {display}.")
        };
        line = words(line, who);
        format!("Except your local network. Crake doesn\u{2019}t learn anything for {display}.")
    } else if tunnel.managed && !listed.is_empty() {
        line = words(
            line,
            format!("{} allows learning for ", organisation(tunnel)),
        );
        line = patterns(line, &listed);
        "Anything else stays direct, even if it doesn\u{2019}t answer. You can pin and forget hosts, but not widen the rule.".to_owned()
    } else if mode == "list" && !listed.is_empty() {
        line = words(line, format!("Only these go through {display}: "));
        line = patterns(line, &listed);
        "Anything else stays direct, even if it doesn\u{2019}t answer.".to_owned()
    } else if mode == "ask" {
        line = words(
            line,
            format!("Crake asks before sending a host through {display}."),
        );
        "A notification for each new host. You can pin, forget and add hosts.".to_owned()
    } else {
        line = words(line, format!("Crake learns what needs {display}."));
        format!(
            "A host goes through {display} when it only answers through {display}, or when only {display}\u{2019}s DNS knows its name. You can pin, forget and add hosts."
        )
    };
    div()
        .mt(px(14.))
        .flex()
        .gap(px(12.))
        .items_start()
        .py(px(14.))
        .px(px(16.))
        .rounded(px(8.))
        .bg(t.surface)
        .child(
            svg()
                .path("crake/shield.svg")
                .size(px(20.))
                .flex_none()
                .mt(px(1.))
                .text_color(t.ink),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(line)
                .child(div().text_size(px(13.)).text_color(t.muted).child(sub)),
        )
}

pub(super) fn organisation(tunnel: &Tunnel) -> String {
    if tunnel.organisation.is_empty() {
        "IT".to_owned()
    } else {
        tunnel.organisation.clone()
    }
}

pub fn protocol_name(protocol: &str) -> &str {
    match protocol {
        "anyconnect" => "AnyConnect",
        "gp" => "GlobalProtect",
        "pulse" => "Ivanti Connect Secure",
        "nc" => "Network Connect",
        "fortinet" => "Fortinet SSL VPN",
        "f5" => "F5 BIG-IP",
        "array" => "Array Networks",
        "openvpn3" => "OpenVPN 3",
        other => other,
    }
}

/// "4 h 53 min", "12 min", "2 days 3 h".
pub(super) fn duration(secs: u64) -> String {
    let (d, h, m) = (
        secs / crate::units::SECS_PER_DAY,
        secs % crate::units::SECS_PER_DAY / crate::units::SECS_PER_HOUR,
        secs % crate::units::SECS_PER_HOUR / crate::units::SECS_PER_MINUTE,
    );
    match (d, h) {
        (0, 0) => format!("{} min", m.max(1)),
        (0, _) => format!("{h} h {m} min"),
        (1, _) => format!("1 day {h} h"),
        _ => format!("{d} days {h} h"),
    }
}

/// In trouble, or off after the panel's Dismiss: the whole sentence craked
/// logged, under the title, until the tunnel is switched on again.
pub(super) fn why(tunnel: &Tunnel, t: &Theme) -> impl IntoElement {
    let stopped = matches!(tunnel.state.as_str(), "trouble" | "off");
    div().when(stopped && !tunnel.why.is_empty(), |d| {
        d.mt(px(6.))
            .text_size(px(13.))
            .text_color(t.muted)
            .child(tunnel.why.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel_detail::hosts::learned_how;

    #[test]
    fn durations_read_like_the_design() {
        assert_eq!(duration(4 * 3600 + 53 * 60 + 10), "4 h 53 min");
        assert_eq!(duration(20), "1 min");
        assert_eq!(duration(12 * 60), "12 min");
        assert_eq!(duration(86_400 + 3 * 3600), "1 day 3 h");
    }

    #[test]
    fn learned_hosts_say_when_and_why() {
        let now = jiff::Timestamp::now().as_second() as u64;
        let r = HostRow {
            target: "10.62.0.18".into(),
            kind: "learned".into(),
            learned: Some((now, "probe".into())),
            ..Default::default()
        };
        assert!(learned_how(&r).starts_with("Learned at "));
        assert!(learned_how(&r).ends_with("\u{b7} no answer directly"));
        let old = HostRow { learned: None, ..r };
        assert_eq!(learned_how(&old), "Learned earlier");
    }
}
