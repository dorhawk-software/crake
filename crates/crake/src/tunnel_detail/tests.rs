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
        cx.set_global(crate::chrome::Chrome {
            system_buttons: false,
        });
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

/// A host's name copies whole on a click, however it's cut short on screen
/// (design/proposals/host-full-name.html, A); it asks the app nothing.
#[gpui_kit::test]
fn a_hosts_whole_name_is_copied(cx: &mut TestAppContext) {
    let w = open(cx);
    let shown = learned(&w.rows(cx));
    let i = shown
        .iter()
        .position(|r| r.name.is_some())
        .expect("a named host in the sample");
    w.click(("host-name", i), cx);
    let copied = cx.read_from_clipboard().and_then(|c| c.text());
    assert_eq!(copied, shown[i].name.clone());
    let name = shown[i].name.clone().unwrap_or_default().into();
    assert!(
        cx.read(|cx| crate::full_name::just_copied(&name, cx)),
        "\u{201c}Copied\u{201d} shows after it"
    );
    assert!(w.next().is_none(), "nothing asked of the app");
}

/// An OpenVPN 3 tunnel's Profile fact has "Edit…", which asks the app for
/// the profile editor (design/proposals/edit-ovpn-profile.html, A); its
/// name copies whole.
#[gpui_kit::test]
fn an_openvpn3_profile_can_be_edited(cx: &mut TestAppContext) {
    let w = open(cx);
    w.model.update(cx, |m, cx| {
        let acme = m.tunnels.iter_mut().find(|t| t.name == "acme").unwrap();
        acme.protocol = "openvpn3".into();
        acme.gateway = "initech-production-eu-west-2-split".into();
        cx.notify();
    });
    cx.run_until_parked();
    while w.next().is_some() {}
    w.click("edit-profile", cx);
    assert!(matches!(w.next(), Some(Request::EditProfile(t)) if t == "acme"));
    w.click("gateway-name", cx);
    let copied = cx.read_from_clipboard().and_then(|c| c.text());
    assert_eq!(
        copied.as_deref(),
        Some("initech-production-eu-west-2-split")
    );
}

/// "Forget all…" (design/proposals/forget-all.html): asked in place first;
/// Cancel asks the app nothing, Forget all asks it to forget everything
/// learned for the tunnel, and the list is fetched again at once.
#[gpui_kit::test]
fn forget_all_asks_first(cx: &mut TestAppContext) {
    let w = open(cx);
    w.click("forget-all", cx);
    assert!(w.view.read_with(cx, |d, _| d.confirm_forget_all));
    w.click("forget-all-cancel", cx);
    assert!(!w.view.read_with(cx, |d, _| d.confirm_forget_all));
    assert!(w.next().is_none(), "Cancel asks nothing");
    w.click("forget-all", cx);
    w.click("forget-all-confirm", cx);
    assert!(matches!(w.changed(), Request::ForgetLearned(t) if t == "acme"));
    assert!(!w.view.read_with(cx, |d, _| d.confirm_forget_all));
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

/// With macOS's own window buttons (top left): no × of Crake's, and the
/// header starts after them.
#[gpui_kit::test]
fn with_the_systems_window_buttons(cx: &mut TestAppContext) {
    let w = open(cx);
    cx.update(|cx| {
        cx.set_global(crate::chrome::Chrome {
            system_buttons: true,
        })
    });
    let (close, back) = cx
        .update_window(w.window, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            (window.try_find("close"), window.find("back").bounds())
        })
        .unwrap();
    assert!(close.is_none(), "Crake's own × is there too");
    assert!(
        f32::from(back.origin.x) >= crate::chrome::BUTTONS_WIDTH,
        "\"Tunnels\" starts at {:.0} px, under the system's buttons",
        f32::from(back.origin.x)
    );
    // Level with the buttons, which macOS centres HEADER_HEIGHT / 2 down
    // (`chrome::traffic_lights`); the card's top border (none on Windows) above it.
    let middle = f32::from(back.origin.y + back.size.height / 2.);
    let line = crate::chrome::HEADER_HEIGHT / 2. + crate::chrome::CARD_BORDERS / 2.;
    assert!(
        (middle - line).abs() <= 1.,
        "\"Tunnels\" is centred at {middle:.1} px, the buttons at {line:.1}"
    );
}

/// A learned host since pinned is listed under Learned but counted once,
/// under Pinned (design/proposals/critique-2026-10-15, 5).
#[gpui_kit::test]
fn a_pinned_host_counts_once(cx: &mut TestAppContext) {
    let w = open(cx);
    let rows = w.rows(cx);
    let [learned_count, pinned_count, _] = super::hosts::tab_counts(&rows, 0);
    let listed = learned(&rows).len();
    let pinned_learned = rows
        .iter()
        .filter(|r| r.kind == "pinned" && r.learned.is_some())
        .count();
    assert!(
        pinned_learned > 0,
        "the sample has a pinned host that was learned"
    );
    assert_eq!(learned_count, listed - pinned_learned);
    assert_eq!(
        pinned_count,
        rows.iter().filter(|r| r.kind == "pinned").count()
    );
}

/// The keyboard reaches tunnel detail's controls too (critique 2026-10-15):
/// Tab moves on, Shift+Tab back, and Enter acts as a click would.
#[gpui_kit::test]
fn the_keyboard_reaches_the_controls(cx: &mut TestAppContext) {
    let w = open(cx);
    let focused = |cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| window.focused(cx))
            .unwrap()
    };
    let press = |key: &str, cx: &mut TestAppContext| {
        cx.update_window(w.window, |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
    };
    let start = focused(cx);
    assert!(start.is_some(), "the window has the keyboard as it opens");
    press("tab", cx);
    let first = focused(cx);
    assert_ne!(first, start, "Tab moves it to a control");
    press("tab", cx);
    press("shift-tab", cx);
    assert_eq!(focused(cx), first, "Shift+Tab goes back");
    while w.next().is_some() {}
    press("enter", cx);
    assert!(w.next().is_some(), "Enter acts on it");
}

/// `count` learned hosts in place of the sample's.
fn many(w: &Opened, count: usize, cx: &mut TestAppContext) {
    let one = w
        .rows(cx)
        .into_iter()
        .find(|r| r.kind == "learned")
        .expect("a learned host in the sample");
    w.model.update(cx, |m, _| {
        let rows = (0..count)
            .map(|i| HostRow {
                name: Some(format!("host-{i}.acme.internal")),
                target: format!("10.40.{}.{}", i / 250, i % 250),
                ..one.clone()
            })
            .collect();
        m.host_rows.insert("acme".into(), rows);
    });
    cx.run_until_parked();
}

/// What `id` is, drawn afresh: None when it isn't built.
fn found(
    w: &Opened,
    id: impl Into<ElementId>,
    cx: &mut TestAppContext,
) -> Option<gpui_kit::test::ElementSnapshot> {
    let id = id.into();
    cx.update_window(w.window, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        window.try_find(id)
    })
    .unwrap()
}

/// The rows in view are counted ROW_HEIGHT apart: a row drawn any other
/// height would leave the spacers wrong and rows out of place.
#[gpui_kit::test]
fn a_host_row_is_as_tall_as_counted(cx: &mut TestAppContext) {
    let w = open(cx);
    many(&w, 3, cx);
    let top = |i: usize, cx: &mut TestAppContext| {
        f32::from(found(&w, ("pin", i), cx).expect("its Pin").bounds().top())
    };
    assert_eq!(top(1, cx) - top(0, cx), super::hosts::ROW_HEIGHT);
    assert_eq!(top(2, cx) - top(1, cx), super::hosts::ROW_HEIGHT);
}

/// Hundreds of hosts: only the rows in view are built (each frame took
/// 20 ms at 500 rows, 89 ms at 2000), and scrolling down brings the last.
#[gpui_kit::test]
fn hundreds_of_hosts_are_built_as_they_come_into_view(cx: &mut TestAppContext) {
    let w = open(cx);
    many(&w, 500, cx);
    assert!(found(&w, ("pin", 0usize), cx).is_some(), "the first");
    assert!(
        found(&w, ("pin", 499usize), cx).is_none(),
        "not the last yet"
    );
    for _ in 0..40 {
        cx.update_window(w.window, |_, window, cx| {
            let down = gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                gpui_kit::px(0.),
                gpui_kit::px(-2000.),
            ));
            window.scroll("body", down, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    let last = found(&w, ("pin", 499usize), cx).expect("the last, scrolled to");
    assert!(last.visible(), "and in view");
    assert!(
        found(&w, ("pin", 0usize), cx).is_none(),
        "the first no longer built"
    );
}

#[test]
fn the_rows_in_view_and_a_margin_are_built() {
    use super::hosts::{ROW_HEIGHT, rows_in_view};
    // At the top, ten rows tall: those and the margin below.
    assert_eq!(rows_in_view(500, 0., 10. * ROW_HEIGHT), 0..30);
    // Halfway down.
    assert_eq!(
        rows_in_view(500, 200. * ROW_HEIGHT, 10. * ROW_HEIGHT),
        180..230
    );
    // At the end, and above the list (the chart in view).
    assert_eq!(
        rows_in_view(500, 495. * ROW_HEIGHT, 10. * ROW_HEIGHT),
        475..500
    );
    assert_eq!(rows_in_view(500, -900., 600.), 0..20);
    assert_eq!(rows_in_view(0, 0., 600.), 0..0);
}

/// Each tab says how long its hosts are kept, in one line of one height
/// (design/screens/tunnel-detail.html).
#[test]
fn each_tab_says_how_long_its_hosts_are_kept() {
    use super::hosts::kept_line;
    assert_eq!(
        kept_line(Tab::Learned, "ACME", "7"),
        "Crake forgets a learned host after 7 days unused."
    );
    assert_eq!(
        kept_line(Tab::Learned, "ACME", "1"),
        "Crake forgets a learned host after a day unused."
    );
    assert_eq!(
        kept_line(Tab::Learned, "ACME", "never"),
        "Crake keeps a learned host until you forget it."
    );
    assert_eq!(
        kept_line(Tab::Pinned, "ACME", "7"),
        "Pinned hosts are kept until you forget them."
    );
    assert_eq!(
        kept_line(Tab::Gateway, "ACME", "7"),
        "ACME\u{2019}s gateway sent these when it connected."
    );
}

/// Opened from the log's "Show in ACME": the host's tab, its row lit; a
/// host only pinned opens Pinned.
#[gpui_kit::test]
fn the_log_opens_it_on_a_host(cx: &mut TestAppContext) {
    let w = open(cx);
    let host = w.rows(cx)[0].target.clone();
    w.view.update(cx, |d, cx| d.show_host(host.clone(), cx));
    w.view.read_with(cx, |d, _| {
        assert!(d.tab == Tab::Learned);
        assert_eq!(d.shown_host.as_deref(), Some(host.as_str()));
    });
    w.model.update(cx, |m, _| {
        m.host_rows.get_mut("acme").unwrap().push(HostRow {
            target: "10.40.9.9".into(),
            name: None,
            kind: "pinned".into(),
            learned: None,
            used_ago: None,
        });
    });
    w.view
        .update(cx, |d, cx| d.show_host("10.40.9.9".into(), cx));
    w.view.read_with(cx, |d, _| assert!(d.tab == Tab::Pinned));
    // Drawn with the row lit and scrolled to.
    assert!(found(&w, ("pin", 0usize), cx).is_some());
}
