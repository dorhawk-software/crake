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
