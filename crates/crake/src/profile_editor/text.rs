//! A profile's text as the editor shows it (design/proposals/edit-ovpn-profile.html):
//! each inline block (`<ca>…</ca>`, `<key>…</key>`, …) folded into one line
//! saying what it is, kept exactly as it was, and put back on save.

use std::ops::Range;

/// One folded block: the line the editor shows for it, and the block as
/// it was (its tags included, without the line break after it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub shown: String,
    pub whole: String,
}

/// The text with its blocks folded, and the blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folded {
    pub shown: String,
    pub blocks: Vec<Block>,
}

/// Fold every `<tag>` … `</tag>` block of `text` into one line.
pub fn fold(text: &str) -> Folded {
    let mut shown = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let Some(tag) = opening(line) else {
            shown.push(line.to_owned());
            continue;
        };
        let close = format!("</{tag}>");
        let mut whole = vec![line.to_owned()];
        let mut closed = line.trim_end().ends_with(&close);
        while !closed {
            let Some(next) = lines.next() else { break };
            whole.push(next.to_owned());
            closed = next.trim() == close;
        }
        // A block that never closes isn't one: shown as it is.
        if !closed {
            shown.extend(whole);
            continue;
        }
        let inside = whole.len().saturating_sub(2);
        let mut line = format!(
            "<{tag}> {}, {inside} {}, kept as {} </{tag}>",
            what(&tag),
            if inside == 1 { "line" } else { "lines" },
            if inside == 1 { "it is" } else { "they are" },
        );
        // Two alike (two keys of one kind) stay apart.
        let mut n = 2;
        while blocks.iter().any(|b| b.shown == line) {
            line = format!(
                "<{tag}> {}, {inside} lines, kept as they are ({n}) </{tag}>",
                what(&tag)
            );
            n += 1;
        }
        shown.push(line.clone());
        blocks.push(Block {
            shown: line,
            whole: whole.join("\n"),
        });
    }
    let mut shown = shown.join("\n");
    if text.ends_with('\n') {
        shown.push('\n');
    }
    Folded { shown, blocks }
}

/// The text to give openvpn3: `edited` with each folded line back as the
/// block it stands for. A folded line deleted drops its block; one that was
/// changed can't be put back, and says so.
pub fn unfold(edited: &str, blocks: &[Block]) -> Result<String, String> {
    let mut out = Vec::new();
    for line in edited.lines() {
        if let Some(b) = blocks.iter().find(|b| b.shown == line.trim()) {
            out.push(b.whole.clone());
        } else if let Some(tag) = opening(line)
            && line.contains(", kept as ")
        {
            return Err(format!(
                "The folded <{tag}> line was changed, so it can\u{2019}t be put back. Undo the change, or delete the whole line to leave the block out."
            ));
        } else {
            out.push(line.to_owned());
        }
    }
    let mut text = out.join("\n");
    if edited.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// The lines of `now` that aren't in `before` (by a longest common
/// subsequence of lines), as byte ranges of `now`, each a whole line: what
/// the editor tints.
pub fn changed_lines(before: &str, now: &str) -> Vec<Range<usize>> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = now.lines().collect();
    // lcs[i][j]: the longest common run of a[i..] and b[j..].
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut same = vec![false; b.len()];
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            same[j] = true;
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let mut ranges = Vec::new();
    let mut at = 0;
    for (line, kept) in now
        .split('\n')
        .zip(same.iter().chain(std::iter::repeat(&false)))
    {
        if !kept && !line.is_empty() {
            ranges.push(at..at + line.len());
        }
        at += line.len() + 1;
    }
    ranges
}

/// The tag a line opens a block with: `<ca>` alone (or with the block on
/// the same line), not a closing tag.
fn opening(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix('<')?;
    let end = rest.find('>')?;
    let tag = &rest[..end];
    (!tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| tag.to_owned())
}

/// What a block holds, in a word or two.
fn what(tag: &str) -> &'static str {
    match tag {
        "ca" | "cert" | "extra-certs" => "certificate",
        "key" => "private key",
        "pkcs12" => "certificate and key",
        "tls-auth" | "tls-crypt" | "tls-crypt-v2" | "secret" => "key",
        "dh" => "parameters",
        "crl-verify" => "revocation list",
        _ => "block",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "client\ndev tun\nremote vpn.initech.example 1194\n<ca>\n-----BEGIN CERTIFICATE-----\nMIIB\nAAAA\n-----END CERTIFICATE-----\n</ca>\n<key>\nKKKK\n</key>\nverb 3\n";

    #[test]
    fn blocks_fold_into_a_line_each_and_come_back_byte_for_byte() {
        let f = fold(PROFILE);
        assert_eq!(
            f.shown,
            "client\ndev tun\nremote vpn.initech.example 1194\n<ca> certificate, 4 lines, kept as they are </ca>\n<key> private key, 1 line, kept as it is </key>\nverb 3\n"
        );
        assert_eq!(unfold(&f.shown, &f.blocks).unwrap(), PROFILE);
        // An edit elsewhere keeps the blocks.
        let edited = f.shown.replace("vpn.initech", "vpn2.initech");
        assert_eq!(
            unfold(&edited, &f.blocks).unwrap(),
            PROFILE.replace("vpn.initech", "vpn2.initech")
        );
    }

    #[test]
    fn a_folded_line_deleted_drops_its_block_and_one_changed_is_refused() {
        let f = fold(PROFILE);
        let without_key: String = f
            .shown
            .lines()
            .filter(|l| !l.starts_with("<key>"))
            .map(|l| format!("{l}\n"))
            .collect();
        assert!(!unfold(&without_key, &f.blocks).unwrap().contains("KKKK"));
        let changed = f.shown.replace("4 lines", "5 lines");
        let e = unfold(&changed, &f.blocks).unwrap_err();
        assert!(e.contains("<ca>"), "{e}");
    }

    #[test]
    fn a_block_that_never_closes_stays_as_it_is() {
        let text = "client\n<ca>\nAAAA\n";
        assert_eq!(fold(text).shown, text);
    }

    #[test]
    fn changed_lines_are_the_ones_not_in_the_original() {
        let before = "client\ndev tun\nremote a 1194\nverb 3";
        let now = "client\ndev tun\nremote b 1194\nroute 10.20.0.0 255.255.0.0\nverb 3";
        let ranges = changed_lines(before, now);
        let lines: Vec<&str> = ranges.iter().map(|r| &now[r.clone()]).collect();
        assert_eq!(lines, ["remote b 1194", "route 10.20.0.0 255.255.0.0"]);
        assert!(changed_lines(before, before).is_empty());
    }
}
