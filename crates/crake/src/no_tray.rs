//! When nothing shows tray icons (design/proposals/no-tray-after.html):
//! stock GNOME on Fedora and Debian shows none without its AppIndicator
//! extension, so Crake would run with nothing on screen. Crake hears when a
//! tray appears or goes (the StatusNotifierWatcher, `sys::tray`), asks
//! GNOME about the extension (`sys::gnome_tray`, Linux) and says what would
//! bring the bird back: in Settings' row, in a note on the panel when
//! started by hand, and in one notification when started at login. With
//! "Show the bird in the tray" off, none of it is said.

/// GNOME's AppIndicator extension: upstream's, and Ubuntu's own (always on
/// in Ubuntu's session; it's never installed or marked by Crake).
pub const EXTENSION: &str = "appindicatorsupport@rgcjonas.gmail.com";
#[cfg(target_os = "linux")]
pub const UBUNTU_EXTENSION: &str = "ubuntu-appindicators@ubuntu.com";

/// What would bring the bird back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Remedy {
    /// GNOME has the extension, switched off: Crake can switch it on
    /// (org.gnome.Shell.Extensions' EnableExtension, given its id).
    SwitchOn(String),
    /// GNOME can't or wouldn't switch it on; its Extensions app says more.
    Refused(Why),
    /// Installed, but GNOME loads a new extension only at the next login.
    /// `marked`: Crake put it in GNOME's enabled-extensions, so it's on then.
    NextLogin { marked: bool },
    /// Not installed: this system's command for it, when Crake knows one.
    Install(Option<String>),
    /// No tray, and nothing Crake knows to switch on (another desktop, or
    /// KDE with its System Tray widget removed).
    NoTray,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The Extensions app's own switch is off ("disable-user-extensions").
    ExtensionsOff,
    /// The company locked GNOME's list of extensions (dconf).
    Locked,
    /// The extension is in an error state or out of date for this GNOME.
    Broken,
}

/// The panel's note, and the notification's title.
pub const TITLE: &str = "Crake has no icon in the tray.";
pub const NOTIFICATION_TITLE: &str = "Crake is running without an icon";
pub const SWITCH_ON: &str = "Switch on AppIndicator";
pub const OPEN_EXTENSIONS: &str = "Open Extensions";
/// Where Crake opens from without the bird (Linux's apps grid).
const UNTIL_THEN: &str = "Until then, open Crake from your apps.";

impl Remedy {
    /// Settings' line under "Show the bird in the tray".
    pub fn line(&self) -> &'static str {
        match self {
            Remedy::SwitchOn(_) => {
                "GNOME shows app icons in the tray only with its AppIndicator extension, which is off."
            }
            Remedy::Refused(Why::ExtensionsOff) => {
                "GNOME didn\u{2019}t switch it on: extensions are switched off on this computer."
            }
            Remedy::Refused(Why::Locked) => {
                "GNOME didn\u{2019}t switch it on: this computer\u{2019}s extensions are set by its administrator."
            }
            Remedy::Refused(Why::Broken) => {
                "GNOME didn\u{2019}t switch it on: its AppIndicator extension doesn\u{2019}t work with this GNOME."
            }
            Remedy::NextLogin { marked: true } => {
                "GNOME\u{2019}s AppIndicator extension is installed. Log out and back in, and the bird appears."
            }
            Remedy::NextLogin { marked: false } => {
                "GNOME\u{2019}s AppIndicator extension is installed. Log out and back in, then switch it on here."
            }
            Remedy::Install(Some(_)) => {
                "GNOME shows app icons in the tray only with its AppIndicator extension. Install it in a terminal:"
            }
            Remedy::Install(None) => {
                "GNOME shows app icons in the tray only with its AppIndicator extension, which isn\u{2019}t installed."
            }
            Remedy::NoTray => "No tray is showing icons here. Open Crake from your apps.",
        }
    }

    /// The panel's note under its title: the line, then how to open Crake
    /// meanwhile (D's line says so itself).
    pub fn note(&self) -> String {
        match self {
            Remedy::NoTray => self.line().to_owned(),
            Remedy::Install(Some(_)) => format!(
                "GNOME shows app icons there only with its AppIndicator extension. Install it in a terminal, then log out and back in. {UNTIL_THEN}"
            ),
            Remedy::SwitchOn(_) => format!(
                "GNOME shows app icons there only with its AppIndicator extension, which is off. {UNTIL_THEN}"
            ),
            _ => format!("{} {UNTIL_THEN}", self.line()),
        }
    }

    /// The notification's body.
    pub fn notification(&self) -> &'static str {
        match self {
            Remedy::SwitchOn(_) => {
                "Switch on GNOME\u{2019}s AppIndicator extension to see it in the tray."
            }
            Remedy::Install(_) => {
                "Install GNOME\u{2019}s AppIndicator extension to see it in the tray."
            }
            Remedy::NoTray => "No tray is showing icons here. Open Crake from your apps.",
            _ => self.line(),
        }
    }

    /// The command to copy, when installing is the way.
    pub fn command(&self) -> Option<&str> {
        match self {
            Remedy::Install(Some(c)) => Some(c),
            _ => None,
        }
    }

    /// Each case is told once at login (`Prefs::no_tray_said`); a new one
    /// (installed now, refused now) is told again.
    pub fn key(&self) -> String {
        match self {
            Remedy::SwitchOn(_) => "switch-on".into(),
            Remedy::Refused(why) => format!("refused-{why:?}").to_lowercase(),
            Remedy::NextLogin { marked } => format!("next-login-{marked}"),
            Remedy::Install(_) => "install".into(),
            Remedy::NoTray => "no-tray".into(),
        }
    }
}

/// CRAKE_DEMO=notray:<a|refused|b|c|d> (development): each case of
/// no-tray-after.html, without asking GNOME.
pub fn demo(name: &str) -> Option<Remedy> {
    Some(match name.strip_prefix("notray:")? {
        "a" => Remedy::SwitchOn(EXTENSION.into()),
        "refused" => Remedy::Refused(Why::ExtensionsOff),
        "b" => Remedy::NextLogin { marked: true },
        "c" => Remedy::Install(Some(
            "sudo dnf install gnome-shell-extension-appindicator".into(),
        )),
        _ => Remedy::NoTray,
    })
}

#[cfg(target_os = "linux")]
/// GNOME Shell's states of an extension (js/misc/extensionUtils.js).
pub mod state {
    pub const ACTIVE: u32 = 1;
    pub const ERROR: u32 = 3;
    pub const OUT_OF_DATE: u32 = 4;
}

#[cfg(target_os = "linux")]
/// What GNOME said about one of the extension's ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    pub state: u32,
    pub enabled: bool,
}

#[cfg(target_os = "linux")]
/// What Crake found out on GNOME (`sys::gnome_tray::look`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Gnome {
    /// The running GNOME Shell knows the extension: (its id, what it said).
    pub known: Option<(String, Known)>,
    /// Its files are on disk though the Shell doesn't know it yet.
    pub installed: bool,
    /// The Extensions app's own switch (UserExtensionsEnabled).
    pub user_extensions: bool,
    /// GNOME's enabled-extensions can be changed (not locked).
    pub writable: bool,
}

#[cfg(target_os = "linux")]
/// What would bring the bird back, from what GNOME said (None: not GNOME).
pub fn decide(gnome: Option<&Gnome>, os_release: &str) -> Remedy {
    let Some(g) = gnome else {
        return Remedy::NoTray;
    };
    match &g.known {
        Some((_, k)) if k.state == state::ERROR || k.state == state::OUT_OF_DATE => {
            Remedy::Refused(Why::Broken)
        }
        Some(_) if !g.user_extensions => Remedy::Refused(Why::ExtensionsOff),
        // On and running, and still no tray: nothing Crake can do.
        Some((_, k)) if k.enabled && k.state == state::ACTIVE => Remedy::NoTray,
        Some(_) if !g.writable => Remedy::Refused(Why::Locked),
        Some((id, _)) => Remedy::SwitchOn(id.clone()),
        None if g.installed => Remedy::NextLogin { marked: g.writable },
        None => Remedy::Install(install_command(os_release)),
    }
}

#[cfg(target_os = "linux")]
/// After "Switch on AppIndicator" didn't bring a tray: why, from what GNOME
/// says now.
pub fn refused(g: &Gnome) -> Remedy {
    Remedy::Refused(match &g.known {
        Some((_, k)) if k.state == state::ERROR || k.state == state::OUT_OF_DATE => Why::Broken,
        _ if !g.user_extensions => Why::ExtensionsOff,
        _ if !g.writable => Why::Locked,
        _ => Why::ExtensionsOff,
    })
}

#[cfg(target_os = "linux")]
/// The command that installs the extension here, from /etc/os-release.
pub fn install_command(os_release: &str) -> Option<String> {
    let field = |key: &str| {
        os_release
            .lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim_matches('"').to_owned())
            .unwrap_or_default()
    };
    let (id, like, variant) = (field("ID"), field("ID_LIKE"), field("VARIANT_ID"));
    let is = |name: &str| id == name || like.split_whitespace().any(|l| l == name);
    let package = "gnome-shell-extension-appindicator";
    let command = if is("fedora") && ["silverblue", "kinoite"].contains(&variant.as_str()) {
        format!("rpm-ostree install {package}")
    } else if is("fedora") || is("rhel") {
        format!("sudo dnf install {package}")
    } else if is("debian") || is("ubuntu") {
        format!("sudo apt install {package}")
    } else if is("arch") {
        format!("sudo pacman -S {package}")
    } else if is("suse") || is("opensuse") || id.starts_with("opensuse") {
        format!("sudo zypper install {package}")
    } else {
        return None;
    };
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GNOME's state of an extension that's off.
    #[cfg(target_os = "linux")]
    const INACTIVE: u32 = 2;

    #[cfg(target_os = "linux")]
    fn gnome(known: Option<(u32, bool)>, installed: bool) -> Gnome {
        Gnome {
            known: known.map(|(state, enabled)| (EXTENSION.to_owned(), Known { state, enabled })),
            installed,
            user_extensions: true,
            writable: true,
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn each_case_has_its_remedy() {
        let fedora = "NAME=\"Fedora Linux\"\nID=fedora\nVERSION_ID=44\n";
        assert_eq!(decide(None, fedora), Remedy::NoTray);
        assert_eq!(
            decide(Some(&gnome(Some((INACTIVE, false)), true)), fedora),
            Remedy::SwitchOn(EXTENSION.into())
        );
        assert_eq!(
            decide(Some(&gnome(None, true)), fedora),
            Remedy::NextLogin { marked: true }
        );
        assert_eq!(
            decide(Some(&gnome(None, false)), fedora),
            Remedy::Install(Some(
                "sudo dnf install gnome-shell-extension-appindicator".into()
            ))
        );
        assert_eq!(
            decide(Some(&gnome(Some((state::OUT_OF_DATE, true)), true)), fedora),
            Remedy::Refused(Why::Broken)
        );
        let off = Gnome {
            user_extensions: false,
            ..gnome(Some((INACTIVE, false)), true)
        };
        assert_eq!(
            decide(Some(&off), fedora),
            Remedy::Refused(Why::ExtensionsOff)
        );
        let locked = Gnome {
            writable: false,
            ..gnome(Some((INACTIVE, false)), true)
        };
        assert_eq!(decide(Some(&locked), fedora), Remedy::Refused(Why::Locked));
        let locked_new = Gnome {
            writable: false,
            ..gnome(None, true)
        };
        assert_eq!(
            decide(Some(&locked_new), fedora),
            Remedy::NextLogin { marked: false }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_command_is_this_systems() {
        let cmd = |r: &str| install_command(r);
        assert_eq!(
            cmd("ID=debian\n").as_deref(),
            Some("sudo apt install gnome-shell-extension-appindicator")
        );
        assert_eq!(
            cmd("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n").as_deref(),
            Some("sudo apt install gnome-shell-extension-appindicator")
        );
        assert_eq!(
            cmd("ID=fedora\nVARIANT_ID=silverblue\n").as_deref(),
            Some("rpm-ostree install gnome-shell-extension-appindicator")
        );
        assert_eq!(
            cmd("ID=arch\n").as_deref(),
            Some("sudo pacman -S gnome-shell-extension-appindicator")
        );
        assert_eq!(
            cmd("ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n").as_deref(),
            Some("sudo zypper install gnome-shell-extension-appindicator")
        );
        assert_eq!(cmd("ID=nixos\n"), None);
    }

    #[test]
    fn the_words_are_sentences_and_keys_differ() {
        let all = [
            Remedy::SwitchOn(EXTENSION.into()),
            Remedy::Refused(Why::ExtensionsOff),
            Remedy::Refused(Why::Locked),
            Remedy::Refused(Why::Broken),
            Remedy::NextLogin { marked: true },
            Remedy::NextLogin { marked: false },
            Remedy::Install(Some("x".into())),
            Remedy::Install(None),
            Remedy::NoTray,
        ];
        for r in &all {
            assert!(r.line().ends_with('.') || r.line().ends_with(':'), "{r:?}");
            assert!(r.note().ends_with('.'), "{r:?}");
            assert!(r.notification().ends_with('.'), "{r:?}");
        }
        let keys: std::collections::HashSet<_> = all.iter().map(Remedy::key).collect();
        // The two installs share a key: the command is a detail.
        assert_eq!(keys.len(), all.len() - 1);
    }
}
