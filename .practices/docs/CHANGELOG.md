# Changelog

What changed for the projects that use the practices. Written by hand,
newest first, in the kinds common/changelog.md describes.

## Unreleased

## 1.3.0 (2026-11-12)

### New

- **A quiet pre-commit.** It prints the tests' output only when they fail.
- **The plugin's version is the practices' version.** It said 1.1.0
  through 1.2.0 and 1.3.0; it now says 1.3.0, and `tests/versions` fails
  when plugin.json or marketplace.json disagrees with the newest release.
- **README stays the map.** `tests/readme` fails when "What's in the
  repository" misses a top-level folder or file, or when a practice, guide
  or script isn't listed. The tree had gone a release without
  `typescript/`, `mobile/` and `templates/`.
- **This repository has a local gate.** `tests/check` runs shellcheck on
  every script, the links and `tests/run`; `.githooks/pre-commit` runs it
  and CI's static job calls it. A shellcheck failure in `tests/run` had
  reached main because the checks ran only in CI.
- **JavaScript and TypeScript projects get the gate.** In a project with a
  package.json, `.practices/bin/lint` runs `tsc --noEmit` (with a
  tsconfig.json) and the npm scripts in `NPM_CHECKS` (default `lint`);
  pre-commit runs the gate and `NPM_TEST` (default `test`) when JS or TS
  files are committed; the Claude Code hook runs the gate after `npm test`,
  `npx jest`, `npx tsc` and the like. `check-size` counts `.ts`, `.tsx`,
  `.js` and `.mjs` files too (tests up to `SIZE_TESTS`), and `SIZE_EXCLUDE`
  leaves generated files out. Rust's checks run only where there's a
  Cargo.toml, and disk-guard only guards cargo's build folder.
- **TypeScript and mobile practices** (`typescript/`, `mobile/`), from the
  Obálka fold-in: conventions, lints and Jest testing; store releases,
  devices and walks, CI for iOS and Android, the app's lifecycle. Projects
  with a package.json, an ios/ or an android/ folder get their rules in
  `rules.md`.
- **Gates that can fail** (`common/gates.md`) and **crash reports and
  logs** (`common/crash-reports.md`), with new sections in testing,
  commits, CLAUDE.md and design before build.
- **The Claude Code hook settings for every project** (`--templates`), not
  only Rust ones: `templates/claude-settings.json`.

### Fixed

- **No example headings in `rules.md`.** The digest listed a heading from
  a code example ("Unreleased" in common/changelog.md) as a rule; it
  reads headings outside code blocks only.
- **The guides and the adopt skill know JS and TS projects.** They still
  said the kit had no checks for TypeScript.

## 1.2.0 (2026-11-12)

### New

- **Agents get the practices with the kit.** `bin/install` now copies the
  practices into `.practices/docs/`, writes `.practices/rules.md` (a line
  per rule, about 120) and the line `@.practices/rules.md` in CLAUDE.md,
  so every session loads them, and puts the skills in `.claude/skills/`:
  new ones for UI design, testing, commits, releases, CI and docs, beside
  the gate and adopt skills. It ends by saying to restart the agent
  session. Before, only the scripts reached a project, and agents saw the
  rules only where someone had copied them into CLAUDE.md.
- **Each project pulls its own updates.** `bin/install` adds
  `.github/workflows/practices.yml`, which calls the new reusable
  `practices-update.yml` weekly: the newest release of the same major, as
  a pull request, with CI started on it. No token and no list of
  projects. `projects.json` and `sync-projects.yml` are gone: the push
  sync needed a token that was never set, so no project ever got a sync
  pull request.
- **An identity pass, through the whole app** (`ui/identity-pass.md`).
  Ideally the identity is researched before the first screen: audience,
  the product's job and process, inspiration, the market, and the facts
  behind its metaphor. Then three iterations, each agreed with the owner:
  establish the identity, redesign a few parts, then redesign the whole
  app, re-audit and polish. How far to push depends on the identity
  already there, but every screen is reviewed. From the Obálka, Crake and
  tern passes.
- **Visual craft** (`ui/visual-craft.md`). Tone instead of outlines, every
  stroke end matching, one drawing per object, a line weight per family,
  compact screens, an 11 px floor, one anatomy per job, real context in
  proposals, the product's voice, motion with a reason.
- **UI design practices, in `ui/`.** Window buttons where each system puts
  them: macOS draws its own on the left and the header makes room, from
  Crake's Mac fixes, with the GPUI how-to in `rust/gpui.md`. Design before
  build moved here from `common/` (its old link is gone).
- **Keyboard shortcuts the way each system writes them** (`ui/`). Bind
  the system's main modifier (⌘ on a Mac, Ctrl elsewhere) and show
  shortcuts as each system does: ⇧⌘K on a Mac, Ctrl+Shift+K elsewhere.
  Translations hold no key names. From tern's first Mac run, where the
  search field said "Ctrl K" and ⌘K did nothing. The GPUI how-to is in
  `rust/gpui.md`.

### Better

- **Window buttons on macOS take 96 px, not 88.** macOS 26 draws its
  three buttons 60 px across, not 54, so 88 px left 10 px before the
  header's content against 18 px before the first button. The room now
  keeps as much after them as before.
- **Dependabot in a private repository.** The template shows how to group
  major updates into one pull request with at most two open, so the first
  run doesn't open one build per crate (tern's opened six).

## 1.1.1 (2026-11-11)

### Fixed

- **The version a project records.** `bin/install` wrote `v1` into
  `.practices/VERSION` when the moving `v1` tag was newer than the release
  tag on the same commit; it names the release (`v1.1.1`) now.

## 1.1.0 (2026-11-11)

### Better

- **Starting a project in any language.** The start guide is now for every
  project (identity, CLAUDE.md, privacy, the git hooks, CI's cost,
  changelog, not-now), with the Rust steps on a page of their own,
  `guides/start-a-rust-project.md`. `bin/install` puts the Rust parts
  (coverage floors, templates) only into a project with a `Cargo.toml`;
  the hooks work in any repository and never call cargo, which a test now
  checks.

### Fixed

- **Closed-source apps' own crates in cargo deny.** The closed-source
  licence list in the deny.toml template now comes with
  `[licenses.private] ignore = true`; without it, cargo deny called each
  of the app's own unpublished crates unlicensed.
- **Cache pruning in private repositories.** The `prune-caches` action
  asked GitHub's API for the default branch, which the job's token may not
  read in a private repository ("Resource not accessible by
  integration"); the workflow now passes the branch in. Found by tern's
  first CI run.
- **A quiet pre-commit hook.** check-coverage listed every test binary
  and its result even when all passed, filling an agent's context on each
  commit (found adopting tern). It lists them only in CI now.

## 1.0.0 (2026-11-11)

### New

- **The practices, from Crake.** Ten common practices and four Rust ones,
  each with the incident that taught it, how to apply it and how to check
  it, plus guides, a worked example and a roadmap.
- **The kit.** `bin/install` puts the lint gate and its checks (private
  names, file sizes, unsafe code, coverage floors, commit messages, the
  disk floor) and git hooks into a project's `.practices/`, keeping what
  the project owns.
- **Actions and reusable workflows.** `rust-setup`, `rust-lint`,
  `rust-coverage`, `prune-caches`; `rust-ci.yml`, `rust-advisories.yml`,
  `rust-toolchain-bump.yml`; and `sync-projects.yml`, which brings kit
  updates to each project as a pull request.
- **A Claude Code plugin** with skills to adopt the kit, fix what the gate
  finds, and fold in another project's practices.

### Better

- **Checks that fail closed** where Crake's could pass silently: the
  privacy check without its terms file, a lowered coverage floor, a
  skipped scenario, `#[cfg(all(test, ...))]` test modules, untracked files
  with unsafe code.
