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
}

impl Engines {
    /// Look (blocking, quick: a few file checks).
    pub fn find() -> Self {
        let program =
            std::env::var_os("CRAKE_OPENCONNECT").unwrap_or_else(|| "openconnect".into());
        Self {
            openconnect: runnable(Path::new(&program)),
            openvpn3: Path::new(OPENVPN3_SERVICE).exists(),
        }
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
        };
        assert!(none.missing("gp").unwrap().contains("sudo apt install openconnect"));
        assert!(none.missing("openvpn3").unwrap().contains("OpenVPN 3"));
        let both = Engines {
            openconnect: true,
            openvpn3: true,
        };
        assert_eq!(both.missing("anyconnect"), None);
        assert_eq!(both.missing("openvpn3"), None);
        let oc = Engines {
            openconnect: true,
            openvpn3: false,
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
}
