//! The Traffic window in a headless window, with the design's sample
//! (model::demo, its counters from model::live::demo): it asks craked
//! while open, draws each way and its busiest destinations, and lists
//! connections and rules in its tabs.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, ElementId, TestAppContext, WindowOptions};

use super::*;

struct Opened {
    window: AnyWindowHandle,
    view: Entity<Traffic>,
    model: Entity<Model>,
    requests: async_channel::Receiver<Request>,
}

fn open(cx: &mut TestAppContext) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::chrome::Chrome {
            system_buttons: false,
        });
        cx.set_global(crate::prefs::Prefs {
            scratch: true,
            ..Default::default()
        });
    });
    let model = cx.new(|_| crate::model::demo());
    let (tx, requests) = async_channel::unbounded();
    let shown = model.clone();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Traffic::new(shown, tx, window, cx))
        })
        .expect("a window")
    });
    cx.run_until_parked();
    Opened {
        window,
        view,
        model,
        requests,
    }
}

impl Opened {
    /// Two looks at the sample's counters, a second apart.
    fn feed(&self, cx: &mut TestAppContext) {
        self.model.update(cx, |m, cx| {
            let t0 = Instant::now();
            let live = std::sync::Arc::make_mut(&mut m.live);
            live.add(crate::model::live::demo(100), t0);
            live.add(crate::model::live::demo(101), t0 + Duration::from_secs(1));
            cx.notify();
        });
        cx.run_until_parked();
    }

    fn shows(&self, id: impl Into<ElementId>, cx: &mut TestAppContext) -> bool {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    }

    fn click(&self, id: impl Into<ElementId>, cx: &mut TestAppContext) {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
}

fn name(s: &str) -> ElementId {
    ElementId::Name(s.to_owned().into())
}

/// While it's open, craked is asked for the counters, and each tunnel for
/// its hosts ("Where things go").
#[gpui_kit::test]
fn opening_asks_for_the_counters_and_the_hosts(cx: &mut TestAppContext) {
    let w = open(cx);
    let tunnels = w.model.read_with(cx, |m, _| m.tunnels.len());
    let mut asked = Vec::new();
    while let Ok(r) = w.requests.try_recv() {
        asked.push(r);
    }
    assert!(matches!(asked.first(), Some(Request::Live)), "{asked:?}");
    let hosts = asked
        .iter()
        .filter(|r| matches!(r, Request::Detail(_)))
        .count();
    assert_eq!(hosts, tunnels, "every tunnel's hosts");
}

/// The graph: each tunnel that's up and direct, each with its busiest
/// destinations; past four, the last line counts the rest.
#[gpui_kit::test]
fn each_way_shows_its_busiest_destinations(cx: &mut TestAppContext) {
    let w = open(cx);
    w.feed(cx);
    let up: Vec<String> = w.model.read_with(cx, |m, _| {
        m.tunnels
            .iter()
            .filter(|t| t.state == "connected")
            .map(|t| t.display.clone())
            .collect()
    });
    assert!(!up.is_empty(), "the sample has a tunnel up");
    for label in up.iter().map(String::as_str).chain(["Direct"]) {
        assert!(w.shows(name(&format!("way:{label}")), cx), "{label}");
    }
    assert!(w.shows(name("dest:Direct:teams.microsoft.com"), cx));
    let ways = w.model.read_with(cx, |m, _| graph::ways(m));
    assert!(ways.iter().all(|w| w.hosts.len() <= graph::HOSTS_PER_WAY));
    let direct = ways.last().unwrap();
    assert!(direct.rate > 0., "direct traffic flows");
}

/// The tabs: the open connections, then every rule Crake has.
#[gpui_kit::test]
fn the_tabs_list_connections_and_rules(cx: &mut TestAppContext) {
    let w = open(cx);
    w.feed(cx);
    let rows = w
        .model
        .read_with(cx, |m, _| tables::right_now(m, Instant::now()));
    assert_eq!(rows.len(), 10, "the sample's connections");
    assert_eq!(rows[0].dest, "git.acme.internal:443", "busiest first");
    // Crake learned it: the row and the graph wear the two-stroke tick; a
    // direct destination gets a dot (design/proposals/audit-2026-10-17.html, 1).
    assert!(rows[0].learned);
    assert!(rows.iter().any(|r| r.goes == "direct" && !r.learned));
    assert!(w.shows(name("tick:ACME:git.acme.internal"), cx));
    assert!(!w.shows(name("tick:Direct:teams.microsoft.com"), cx));
    // The rate shown is the one they're ordered by: never more than above.
    let bytes = |s: &str| {
        let (n, unit) = s.split_once(' ').unwrap();
        let n: f64 = n.parse().unwrap();
        if unit.starts_with("MB") {
            n * 1e6
        } else {
            n * 1e3
        }
    };
    let shown: Vec<f64> = rows.iter().map(|r| bytes(&r.cols[0])).collect();
    assert!(shown.windows(2).all(|p| p[0] >= p[1]), "{shown:?}");
    w.click("tab-rules", cx);
    assert!(w.view.read_with(cx, |v, _| v.tab == Tab::Rules));
    w.click("tab-now", cx);
    assert!(w.view.read_with(cx, |v, _| v.tab == Tab::Now));
}

/// A destination copies whole on a click (design/proposals/host-full-name.html, A).
#[gpui_kit::test]
fn a_destination_is_copied_whole(cx: &mut TestAppContext) {
    let w = open(cx);
    w.feed(cx);
    let rows = w
        .model
        .read_with(cx, |m, _| tables::right_now(m, Instant::now()));
    w.click(("dest", 0usize), cx);
    let copied = cx.read_from_clipboard().and_then(|c| c.text());
    assert_eq!(copied.as_deref(), Some(rows[0].dest.as_str()));
}

/// A destination in the graph copies on a click too, as in the tables.
#[gpui_kit::test]
fn a_destination_in_the_graph_is_copied(cx: &mut TestAppContext) {
    let w = open(cx);
    w.feed(cx);
    w.click(name("copy:Direct:teams.microsoft.com"), cx);
    let copied = cx.read_from_clipboard().and_then(|c| c.text());
    assert_eq!(copied.as_deref(), Some("teams.microsoft.com"));
}

/// When craked couldn't list the connections, the ways out still show;
/// the table says why it's empty.
#[gpui_kit::test]
fn without_connections_the_ways_still_show(cx: &mut TestAppContext) {
    let w = open(cx);
    w.model.update(cx, |m, cx| {
        let t0 = Instant::now();
        let live = std::sync::Arc::make_mut(&mut m.live);
        let counters = |n: u64| crake_proto::live::Live {
            physical: n * 1000,
            ..Default::default()
        };
        live.add(counters(1), t0);
        live.add(counters(2), t0 + Duration::from_secs(1));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(w.shows(name("way:Direct"), cx));
    assert_eq!(
        w.model.read_with(cx, |m, _| m.live.lists_connections()),
        Some(false)
    );
}

/// The close button and Escape ask the app to close the window.
#[gpui_kit::test]
fn close_asks_the_app(cx: &mut TestAppContext) {
    let w = open(cx);
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = closed.clone();
    cx.update(|cx| {
        cx.subscribe(&w.view, move |_, _: &Close, _| seen.set(true))
            .detach();
    });
    w.click("close", cx);
    assert!(closed.get());
}

/// With macOS's own window buttons: the title starts after them and is
/// level with them, not taller (the user, 2026-10-26: "the buttons should
/// be the topmost thing, or level with it").
#[gpui_kit::test]
fn the_title_is_level_with_the_systems_buttons(cx: &mut TestAppContext) {
    let w = open(cx);
    cx.update(|cx| {
        cx.set_global(crate::chrome::Chrome {
            system_buttons: true,
        })
    });
    let title = cx
        .update_window(w.window, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.find("traffic-title").bounds()
        })
        .unwrap();
    assert!(f32::from(title.origin.x) >= crate::chrome::BUTTONS_WIDTH);
    let middle = f32::from(title.origin.y + title.size.height / 2.);
    let line = crate::chrome::HEADER_HEIGHT / 2. + 1.;
    assert!(
        (middle - line).abs() <= 1.,
        "\"Traffic\" is centred at {middle:.1} px, the buttons at {line:.1}"
    );
}
