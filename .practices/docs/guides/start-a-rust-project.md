# Start a Rust project

What a Rust project adds on its first day, after the steps every project
takes in [start a new project](start-a-new-project.md) (identity,
CLAUDE.md, privacy, the git hooks, CI's cost, changelog, not-now). Do those
first; this page is that guide's step 7 for Rust.

The [example workspace](../examples/rust-workspace/) is the result of both
pages. Copy from it freely.

## 1. The workspace

```toml
# Cargo.toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
publish = false
```

Each crate inherits `version`, `edition`, `license` and `publish`, and opts
into the workspace lints. Shared dependencies go in
`[workspace.dependencies]` from the start
([rust/conventions.md](../rust/conventions.md)). Add `target/` to
`.gitignore`.

## 2. The kit, with its templates

`bin/install` sees the `Cargo.toml` and adds the Rust part: the coverage
floors file, and with `--templates` the starting templates.

```sh
~/src/practices/bin/install --templates .
```

Then paste the printed `[workspace.lints]` block into `Cargo.toml`, give
each crate `[lints] workspace = true`, and:

```sh
$EDITOR deny.toml                 # keep the licence list for your licence
$EDITOR rust-toolchain.toml       # today's stable: rustc --version
.practices/bin/install-hooks
for c in crates/*; do printf '%-24s 0 0\n' "$c"; done >>.practices/coverage-floors
nice -n 19 cargo build && .practices/bin/lint
```

- **Licences:** `deny.toml` has two lists. Keep the GPL one for a GPL
  project. For a closed-source app keep the permissive one, and its
  `[licenses.private]` lines: your own unpublished crates have no open
  licence, and cargo deny would call them unlicensed
  ([rust/lints.md](../rust/lints.md)).
- **Toolchain:** pinned, so every machine and CI build with the same
  compiler; a weekly workflow moves it (step 3).
- **Unsafe code:** forbidden. If one crate must hold it, name its folder in
  `UNSAFE_ALLOWED` in `.practices/config`
  ([rust/conventions.md](../rust/conventions.md)).

On a new project, every check passes on the first run. Keep it that way:
raise the coverage floors as soon as the first tests exist
([rust/coverage.md](../rust/coverage.md)).

The Claude Code settings from `--templates` run the gate after every cargo
build or test an agent starts, and keep the disk floor
([how it works](how-it-works.md)).

## 3. CI

Copy the three workflows from the example:
[ci.yml](../examples/rust-workspace/.github/workflows/ci.yml),
[advisories.yml](../examples/rust-workspace/.github/workflows/advisories.yml),
[toolchain.yml](../examples/rust-workspace/.github/workflows/toolchain.yml),
and the template [dependabot.yml](../rust/templates/dependabot.yml) (already
copied by `--templates`).

- `ci.yml` calls the reusable `rust-ci.yml`: lint, tests with coverage, and
  the cache kept to what main last built.
- `advisories.yml` runs cargo deny's advisories daily, so a vulnerability
  published after your last push still turns CI red.
- `toolchain.yml` opens a pull request when a newer stable Rust has been
  out for 14 days.
- For a private repository, keep `ci.yml` to its one Linux job, as decided
  in [step 5 of the common guide](start-a-new-project.md#5-ci-decide-what-it-costs).

## Checklist

- [ ] Workspace: resolver 3, edition 2024, `[workspace.package]`, `publish = false`
- [ ] `target/` in `.gitignore`
- [ ] `bin/install --templates` run; lints pasted; each crate has `[lints] workspace = true`
- [ ] `deny.toml` keeps one licence list; `rust-toolchain.toml` pinned
- [ ] `.practices/bin/lint` passes
- [ ] Coverage floors set, raised once tests exist
- [ ] `ci.yml`, `advisories.yml`, `toolchain.yml`, `dependabot.yml`
