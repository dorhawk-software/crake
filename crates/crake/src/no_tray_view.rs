//! What brings the bird back, as Settings' row and the panel's note show it
//! (design/proposals/no-tray-after.html): a bordered button when Crake can
//! do it (switch the extension on, open GNOME's Extensions), or the command
//! to install it, on one line with Copy inside ("Copied" for a moment).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, ClipboardItem, FontWeight, TestSupportExt, div, px, svg};

use crate::Request;
use crate::focus::Keys as _;
use crate::no_tray::{OPEN_EXTENSIONS, Remedy, SWITCH_ON};
use crate::theme::{Theme, fonts};
use crate::tray_host::Event;

/// How long "Copied" stays after Copy.
const COPIED_FOR: Duration = Duration::from_secs(3);
/// When the command was last copied (one command, wherever it's shown).
static COPIED: Mutex<Option<Instant>> = Mutex::new(None);

fn copied() -> bool {
    COPIED
        .lock()
        .is_ok_and(|at| at.is_some_and(|at| at.elapsed() < COPIED_FOR))
}

/// The remedy's control, or none when there's nothing to press.
pub fn remedy(
    remedy: &Remedy,
    requests: &async_channel::Sender<Request>,
    t: &Theme,
) -> Option<AnyElement> {
    match remedy {
        Remedy::SwitchOn(id) => Some(
            button(
                "no-tray-switch",
                SWITCH_ON,
                t,
                requests,
                Event::SwitchOn(id.clone()),
            )
            .into_any_element(),
        ),
        Remedy::Refused(_) => Some(
            button(
                "no-tray-extensions",
                OPEN_EXTENSIONS,
                t,
                requests,
                Event::OpenExtensions,
            )
            .into_any_element(),
        ),
        Remedy::Install(Some(command)) => Some(chip(command, t).into_any_element()),
        _ => None,
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    t: &Theme,
    requests: &async_channel::Sender<Request>,
    event: Event,
) -> impl IntoElement + use<> {
    let requests = requests.clone();
    div()
        .id(id)
        .flex_none()
        .px(px(12.))
        .py(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(t.line_strong)
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.ink)
        .cursor_pointer()
        .child(label)
        .keys()
        .on_click(move |_, _, _| {
            requests.send_blocking(Request::NoTray(event.clone())).ok();
        })
        .test_support()
}

/// The command on one line (it scrolls rather than breaks), Copy inside.
fn chip(command: &str, t: &Theme) -> impl IntoElement + use<> {
    let text = command.to_owned();
    let done = copied();
    let ink = if done { t.muted } else { t.ink };
    div()
        .w_full()
        .flex()
        .text_size(px(13.))
        .border_1()
        .border_color(t.line_strong)
        .rounded(px(6.))
        .overflow_hidden()
        .child(
            div()
                .id("no-tray-command")
                .flex_1()
                .min_w_0()
                .px(px(9.))
                .py(px(6.))
                .font_family(fonts::MONO)
                .whitespace_nowrap()
                .overflow_x_scroll()
                .child(text.clone()),
        )
        .child(
            div()
                .id("no-tray-copy")
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .border_l_1()
                .border_color(t.line_strong)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ink)
                .cursor_pointer()
                .child(
                    svg()
                        .path(if done {
                            "crake/check.svg"
                        } else {
                            "crake/clipboard.svg"
                        })
                        .size(px(14.))
                        .text_color(ink),
                )
                .child(if done { "Copied" } else { "Copy" })
                .keys()
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                    if let Ok(mut at) = COPIED.lock() {
                        *at = Some(Instant::now());
                    }
                    cx.refresh_windows();
                    cx.spawn(async move |cx| {
                        cx.background_executor().timer(COPIED_FOR).await;
                        cx.update(|cx| cx.refresh_windows());
                    })
                    .detach();
                })
                .test_support(),
        )
}
