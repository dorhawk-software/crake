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
  [an identity pass](ui/identity-pass.md), [visual craft](ui/visual-craft.md),
  [dates, times and numbers](ui/pickers.md), [consent](ui/consent.md),
  [feedback while it works](ui/feedback.md)
- [x] Rust docs: [conventions](rust/conventions.md), [lints](rust/lints.md),
  [coverage](rust/coverage.md), [GPUI](rust/gpui.md)
- [x] Templates: lints, clippy, deny (GPL and closed-source lists), toolchain,
  Dependabot, Claude settings
- [x] The kit: lint, check-size (with the shrinking allowlist),
  check-private (fails closed), check-unsafe, check-coverage (floors per
  OS, never lowered, skips counted), check-commit-msg, install-hooks,
  claude-hook, disk-guard, prune-caches, sweep-target; `bin/install`
- [x] check-gpui-clip: in a GPUI app, no element both rounded and clipped
  in one builder chain (Unreleased)
- [ ] check-gpui-clip follows an element built over several statements
  through a variable (`let mut r = div()…; r = r.overflow_hidden()`), and
  one rounded in a helper and clipped by its caller
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
- **`scripts/check-ui`, from Scops and the Windows-app runner:** the UI
  rules' geometry, measured. Scops' misses of 2026-11-15 (targets 2 px
  apart, buttons in the home indicator, a halo 4 px from text, an
  off-centre line, two pickers of different sizes) and the runner's
  button sticking out of its panel were all geometry, and all invisible
  on a scaled-down board.
  - [x] One runner over every board, at each width, light and dark, from
    claude-hook (after edits, and before the turn ends), pre-commit and
    CI: contain, clip, align, gap, text-size, ring, shadow, halo, centre,
    state, icon, same-look, contrast (on the pixels behind the text).
    Each check has a planted fault in `tests/run` and a mutant in
    `tests/mutate` (1.6.0; the stored mutants were dropped on
    2026-11-18, each check now broken once while it is added). ink: one hue per glyph (1.6.3). inset: what
    covers an edge takes room from the padding (1.6.5).
  - [x] Scops' round 2 (2026-11-16, 903 findings on 87 boards, then 0):
    both safe areas of a phone frame, labels on one line, the page behind
    an open sheet; inert pages and groups of pressed toggles understood;
    a halo measured to the text's ink; a button's accessible name read;
    fixed-width boards at their own width and boards without dark rules
    in one theme (348 renders where 87 would do); Chrome's first page
    opened or retried, and `--no-sandbox` for root (1.7.0).
  - [x] `check-copy`: each language's grammar by its own rules file (cs,
    en, es, and a project's own), copy that names no language or a
    language without rules as findings, one wording per `data-fact`
    across the boards; the button words `outcome` refuses moved into the
    same files (1.7.0).
  - [x] Feedback while it works, from the Windows-app runner (2026-11-18:
    a still egg for minutes of work, and an Open button that gave no sign
    for 10 to 30 s): busy, a control marked `aria-busy="true"` disabled,
    without a pointer cursor, its words ending in "…"; working, something
    in it moving with motion allowed; motion, nothing looping with
    reduced motion (Unreleased).
  - [x] A resize never shows a squashed frame: squash, words drawn
    scaled out of their shape, now or by a `@keyframes` rule
    (Unreleased). A transition to a scaled state isn't read yet.
  - [ ] Motion with reduced motion, the rest: finite animations and
    transitions that still play, SVG `<animate>` loops, generic spinners.
  - [ ] `check-copy` reads a board as written: text a script writes at
    load isn't read. Read the rendered DOM where `check-ui` renders.
  - [ ] A click-through test of a prototype: each `data-fact` first shown
    after the step that collects it (ui/design-before-build.md, "Every
    fact a screen uses was entered earlier").
  - [ ] The next checks, most value for least effort (the 2026-11-15
    audit, [ENFORCEMENT.md](ENFORCEMENT.md)): values off the project's
    spacing and type scale; a border census (borders and inset rings that aren't focus or
    a checkbox, nested cards); leftover text (TODO, lorem, "Option A");
    the face that actually renders (CDP `CSS.getPlatformFontsForNode`);
    one shadow recipe per elevation and a declared light direction;
    a row partly covered
    by a drawing; one stroke cap and width per icon family; grain where
    the identity declares texture.
  - [ ] `actions/check-ui`: the gate in a project's CI, on macOS (boards
    that use the system's fonts render the same as on the designer's Mac).
  - [ ] `ui/frames/`: verified phone frames (iOS status bar and Dynamic
    Island, the lock screen with a Live Activity, Android's status bar
    with an ongoing notification), taken from the platforms' own kits.
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
- Typed values (`ui/pickers.md`) are a project's own script until
  `check-ui` measures them (phase 7).

## Every rule a machine can check, a gate

The rule since 1.6.0 (CLAUDE.md): a rule a script can decide is enforced
by one the agent can't skip, and its findings go back to the agent. The
audit of 2026-11-15 classified every rule ([ENFORCEMENT.md](ENFORCEMENT.md),
its counts line says how many stand where today). The todo and partial
rows are this list's source; the most valuable first.

- **Holes in what already runs** (each found by reading the script, not
  its doc):
  - [ ] `lint` runs clippy without `-D warnings`, and nothing checks that
    `[workspace.lints]` and each crate's `[lints] workspace = true` exist:
    a project that never pasted Cargo-lints.toml passes with warnings.
  - [ ] Coverage floors: a crate missing from `coverage-floors` is never
    measured, and deleting a crate's line gets past "never lowered".
  - [ ] `lint` skips a `lint.d` file that isn't executable, silently.
  - [ ] `disk-guard` does nothing without a Cargo.toml: Xcode and Gradle
    builds have no disk floor.
  - [ ] `rust-ci.yml` never re-checks pushed commit messages
    (`--no-verify` gets through); pull request bodies aren't checked.
  - [ ] `actions/rust-lint` puts the private-terms secret in the
    environment of the step that runs clippy ("Secrets reach only the
    steps that need them").
- **Common:**
  - [ ] check-commit-msg: the Co-Authored-By trailer parses
    (`git interpret-trailers --parse`), subjects stay under 72 characters,
    no `feat:`/`fix:` prefixes.
  - [ ] The commit identity: `user.email` set and equal to a `GIT_EMAIL`
    in `.practices/config`.
  - [ ] The changelog's shape: the five headings, `- **headline** body`,
    and a lockfile change comes with an Unreleased line.
  - [ ] A workflow lint: third-party actions pinned by SHA, no secrets in
    the environment of build steps, `run_attempt` in artifact names, no
    test retries, `cancel-in-progress: false` on release workflows.
  - [ ] A PreToolUse claude-hook: refuse heavy commands without
    `nice -n 19`, a `CARGO_TARGET_DIR` or `--target-dir` override, and
    wiping a simulator or emulator that isn't on the demo list.
  - [ ] Releases (common/releases.md, mobile/releases.md) and mobile CI
    have no script in the kit yet: phase 2's release script carries them.
  - [ ] The PreToolUse claude-hook also refuses a command that polls a
    `*.lock` path in a loop ("Only the lock's own script waits for the
    lock").
  - [ ] A Stop hook's slow gate skips while its lock is held, and when
    nothing it checks changed since its last clean run (common/gates.md).
  - [ ] The PreToolUse claude-hook also refuses booting a second
    simulator while one is booted ("Simulators run one at a time until
    the machine is measured"), and an edit to a shell script a running
    process executes ("Don't edit a script while it runs").
  - [ ] The kit's slow gates (check-ui, check-ui-tests) print each
    phase's time and the total (common/gates.md, "A slow gate prints each
    phase's time").
  - [x] check-voice reads JSX text between tags (1.6.1).
  - [x] check-voice reads the copy on design boards and web pages, in
    each element's lang, and takes COPY_LANG for copy that names none
    (1.7.3).
  - [ ] check-voice knows more languages' we-words than English and
    Czech.
  - [x] A check is proven by breaking it once, while it is added, and
    the commit says so: `check-proven` asks for a `Proven:` line
    (Unreleased). Until 2026-11-18 this item asked tests/mutate to keep
    a mutant for every check; tests/mutate and its CI shards are gone.
- **Rust:**
  - [ ] `clippy::allow_attributes` and `allow_attributes_without_reason`
    denied in Cargo-lints.toml.
  - [ ] Fail on a `[[patch.unused]]` in Cargo.lock (a `[patch]` silently
    not applied).
  - [ ] A case-clash check for every project (`git ls-files` folded to
    lower case has no duplicates).
- **Mobile, from App Review** ([mobile/app-review.md](mobile/app-review.md),
  three rejections of an iPhone app in November 2026):
  - [x] `check-permission-words`, `check-xcode`, `check-ios-artefact`
    and `check-ui-tests`, in lint, claude-hook and pre-commit, each with
    its planted faults in `tests/run` (Unreleased).
  - [ ] check-ios-artefact before every upload: phase 2's release script
    and the reusable mobile release workflow run it, not only the agent's
    archive.
  - [ ] check-ui-tests reads Espresso, Detox and Maestro tests, and a
    dark-mode run.
  - [ ] Every screen audited and its main button asserted on screen: a
    list of the app's screens checked against the UI tests.
  - [ ] check-permission-words follows a view to the file that asks for
    the permission, so a bare "Allow" there is found too.
- **TypeScript:**
  - [ ] A shared ESLint flat config (`curly: all`, `no-console`, comments
    that disable a rule say why, `--max-warnings 0`) and a reusable Node
    CI workflow that runs `.practices/bin/lint`.
- **UI:** the next checks for `check-ui`, in phase 7 above.
  - [x] A commit that changes an existing board names its before/after
    page (`check-commit-msg`, 1.6.2).
  - [ ] A `before-after` script: renders a board's object (a selector) at
    HEAD and in the working tree, both themes, at the device's pixel
    ratio, and writes the page the trailer names.
  - [ ] Every control on a board records an event or only navigates: a
    check matches each button's label to the spec's event table or a
    `data-nav` mark.
  - [ ] A pixel match measures each line of text, not only the
    paragraph's box.
  - [ ] State × event: a test pattern for a project's reference engine
    that fails on a pair the engine doesn't define.
  - [x] Type: text drawn in a system fallback (`fallback`) and times in
    proportional figures (`tabular`), in check-ui (Unreleased).
  - [ ] Type roles: a page declares its faces' roles (`meta type-roles`,
    `data-role`) and check-ui compares each element's face with its role.
  - [ ] A face change keeps the ink: small text's ink on a derived board
    against the board it came from, within 5 %.
  - [x] `check-page`: a proposal page read at 375 and 390 px, as the
    owner reads it on a phone: sideways scroll, a one-letter word at a
    line's end where its language ties it, images past the screen or
    blurred past 2x, the doctype, charset and language (Unreleased).
  - [ ] check-page measures the rest a script can: a before/after pair
    taller than one 812 px screen at 375 px, and a page's weight (about
    1 MB).
  - [ ] Scops' screenshot match as a kit script: boards rendered in one
    Chrome and app shots, each kept under a key of what it is made of,
    `--only` for iterating, each phase timed (common/gates.md,
    ui/design-before-build.md). Until then it lives in that project.
  - [ ] check-shots takes a dither tolerance (`--dither 2`), so it can
    serve as a screenshot gate's switch guard (ui/design-before-build.md,
    "A screenshot gate launches the app once").
