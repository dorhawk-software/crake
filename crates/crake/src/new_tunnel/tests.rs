//! New tunnel in a headless window: real clicks and typing through the
//! steps, checked against what "Add and connect" asks the app to do.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, TestAppContext, WindowOptions};

use super::*;

struct Opened {
    window: AnyWindowHandle,
    view: Entity<NewTunnel>,
    requests: async_channel::Receiver<Request>,
}

fn open(cx: &mut TestAppContext) -> Opened {
    cx.update(gpui_kit::init);
    let model = cx.new(|_| Model::default());
    let (tx, requests) = async_channel::unbounded();
    let (window, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| NewTunnel::new(model, tx, None, window, cx))
        })
        .expect("a window")
    });
    // The profiles and engines (look_around's fixed pair) arrive.
    cx.run_until_parked();
    Opened {
        window,
        view,
        requests,
    }
}

impl Opened {
    fn click(&self, id: impl Into<gpui_kit::ElementId>, cx: &mut TestAppContext) {
        let id = id.into();
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    /// Type into the field `field` picks (it gets the focus first).
    fn type_into(
        &self,
        field: impl Fn(&NewTunnel) -> Entity<InputState>,
        text: &str,
        cx: &mut TestAppContext,
    ) {
        let input = self.view.read_with(cx, |this, _| field(this));
        cx.update_window(self.window, |_, window, cx| {
            input.update(cx, |f, cx| f.focus(window, cx));
            window.render_frame(cx);
            window.input(text, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn read<T>(&self, cx: &mut TestAppContext, f: impl Fn(&NewTunnel) -> T) -> T {
        self.view.read_with(cx, |this, _| f(this))
    }

    fn shown(&self, id: &'static str, cx: &mut TestAppContext) -> bool {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .unwrap()
    }

    /// The request "Add and connect" sent.
    fn added(&self) -> HashMap<String, String> {
        match self.requests.try_recv() {
            Ok(Request::AddTunnel(fields, None)) => fields,
            other => panic!("expected AddTunnel, got {other:?}"),
        }
    }
}

const OPENVPN3: usize = 8;
const ANYCONNECT: usize = 1;

/// The OpenVPN 3 path, where two bugs were: the chosen profile jumped back
/// to "Detect automatically", and the routing step had no Continue.
#[gpui_kit::test]
fn an_openvpn3_profile_gets_added(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Initech", cx);
    w.click(("protocol", OPENVPN3), cx);
    assert_eq!(w.read(cx, |t| t.protocol), "openvpn3");
    w.click("profile", cx);
    // The last of three: its menu row lies over the protocol list, where a
    // click once went through and picked "Detect automatically".
    w.click(("profile", 2usize), cx);
    assert_eq!(
        w.read(cx, |t| t.profile.clone()).as_deref(),
        Some("initech")
    );
    assert_eq!(w.read(cx, |t| t.protocol), "openvpn3", "the choice stays");

    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 3);
    assert!(w.shown("next", cx), "routing has its button");
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["display_name"], "Initech");
    assert_eq!(fields["protocol"], "openvpn3");
    assert_eq!(
        fields["gateway"], "initech",
        "an OpenVPN 3 tunnel's gateway is its profile"
    );
    assert_eq!(fields["group"], "");
    assert_eq!(fields["mode"], "learn");
}

#[gpui_kit::test]
fn a_gateway_with_a_password_and_ask_me_first(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "ACME", cx);
    w.click(("protocol", ANYCONNECT), cx);
    w.type_into(|t| t.gateway.clone(), "vpn.acme.example", cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 2);

    w.click(("method", 1usize), cx);
    w.type_into(|t| t.user.clone(), "jane", cx);
    w.click("next", cx);
    w.click(("mode", 1usize), cx);
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["display_name"], "ACME");
    assert_eq!(fields["protocol"], "anyconnect");
    assert_eq!(fields["gateway"], "vpn.acme.example");
    assert_eq!(fields["sign_in"], "password");
    assert_eq!(fields["user"], "jane");
    assert_eq!(fields["mode"], "ask");
}

#[gpui_kit::test]
fn only_what_i_list_keeps_the_list(cx: &mut TestAppContext) {
    let w = open(cx);
    w.type_into(|t| t.name.clone(), "Globex", cx);
    w.click(("protocol", ANYCONNECT), cx);
    w.type_into(|t| t.gateway.clone(), "vpn.globex.example", cx);
    w.click("next", cx);
    w.click("next", cx);
    w.click(("mode", 2usize), cx);
    w.click("add-listed", cx);
    w.type_into(|t| t.list_input.clone(), "*.globex.internal", cx);
    cx.update_window(w.window, |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(w.read(cx, |t| t.list.clone()), ["*.globex.internal"]);
    w.click("next", cx);

    let fields = w.added();
    assert_eq!(fields["mode"], "list");
    assert_eq!(fields["list"], "*.globex.internal");
}

/// Continue on an incomplete step stays there and says what's missing.
#[gpui_kit::test]
fn an_incomplete_step_says_what_is_missing(cx: &mut TestAppContext) {
    let w = open(cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 1);
    assert_eq!(
        w.read(cx, |t| t.problem.clone()).as_deref(),
        Some("Give it a name.")
    );

    w.type_into(|t| t.name.clone(), "Initech", cx);
    w.click(("protocol", OPENVPN3), cx);
    w.click("next", cx);
    assert_eq!(w.read(cx, |t| t.step), 1);
    assert_eq!(
        w.read(cx, |t| t.problem.clone()).as_deref(),
        Some("Which OpenVPN 3 profile?")
    );

    w.click(("protocol", ANYCONNECT), cx);
    w.click("next", cx);
    assert_eq!(
        w.read(cx, |t| t.problem.clone()).as_deref(),
        Some("Which gateway? Its name or address, like vpn.example.com.")
    );
    assert!(w.requests.try_recv().is_err(), "nothing was added");
}
