//! The tray on Windows: an icon in the notification area, its menu the
//! same fallback as on Linux. The taskbar is light or dark with the
//! system's theme, so the head is drawn white or dark to match.

use std::cell::RefCell;
use std::rc::Rc;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};

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
        let icon = if light_taskbar() {
            on_light(&icon)
        } else {
            icon
        };
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
            let at = (position.x as i32, position.y as i32);
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

/// Windows' own setting for the taskbar: "Choose your Windows mode".
fn light_taskbar() -> bool {
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("SystemUsesLightTheme");
    let mut light: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: NUL-terminated names; `light` holds the DWORD asked for.
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut light).cast(),
            &raw mut size,
        )
    };
    read == 0 && light == 1
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// The image for a light taskbar: the white head drawn dark, the coloured
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
