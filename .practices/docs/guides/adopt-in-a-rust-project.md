# Adopt the practices in an existing Rust project

This guide takes a Rust workspace that already exists and puts the practices
in it, step by step. Each step has the command, what you'll see, and what to
do when it fails. The worked case is tern: a closed-source GPUI mail client
with 11 crates, a private repository and no checks at all before.

A project that already exists has files, functions and coverage that don't
meet the limits yet. The rule for adopting is: **stop new drift today, and
work the old drift down with a list that only shrinks.** Nothing is
silenced, and nothing is left without a way back to it.

Read first, if you haven't: [how it works](how-it-works.md) (what the kit
is and what the project owns).

## 0. Before you start

- The build disk is there and has room. On a machine where cargo's
  `build-dir` points at an external disk, plug it in first
  ([agent working rules](../common/agent-working-rules.md)).
- The tools: `cargo install cargo-deny cargo-machete cargo-llvm-cov`, plus
  `jq` (the coverage check and the Claude hook read JSON with it).
- A clean working tree: `git status` shows nothing. Each step below is its
  own commit, so a step that goes wrong is easy to undo.

## 1. Put the kit in

```sh
git clone https://github.com/dorhawk-software/practices ~/src/practices
~/src/practices/bin/install --templates ~/src/tern
```

**What you'll see:** one line per file. `created:` for each template copied
(`clippy.toml`, `deny.toml`, `rust-toolchain.toml`,
`.github/dependabot.yml`, `.claude/settings.json`), `kept:` for any the
project already had. Then the `[workspace.lints]` block to paste, because
`Cargo.toml` can't be merged safely by a script.

**If it fails:** `bin/install` needs the target to be a git repository.
Nothing in the project is overwritten except `.practices/bin` and
`.practices/hooks`, which the kit owns.

Commit: `Take the practices kit`.

## 2. Pin the toolchain

`rust-toolchain.toml` now names an exact release. Set it to what the
project builds with today (`rustc --version`), not the newest, so this step
changes nothing else.

```sh
rustc --version     # e.g. rustc 1.99.1
$EDITOR rust-toolchain.toml
cargo build
```

**Why:** a new stable release brings new clippy lints, and with lints as
errors that breaks CI on a day nobody changed anything
([rust/lints.md](../rust/lints.md)). The weekly bump workflow (step 9) moves
it on once a release is two weeks old.

## 3. Lints are errors

Paste the block `bin/install` printed into the workspace's `Cargo.toml`
(the template is [rust/templates/Cargo-lints.toml](../rust/templates/Cargo-lints.toml)),
and give every crate:

```toml
[lints]
workspace = true
```

Then:

```sh
nice -n 19 cargo clippy --workspace --all-targets 2>&1 | grep -E '^(error|warning)' | sort | uniq -c | sort -rn
```

**What you'll see:** a count per lint. On a project that only "kept clippy
clean" by hand, expect dozens: unused imports, `needless_*`, `too_many_lines`
and the like.

**What to do:**
- Fix every lint except `too_many_lines`. Most are mechanical; `cargo clippy
  --fix --workspace --allow-dirty` does many of them. Never add `#[allow]`.
- `too_many_lines` (a function over 200 lines, the limit in `clippy.toml`):
  don't split them all now. Mark each one so it is visible and comes back:

  ```rust
  #[expect(clippy::too_many_lines, reason = "split by section: docs/engineering/not-now.md")]
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
  ```

  `#[expect]` is the one exception to "never silence": it fails the build
  once the function is short enough, so it can't outlive its reason. List
  the functions in `docs/engineering/not-now.md` ([common/not-now.md](../common/not-now.md)).
  In tern this was about 11 functions, most of them GPUI `render`s.

Commit: `Lints are errors`.

## 4. Unsafe code in one place

```sh
git grep -n '\bunsafe\b' -- '*.rs'
```

**What you'll see:** every `unsafe`. In tern: two `std::env::set_var` calls
in the WebKit helper process (since edition 2024, `set_var` is unsafe).

**What to do:**
- Every crate root that has none gets `#![forbid(unsafe_code)]`.
- The calls that remain move behind safe functions in one place: a small
  `-os` crate (Crake's `crake-os`), or a `sys` folder in one crate. Name it
  in `.practices/config`:

  ```sh
  UNSAFE_ALLOWED='crates/tern-os'
  ```

- Each `unsafe` block gets a `// SAFETY:` comment saying why it holds.

**Check:** `.practices/bin/check-unsafe` passes. It also sees code a `cfg`
hides on this system, tests and build scripts
([rust/conventions.md](../rust/conventions.md)).

## 5. Dependencies: deny and machete

`deny.toml` has two licence lists. Keep the one for the project's own
licence. For a closed-source app (tern), keep the **proprietary** list: no
GPL, AGPL or LGPL crates, because Rust links them into the program. Delete
the other list.

```sh
cargo deny check
cargo machete
```

**What you'll see:**
- `cargo deny`: licences not on the list, crates from git, advisories.
- `cargo machete`: dependencies a crate declares and doesn't use.

**What to do:**
- A licence not on the list: find which crate brings it
  (`cargo tree -i <crate>`). Replace the crate, or add the licence with a
  comment saying which crate brings it, if it is compatible.
- An advisory: update the crate (`cargo update -p <crate>`). If no fix exists
  yet, add it to `ignore` with why and when to look again.
- An unused dependency: remove it.
- Dependencies repeated across crates (tern had `ureq`, `serde_json`, `sha2`,
  `base64` in several): move them to `[workspace.dependencies]` and write
  `ureq.workspace = true` in each crate.
- Give the workspace a `license` in `[workspace.package]`. A closed-source
  app writes `license = "LicenseRef-Proprietary"` and `publish = false`,
  and tells cargo-deny that its own unpublished crates aren't checked
  against the list:

  ```toml
  [licenses.private]
  ignore = true
  ```

Commit: `Dependencies: licences, advisories, nothing unused`.

## 6. File sizes, with an allowlist that only shrinks

```sh
.practices/bin/check-size
```

**What you'll see:** one line per file over 700 lines of code (an inline
`#[cfg(test)] mod tests { … }` isn't counted). Tern had 14, the biggest at
5,898.

**What to do:** record today's numbers, so no file may grow:

```sh
.practices/bin/check-size |
    sed -n 's/^\(.*\): \([0-9]*\) lines (at most .*/\1 \2/p' >.practices/size-allowlist
.practices/bin/check-size && echo ok
```

From now on a listed file may not grow. When it shrinks, the check asks you
to lower its number. When it is under the limit, the check asks you to
remove it. Split by topic into a folder module, one file per part
([rust/conventions.md](../rust/conventions.md)).

Commit: `File sizes: today's big files may only shrink`.

## 7. Coverage floors at today's share

```sh
nice -n 19 cargo llvm-cov --workspace --summary-only
```

**What you'll see:** a table per file. Add up each crate's lines, or simply
run the check with a floor of 0 and read what it reports:

```sh
for c in crates/*; do printf '%-24s 0 0\n' "$c"; done >>.practices/coverage-floors
.practices/bin/check-coverage
```

**What to do:** the check says "well over its floor" for each crate with its
share. Write each crate's measured share, rounded down, as its floor, in the
Linux column. Measure on macOS too if anyone commits from a Mac, and fill
that column; `-` means no floor on that system.

```text
# crate folder        linux  macos
crates/tern-core      74     74
crates/tern-ui        31     28
```

Floors only rise. CI refuses a floor lower than the base branch's
([rust/coverage.md](../rust/coverage.md)).

**If it fails:** a failing test fails the check. Fix the test first; a test
that can't run on a system records a skip
([common/testing.md](../common/testing.md)), never a quiet pass.

Commit: `Coverage floors at today's share`.

## 8. Hooks, for people and for agents

```sh
.practices/bin/install-hooks
mkdir -p dev && printf '# names never to commit, one PCRE pattern per line\n' >dev/private-terms
echo dev/private-terms >>.gitignore
```

- Git now runs `.practices/hooks/pre-commit` (private names, then the lint
  gate and coverage when Rust files change) and `commit-msg` (no session
  IDs, no private names).
- `.claude/settings.json` runs `.practices/bin/claude-hook` after each shell
  command an agent runs: the disk floor always, the lint gate after cargo
  builds and tests.
- If the project has nothing private to keep out, set `PRIVATE_TERMS=none`
  in `.practices/config` instead of the file. The check never assumes
  "nothing to check" on its own ([common/privacy-public-repos.md](../common/privacy-public-repos.md)).

**Check:** `.practices/bin/lint` passes end to end. It fails while the hooks
aren't installed in this clone.

## 9. CI, on a budget for a private repository

A private repository pays for CI minutes: on GitHub's free plan for
organisations, $12 of compute a month, about 2,000 Linux minutes or under
200 macOS minutes ([common/ci-caching.md](../common/ci-caching.md)). So the
local hooks are the real gate, and CI confirms them with one Linux job.

```yaml
# .github/workflows/ci.yml
name: CI
on:
  push:
    branches-ignore: ["dependabot/**"]
    paths-ignore: ["**.md", "docs/**"]
  pull_request:
    paths-ignore: ["**.md", "docs/**"]
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
jobs:
  rust:
    permissions:
      contents: read
      actions: write   # pruning replaced caches
    uses: dorhawk-software/practices/.github/workflows/rust-ci.yml@v1
    with:
      gpui: true
    secrets:
      private-terms: ${{ secrets.PRIVATE_TERMS }}
```

Add the two scheduled workflows from the [example](../examples/rust-workspace/.github/workflows/):
`advisories.yml` (daily `cargo deny check advisories`, so a new advisory
shows up before it blocks a branch) and `toolchain.yml` (the weekly bump).

**On a budget:**
- macOS and Windows on hosted runners only for releases, or by hand.
- When a self-hosted runner exists (a spare Linux machine, never the one
  you work on), pass `runs-on: '["self-hosted", "linux"]'`. Self-hosted
  minutes are free.
- Check the month's use on the organisation's billing page. Quote measured
  run lengths, not guesses.

**If it fails in CI but not locally:** the toolchain or the system packages
differ. CI installs what `rust-setup` lists; GPUI's packages come with
`gpui: true`.

## 10. Write it down

- `CLAUDE.md`: `bin/install` added `@.practices/rules.md`, which loads every
  shared rule. Add only the project's own settings, and delete older copies
  of shared rules ([common/claude-md.md](../common/claude-md.md)).
- Restart the agent session, so it loads the rules and the skills.
- `docs/engineering/not-now.md`: the big files, the long functions, the
  crates under a low floor, each with what's left
  ([common/not-now.md](../common/not-now.md)).
- `CHANGELOG.md`: nothing. Users don't see this; it isn't a user-facing
  change ([common/changelog.md](../common/changelog.md)).

## Order, and what it cost in tern

| Step | Effort | Why this order |
| --- | --- | --- |
| Kit, toolchain, lints | An afternoon of fixes | Stops new drift at once |
| Unsafe, deny, machete | An hour | Small, and closes whole classes of risk |
| Size allowlist, coverage floors | Minutes | Records today, so it can't get worse |
| Hooks, CI | Minutes | Turns the rules into gates |
| Splitting files and functions | Weeks, piece by piece | The allowlist and `#[expect]` keep it honest |
