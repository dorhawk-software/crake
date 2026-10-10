//! Tunnel detail in a headless window, with the design's sample
//! (model::demo_detail, ACME): its hosts' buttons, "Add a host" and the
//! header ask the app for what they say.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, ElementId, TestAppContext, WindowOptions};

use super::*;
use crate::model::HostRow;

struct Opened {
    window: AnyWindowHandle,
    view: Entity<TunnelDetail>,
    model: Entity<Model>,
    requests: async_channel::Receiver<Request>,
}

fn open(cx: &mut TestAppContext) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(crate::prefs::Prefs {
            scratch: true,
            ..Default::default()
        });
    });
    let model = cx.new(|_| crate::model::demo_detail());
    let (tx, requests) = async_channel::unbounded();
    let shown = model.clone();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| TunnelDetail::new("acme".into(), shown, tx, window, cx))
        })
        .expect("a window")
    });
    cx.run_until_parked();
    let w = Opened {
        window,
        view,
        model,
        requests,
    };
    // Opening asks for its hosts and traffic.
    assert!(matches!(w.next(), Some(Request::Detail(n)) if n == "acme"));
    w
}

impl Opened {
    fn click(&self, id: impl Into<ElementId>, cx: &mut TestAppContext) {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn next(&self) -> Option<Request> {
        self.requests.try_recv().ok()
    }

    /// What a host's button asked, then the refresh that shows it at once.
    fn changed(&self) -> Request {
        let r = self.next().expect("a request");
        assert!(
            matches!(self.next(), Some(Request::Detail(n)) if n == "acme"),
            "a refresh follows"
        );
        r
    }

    fn rows(&self, cx: &mut TestAppContext) -> Vec<HostRow> {
        self.model.read_with(cx, |m, _| m.host_rows["acme"].clone())
    }

    fn type_into_add(&self, text: &str, cx: &mut TestAppContext) {
        let input = self.view.read_with(cx, |d, _| d.add.clone());
        cx.update_window(self.window, |_, window, cx| {
            input.update(cx, |f, cx| f.focus(window, cx));
            window.render_frame(cx);
            window.input(text, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
}

/// Learned hosts, newest first (as the tab shows them).
fn learned(rows: &[HostRow]) -> Vec<HostRow> {
    let mut l: Vec<HostRow> = rows
        .iter()
        .filter(|r| r.kind == "learned" || (r.kind == "pinned" && r.learned.is_some()))
        .cloned()
        .collect();
    l.sort_by_key(|r| std::cmp::Reverse(r.learned.as_ref().map_or(0, |l| l.0)));
    l
}

#[gpui_kit::test]
fn a_hosts_pin_and_forget(cx: &mut TestAppContext) {
    let w = open(cx);
    let shown = learned(&w.rows(cx));
    let i = shown
        .iter()
        .position(|r| r.kind == "learned")
        .expect("a learned host in the sample");
    let target = shown[i].target.clone();

    w.click(("pin", i), cx);
    assert!(matches!(w.changed(), Request::Pin(t, h) if t == "acme" && h == [target.clone()]));
    w.click(("forget", i), cx);
    assert!(matches!(w.changed(), Request::Forget(t, h) if t == "acme" && h == target));

    // Pinned: its button unpins.
    w.click(("tab", 1usize), cx);
    assert!(w.view.read_with(cx, |d, _| d.tab == Tab::Pinned));
    let pinned: Vec<HostRow> = w
        .rows(cx)
        .into_iter()
        .filter(|r| r.kind == "pinned")
        .collect();
    assert!(!pinned.is_empty(), "the sample pins something");
    w.click(("pin", 0usize), cx);
    assert!(matches!(w.changed(), Request::Unpin(t, h) if t == "acme" && h == pinned[0].target));
}

/// "Add a host": a name or an address goes through, a network is pinned.
#[gpui_kit::test]
fn add_a_host(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into_add("wiki.acme.internal", cx);
    w.click("send", cx);
    assert!(matches!(
        w.changed(),
        Request::Send(t, h) if t == "acme" && h == ["wiki.acme.internal"]
    ));
    let left = w
        .view
        .read_with(cx, |d, cx| d.add.read(cx).value().to_string());
    assert_eq!(left, "", "the field empties");

    w.type_into_add("10.41.0.0/16", cx);
    cx.update_window(w.window, |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(matches!(
        w.changed(),
        Request::Pin(t, h) if t == "acme" && h == ["10.41.0.0/16"]
    ));
}

#[gpui_kit::test]
fn the_header(cx: &mut TestAppContext) {
    let w = open(cx);
    let on = w.model.read_with(cx, |m, _| {
        m.tunnels
            .iter()
            .any(|t| t.name == "acme" && t.state == "connected")
    });
    w.click("switch", cx);
    assert!(matches!(w.next(), Some(Request::Switch(t, to)) if t == "acme" && to == !on));
    // Back to Settings → Tunnels.
    w.click("back", cx);
    assert!(matches!(w.next(), Some(Request::Settings)));
}
