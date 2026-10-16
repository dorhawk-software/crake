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
