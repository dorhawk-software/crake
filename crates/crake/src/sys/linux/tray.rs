//! The tray on Linux: a StatusNotifierItem (KDE shows it; GNOME with the
//! AppIndicator extension). GNOME's bar is dark in light and dark alike, so
//! the icons are drawn white.

use crate::Request;
use crate::tray::State;

struct Sni {
    state: State,
    requests: async_channel::Sender<Request>,
    /// Toggled by `settle`: the largest size listed twice, so the same
    /// picture still counts as a new icon.
    nudge: bool,
}

impl Sni {
    fn tell(&self, there: bool) {
        self.requests
            .send_blocking(Request::NoTray(crate::tray_host::Event::Host(there)))
            .ok();
    }
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
        let largest = ksni::Icon {
            width: icon.width as i32,
            height: icon.height as i32,
            data: icon.data,
        };
        let again = self.nudge.then(|| largest.clone());
        small.chain([largest]).chain(again).collect()
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

    /// A tray appeared (GNOME's AppIndicator extension switched on, a slow
    /// login's shell up).
    fn watcher_online(&self) {
        self.tell(true);
    }

    /// Nothing shows tray icons (now, or from the start): Crake keeps the
    /// icon ready for when something does, and says what would (`no_tray`).
    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        self.tell(false);
        true
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
                label: "New tunnel\u{2026}".into(),
                activate: Box::new(|this: &mut Self| {
                    this.requests.send_blocking(Request::NewTunnel(None)).ok();
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

    /// Tell the bar about the icon once more, the picture unchanged. GNOME's
    /// AppIndicator extension drops a NewIcon that arrives while it is still
    /// fetching the previous one (its `_signalsAccumulator` is cleared only
    /// after the fetch), so an animation's last change could be lost and its
    /// last frame stay on the bar.
    pub fn settle(&self) {
        self.0.update(|sni| sni.nudge = !sni.nudge);
    }

    /// Take the icon off the bar.
    pub fn shutdown(&self) {
        self.0.shutdown();
    }
}

/// On the session bus. With nothing there that shows trays it waits for
/// one, and says so (`watcher_offline`); none only without a session bus.
pub fn start(requests: async_channel::Sender<Request>) -> Option<Handle> {
    use ksni::blocking::TrayMethods;
    Sni {
        state: State::default(),
        requests,
        nudge: false,
    }
    .assume_sni_available(true)
    .spawn()
    .ok()
    .map(Handle)
}

#[cfg(test)]
mod tests {
    use ksni::Tray;

    #[test]
    fn settling_tells_the_bar_again_with_the_same_picture() {
        let (requests, _) = async_channel::unbounded();
        let mut sni = super::Sni {
            state: crate::tray::State::default(),
            requests,
            nudge: false,
        };
        let before = sni.icon_pixmap();
        sni.nudge = !sni.nudge;
        let after = sni.icon_pixmap();
        // A change ksni sends to the bar (it compares the lists)...
        assert_ne!(before.len(), after.len());
        // ...of the same drawings.
        assert!(after.iter().all(|a| {
            before
                .iter()
                .any(|b| (b.width, &b.data) == (a.width, &a.data))
        }));
    }
}
