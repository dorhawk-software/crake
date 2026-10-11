//! Which VPN engines this computer has (CLAUDE.md: Packaging and
//! dependencies). Crake doesn't bundle them on Linux: openconnect speaks most
//! protocols, OpenVPN 3 runs its own profiles. A protocol whose engine is
//! missing says how to get it, instead of failing at connect.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

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
        let program = openconnect();
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

/// openconnect's file name.
const PROGRAM: &str = if cfg!(windows) {
    "openconnect.exe"
} else {
    "openconnect"
};

/// The openconnect to run: CRAKE_OPENCONNECT (development), openconnect on
/// PATH, or one where Crake knows to look (`extra_dirs`).
pub fn openconnect() -> OsString {
    let path = std::env::var_os("PATH");
    find_openconnect(
        std::env::var_os("CRAKE_OPENCONNECT"),
        path.as_deref(),
        &extra_dirs(),
    )
}

/// Where openconnect may be besides PATH. Linux: the administrators'
/// folders (Debian installs it in /usr/sbin, which its users' PATH leaves
/// out). Windows: the copy Crake ships in its own folder.
fn extra_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        std::env::current_exe()
            .ok()
            .and_then(|exe| Some(vec![exe.parent()?.join("openconnect")]))
            .unwrap_or_default()
    } else {
        vec!["/usr/sbin".into(), "/sbin".into()]
    }
}

fn find_openconnect(given: Option<OsString>, path: Option<&OsStr>, extra: &[PathBuf]) -> OsString {
    if let Some(program) = given {
        return program;
    }
    let on_path = path.is_some_and(|p| {
        std::env::split_paths(p).any(|dir| crate::sys::executable(&dir.join("openconnect")))
    });
    if !on_path
        && let Some(found) = extra
            .iter()
            .map(|dir| dir.join(PROGRAM))
            .find(|p| crate::sys::executable(p))
    {
        return found.into_os_string();
    }
    "openconnect".into()
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
    let executable = crate::sys::executable;
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
        // The system's shell, by name and by path.
        #[cfg(unix)]
        let (name, path) = ("sh", "/bin/sh");
        #[cfg(windows)]
        let (name, path) = ("cmd", r"C:\Windows\System32\cmd.exe");
        assert!(runnable(Path::new(name)));
        assert!(runnable(Path::new(path)));
        assert!(!runnable(Path::new("surely-not-a-program-here")));
    }

    #[cfg(unix)]
    #[test]
    fn finds_openconnect_where_debian_puts_it() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let (bin, sbin) = (root.path().join("bin"), root.path().join("sbin"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&sbin).unwrap();
        let program = sbin.join("openconnect");
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let admin = [sbin.clone()];
        // A user's PATH without sbin (Debian): found there, by its path.
        assert_eq!(
            find_openconnect(None, Some(bin.as_os_str()), &admin),
            program.as_os_str()
        );
        // On PATH (Ubuntu's users have sbin): by name, as PATH finds it.
        let path = std::env::join_paths([&bin, &sbin]).unwrap();
        assert_eq!(find_openconnect(None, Some(&path), &admin), "openconnect");
        // Nowhere: by name, so the error names it.
        assert_eq!(
            find_openconnect(None, Some(bin.as_os_str()), &[]),
            "openconnect"
        );
        // CRAKE_OPENCONNECT wins.
        assert_eq!(
            find_openconnect(Some("/opt/oc".into()), Some(&path), &admin),
            "/opt/oc"
        );
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
