//! The profile editor in a headless window, against a stand-in for
//! openvpn3: what it's given, what it hands back, and what it says.

use std::sync::Mutex;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, TestAppContext, WindowOptions};

use super::*;

const PROFILE: &str =
    "client\ndev tun\nremote vpn.initech.example 1194\n<ca>\nAAAA\nBBBB\n</ca>\nverb 3\n";

/// openvpn3 as a test wants it: a profile (or a lock), and what it was given.
struct Fake {
    fetched: Fetched,
    refuse: Option<String>,
    given: Mutex<Vec<(String, String)>>,
}

impl Store for Fake {
    fn fetch(&self, _: &str) -> Result<Fetched, String> {
        Ok(self.fetched.clone())
    }
    fn replace(&self, profile: &str, text: &str) -> Result<(), String> {
        self.given
            .lock()
            .unwrap()
            .push((profile.to_owned(), text.to_owned()));
        self.refuse.clone().map_or(Ok(()), Err)
    }
}

struct Opened {
    window: AnyWindowHandle,
    view: Entity<ProfileEditor>,
    fake: Arc<Fake>,
    requests: async_channel::Receiver<Request>,
}

fn open(cx: &mut TestAppContext, fetched: Fetched, refuse: Option<&str>) -> Opened {
    cx.update(|cx| {
        gpui_kit::init(cx);
        // Crake's own ×, as on Linux and Windows (macOS draws its own).
        cx.set_global(crate::chrome::Chrome {
            system_buttons: false,
        });
        cx.set_global(crate::prefs::Prefs {
            scratch: true,
            ..Default::default()
        });
    });
    let fake = Arc::new(Fake {
        fetched,
        refuse: refuse.map(str::to_owned),
        given: Mutex::default(),
    });
    let (tx, requests) = async_channel::unbounded();
    let store: Arc<dyn Store> = fake.clone();
    let mut made = None;
    let (window, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            let view = cx.new(|cx| {
                ProfileEditor::new(
                    "initech".into(),
                    "Initech".into(),
                    "initech".into(),
                    store,
                    tx,
                    window,
                    cx,
                )
            });
            made = Some(view.clone());
            view
        })
        .expect("a window")
    });
    cx.run_until_parked();
    Opened {
        window,
        view: made.expect("the view"),
        fake,
        requests,
    }
}

impl Opened {
    /// Put `text` in the editor, as typing it would.
    fn type_text(&self, text: &str, cx: &mut TestAppContext) {
        let view = self.view.clone();
        cx.update_window(self.window, |_, window, cx| {
            view.update(cx, |v, cx| {
                v.editor
                    .update(cx, |e, cx| e.set_value(text.to_owned(), window, cx));
                v.edited(cx);
            });
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn click(&self, id: &'static str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn shown(&self, cx: &mut TestAppContext) -> String {
        self.view
            .read_with(cx, |v, cx| v.editor.read(cx).value().to_string())
    }
}

#[gpui_kit::test]
fn the_profile_is_shown_with_its_blocks_folded(cx: &mut TestAppContext) {
    let w = open(cx, Fetched::Text(PROFILE.into()), None);
    assert_eq!(w.view.read_with(cx, |v, _| v.stage.clone()), Stage::Editing);
    let shown = w.shown(cx);
    assert!(shown.contains("<ca> certificate, 2 lines, kept as they are </ca>"));
    assert!(
        !shown.contains("AAAA"),
        "the certificate stays out of sight"
    );
}

/// Save gives openvpn3 the edited text with the blocks back in, asks the
/// app to reconnect the tunnel, and closes.
#[gpui_kit::test]
fn save_gives_openvpn3_the_whole_new_text_and_reconnects(cx: &mut TestAppContext) {
    let w = open(cx, Fetched::Text(PROFILE.into()), None);
    // Nothing changed: Save does nothing.
    w.click("save", cx);
    assert!(w.fake.given.lock().unwrap().is_empty());
    let edited = w.shown(cx).replace("vpn.initech", "vpn2.initech");
    w.type_text(&edited, cx);
    w.click("save", cx);
    let given = w.fake.given.lock().unwrap().clone();
    assert_eq!(
        given,
        [(
            "initech".to_owned(),
            PROFILE.replace("vpn.initech", "vpn2.initech")
        )]
    );
    assert!(matches!(w.requests.try_recv(), Ok(Request::Reconnect(t)) if t == "initech"));
}

/// openvpn3 refusing the text: its words under the text, nothing else.
#[gpui_kit::test]
fn a_refusal_is_said_and_changes_nothing(cx: &mut TestAppContext) {
    let refusal =
        "OpenVPN 3 refused it: \u{201c}remote: unknown protocol udpp\u{201d}. Nothing was changed.";
    let w = open(cx, Fetched::Text(PROFILE.into()), Some(refusal));
    let edited = w.shown(cx).replace("1194", "1194 udpp");
    w.type_text(&edited, cx);
    w.click("save", cx);
    let (stage, note) = w
        .view
        .read_with(cx, |v, _| (v.stage.clone(), v.note.clone()));
    assert_eq!(stage, Stage::Editing);
    assert_eq!(note.as_deref(), Some(refusal));
    assert!(w.requests.try_recv().is_err(), "no reconnect");
}

/// A folded line changed by hand can't be put back: said, nothing given.
#[gpui_kit::test]
fn a_changed_folded_line_is_refused_before_openvpn3(cx: &mut TestAppContext) {
    let w = open(cx, Fetched::Text(PROFILE.into()), None);
    let edited = w.shown(cx).replace("2 lines", "3 lines");
    w.type_text(&edited, cx);
    w.click("save", cx);
    assert!(w.fake.given.lock().unwrap().is_empty());
    let note = w.view.read_with(cx, |v, _| v.note.clone());
    assert!(note.is_some_and(|n| n.contains("<ca>")));
}

#[gpui_kit::test]
fn a_locked_profile_says_so(cx: &mut TestAppContext) {
    let w = open(cx, Fetched::Locked, None);
    assert_eq!(w.view.read_with(cx, |v, _| v.stage.clone()), Stage::Locked);
}

#[gpui_kit::test]
fn close_and_cancel_ask_the_app_to_close(cx: &mut TestAppContext) {
    let w = open(cx, Fetched::Text(PROFILE.into()), None);
    let closed = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = closed.clone();
    cx.update(|cx| {
        cx.subscribe(&w.view, move |_, _: &Close, _| seen.set(seen.get() + 1))
            .detach();
    });
    w.click("cancel", cx);
    w.click("close", cx);
    assert_eq!(closed.get(), 2);
}
