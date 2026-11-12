---
name: adopt-practices
description: Bring the dorhawk-software/practices kit into a Rust, TypeScript or JavaScript project, or update it - install .practices (lint gate, size, privacy, unsafe, coverage checks, git hooks), copy the starting templates, measure today's coverage and big files so the limits apply from now on, and wire CI to the reusable workflow. Use when the person asks to adopt, set up, install or update "the practices" in a project, or starts a new project.
---

# Adopt the practices in a project

The full guide, with what you'll see at each step:
https://github.com/dorhawk-software/practices/blob/main/guides/adopt-in-a-rust-project.md
(a new project: guides/start-a-new-project.md for any language, then
guides/start-a-rust-project.md). Follow it; this is the order and the
rules that matter most. A project with a package.json gets the JS/TS
gate instead of the Rust one: set `NPM_CHECKS`, `NPM_TEST` and
`SIZE_EXCLUDE` in `.practices/config` (typescript/lints.md, "How
`.practices/bin/lint` runs it"); the Rust-only steps below (Cargo.toml
lints, deny.toml, clippy, coverage floors) don't apply. A project in
another language gets only the language-neutral part, the git hooks
(privacy and commit messages): start-a-new-project.md step 4.

## 1. Get the kit

```sh
git clone --depth 1 https://github.com/dorhawk-software/practices "$TMPDIR/practices"
"$TMPDIR/practices/bin/install" --templates .
```

It never overwrites what the project owns (`.practices/config`,
coverage floors, deny.toml, clippy.toml, ...). It prints the Cargo.toml
lints block to paste when the workspace has none: paste it, and give each
crate `[lints] workspace = true`.

## 2. Settle the settings with the person

Ask, don't guess:
- The project's licence: deny.toml keeps the GPL list or the closed-source
  one (no GPL, AGPL or LGPL crates). Delete the other.
- Does the project work near client data? Then a git-ignored
  `dev/private-terms` with the names (ask the person to fill it; never
  write client names yourself) and `CLAUDE.local.md` for real-world notes.
  Otherwise `PRIVATE_TERMS=none` in `.practices/config`.
- Where unsafe code may live (UNSAFE_ALLOWED), if anywhere.
- Private repository? CI minutes cost money: one Linux job, docs-only
  pushes skipped, the local hooks as the main gate.

## 3. Start from where the project is

Never lower a limit to get started; record today's state so it can only
improve:
- Files over 700 lines: list each in `.practices/size-allowlist` with its
  current count (`.practices/bin/check-size` prints them).
- Functions over 200 lines: `#[expect(clippy::too_many_lines, reason =
  "split pending")]` on each, and a line in the project's not-now list.
- Coverage: run `.practices/bin/check-coverage` with each crate's floor at
  0, read the measured shares, and set each floor to its share.
- Fix every other clippy, deny and machete finding: they're cheap.

## 4. Hooks and CI

```sh
.practices/bin/install-hooks
```

`.github/workflows/ci.yml` calls
`dorhawk-software/practices/.github/workflows/rust-ci.yml@v1` (the example
in examples/rust-workspace/.github/workflows/). Add the schedule-driven
`rust-advisories.yml` and `rust-toolchain-bump.yml`. Install already made
`.github/workflows/practices.yml`, which pulls kit updates weekly as pull
requests; ask the person to allow GitHub Actions to create pull requests
in the repository's settings (Settings, Actions, General).

## 5. Write it down

- CLAUDE.md: install added `@.practices/rules.md`, which loads every
  rule. Add only the project's own settings around it (its floors, its
  allowlist, its CI budget), never a copy of a shared rule.
- Tell the person to restart the agent session: a running one doesn't
  load the new rules and skills.
- The not-now list: every allowlisted file and expected function, as
  work to come back to.
- Commit in steps the person can review: the kit, then the fixes, then CI.
