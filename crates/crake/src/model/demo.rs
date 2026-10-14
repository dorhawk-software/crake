//! The design's sample data, for CRAKE_DEMO: the panel, tunnel detail and
//! the log, as in design/screens.

use super::*;

// literal: the design's sample data (design/screens), for CRAKE_DEMO
/// The design's sample (design/screens/tray-panel.html), for `CRAKE_DEMO=1`:
/// a panel to compare with the screen, with no craked needed.
pub fn demo() -> Model {
    let now = jiff::Timestamp::now();
    let at = |mins_ago: i64| {
        now.checked_sub(jiff::SignedDuration::from_mins(mins_ago))
            .unwrap_or(now)
            .to_string()
    };
    let entry =
        |mins: i64, kind: LogKind, tunnel: &str, msg: &str, why: &str, hosts: &[&str]| LogEntry {
            ts: at(mins),
            kind,
            tunnel: Some(tunnel.into()),
            message: msg.into(),
            reason: Some(why.into()),
            hosts: hosts.iter().map(|h| h.to_string()).collect(),
            others: Vec::new(),
        };
    let tunnel = |name: &str, display: &str, state: &str, hosts: u32| Tunnel {
        name: name.into(),
        display: display.into(),
        state: state.into(),
        hosts,
        address: String::new(),
        ..Default::default()
    };
    // Steady traffic with two busier stretches, like the design's.
    let minute = now.as_second() as u64 / 60;
    let mut walk = Walk(5);
    let traffic = (0..=60u64)
        .map(|i| {
            let seed = walk.next();
            let busy = (8..22).contains(&i) || (40..52).contains(&i);
            let bytes = 200_000 + seed * if busy { 40 } else { 8 };
            (minute - 60 + i, bytes)
        })
        .collect();
    Model {
        reachable: Some(true),
        tunnels: vec![
            Tunnel {
                managed: true,
                policy: "ACME IT allows learning for *.acme.internal".into(),
                managed_file: "/etc/crake/managed.d/acme.toml".into(),
                learn: vec!["*.acme.internal".into(), "10.40.0.0/16".into()],
                dns_domains: vec!["acme.internal".into()],
                source: "managed".into(),
                organisation: "ACME IT".into(),
                gateway: "vpn.acme.example".into(),
                protocol: "anyconnect".into(),
                ..tunnel("acme", "ACME", "connected", 5)
            },
            Tunnel {
                dns_domains: vec!["nordbank.example".into()],
                source: "app".into(),
                gateway: "vpn.nordbank.example".into(),
                protocol: "gp".into(),
                split_include: vec!["172.22.0.0/16".into()],
                own: [
                    ("display_name", "Nordbank"),
                    ("gateway", "vpn.nordbank.example"),
                    ("protocol", "gp"),
                    ("sign_in", "browser"),
                    ("mode", "learn"),
                    ("forget_after_days", "7"),
                    ("keep_signed_in", "true"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
                ..tunnel("nord", "Nordbank", "connected", 2)
            },
            Tunnel {
                gateway: "vpn.globex.example".into(),
                protocol: "anyconnect".into(),
                auth_args: vec!["--certificate=/home/you/Downloads/globex.p12".into()],
                managed: true,
                policy: "Full tunnel required by Globex IT. Crake won\u{2019}t learn here.".into(),
                full_tunnel: true,
                managed_file: "/etc/crake/managed.d/globex.toml".into(),
                source: "managed".into(),
                organisation: "Globex IT".into(),
                ..tunnel("globex", "Globex", "off", 0)
            },
            Tunnel {
                source: "app".into(),
                gateway: "203.0.113.7".into(),
                protocol: "anyconnect".into(),
                own: [
                    ("display_name", "Home lab"),
                    ("gateway", "203.0.113.7"),
                    ("protocol", "anyconnect"),
                    ("sign_in", "password"),
                    ("user", "me"),
                    ("mode", "learn"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
                ..tunnel("home", "Home lab", "off", 3)
            },
        ],
        log: vec![
            entry(
                140,
                LogKind::Routed,
                "nord",
                "Sent sftp.nordbank.example through Nordbank.",
                "Refused directly. Through Nordbank it answered in 61 ms.",
                &["10.20.0.4"],
            ),
            entry(
                22,
                LogKind::LeftAlone,
                "acme",
                "Left 10.62.0.77 alone.",
                "No answer directly or through ACME. Probably down.",
                &["10.62.0.77"],
            ),
            entry(
                3,
                LogKind::Routed,
                "acme",
                "Sent git.acme.internal through ACME.",
                "It didn\u{2019}t answer directly. Through ACME it answered in 38 ms.",
                &["10.40.1.20"],
            ),
        ]
        .into_iter()
        // CRAKE_DEMO=busy: two more right next to it, to see a merged tick.
        .chain(
            (std::env::var("CRAKE_DEMO").as_deref() == Ok("busy"))
                .then(|| {
                    [
                        entry(
                            3,
                            LogKind::Routed,
                            "acme",
                            "Sent wiki.acme.internal through ACME.",
                            "Only ACME\u{2019}s DNS knows it.",
                            &["10.40.1.21"],
                        ),
                        entry(
                            2,
                            LogKind::Routed,
                            "acme",
                            "Sent jira.acme.internal through ACME.",
                            "Only ACME\u{2019}s DNS knows it.",
                            &["10.40.1.22"],
                        ),
                    ]
                })
                .into_iter()
                .flatten(),
        )
        .collect(),
        traffic,
        // As in settings.html.
        settings: [
            ("probe_wait_secs", "3"),
            ("forget_after_days", "7"),
            ("order", "acme\nnord\nhome"),
            ("both", "ask"),
            ("never", "*.mybank.example\n192.168.1.0/24"),
            ("ask_unknown", "true"),
            ("keep_private", "true"),
            ("log_days", "30"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect(),
        ..Default::default()
    }
}

/// The demo with Nordbank's session expired (CRAKE_DEMO=expired).
pub fn demo_expired(mut m: Model) -> Model {
    if let Some(nord) = m.tunnels.iter_mut().find(|t| t.name == "nord") {
        nord.state = "trouble".into();
        nord.hosts = 0;
        nord.why = format!(
            "{}: the server ends sessions after a while (this one after 54 minutes). {}",
            crake_proto::trouble::EXPIRED,
            crake_proto::trouble::SIGN_IN_AGAIN
        );
    }
    m
}

// literal: the design's sample data (design/screens), for CRAKE_DEMO
/// Tunnel detail's sample (design/screens/tunnel-detail.html), for
/// `CRAKE_DEMO=1 CRAKE_SHOW=detail:acme`: ACME connected for 4 h 53 min,
/// its five learned hosts (wiki pinned), its DNS and pushed network, and
/// traffic since the morning, all relative to now.
pub fn demo_detail() -> Model {
    let mut m = demo();
    let now = jiff::Timestamp::now().as_second() as u64;
    if let Some(acme) = m.tunnels.iter_mut().find(|t| t.name == "acme") {
        acme.address = "10.40.12.7".into();
        acme.since = now - (4 * 60 + 53) * 60;
        acme.transport = "DTLS".into();
        acme.dns = vec!["10.40.0.53".into()];
        acme.split_include = vec!["10.40.0.0/16".into()];
        acme.hosts = 6;
    }
    // (target, name, minutes since learned, how, minutes since used, pinned)
    let hosts = [
        ("10.40.3.21", "git.acme.internal", 3, "probe", 0, false),
        ("10.62.0.18", "", 157, "probe", 12, false),
        ("10.40.8.4", "wiki.acme.internal", 250, "dns", 60, true),
        ("10.40.8.9", "jira.acme.internal", 252, "dns", 3, false),
        (
            "10.40.9.30",
            "builds.acme.example",
            284,
            "split",
            120,
            false,
        ),
    ];
    m.host_rows.insert(
        "acme".into(),
        hosts
            .iter()
            .map(|(target, name, learned, how, used, pinned)| HostRow {
                target: (*target).into(),
                name: (!name.is_empty()).then(|| (*name).into()),
                kind: if *pinned { "pinned" } else { "learned" }.into(),
                learned: Some((now - learned * 60, (*how).into())),
                used_ago: (!pinned).then_some(used * 60),
            })
            .collect(),
    );
    // Busy since about 4 h 54 min ago, as the design's 8:30 to 13:24.
    let minute = now / 60;
    let mut walk = Walk(9);
    let traffic = (0..=294u64)
        .map(|i| {
            let seed = walk.next();
            (minute - 294 + i, 50_000 + seed * 40)
        })
        .collect();
    m.tunnel_traffic.insert("acme".into(), traffic);
    for (target, name, learned, how, ..) in hosts {
        let ts = jiff::Timestamp::from_second((now - learned * 60) as i64)
            .unwrap_or_else(|_| jiff::Timestamp::now())
            .to_string();
        let shown = if name.is_empty() { target } else { name };
        m.log.push(LogEntry {
            ts,
            kind: LogKind::Routed,
            tunnel: Some("acme".into()),
            message: format!("Sent {shown} through ACME."),
            reason: Some(how.into()),
            hosts: vec![target.into()],
            others: Vec::new(),
        });
    }
    m.log.sort_by(|a, b| a.ts.cmp(&b.ts));
    m
}

// literal: the design's sample data (design/screens), for CRAKE_DEMO
/// The log design's sample (design/screens/log.html), for `CRAKE_DEMO=log`:
/// the same rows at the same times today and yesterday, and a day of
/// traffic that gets busy at 8:18, as in its chart.
pub fn demo_log() -> Model {
    log_until(jiff::Timestamp::now())
}

// literal: the design's sample data (design/screens), for CRAKE_DEMO
/// The whole of today's sample, later entries too: what tests use, so they
/// don't depend on the time of day.
#[cfg(test)]
pub fn demo_log_whole_day() -> Model {
    log_until(jiff::Timestamp::MAX)
}

// literal: the design's sample data (design/screens), for CRAKE_DEMO
/// The sample as it stands at `until`: entries after it haven't happened.
fn log_until(until: jiff::Timestamp) -> Model {
    use LogKind::*;
    let mut m = demo();
    let today = jiff::Zoned::now();
    let at = |days_ago: i64, hm: &str| {
        let (h, mi) = hm.split_once(':').unwrap();
        today
            .checked_sub(jiff::SignedDuration::from_hours(24 * days_ago))
            .ok()
            .and_then(|z| {
                z.with()
                    .hour(h.parse().unwrap())
                    .minute(mi.parse().unwrap())
                    .second(0)
                    .build()
                    .ok()
            })
            .map(|z| z.timestamp().to_string())
            .unwrap_or_default()
    };
    let row =
        |d: i64, hm: &str, kind: LogKind, tunnel: Option<&str>, msg: &str, why: &str| LogEntry {
            ts: at(d, hm),
            kind,
            tunnel: tunnel.map(str::to_owned),
            message: msg.into(),
            reason: Some(why.into()),
            hosts: vec!["10.0.0.1".into()],
            others: Vec::new(),
        };
    let mut log = vec![
        row(
            1,
            "14:03",
            Routed,
            Some("nord"),
            "Sent 172.22.8.4 through Nordbank.",
            "No answer directly. Nordbank answered in 66 ms.",
        ),
        row(
            1,
            "16:20",
            LeftAlone,
            Some("nord"),
            "Left status.nordbank.example alone.",
            "No answer anywhere. It was down for 4 minutes.",
        ),
        row(
            1,
            "17:55",
            System,
            None,
            "Nordbank disconnected.",
            "You switched it off.",
        ),
        row(
            0,
            "08:31",
            System,
            None,
            "ACME and Nordbank connected.",
            "Started with your session.",
        ),
        row(
            0,
            "08:40",
            Routed,
            Some("acme"),
            "Sent builds.acme.example through ACME.",
            "Public name, private address. ACME answered in 47 ms.",
        ),
        row(
            0,
            "09:12",
            Routed,
            Some("acme"),
            "Sent jira.acme.internal through ACME.",
            "Only ACME\u{2019}s DNS knows the name. Routed before the first packet.",
        ),
        row(
            0,
            "09:14",
            Routed,
            Some("acme"),
            "Sent wiki.acme.internal through ACME.",
            "Only ACME\u{2019}s DNS knows the name. Routed before the first packet.",
        ),
        row(
            0,
            "10:12",
            Undone,
            Some("acme"),
            "You sent cdn.acme.example direct again.",
            "It was public after all. Crake won\u{2019}t relearn it on its own.",
        ),
        row(
            0,
            "10:47",
            Routed,
            Some("acme"),
            "Sent 10.62.0.18 through ACME.",
            "No answer directly. ACME answered in 52 ms.",
        ),
        row(
            0,
            "11:04",
            Routed,
            Some("nord"),
            "Sent sftp.nordbank.example through Nordbank.",
            "Refused directly. Nordbank answered in 61 ms.",
        ),
        row(
            0,
            "11:30",
            Routed,
            Some("nord"),
            "Sent 172.22.8.4 through Nordbank.",
            "No answer directly. Nordbank answered in 70 ms.",
        ),
        row(
            0,
            "13:02",
            LeftAlone,
            Some("acme"),
            "Left 10.62.0.77 alone.",
            "No answer directly or through ACME. Probably down.",
        ),
        row(
            0,
            "13:21",
            Routed,
            Some("acme"),
            "Sent git.acme.internal through ACME.",
            "No answer directly. ACME answered in 38 ms.",
        ),
    ];
    log.retain(|e| e.ts.parse::<jiff::Timestamp>().is_ok_and(|t| t <= until));
    m.log = log;
    let midnight = today
        .start_of_day()
        .map(|z| z.timestamp().as_second() as u64 / 60)
        .unwrap_or(0);
    let now = jiff::Timestamp::now().as_second() as u64 / 60;
    let mut walk = Walk(3);
    m.days = (0..)
        .map(|i| midnight + i)
        .take_while(|slot| *slot <= now)
        .map(|slot| {
            let seed = walk.next();
            let hour = (slot - midnight) as f32 / 60.;
            let busy = hour > 8.3;
            let bytes = if busy {
                2_000_000 + seed * 120
            } else if seed % 10 < 3 {
                300_000 + seed * 4
            } else {
                0
            };
            (slot, bytes)
        })
        .collect();
    m
}

/// A small random walk for the demo's charts, the same every run: a linear
/// congruential generator with Numerical Recipes' short constants.
struct Walk(u64);

impl Walk {
    const MULTIPLIER: u64 = 9301;
    const INCREMENT: u64 = 49297;
    const MODULUS: u64 = 233_280;

    fn next(&mut self) -> u64 {
        self.0 = (self.0 * Self::MULTIPLIER + Self::INCREMENT) % Self::MODULUS;
        self.0
    }
}
