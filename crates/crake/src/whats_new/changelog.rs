//! CHANGELOG.md, built into the app: each release's changes under its
//! kinds, as written for the people using Crake. Releases before the kinds
//! (0.9.0 and earlier) keep what they had: their own headings, or none.

use std::sync::OnceLock;

/// The changelog this Crake was built with.
pub const TEXT: &str = include_str!("../../../../CHANGELOG.md");

/// What a change is, in the order a release shows them.
pub const KINDS: [&str; 5] = ["New", "Better", "Changed", "Fixed", "Underneath"];

/// What Crake is built on, updated: listed, never told in the panel.
pub const UNDERNEATH: &str = "Underneath";

/// A development build's section, before it has a version.
pub const UNRELEASED: &str = "Unreleased";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// "0.15.0", or `UNRELEASED`.
    pub version: String,
    /// As the changelog writes it: "2026-10-22".
    pub date: Option<String>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// A kind, an early release's own heading ("What it does"), or empty.
    pub title: String,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The bold sentence a change starts with, when it has one.
    pub headline: Option<String>,
    pub body: String,
}

impl Release {
    /// The changes worth telling: all but what's underneath.
    pub fn told(&self) -> usize {
        self.groups
            .iter()
            .filter(|g| g.title != UNDERNEATH)
            .map(|g| g.changes.len())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|g| g.changes.is_empty())
    }
}

/// The releases, newest first, as built in.
pub fn releases() -> &'static [Release] {
    static ALL: OnceLock<Vec<Release>> = OnceLock::new();
    ALL.get_or_init(|| parse(TEXT))
}

/// A block being read: a bullet's lines, or a paragraph's.
struct Block {
    bullet: bool,
    lines: Vec<String>,
}

pub fn parse(text: &str) -> Vec<Release> {
    let mut releases: Vec<Release> = Vec::new();
    let mut block: Option<Block> = None;
    let mut in_comment = false;
    for line in text.lines() {
        if in_comment {
            in_comment = !line.contains("-->");
            continue;
        }
        if line.trim_start().starts_with("<!--") {
            in_comment = !line.contains("-->");
            continue;
        }
        if let Some(head) = line.strip_prefix("## ") {
            end(&mut block, &mut releases);
            releases.push(release(head.trim()));
            continue;
        }
        // Before the first release: the file's own title and words.
        if releases.is_empty() {
            continue;
        }
        if let Some(title) = line.strip_prefix("### ") {
            end(&mut block, &mut releases);
            if let Some(r) = releases.last_mut() {
                r.groups.push(Group {
                    title: title.trim().to_owned(),
                    changes: Vec::new(),
                });
            }
            continue;
        }
        if line.trim().is_empty() {
            end(&mut block, &mut releases);
        } else if let Some(rest) = line.strip_prefix("- ") {
            end(&mut block, &mut releases);
            block = Some(Block {
                bullet: true,
                lines: vec![rest.trim().to_owned()],
            });
        } else if let Some(b) = block.as_mut() {
            b.lines.push(line.trim().to_owned());
        } else {
            block = Some(Block {
                bullet: false,
                lines: vec![line.trim().to_owned()],
            });
        }
    }
    end(&mut block, &mut releases);
    releases
}

/// "0.14.0 (2026-10-21)", or "Unreleased".
fn release(head: &str) -> Release {
    let (version, date) = match head.split_once(" (") {
        Some((v, d)) => (v, Some(d.trim_end_matches(')').to_owned())),
        None => (head, None),
    };
    Release {
        version: version.to_owned(),
        date,
        groups: Vec::new(),
    }
}

/// The block read so far becomes a change of the last group (a group of
/// its own, untitled, if the release has none yet).
fn end(block: &mut Option<Block>, releases: &mut [Release]) {
    let (Some(b), Some(r)) = (block.take(), releases.last_mut()) else {
        return;
    };
    if r.groups.is_empty() {
        r.groups.push(Group {
            title: String::new(),
            changes: Vec::new(),
        });
    }
    let text = b.lines.join(" ");
    let change = if b.bullet {
        change(&text)
    } else {
        Change {
            headline: None,
            body: inline(&text),
        }
    };
    if let Some(g) = r.groups.last_mut() {
        g.changes.push(change);
    }
}

/// A bullet: its bold start is the headline when it's a phrase of its own
/// ("**IPsec on NixOS.** …", "**Networks that block UDP:** …"); a bold word
/// inside a sentence ("**Tab** goes from …") isn't.
fn change(text: &str) -> Change {
    if let Some(rest) = text.strip_prefix("**")
        && let Some(end) = rest.find("**")
    {
        let bold = &rest[..end];
        if bold.ends_with(['.', ':', ',', '?']) {
            return Change {
                headline: Some(inline(bold)),
                body: inline(rest[end + 2..].trim()),
            };
        }
    }
    Change {
        headline: None,
        body: inline(text),
    }
}

/// Markdown's marks taken out: bold, code, links (their words kept).
pub fn inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after
            .split_once("](")
            .and_then(|(words, more)| more.find(')').map(|close| (words, &more[close + 1..])))
        {
            Some((words, more)) => {
                out.push_str(words);
                rest = more;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versions;

    /// The first release written with kinds.
    const FIRST_WITH_KINDS: &str = "0.10.0";

    #[test]
    fn the_built_in_changelog_reads_whole() {
        let all = releases();
        let this = versions::this();
        assert!(
            all.iter()
                .any(|r| r.version == this || r.version == UNRELEASED),
            "a section for {this}, or Unreleased in a development build"
        );
        let mut seen_kinds = false;
        for r in all {
            if r.version == UNRELEASED || !versions::newer(FIRST_WITH_KINDS, &r.version) {
                if r.is_empty() {
                    continue;
                }
                seen_kinds = true;
                for g in &r.groups {
                    assert!(
                        KINDS.contains(&g.title.as_str()),
                        "{}: \"{}\" isn't a kind ({KINDS:?})",
                        r.version,
                        g.title
                    );
                    for c in &g.changes {
                        assert!(
                            c.headline.is_some(),
                            "{}: a change without its bold headline: {}",
                            r.version,
                            c.body
                        );
                    }
                }
            }
        }
        assert!(seen_kinds);
        // The first release, as written: its paragraph and its own headings.
        let first = all.last().unwrap();
        assert_eq!(first.version, "0.1.0");
        assert!(
            first.groups[0].changes[0]
                .body
                .starts_with("The first release.")
        );
        assert!(first.groups.iter().any(|g| g.title == "What it does"));
    }

    #[test]
    fn entries_keep_their_words() {
        let text = "# Changelog\n\n<!-- how it works\n## not a release -->\n\n## 0.2.0 (2026-10-13)\n\n### Fixed\n\n\
            - **A thing works.** It didn't, and the `curl`\n  line wrapped [here](https://x).\n\n\
            - **Tab** goes from one field to the next.\n\n## 0.1.0 (2026-10-12)\n\nThe first release.\n";
        let r = parse(text);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].version, "0.2.0");
        assert_eq!(r[0].date.as_deref(), Some("2026-10-13"));
        let fixed = &r[0].groups[0];
        assert_eq!(fixed.title, "Fixed");
        assert_eq!(fixed.changes[0].headline.as_deref(), Some("A thing works."));
        assert_eq!(
            fixed.changes[0].body,
            "It didn't, and the curl line wrapped here."
        );
        assert_eq!(fixed.changes[1].headline, None);
        assert_eq!(
            fixed.changes[1].body,
            "Tab goes from one field to the next."
        );
        assert_eq!(r[1].groups[0].title, "");
        assert_eq!(r[1].told(), 1);
    }
}
