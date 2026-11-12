# Roadmap

What's built, what comes next, and what the docs promise that isn't built
yet. Each later phase starts when a second project needs it, so every
practice has two users before it's called shared. Crake (public,
[github.com/dorhawk-software/crake](https://github.com/dorhawk-software/crake))
is the source of everything Rust here; its commit links are in each doc.

## Phase 1: the gate (built)

- [x] Common docs: [commits](common/commits.md), [privacy](common/privacy-public-repos.md),
  [CLAUDE.md](common/claude-md.md), [not-now](common/not-now.md),
  [changelog](common/changelog.md), [releases](common/releases.md),
  [testing](common/testing.md), [CI and caching](common/ci-caching.md),
  [agent working rules](common/agent-working-rules.md)
- [x] UI docs: [design before build](ui/design-before-build.md),
  [window buttons](ui/window-buttons.md),
  [keyboard shortcuts](ui/keyboard-shortcuts.md),
  [an identity pass](ui/identity-pass.md), [visual craft](ui/visual-craft.md)
- [x] Rust docs: [conventions](rust/conventions.md), [lints](rust/lints.md),
  [coverage](rust/coverage.md), [GPUI](rust/gpui.md)
- [x] Templates: lints, clippy, deny (GPL and closed-source lists), toolchain,
  Dependabot, Claude settings
- [x] The kit: lint, check-size (with the shrinking allowlist),
  check-private (fails closed), check-unsafe, check-coverage (floors per
  OS, never lowered, skips counted), check-commit-msg, install-hooks,
  claude-hook, disk-guard, prune-caches, sweep-target; `bin/install`
- [x] Tests of the kit (`tests/run`), under bash 3.2 on macOS
- [x] Guides: [how it works](guides/how-it-works.md),
  [adopt in a Rust project](guides/adopt-in-a-rust-project.md),
  [start a new project](guides/start-a-new-project.md) (any language),
  [start a Rust project](guides/start-a-rust-project.md),
  [fold in a project](guides/fold-in-a-project.md)
- [x] [Example workspace](examples/rust-workspace/)
- [x] Composite actions `rust-setup`, `rust-lint`, `rust-coverage`,
  `prune-caches`; reusable workflows `rust-ci.yml`, `rust-advisories.yml`,
  `rust-toolchain-bump.yml`; the update workflow (pulled by each project, `practices-update.yml`)
- [x] The Claude Code plugin's skills
- [ ] Crake switched to the kit, with no loss in what its CI catches; then
  the `v1` tag
- [ ] tern adopts it ([the adopt guide](guides/adopt-in-a-rust-project.md))

## Phase 2: releases

- **What:** the release script (green CI first, tags exactly the commit CI
  passed, resumable, dry run first) and the release notes script, with the
  version files as a setting; a `release-gate` workflow (a dated changelog
  section, versions that agree, a tagged commit that passed CI); a
  rehearsal on `workflow_dispatch`; publishing with SHA256SUMS; deleting the
  run's artifacts once published. CI checks that a `Cargo.lock` change
  touches Unreleased, and that released sections never change.
- **From Crake:** `scripts/release.sh`, `scripts/release-notes.sh`,
  `.github/workflows/release.yml`.
- **Starts when:** tern's first release.

## Phase 3: Linux packages

- **What:** `build-deb` on the oldest supported Ubuntu (one .deb for every
  Ubuntu and Debian), the .rpm repacked from the same tree, a PKGBUILD
  (`!lto`) and AUR publishing, Nix with GPUI's runtime libraries on the
  rpath and a dependency layer without the version (no rebuild per
  release), publishing to the shared apt/dnf repository, and an AppStream
  metainfo file (Crake lacks one).
- **From Crake:** `scripts/build-deb.sh`, `build-rpm.sh`, `packaging/arch/`,
  `packaging/nix/`, `scripts/nix-deps-check.sh`.
- **Starts when:** tern ships to people.

## Phase 4: the GPUI kit

- **What:** a journey kit (`until`, `click`, `settle`, `type_into`) and an
  app `start(cx, Setup)` that tests can drive; `.test_support()` rules;
  demo and show switches parsed by one enum that rejects unknown values;
  capturing a window without flashing the screen; real GPUI windows on
  Xvfb in CI.
- **From Crake:** `journey_kit.rs`, `test_app.rs`, `scripts/xwin.py`,
  `scripts/x11-test.sh`.
- **Starts when:** tern writes its journey suite. Possibly upstream, into
  gpui-kit's test module.

## Phase 5: crates

- **What:**
  - `lint-literals`, the magic-number linter, published: a config file for
    the free numbers and the layout calls (a GPUI preset for
    `px`/`rgb`/`rgba`), workspace discovery through `cargo metadata`.
  - A size check built on `syn` (the test module found by parsing, not by an
    awk pattern), in the same tool.
  - A changelog crate: the parser, the What's new notice, the daily
    `versions.json` update check that stays quiet where a package manager
    owns the install.
- **From Crake:** `crates/lint-literals`, `crates/crake/src/whats_new/`,
  `crates/crake/src/versions.rs`.
- **Starts when:** phase 1 is stable in two projects.

## Phase 6: Windows and macOS

- **What:** `crt-static` and a check that no program needs `VCRUNTIME140.dll`;
  clippy for other systems from Linux; `macos-sign-notarize`; a Homebrew
  cask workflow; the weekly update bot (a pull request, or an issue when
  the update can't apply).
- **From Crake:** `.cargo/config.toml`, `scripts/windows-check.sh`,
  `scripts/macos-signing.sh`, `.github/workflows/gpui.yml`, `release.yml`.
- **Starts when:** tern ships for macOS (signing) and, later, Windows (once
  a self-hosted runner exists).

## Phase 7: other languages

- **`typescript/` and `mobile/`, from Obálka** (React Native): folded in
  with [the same method](guides/fold-in-a-project.md), started 2026-11-12
  ([FROM-OBALKA.md](FROM-OBALKA.md)).
  - [x] Docs: [typescript/](typescript/), [mobile/](mobile/),
    [gates](common/gates.md), [crash reports](common/crash-reports.md),
    and new sections in testing, commits, CLAUDE.md and design before
    build.
  - [x] The kit's gate in a project with a package.json: tsc, the npm
    scripts in `NPM_CHECKS`, the tests on commit, file sizes (1.3.0).
  - [ ] A "start a TypeScript project" guide, like the Rust one.
  - [ ] Actions `ts-setup`, `ts-lint` and a reusable `ts-ci.yml`.
  - [ ] The changelog check (two languages, store limits, checked after
    folding) as a shared tool, filling phase 2's changelog check.
  - [ ] A reusable mobile release workflow: verify, iOS and Android
    builds, the signing keychain, store uploads, the artefact check.
  - [ ] The secret scan with its canary as a composite action.
  - [ ] Obálka's own fixes from the audit (its not-now list): the release
    script's push check and rehearsal, actions pinned by commit, the
    canary on the scan's own binary, CI running the changelog check,
    coverage floors, the size allowlist worked down.
- **`web/`, from the dorhawk.software website:** conventions, deploy,
  templates.
- **`ui/`, more pages:** tern's design canvas rules (an agreed design only,
  captures of the built app, every screen checked in each language), and
  what Obálka and the website teach about UI.
- **Starts when:** Rust has settled (phase 1 proven in Crake and tern).

## Known gaps

Things the docs describe that aren't built yet, or that only hold by
convention. Each one leaves this list when it's built.

- The actions, the reusable workflows, the sync and the plugin (phase 1,
  above) are being built. Until then, the CI snippets in the guides and
  the example's workflows refer to them by their coming names.
- The "never lower a coverage floor" check needs `PRACTICES_BASE_REF`, which
  `actions/rust-coverage` sets. Locally it only runs if you set it.
- "A changelog line in the same commit" and "released sections never
  change" are conventions; their CI checks come with phase 2.
- `check-size` finds the inline test module with an awk pattern at column 0.
  A test module indented or after other attributes counts as code (it fails
  safe). The `syn` version is phase 5.
- The magic-number rule is a convention outside Crake until `lint-literals`
  is published (phase 5).
- Scenario suites (one test per scenario, run everywhere) are a documented
  pattern; the shared macro is phase 4.
