//! The report (the log window's "Save a report…", `crake report`;
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

/// Words that mark a line as carrying sign-in material.
const SIGN_IN: [&str; 9] = [
    "cookie",
    "token",
    "password",
    "passwd",
    "secret",
    "webvpn=",
    "authorization",
    "web_auth",
    "open_url",
];

/// A line as it may go in a report: None when it carries sign-in material
/// (a session cookie, an OpenVPN token, a password, a sign-in page);
/// otherwise with web addresses as "<a web address>" and file paths as
/// "<a file>" (a profile's keys and certificates are named by their
/// paths).
pub fn private(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if SIGN_IN.iter().any(|w| lower.contains(w)) {
        return None;
    }
    Some(
        line.split(' ')
            .map(|word| {
                if word.contains("://") {
                    "<a web address>".to_owned()
                } else if is_path(word) {
                    "<a file>".to_owned()
                } else {
                    word.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// "/var/lib/crake/profiles/initech.ovpn", "~/certs/me.p12",
/// "C:\ProgramData\Crake\x": a path, not "/32" or "10.40.0.0/16".
fn is_path(word: &str) -> bool {
    let w = word.trim_start_matches(['"', '\'', '(', '[', '=']);
    let unix = (w.starts_with('/') || w.starts_with("~/")) && w.matches('/').count() >= 2;
    let windows = matches!(
        w.as_bytes(),
        [drive, b':', b'\\' | b'/', _, ..] if drive.is_ascii_alphabetic()
    );
    unix || windows
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
