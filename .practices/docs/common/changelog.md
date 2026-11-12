# Changelog

`CHANGELOG.md` is written by hand, for the people who use the software, in
the product's own voice. It is never generated from commits. Every
user-facing change adds its line under "Unreleased" in the same commit,
and so does every dependency update.

## Written for users, in the product's voice

**What.** Each entry is a bold headline sentence saying what's different
for the user, followed by a sentence or two of explanation:

```markdown
## Unreleased

### Fixed

- **A question Crake asks waits for you.** Until you choose, the name
  doesn't open; before, the question could scroll out of sight.
```

**Why.** Commit subjects are written for developers and are too many to
read. A generated list says what changed in the code, not what changed
for the person reading it. Crake's changelog
([49b206e](https://github.com/dorhawk-software/crake/commit/49b206e),
decided 2026-10-13) is also the GitHub release text and the in-app "What's
new" window, so one well-written line serves all three.

**Apply.** Write the line while the change is fresh, as part of the same
commit. Reuse the product's copy voice: the same words the UI uses.

**Check.** In review: would someone who only uses the app understand the
line?

## Fixed kinds

**What.** Every change sits under one of five headings, in this order:
`### New`, `### Better`, `### Changed`, `### Fixed`, `### Underneath`.
Underneath covers what the software is built on: dependencies, toolchain,
packaging. A dependency's security fix goes under Fixed.

**Why.** A fixed set lets the app group What's new the same way every
time. It also lets a check refuse an entry that has no kind. Crake
introduced the kinds when What's new was built
([c8cd76c](https://github.com/dorhawk-software/crake/commit/c8cd76c),
2026-10-21). A unit test parses the whole changelog, so a malformed entry
fails the build.

**Apply.** Pick the kind by what the user experiences, not by what the
code did. A refactor with no visible effect needs no line.

**Check.** The release script refuses any bullet outside the five kinds.
If the app shows the changelog, a test parses it whole.

## Same commit, dependency updates too

**What.** A change and its changelog line land together. A merged
Dependabot update, or one done by hand, adds a line under Underneath. The
line says what was updated and what it means for users, often "nothing
you'd notice". A security fix in a dependency says so, and warrants at
least a hotfix release.

**Why.** For Crake's 0.2.0 the lines hadn't been written along the way.
The release had to reconstruct them from the log, by hand (2026-10-13).
Lines have also landed under an already-released section
([d890718](https://github.com/dorhawk-software/crake/commit/d890718)).
Dependabot merges needed follow-up commits just for the line
([135f110](https://github.com/dorhawk-software/crake/commit/135f110)).
The dependency rule was asked for 2026-10-19: without a line, a security
hotfix is impossible, because the release script refuses an empty
Unreleased section.

**Apply.** After merging a dependency update, add its line before anything
else. If its notes mention RUSTSEC, "unsound" or a CVE, say so in the line
and consider a hotfix.

**Check.** Two CI checks belong here; neither is enforced in Crake today:
- a change to `Cargo.lock` must also touch the Unreleased section (or the
  PR carries a skip label);
- every released section must match its text at the release's tag.

## The changelog is the release notes and What's new

**What.** The release section becomes the GitHub release text. If the app
has a What's new window, it is the same text, built in with
`include_str!`. A release with only Underneath changes shows no notice in
the app.

**Why.** One source means one place to write well. Crake's What's new
([c8cd76c](https://github.com/dorhawk-software/crake/commit/c8cd76c))
remembers the last version seen, so a fresh install says nothing and a
downgrade doesn't repeat old notes.

**Apply.** Parse `## <version> (<date>)`, `### <kind>` and `- **headline**
body`. A small parser plus a test that reads the real file is enough.
(A shared crate for this is planned for a later phase.)

**Check.** A unit test reads the built-in changelog whole.
