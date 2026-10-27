//! The tray on Windows and macOS (tray-icon): an icon in the notification
//! area or the menu bar, its menu the same fallback as on Linux. The bar
//! is light or dark (`tray_on_light_bar`: the Mac's menu bar's own
//! appearance, Windows' theme), so the head is drawn white or dark to match.

use std::cell::RefCell;
use std::rc::Rc;

/// A pixel's four bytes; the head's ink on a light bar.
const ARGB: usize = 4;
const DARK: [u8; 3] = [0x1f; 3];

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::{set_tray_image, tray_click, tray_icon_size, tray_on_light_bar};
use crate::Request;
use crate::tray::{Icon, MenuTunnel, State};

struct Shown {
    tray: TrayIcon,
    state: State,
    /// The menu as last built: its summary and tunnels.
    menu: Option<(String, Vec<MenuTunnel>)>,
}

/// The running tray; lives on the app's main thread.
#[derive(Clone)]
pub struct Handle(Rc<RefCell<Shown>>);

impl Handle {
    pub fn update(&self, f: impl FnOnce(&mut State)) {
        let mut shown = self.0.borrow_mut();
        f(&mut shown.state);
        shown.show();
    }

    /// Linux tells its bar again after a change (`sys::linux::tray`); this
    /// tray sets its image itself.
    pub fn settle(&self) {}

    /// Take the icon out of the notification area.
    pub fn shutdown(&self) {
        let _ = self.0.borrow().tray.set_visible(false);
    }
}

impl Shown {
    fn show(&mut self) {
        // Drawn at the size the bar shows it: shrunk by the system, the
        // head looked choppy.
        let icon = self.state.icon();
        let light = tray_on_light_bar(&self.tray);
        let drawings: Vec<_> = icon
            .mac
            .iter()
            .map(|(size, data)| (*size, rgba(data, light)))
            .collect();
        if !set_tray_image(&self.tray, &drawings) {
            let (size, data) = drawn_at(&icon, tray_icon_size());
            if let Ok(icon) = tray_icon::Icon::from_rgba(rgba(data, light), size, size) {
                let _ = self.tray.set_icon(Some(icon));
            }
        }
        let _ = self.tray.set_tooltip(Some(&self.state.summary));
        let now = (self.state.summary.clone(), self.state.tunnels.clone());
        if self.menu.as_ref() != Some(&now) {
            self.tray.set_menu(Some(Box::new(menu(&now.0, &now.1))));
            self.menu = Some(now);
        }
    }
}

/// The fallback menu: the summary, a check mark per tunnel, Open Crake,
/// New tunnel, Settings, Quit. Each item's id says what it asks for (`on_menu`).
fn menu(summary: &str, tunnels: &[MenuTunnel]) -> Menu {
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id("summary", summary, false, None));
    for t in tunnels {
        let id = format!("switch:{}:{}", !t.on, t.name);
        let _ = menu.append(&CheckMenuItem::with_id(id, &t.display, true, t.on, None));
    }
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("open", "Open Crake", true, None));
    let _ = menu.append(&MenuItem::with_id(
        "new-tunnel",
        "New tunnel\u{2026}",
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(
        "settings",
        "Settings\u{2026}",
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));
    menu
}

/// What a menu item asks for, by its id.
fn on_menu(id: &str) -> Option<Request> {
    match id {
        "open" => Some(Request::Panel(None)),
        "new-tunnel" => Some(Request::NewTunnel(None)),
        "settings" => Some(Request::Settings),
        "quit" => Some(Request::Quit),
        _ => {
            let (on, name) = id.strip_prefix("switch:")?.split_once(':')?;
            Some(Request::Switch(name.to_owned(), on == "true"))
        }
    }
}

pub fn start(requests: async_channel::Sender<Request>) -> Option<Handle> {
    let clicks = requests.clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            position,
            rect,
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            let at = tray_click(position, rect);
            clicks.send_blocking(Request::Panel(Some(at))).ok();
        }
    }));
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if let Some(r) = on_menu(&e.id.0) {
            requests.send_blocking(r).ok();
        }
    }));
    let tray = TrayIconBuilder::new()
        .with_tooltip("Crake")
        .with_menu_on_left_click(false)
        .build()
        .ok()?;
    let mut shown = Shown {
        tray,
        state: State::default(),
        menu: None,
    };
    shown.show();
    Some(Handle(Rc::new(RefCell::new(shown))))
}

/// The drawing for a `size` px slot: that size if there is one, else the
/// smallest larger one (shrunk a little by the system), else the largest.
fn drawn_at(icon: &Icon, size: u32) -> (u32, &[u8]) {
    icon.small
        .iter()
        .filter(|(s, _)| *s >= size)
        .min_by_key(|(s, _)| *s)
        .map_or((icon.width, &icon.data), |(s, d)| (*s, d))
}

/// ARGB (the tray art's order) as RGBA, what tray-icon and macOS take;
/// for a light bar, the white head drawn dark (the coloured marks as they
/// are).
fn rgba(argb: &[u8], light: bool) -> Vec<u8> {
    argb.as_chunks::<ARGB>()
        .0
        .iter()
        .flat_map(|&[a, r, g, b]| {
            let [r, g, b] = if light {
                crate::tray::light_bar_pixel([r, g, b], DARK)
            } else {
                [r, g, b]
            };
            [r, g, b, a]
        })
        .collect()
}
