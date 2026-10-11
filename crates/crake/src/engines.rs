//! Which VPN engines this computer has (CLAUDE.md: Packaging and
//! dependencies). Crake doesn't bundle them on Linux: openconnect speaks most
//! protocols, OpenVPN 3 runs its own profiles. A protocol whose engine is
//! missing says how to get it, instead of failing at connect.

use std::path::Path;

/// openvpn3's configuration service, which Crake talks to over D-Bus.
const OPENVPN3_SERVICE: &str =
    "/usr/share/dbus-1/system-services/net.openvpn.v3.configuration.service";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Engines {
    pub openconnect: bool,
    pub openvpn3: bool,
    /// openconnect can sign in through the browser (`--external-browser`,
    /// openconnect 9 and newer; Ubuntu 22.04 has 8.20).
    pub browser: bool,
    /// openconnect's version (major, minor), when it said.
    pub openconnect_version: Option<(u16, u16)>,
}

impl Engines {
    /// All there, as tests and demo mode assume.
    pub const ALL: Self = Self {
        openconnect: true,
        openvpn3: true,
        browser: true,
        openconnect_version: Some((9, 12)),
    };

    /// Look (blocking, quick: a few file checks and openconnect's --help).
    pub fn find() -> Self {
        let program = std::env::var_os("CRAKE_OPENCONNECT").unwrap_or_else(|| "openconnect".into());
        let openconnect = runnable(Path::new(&program));
        let said = |arg: &str| {
            std::process::Command::new(&program)
                .arg(arg)
                .env("LC_ALL", "C")
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout).into_owned()
                        + &String::from_utf8_lossy(&o.stderr)
                })
                .unwrap_or_default()
        };
        let (browser, openconnect_version) = if openconnect {
            (
                said("--help").contains("--external-browser"),
                version(&said("--version")),
            )
        } else {
            (false, None)
        };
        Self {
            openconnect,
            openvpn3: Path::new(OPENVPN3_SERVICE).exists(),
            browser,
            openconnect_version,
        }
    }

    /// Why signing in through the browser can't work here, if it can't.
    pub fn no_browser(&self) -> Option<String> {
        if !self.openconnect || self.browser {
            return None;
        }
        let has = self
            .openconnect_version
            .map(|(a, b)| format!("; this computer has {a}.{b}"))
            .unwrap_or_default();
        Some(format!(
            "Signing in through your browser needs openconnect 9 or newer{has}. \
             Use a username and password or a certificate."
        ))
    }

    /// What's missing for `protocol`, in a sentence with how to get it.
    pub fn missing(&self, protocol: &str) -> Option<&'static str> {
        match protocol {
            "" => None,
            "openvpn3" if !self.openvpn3 => Some(
                "Needs OpenVPN 3, which isn\u{2019}t installed. It comes from OpenVPN\u{2019}s own repository (openvpn.net, \u{201c}OpenVPN 3 Linux\u{201d}).",
            ),
            "openvpn3" => None,
            _ if !self.openconnect => Some(
                "Needs openconnect, which isn\u{2019}t installed: sudo apt install openconnect",
            ),
            _ => None,
        }
    }
}

/// openconnect's version from `openconnect --version` ("OpenConnect version
/// v8.20-1").
fn version(said: &str) -> Option<(u16, u16)> {
    let v = said
        .split_whitespace()
        .filter_map(|w| w.strip_prefix('v'))
        .find(|v| v.starts_with(|c: char| c.is_ascii_digit()))?;
    let mut parts = v.split(|c: char| !c.is_ascii_digit());
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// `program` can be run: a path that exists, or a name found on PATH.
fn runnable(program: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let executable = |p: &Path| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if program.components().count() > 1 {
        return executable(program);
    }
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| executable(&dir.join(program)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_a_protocol_is_missing() {
        let none = Engines {
            openconnect: false,
            openvpn3: false,
            browser: false,
            openconnect_version: None,
        };
        assert!(
            none.missing("gp")
                .unwrap()
                .contains("sudo apt install openconnect")
        );
        assert!(none.missing("openvpn3").unwrap().contains("OpenVPN 3"));
        let both = Engines::ALL;
        assert_eq!(both.missing("anyconnect"), None);
        assert_eq!(both.missing("openvpn3"), None);
        let oc = Engines {
            openvpn3: false,
            ..Engines::ALL
        };
        assert_eq!(oc.missing("auto"), None);
        assert!(oc.missing("openvpn3").is_some());
    }

    #[test]
    fn finds_programs_on_the_path() {
        assert!(runnable(Path::new("sh")));
        assert!(runnable(Path::new("/bin/sh")));
        assert!(!runnable(Path::new("surely-not-a-program-here")));
    }

    #[test]
    fn browser_sign_in_needs_openconnect_9() {
        assert_eq!(
            version("OpenConnect version v8.20-1\nUsing GnuTLS"),
            Some((8, 20))
        );
        assert_eq!(version("OpenConnect verze v9.12-3.3"), Some((9, 12)));
        assert_eq!(version("nothing"), None);
        let jammy = Engines {
            browser: false,
            openconnect_version: Some((8, 20)),
            ..Engines::ALL
        };
        assert_eq!(
            jammy.no_browser().unwrap(),
            "Signing in through your browser needs openconnect 9 or newer; \
             this computer has 8.20. Use a username and password or a certificate."
        );
        assert_eq!(Engines::ALL.no_browser(), None);
    }
}
