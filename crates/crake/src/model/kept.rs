//! The "?" after the panel's "What Crake did": what a line there means, and
//! how long Crake keeps a host, as the settings say
//! (design/proposals/what-crake-did-help.html).

use super::*;

/// "another 7 days", "another day".
fn another(days: &str) -> String {
    match days {
        "1" => "another day".into(),
        d => format!("another {d} days"),
    }
}

impl Model {
    /// The card's paragraphs; the last, when there is one, is the example
    /// (shown muted).
    pub fn what_crake_did_help(&self) -> Vec<String> {
        let global = self.setting("forget_after_days");
        let keeps = match global {
            "never" => {
                "Crake keeps it until you forget it in the tunnel\u{2019}s details".to_owned()
            }
            d => format!("every time you use it Crake keeps it {}", another(d)),
        };
        let mut first = format!(
            "A host shows up here once: the first time Crake sends it through a tunnel. \
             After that it just goes through, and {keeps}, as set in Settings \u{2192} Learning."
        );
        // Tunnels added in the app that keep theirs differently, by value.
        let mut own: Vec<(&str, Vec<&str>)> = Vec::new();
        for t in &self.tunnels {
            let Some(v) = t
                .own
                .get("forget_after_days")
                .map(String::as_str)
                .filter(|v| !v.is_empty() && *v != global)
            else {
                continue;
            };
            match own.iter_mut().find(|(value, _)| *value == v) {
                Some((_, names)) => names.push(&t.display),
                None => own.push((v, vec![&t.display])),
            }
        }
        for (v, names) in own {
            let how = match v {
                "never" => "until you forget them".to_owned(),
                "1" => "a day".to_owned(),
                d => format!("{d} days"),
            };
            let who = join_and(&names);
            first += &if names.len() == 1 {
                format!(" {who} keeps its hosts {how}.")
            } else {
                format!(" {who} keep theirs {how}.")
            };
        }
        let mut card = vec![
            first,
            "To see when a host was last used, open its tunnel in Settings \u{2192} Tunnels."
                .to_owned(),
        ];
        // The newest host sent through, as the example.
        let newest = self.decisions(usize::MAX).into_iter().find_map(|d| {
            let rest = d.entry.message.strip_prefix("Sent ")?;
            let (host, tunnel) = rest.strip_suffix('.')?.split_once(" through ")?;
            Some(format!(
                "Example: {host} showed up at {}. Opening it again tomorrow goes through \
                 {tunnel} without a new line here.",
                d.time
            ))
        });
        card.extend(newest);
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(global: &str, own: &[(&str, &str)]) -> Model {
        let mut m = crate::model::demo();
        m.settings
            .insert("forget_after_days".into(), global.to_owned());
        for t in &mut m.tunnels {
            t.own.remove("forget_after_days");
        }
        for (display, v) in own {
            let t = m
                .tunnels
                .iter_mut()
                .find(|t| t.display == *display)
                .unwrap();
            t.own.insert("forget_after_days".into(), (*v).to_owned());
        }
        m
    }

    #[test]
    fn says_how_long_the_settings_keep_a_host() {
        let card = with("7", &[]).what_crake_did_help();
        assert!(
            card[0].ends_with("every time you use it Crake keeps it another 7 days, as set in Settings \u{2192} Learning."),
            "{}",
            card[0]
        );
        assert!(with("1", &[]).what_crake_did_help()[0].contains("keeps it another day,"));
        assert!(
            with("never", &[]).what_crake_did_help()[0]
                .contains("Crake keeps it until you forget it in the tunnel\u{2019}s details")
        );
    }

    #[test]
    fn names_the_tunnels_that_keep_theirs_differently() {
        let names: Vec<String> = crate::model::demo()
            .tunnels
            .iter()
            .map(|t| t.display.clone())
            .collect();
        let (a, b) = (names[0].as_str(), names[1].as_str());
        let one = with("7", &[(a, "30"), (b, "7")]).what_crake_did_help();
        assert!(
            one[0].ends_with(&format!(" {a} keeps its hosts 30 days.")),
            "{}",
            one[0]
        );
        let two = with("7", &[(a, "never"), (b, "never")]).what_crake_did_help();
        assert!(
            two[0].ends_with(&format!(" {a} and {b} keep theirs until you forget them.")),
            "{}",
            two[0]
        );
    }

    #[test]
    fn the_example_is_the_newest_host_sent_through() {
        let m = crate::model::demo();
        let card = m.what_crake_did_help();
        let example = card.last().unwrap();
        let d = m
            .decisions(usize::MAX)
            .into_iter()
            .find(|d| d.entry.message.starts_with("Sent "))
            .unwrap();
        assert!(example.starts_with("Example: "), "{example}");
        assert!(
            example.contains(&format!("showed up at {}.", d.time)),
            "{example}"
        );
        let mut empty = m.clone();
        empty.log.clear();
        assert_eq!(
            empty.what_crake_did_help().len(),
            2,
            "no example without one"
        );
    }
}
