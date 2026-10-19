//! The report (Settings → Diagnostics → "Save a report…", `crake report`;
//! design/proposals/report.html): what craked sends of the computer (D-Bus
//! `Report`), and what never goes in one, whoever asks.

use serde::{Deserialize, Serialize};

/// craked's own log in a report: the last hour, at most this many lines.
pub const OWN_LOG_LINES: usize = 200;
/// A part of the computer's view longer than this is cut, saying so.
pub const PART_LINES: usize = 400;

/// The computer's side of a report, as craked sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Computer {
    /// Tried while the report was made: "The internet: …", "ACME's
    /// names: …".
    pub checks: Vec<String>,
    /// The system's own view: its DNS, its routes, Crake's rules.
    pub parts: Vec<Part>,
    /// craked's own log, the last hour (at most OWN_LOG_LINES lines).
    pub own_log: Vec<String>,
    /// The computer's own plumbing (its addresses and networks, each
    /// tunnel's address and DNS servers, the router, craked's own DNS):
    /// hiding host names and addresses leaves these, or the routes and DNS
    /// parts would say nothing.
    pub keep: Vec<String>,
}

/// One part: a title saying where it comes from, and its lines.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    pub title: String,
    pub lines: Vec<String>,
}

impl Part {
    /// `lines` as a report may hold them: sign-in material out
    /// (`private`), the end trimmed, cut at PART_LINES.
    pub fn new(title: impl Into<String>, lines: impl IntoIterator<Item = String>) -> Self {
        let mut kept: Vec<String> = lines
            .into_iter()
            .filter_map(|l| private(l.trim_end()))
            .collect();
        while kept.last().is_some_and(|l| l.trim().is_empty()) {
            kept.pop();
        }
        while kept.first().is_some_and(|l| l.trim().is_empty()) {
            kept.remove(0);
        }
        if kept.len() > PART_LINES {
            let more = kept.len() - PART_LINES;
            kept.truncate(PART_LINES);
            kept.push(format!("… and {more} more lines"));
        }
        Self {
            title: title.into(),
            lines: kept,
        }
    }
}

/// Words that mark a line as carrying sign-in material: openconnect's
/// and OpenVPN's, F5's (MRHSession) and Pulse's (DSID) session cookies,
/// OpenVPN's challenge (CRV1, which carries the username).
const SIGN_IN: [&str; 12] = [
    "cookie",
    "token",
    "password",
    "passwd",
    "secret",
    "webvpn=",
    "authorization",
    "web_auth",
    "open_url",
    "mrhsession",
    "dsid=",
    "crv1",
];

/// The groups in a hardware (MAC) address.
const MAC_GROUPS: usize = 6;
/// What a hardware address becomes.
const A_DEVICE: &str = "<a device>";

/// A line as it may go in a report: None when it carries sign-in material
/// (a session cookie, an OpenVPN token, a password, a sign-in page);
/// otherwise with web addresses as "<a web address>", file paths as
/// "<a file>" wherever they start ("path=/home/…", "C:\Users\Jane Doe\…":
/// a profile's keys and certificates are named by their paths, and a home
/// folder by its user), e-mail addresses as "<an e-mail address>", and
/// hardware addresses as "<a device>" (a Mac's routes name the router's
/// and the other devices' on the network, Windows' each adapter's).
pub fn private(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if SIGN_IN.iter().any(|w| lower.contains(w)) {
        return None;
    }
    let words: Vec<String> = line
        .split(' ')
        .map(|word| {
            if word.contains("://") {
                "<a web address>".to_owned()
            } else if is_email(word) {
                "<an e-mail address>".to_owned()
            } else if let Some(at) = path_at(word) {
                // "path=" stays; a quote or bracket alone goes with it.
                let before = &word[..at];
                let before = if before.chars().all(|c| "\"'([{<".contains(c)) {
                    ""
                } else {
                    before
                };
                format!("{before}<a file>")
            } else if word.matches('\\').count() >= 2 {
                // The rest of a Windows path with a space in it.
                "<a file>".to_owned()
            } else if is_mac(word) {
                A_DEVICE.to_owned()
            } else {
                word.to_owned()
            }
        })
        .collect();
    Some(spaced_macs(words).join(" "))
}

/// "aa:bb:cc:dd:ee:ff", "0:1c:42:0:0:18" (macOS drops leading zeros),
/// "aa-bb-cc-dd-ee-ff": six groups of one or two hex digits. Not an IPv6
/// address, which has eight groups or a "::".
fn is_mac(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_ascii_hexdigit());
    [':', '-'].into_iter().any(|sep| {
        let groups: Vec<&str> = w.split(sep).collect();
        groups.len() == MAC_GROUPS
            && groups
                .iter()
                .all(|g| (1..=2).contains(&g.len()) && g.chars().all(|c| c.is_ascii_hexdigit()))
    })
}

/// Windows' `route print` writes an adapter's address as six words:
/// "12...00 15 5d 01 02 03 ......Intel(R) Ethernet".
fn spaced_macs(mut words: Vec<String>) -> Vec<String> {
    let pair = |w: &str| w.len() == 2 && w.chars().all(|c| c.is_ascii_hexdigit());
    let mut i = 0;
    while i + MAC_GROUPS <= words.len() {
        let (lead, first) = match words[i].rsplit_once("...") {
            Some((lead, first)) => (format!("{lead}..."), first.to_owned()),
            None => (String::new(), words[i].clone()),
        };
        if pair(&first) && words[i + 1..i + MAC_GROUPS].iter().all(|w| pair(w)) {
            words.splice(i..i + MAC_GROUPS, [format!("{lead}{A_DEVICE}")]);
        }
        i += 1;
    }
    words
}

/// Where a path starts in a word: at its start, or after "=", a quote, a
/// bracket, a colon or a tab ("path=/home/…", "Some(\"/home/…").
fn path_at(word: &str) -> Option<usize> {
    word.char_indices()
        .filter(|&(i, _)| {
            i == 0 || word[..i].ends_with(['=', '"', '\'', '(', '[', '{', ':', ',', '\t', '<'])
        })
        .map(|(i, _)| i)
        .find(|&i| is_path(&word[i..]))
}

/// "/var/lib/crake/profiles/initech.ovpn", "~/certs/me.p12",
/// "C:\ProgramData\Crake\x", "\\?\C:\Users\…": a path, not "/32" or
/// "10.40.0.0/16".
fn is_path(w: &str) -> bool {
    let unix = (w.starts_with('/') || w.starts_with("~/")) && w.matches('/').count() >= 2;
    let windows = matches!(
        w.as_bytes(),
        [drive, b':', b'\\' | b'/', _, ..] if drive.is_ascii_alphabetic()
    ) || w.starts_with("\\\\?\\");
    unix || windows
}

/// "alice@acme.example", "user=alice@acme.example": someone's address.
fn is_email(word: &str) -> bool {
    word.split_once('@').is_some_and(|(who, domain)| {
        !who.is_empty() && domain.contains('.') && !domain.starts_with('.')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_ins_never_go_in_a_report() {
        for line in [
            "openconnect: Set-Cookie: webvpn=4F2A...; secure",
            ">PASSWORD:Auth-Token:abcdef",
            "asking for the password of acme",
            "INFOMSG:WEB_AUTH::https://vpn.initech.example/oauth2/start?state=x",
            "OPEN_URL:https://sso.example/",
            "SVPNCOOKIE=xyz",
            "client_secret was refused",
        ] {
            assert_eq!(private(line), None, "{line}");
        }
    }

    #[test]
    fn addresses_of_pages_and_files_are_left_out() {
        assert_eq!(
            private("Opened https://wiki.acme.internal/x?y=1 for ACME").as_deref(),
            Some("Opened <a web address> for ACME")
        );
        assert_eq!(
            private("reading \"/var/lib/crake/profiles/initech.ovpn\": refused").as_deref(),
            Some("reading <a file> refused")
        );
        assert_eq!(
            private(r"C:\ProgramData\Crake\craked.log grew").as_deref(),
            Some("<a file> grew")
        );
        assert_eq!(
            private("~/certs/me.p12 imported").as_deref(),
            Some("<a file> imported")
        );
        // Networks and prefixes aren't paths.
        assert_eq!(
            private("10.40.0.0/16 via crake-acme /32").as_deref(),
            Some("10.40.0.0/16 via crake-acme /32")
        );
    }

    #[test]
    fn hardware_addresses_never_go_in_a_report() {
        // macOS' netstat -rn: the router's, and another device's.
        assert_eq!(
            private("192.168.1.1        0:1c:42:0:0:18     UHLWIir   en0   1172").as_deref(),
            Some("192.168.1.1        <a device>     UHLWIir   en0   1172")
        );
        assert_eq!(
            private("fe80::1%en0  a4:83:e7:2b:9c:1d  UHLWIi  en0").as_deref(),
            Some("fe80::1%en0  <a device>  UHLWIi  en0")
        );
        // Windows' route print, its interface list.
        assert_eq!(
            private(" 12...00 15 5d 01 02 03 ......Intel(R) Ethernet").as_deref(),
            Some(" 12...<a device> ......Intel(R) Ethernet")
        );
        assert_eq!(
            private("Physical address AA-BB-CC-DD-EE-FF.").as_deref(),
            Some("Physical address <a device>")
        );
        // IPv6 addresses and times stay.
        for kept in [
            "fd40:1::10 dev crake-acme",
            "2001:db8:1:2:3:4:5:6 via fe80::1",
            "17:52:03 Sent git.acme.internal through ACME.",
            "  0.0.0.0          0.0.0.0     203.0.113.1    203.0.113.20     35",
        ] {
            assert_eq!(private(kept).as_deref(), Some(kept));
        }
    }

    #[test]
    fn paths_and_people_go_wherever_they_stand() {
        for (line, want) in [
            (
                "loading path=/home/jane/certs/me.p12 now",
                "loading path=<a file> now",
            ),
            ("Some(\"/Users/jane/x.ovpn\") read", "Some(\"<a file> read"),
            ("found\t/Users/jane/Library/x", "found\t<a file>"),
            (
                r"C:\Users\Jane Doe\AppData\Local\Crake\log",
                "<a file> <a file>",
            ),
            (r"\\?\C:\Users\jane\x", "<a file>"),
            (
                "signed in as alice@acme.example.",
                "signed in as <an e-mail address>",
            ),
        ] {
            assert_eq!(private(line).as_deref(), Some(want), "{line}");
        }
        for gone in [
            "AUTH_FAILED,CRV1:R,E:Om01u7Fh4LrGBS7uh0SWmzwabUiGiW6l:YWxpY2U=:Enter code",
            "Set-Cookie: MRHSession=abc123",
            "DSID=5f2a; path=/",
        ] {
            assert_eq!(private(gone), None, "{gone}");
        }
        // Not paths: networks, a ratio, a route's flags.
        for kept in [
            "10.40.0.0/16 via crake-acme",
            "DNSSEC=no/unsupported",
            "UGSc utun4",
        ] {
            assert_eq!(private(kept).as_deref(), Some(kept), "{kept}");
        }
    }

    #[test]
    fn a_part_is_trimmed_and_cut() {
        let p = Part::new(
            "Routes",
            ["", "default via 192.0.2.1  ", "Cookie: x", "", ""].map(String::from),
        );
        assert_eq!(p.lines, vec!["default via 192.0.2.1"]);
        let long = Part::new("Long", (0..PART_LINES + 5).map(|i| i.to_string()));
        assert_eq!(long.lines.len(), PART_LINES + 1);
        assert_eq!(long.lines.last().unwrap(), "… and 5 more lines");
    }
}
