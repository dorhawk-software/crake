//! The tray on Linux: a StatusNotifierItem (KDE shows it; GNOME with the
//! AppIndicator extension). GNOME's bar is dark in light and dark alike, so
//! the icons are drawn white.

use crate::Request;
use crate::tray::State;

struct Sni {
    state: State,
    requests: async_channel::Sender<Request>,
}

impl ksni::Tray for Sni {
    fn id(&self) -> String {
        "crake".into()
    }

    fn title(&self) -> String {
        "Crake".into()
    }

    /// Every size drawn: the bar takes the one that fits its slot.
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let icon = self.state.icon();
        let small = icon.small.into_iter().map(|(size, data)| ksni::Icon {
            width: size as i32,
            height: size as i32,
            data,
        });
        small
            .chain([ksni::Icon {
                width: icon.width as i32,
                height: icon.height as i32,
                data: icon.data,
            }])
            .collect()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Crake".into(),
            description: self.state.summary.clone(),
            ..Default::default()
        }
    }

    /// A click on the icon; GNOME's tray extension passes where it was.
    fn activate(&mut self, x: i32, y: i32) {
        let at = (x != 0 || y != 0).then_some((x, y));
        self.requests.send_blocking(Request::Panel(at)).ok();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{CheckmarkItem, StandardItem};
        let mut items: Vec<ksni::MenuItem<Self>> = vec![
            StandardItem {
                label: self.state.summary.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
        ];
        for t in &self.state.tunnels {
            let name = t.name.clone();
            let on = t.on;
            items.push(
                CheckmarkItem {
                    label: t.display.clone(),
                    checked: t.on,
                    activate: Box::new(move |this: &mut Self| {
                        this.requests
                            .send_blocking(Request::Switch(name.clone(), !on))
                            .ok();
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }
        items.push(ksni::MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Open Crake".into(),
                activate: Box::new(|this: &mut Self| {
                    this.requests.send_blocking(Request::Panel(None)).ok();
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Settings\u{2026}".into(),
                activate: Box::new(|this: &mut Self| {
                    this.requests.send_blocking(Request::Settings).ok();
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    this.requests.send_blocking(Request::Quit).ok();
                }),
                ..Default::default()
            }
            .into(),
        );
        items
    }
}

/// The running tray; cheap to clone.
#[derive(Clone)]
pub struct Handle(ksni::blocking::Handle<Sni>);

impl Handle {
    pub fn update(&self, f: impl FnOnce(&mut State)) {
        self.0.update(|sni| f(&mut sni.state));
    }

    /// Take the icon off the bar.
    pub fn shutdown(&self) {
        self.0.shutdown();
    }
}

/// On the session bus; none when nothing there shows trays.
pub fn start(requests: async_channel::Sender<Request>) -> Option<Handle> {
    use ksni::blocking::TrayMethods;
    Sni {
        state: State::default(),
        requests,
    }
    .spawn()
    .ok()
    .map(Handle)
}
