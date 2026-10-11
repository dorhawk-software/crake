//! The tray on Windows and macOS (tray-icon): an icon in the notification
//! area or the menu bar, its menu the same fallback as on Linux. The bar
//! is light or dark with the system's theme (`light_bar`), so the head is
//! drawn white or dark to match.

use std::cell::RefCell;
use std::rc::Rc;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::{light_bar, logical};
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

    /// Take the icon out of the notification area.
    pub fn shutdown(&self) {
        let _ = self.0.borrow().tray.set_visible(false);
    }
}

impl Shown {
    fn show(&mut self) {
        let icon = self.state.icon();
        let icon = if light_bar() { on_light(&icon) } else { icon };
        if let Some(icon) = rgba(&icon) {
            let _ = self.tray.set_icon(Some(icon));
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
/// Settings, Quit. Each item's id says what it asks for (`on_menu`).
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
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            let at = logical(position.x, position.y);
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

/// The image for a light bar: the white head drawn dark, the coloured
/// marks as they are.
fn on_light(icon: &Icon) -> Icon {
    let mut data = icon.data.clone();
    for p in data.chunks_mut(4) {
        if p[1] == p[2] && p[2] == p[3] {
            p[1..].fill(0x1f);
        }
    }
    Icon { data, ..*icon }
}

/// ARGB (the tray art's order) as the RGBA tray-icon takes.
fn rgba(icon: &Icon) -> Option<tray_icon::Icon> {
    let data = icon
        .data
        .chunks(4)
        .flat_map(|p| [p[1], p[2], p[3], p[0]])
        .collect();
    tray_icon::Icon::from_rgba(data, icon.width, icon.height).ok()
}
