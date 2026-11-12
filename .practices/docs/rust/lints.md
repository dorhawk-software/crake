# Rust: lints are errors

Every rustc warning and every clippy lint fails the build. Fix the code and build again; never silence a lint. The same gate runs in four places: after every build an agent makes, before every commit, in CI on every system, and on a schedule for new advisories.

Links point at commits in [Crake](https://github.com/dorhawk-software/crake).

## Lints denied in the workspace

**What.** `[workspace.lints]` denies `warnings` and `clippy::all`, and each crate opts in with `[lints] workspace = true`.

**Why.** Wanted as "a hard stop during the build", not advice ([0b63fbb](https://github.com/dorhawk-software/crake/commit/0b63fbb)). Three `#[allow]`s remain in Crake's whole tree, each with a reason.

**Apply.** Copy [templates/Cargo-lints.toml](templates/Cargo-lints.toml) into the root `Cargo.toml`, then add this to every member:
```toml
[lints]
workspace = true
```
A lint that must stay for a reason uses `#[expect(lint, reason = "…")]`, never `#[allow]`. `expect` fails once the lint no longer fires, so the exception cleans itself up.

**Check.** `.practices/bin/lint` runs `cargo clippy --workspace --all-targets`. A review question for any `#[allow`: why not fix it?

## Pin the toolchain

**What.** `rust-toolchain.toml` names an exact stable version. A weekly workflow moves it on once a release is two weeks old.

**Why.** Unpinned, a new stable clippy lint turned CI red overnight ([ba10f9e](https://github.com/dorhawk-software/crake/commit/ba10f9e)). Under `warnings = "deny"`, a deprecation is an error too ([01ff76e](https://github.com/dorhawk-software/crake/commit/01ff76e)). Waiting two weeks gives point releases time to land.

**Apply.** Copy [templates/rust-toolchain.toml](templates/rust-toolchain.toml), then call `rust-toolchain-bump.yml` weekly:
```yaml
on: { schedule: [{ cron: "0 5 * * 1" }], workflow_dispatch: {} }
jobs:
  bump:
    uses: dorhawk-software/practices/.github/workflows/rust-toolchain-bump.yml@v1
```

**Check.** CI and every developer build use the same `rustc --version`.

## Lint for every target OS

**What.** Clippy runs per target OS, because code under `#[cfg(target_os = …)]` is invisible elsewhere.

**Why.** Crake's Windows and macOS jobs failed on dead code, an unused `mut` and a re-export that Linux never compiled ([c63df39](https://github.com/dorhawk-software/crake/commit/c63df39), [1f9cd79](https://github.com/dorhawk-software/crake/commit/1f9cd79), [15634a2](https://github.com/dorhawk-software/crake/commit/15634a2)).

**Apply.** Run `cargo clippy --target x86_64-pc-windows-gnu` in CI (mingw on Linux is enough), and a macOS job once the project builds there.

**Check.** CI has a clippy step for each supported OS.

## cargo deny: advisories, licences, sources

**What.** [templates/deny.toml](templates/deny.toml) refuses:
- known vulnerabilities and yanked crates anywhere in the tree;
- unmaintained crates the workspace uses directly;
- licences outside an allow-list;
- anything not from crates.io.

**Why.**
- It stopped a release on RUSTSEC-2026-0385 ([a2938a7](https://github.com/dorhawk-software/crake/commit/a2938a7)) and caught RUSTSEC-2026-0382 ([d9a333d](https://github.com/dorhawk-software/crake/commit/d9a333d)).
- It refused a yanked crate ([01ff76e](https://github.com/dorhawk-software/crake/commit/01ff76e)).
- `unmaintained = "workspace"` exists because GUI toolkits pull in unmaintained crates with no replacement.

**Apply.**
- The template has two licence lists; keep the one that matches your project and delete the other.
  - **GPL projects** (Crake's): permissive licences plus GPL-3.0-or-later, MPL-2.0 and the other GPL-compatible ones.
  - **Proprietary or closed-source apps:** permissive licences only, with no GPL, AGPL or LGPL crates. A statically linked LGPL crate would oblige you to let users relink the app. MPL-2.0 is allowed, because its copyleft covers only the MPL files themselves.
    Keep its `[licenses.private] ignore = true` too: the app's own crates are unpublished and have no open licence, and cargo deny would otherwise report each one as unlicensed (found adopting tern, 2026-11-11).
- A dependency's security fix is a changelog line under Fixed and at least a hotfix release (see [common/changelog.md](../common/changelog.md)).
- Call `rust-advisories.yml` daily, so a new advisory shows up before it turns every branch red at once.

**Check.** `cargo deny check` in `.practices/bin/lint`, plus the daily advisories run.

## cargo machete: no unused dependencies

**What.** `cargo machete` fails on a dependency no code uses.

**Why.** Unused dependencies cost build time and audit surface, and nothing else notices them.

**Apply.** It's a step of `.practices/bin/lint`. A dependency used only through a macro gets `[package.metadata.cargo-machete] ignored = ["name"]`, with a comment.

**Check.** `.practices/bin/lint`.

## Size limits

**What.**
- At most 200 lines per function: `clippy::too_many_lines`, with the threshold in `clippy.toml`.
- At most 700 lines of code per source file, not counting its inline `#[cfg(test)] mod tests { … }`.
- At most 1000 lines per integration-test file.
- Past a limit, split by topic: a folder module with one file per part, or a function's parts as named helpers.

**Why.** A file or function a person can't hold in their head hides bugs. The first split in Crake moved about 6,500 lines across 35 files ([6f74697](https://github.com/dorhawk-software/crake/commit/6f74697)). The checker had a hole: any `#[cfg(test)]` counted as the start of the tests, so a `mod tests;` near the top let one file reach 736 lines unseen ([6de1e6a](https://github.com/dorhawk-software/crake/commit/6de1e6a)).

**Apply.**
- Adopting in a project that is already over: list today's offenders in `.practices/size-allowlist` (`path max-lines`). An entry may only shrink, and `check-size` fails if a file grows past its entry or shrinks without the entry being lowered.
- For long functions, use `#[expect(clippy::too_many_lines, reason = "split pending")]`. It fails once the function is short enough, so the list cleans itself.

**Check.** `.practices/bin/check-size` and clippy, both in `.practices/bin/lint`.

## Unsafe in one place

See [conventions.md](conventions.md#unsafe-code-in-one-crate). `.practices/bin/check-unsafe` fails on `unsafe` outside the paths in `UNSAFE_PATHS` (in `.practices/config`).

## Project-specific steps

**What.** Checks only one project needs go in `.practices/lint.d/`, as executables that `lint` runs after its own steps. Examples: a vendored-patch check, or a magic-number linter.

**Why.** The shared steps stay the same everywhere, and nothing project-specific gets lost in a sync.

**Apply.** Add `.practices/lint.d/50-patch-in-use`. Exit non-zero with a message that says how to fix it.

**Check.** Run `.practices/bin/lint` and see the step's name in the failure output.

## Where the gate runs

| Where | What runs | How |
| --- | --- | --- |
| After an agent's cargo build or test | `lint` | `.practices/bin/claude-hook` (PostToolUse) |
| `git commit` | `check-private`, then `lint` and `check-coverage` when `.rs` files are staged | `.practices/hooks/pre-commit` |
| Commit message | `check-commit-msg` | `.practices/hooks/commit-msg` |
| CI, every push | `lint`, `check-coverage` | `actions/rust-lint`, `actions/rust-coverage` |
| Daily | `cargo deny check advisories` | `rust-advisories.yml` |

Install the hooks once per clone with `.practices/bin/install-hooks`. It sets `core.hooksPath=.practices/hooks`, and `lint` fails locally when they aren't installed.

The output stays quiet unless something fails, so an agent's context holds only what to fix.
