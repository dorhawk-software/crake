//! `lint-literals crates/<crate>...`: the magic numbers in each crate's own
//! code (not its tests), one per line, and a failing exit if there are any.
//! Run by scripts/lint.sh, so they fail the build like any other lint.
//!
//! A number may stay as it is when it is named (a const, a static, an enum
//! discriminant), when it is 0, 1 or 2, when it is UI layout from the design
//! (inside `px(..)`, `rgb(..)`), or when a `// literal: <reason>` comment on
//! its line (or alone on the line above) says why it isn't named.
//!
//! The files are found as rustc finds them: from the crate's roots (src/lib.rs,
//! src/main.rs, src/bin/*.rs) along their `mod` declarations, so a test module
//! (`#[cfg(test)] mod tests;`) and everything in it is left out.

mod check;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut found = 0;
    let mut failed = false;
    let mut seen = HashSet::new();
    for krate in std::env::args().skip(1) {
        let mut queue: Vec<(PathBuf, PathBuf)> = roots(&Path::new(&krate).join("src"));
        while let Some((file, dir)) = queue.pop() {
            if !seen.insert(file.clone()) {
                continue;
            }
            match visit(&file, &dir) {
                Ok((hits, next)) => {
                    found += hits;
                    queue.extend(next);
                }
                Err(e) => {
                    eprintln!("{}: {e}", file.display());
                    failed = true;
                }
            }
        }
    }
    if found > 0 {
        eprintln!(
            "\n{found} magic numbers: name each one (a const), or say why it isn't named with `{} <reason>`.",
            check::MARKER
        );
    }
    if found > 0 || failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// A crate's root files, each with the directory its `mod` declarations
/// resolve in (its own).
fn roots(src: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut files: Vec<PathBuf> = ["lib.rs", "main.rs"]
        .iter()
        .map(|f| src.join(f))
        .filter(|p| p.is_file())
        .collect();
    if let Ok(bins) = std::fs::read_dir(src.join("bin")) {
        files.extend(
            bins.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "rs")),
        );
    }
    files
        .into_iter()
        .filter_map(|f| Some((f.clone(), f.parent()?.to_path_buf())))
        .collect()
}

/// Print `file`'s magic numbers; return how many, and its modules in files of
/// their own (each with the directory its own modules resolve in).
fn visit(file: &Path, dir: &Path) -> Result<(usize, Vec<(PathBuf, PathBuf)>), String> {
    let source = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let report = check::check(&source).map_err(|e| {
        let at = e.span().start();
        format!("{}:{}: {e}", at.line, at.column + 1)
    })?;
    for hit in &report.hits {
        println!(
            "{}:{}:{}: magic number {}",
            file.display(),
            hit.line,
            hit.column,
            hit.text
        );
    }
    let next = report
        .mods
        .iter()
        .map(|m| child(dir, m).ok_or_else(|| format!("no file for `mod {};`", m.name)))
        .collect::<Result<_, _>>()?;
    Ok((report.hits.len(), next))
}

/// Where `mod name;` lives, as rustc looks for it: its `#[path]`, else
/// `name.rs` or `name/mod.rs` (inside any inline modules around it).
fn child(dir: &Path, m: &check::ModDecl) -> Option<(PathBuf, PathBuf)> {
    let base = m.inline.iter().fold(dir.to_path_buf(), |d, i| d.join(i));
    if let Some(path) = &m.path {
        let file = base.join(path);
        let own = file.parent()?.to_path_buf();
        return Some((file, own));
    }
    let flat = base.join(format!("{}.rs", m.name));
    if flat.is_file() {
        return Some((flat, base.join(&m.name)));
    }
    let nested = base.join(&m.name).join("mod.rs");
    nested.is_file().then(|| (nested, base.join(&m.name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Run from the crate's own directory: its source is the example.
    #[test]
    fn follows_modules_as_rustc_does() {
        let src = Path::new("src");
        assert_eq!(
            roots(src),
            [(src.join("main.rs"), src.to_path_buf())],
            "a binary crate's root"
        );
        let decl = |name: &str, path: Option<&str>| check::ModDecl {
            name: name.into(),
            path: path.map(Into::into),
            inline: Vec::new(),
        };
        assert_eq!(
            child(src, &decl("check", None)),
            Some((src.join("check.rs"), src.join("check")))
        );
        assert_eq!(
            child(src, &decl("tray", Some("check.rs"))),
            Some((src.join("check.rs"), src.to_path_buf())),
            "#[path]"
        );
        assert_eq!(child(src, &decl("missing", None)), None);
    }

    #[test]
    fn its_own_code_has_no_magic_numbers() {
        let (hits, mods) = visit(Path::new("src/main.rs"), Path::new("src")).unwrap();
        assert_eq!(hits, 0);
        assert_eq!(
            mods,
            [(PathBuf::from("src/check.rs"), PathBuf::from("src/check"))]
        );
        assert!(visit(Path::new("src/nothing.rs"), Path::new("src")).is_err());
    }
}
