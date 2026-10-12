//! A .ovpn file made self-contained, as openvpn3's configuration service
//! takes it: the files it points to (`ca ca.crt`, `tls-auth ta.key 1`, ...)
//! put inline beside it, as `openvpn3 config-import` does. Paths are the
//! file's own: relative ones from its folder.

use std::path::{Path, PathBuf};

/// Directives whose argument is a file that can go inline (OpenVPN's
/// `<name>...</name>` blocks).
const INLINE: [&str; 10] = [
    "ca",
    "cert",
    "key",
    "extra-certs",
    "tls-auth",
    "tls-crypt",
    "tls-crypt-v2",
    "dh",
    "crl-verify",
    "secret",
];
/// The same, for a binary file: inline as base64.
const INLINE_BASE64: &str = "pkcs12";
/// `tls-auth ta.key 1` and `secret s.key 0`: the direction, which goes on a
/// line of its own once the key is inline.
const WITH_DIRECTION: [&str; 2] = ["tls-auth", "secret"];
/// What OpenVPN writes instead of a path for something already inline.
const ALREADY_INLINE: &str = "[inline]";

/// Why a .ovpn file can't be imported.
#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// It points to a file that isn't there: as written in it, and
    /// whether that was a path from the .ovpn's own folder.
    Missing(String, bool),
    /// A file it points to can't be read.
    Unreadable(String, String),
}

impl Problem {
    /// In the copy voice, after "Couldn't import x.ovpn: ".
    pub fn said(&self) -> String {
        match self {
            Self::Missing(name, true) => format!(
                "it points to {name}, which isn\u{2019}t next to it. Keep the files your IT team sent together and try again."
            ),
            Self::Missing(name, false) => {
                format!("it points to {name}, which isn\u{2019}t there.")
            }
            Self::Unreadable(name, why) => format!("{name} can\u{2019}t be read ({why})."),
        }
    }
}

/// `text` (a .ovpn file from `dir`) with every file it points to inline.
pub fn inline(text: &str, dir: &Path) -> Result<String, Problem> {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let words = words(line);
        let (Some(directive), Some(file)) = (words.first(), words.get(1)) else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let base64 = *directive == INLINE_BASE64;
        if (!INLINE.contains(&directive.as_str()) && !base64) || *file == ALREADY_INLINE {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let relative = Path::new(file).is_relative();
        let path: PathBuf = if relative {
            dir.join(file)
        } else {
            PathBuf::from(file)
        };
        let bytes = std::fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Problem::Missing(file.clone(), relative),
            _ => Problem::Unreadable(file.clone(), e.to_string()),
        })?;
        let body = if base64 {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        out.push_str(&format!(
            "<{directive}>\n{}\n</{directive}>\n",
            body.trim_end()
        ));
        if WITH_DIRECTION.contains(&directive.as_str())
            && let Some(direction) = words.get(2)
        {
            out.push_str(&format!("key-direction {direction}\n"));
        }
    }
    Ok(out)
}

/// A line's words, as OpenVPN splits them: by white space, a quoted word
/// kept whole; nothing from a comment on.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut chars = line.trim().chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            '#' | ';' => break,
            c if c.is_whitespace() => {
                chars.next();
            }
            '"' | '\'' => {
                chars.next();
                words.push(chars.by_ref().take_while(|&x| x != c).collect());
            }
            _ => {
                let mut word = String::new();
                while let Some(&x) = chars.peek() {
                    if x.is_whitespace() {
                        break;
                    }
                    word.push(x);
                    chars.next();
                }
                words.push(word);
            }
        }
    }
    words
}

/// The profile's name for a .ovpn file: its name without the extension.
pub fn profile_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "imported".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_the_files_it_points_to_inline() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ca.crt"),
            "-----BEGIN CERTIFICATE-----\nAAA\n-----END CERTIFICATE-----\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("ta key.key"), "static key\n").unwrap();
        std::fs::write(dir.path().join("id.p12"), [0u8, 1, 2]).unwrap();
        let text = "client\nremote vpn.initech.example 1194\nca ca.crt\ntls-auth \"ta key.key\" 1 # the HMAC\npkcs12 id.p12\n<cert>\nBBB\n</cert>\nkey [inline]\n";
        let out = inline(text, dir.path()).unwrap();
        assert!(out.contains("remote vpn.initech.example 1194\n"));
        assert!(
            out.contains(
                "<ca>\n-----BEGIN CERTIFICATE-----\nAAA\n-----END CERTIFICATE-----\n</ca>\n"
            ),
            "{out}"
        );
        assert!(
            out.contains("<tls-auth>\nstatic key\n</tls-auth>\nkey-direction 1\n"),
            "{out}"
        );
        assert!(out.contains("<pkcs12>\nAAEC\n</pkcs12>\n"), "{out}");
        assert!(out.contains("<cert>\nBBB\n</cert>\n"), "inline blocks stay");
        assert!(out.contains("key [inline]\n"));
        assert!(!out.contains("ca ca.crt"));
    }

    #[test]
    fn says_what_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            inline("ca ca.crt\n", dir.path()),
            Err(Problem::Missing("ca.crt".into(), true))
        );
        // An absolute path, on every system (a drive on Windows).
        let gone = dir.path().join("gone").join("ca.crt");
        let gone = gone.to_string_lossy().into_owned();
        assert_eq!(
            inline(&format!("ca \"{gone}\"\n"), dir.path()),
            Err(Problem::Missing(gone, false))
        );
        assert_eq!(
            Problem::Missing("ca.crt".into(), true).said(),
            "it points to ca.crt, which isn\u{2019}t next to it. Keep the files your IT team sent together and try again."
        );
        assert_eq!(
            profile_name(Path::new("/x/initech-office.ovpn")),
            "initech-office"
        );
    }
}
