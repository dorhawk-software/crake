# Practices

Development practices that held up shipping real software. For each one
you get what it is, the incident that taught it, how to apply it, and how
to check it's applied. Alongside them is a kit that enforces them in a
project: scripts, git hooks, GitHub Actions and a Claude Code plugin.

Everything here was first learned the hard way in
[Crake](https://github.com/dorhawk-software/crake), a Rust VPN client
built for Linux, Windows and macOS in about a month (564 commits). Every
"why" links to the Crake commit where the lesson was paid for. It is
written for anyone starting or running a project, and for the people who
wrote it, the next time they start one.

> Status: phase 1 (Rust). TypeScript/React Native and websites are next;
> see [ROADMAP.md](ROADMAP.md).

## Start here

| You want to | Read |
| --- | --- |
| Know what's here and why it's built this way | [guides/how-it-works.md](guides/how-it-works.md) |
| Start a new project, in any language | [guides/start-a-new-project.md](guides/start-a-new-project.md) |
| Then set up a new Rust project | [guides/start-a-rust-project.md](guides/start-a-rust-project.md) |
| Bring it into an existing Rust project | [guides/adopt-in-a-rust-project.md](guides/adopt-in-a-rust-project.md) |
| See a project that uses everything | [examples/rust-workspace](examples/rust-workspace) |
| Add another project's practices (or a language) | [guides/fold-in-a-project.md](guides/fold-in-a-project.md) |
| See what's planned and what's missing | [ROADMAP.md](ROADMAP.md) |

Any project gets the git hooks that keep private names and session links
out of its commits; a Rust workspace also gets the lint gate and coverage
floors. The short version for a Rust workspace:

```sh
git clone https://github.com/dorhawk-software/practices
practices/bin/install --templates path/to/your-project
cd path/to/your-project && .practices/bin/install-hooks && .practices/bin/lint
```

Then restart your agent session. Every new session loads the rules (a line
each, from `.practices/rules.md`), reads the detail in `.practices/docs/`,
and has the practices' skills in `.claude/skills/`. Updates arrive weekly
as a pull request the project's own workflow opens.

## The practices

### Any project ([common/](common))

| Practice | In one line |
| --- | --- |
| [Commits](common/commits.md) | The subject is the outcome; the body says why, with numbers and proof; never a session ID. |
| [Changelog](common/changelog.md) | Written by hand for users, in five kinds, in the same commit as the change. |
| [Releases](common/releases.md) | Only a green main is released, by one command; a rehearsal publishes nothing. |
| [Not now](common/not-now.md) | Whatever is deferred is written down in the same commit; stubs fail loudly. |
| [Testing](common/testing.md) | Nothing touches your machine; a journey suite is the regression suite; red before green. |
| [CI and caching](common/ci-caching.md) | GitHub's 10 GB cache and private repos' minutes are budgets, spent on purpose. |
| [CLAUDE.md and docs](common/claude-md.md) | Rules and pointers in CLAUDE.md, detail in docs/engineering, every rule dated. |
| [Privacy in public repos](common/privacy-public-repos.md) | Made-up names, documentation addresses, and a denylist the hooks refuse. |
| [Working rules for coding agents](common/agent-working-rules.md) | Low CPU priority, a disk floor, rebuild before "restart", keep going on a plan. |
| [Gates that can fail](common/gates.md) | A canary proves each check works; findings say how to fix them; audit and list licences from what ships. |
| [Crash reports and logs](common/crash-reports.md) | Consent before the SDK starts; every hook checked in the vendor's code; an allow-list scrubber; expected refusals unreported. |

### UI design ([ui/](ui))

| Practice | In one line |
| --- | --- |
| [Design before build](ui/design-before-build.md) | Proposals with options first; build what was chosen, one to one. |
| [Window buttons](ui/window-buttons.md) | macOS draws its own on the left and the header makes room; the app's own on the right elsewhere. |
| [Keyboard shortcuts](ui/keyboard-shortcuts.md) | Bind the system's main modifier (⌘ on a Mac), and show shortcuts the way the system writes them. |
| [An identity pass](ui/identity-pass.md) | Research the identity first (audience, job, inspiration, market, facts); then three agreed iterations: the identity, a few parts, the whole app re-audited. |
| [Visual craft](ui/visual-craft.md) | Tone, not outlines; matching stroke ends; one drawing per object; compact; an 11 px floor. |

### Rust ([rust/](rust))

| Practice | In one line | Enforced by |
| --- | --- | --- |
| [Lints are errors](rust/lints.md) | Every rustc and clippy lint denied; deny, machete, size and unsafe checks; a pinned toolchain. | `.practices/bin/lint` |
| [Coverage floors](rust/coverage.md) | A floor per crate and OS that only rises; under it, the lines no test ran. | `.practices/bin/check-coverage` |
| [Code conventions](rust/conventions.md) | One workspace; unsafe in one crate; OS code behind small modules; errors as sentences. | review, `check-unsafe`, `check-size` |
| [GPUI desktop apps](rust/gpui.md) | Headless window tests, a journey kit, demo switches, frame timing, macOS window buttons, a patched GPUI kept current. | tests, CI |

### TypeScript ([typescript/](typescript))

| Practice | In one line | Enforced by |
| --- | --- | --- |
| [Lints are errors](typescript/lints.md) | tsc and ESLint, errors only, every file visited; no names that clash by case; 700-line files. | `.practices/bin/lint` |
| [Testing with Jest](typescript/testing.md) | Native modules as in-memory fakes; fakes refuse unset calls; no real network; coverage measured. | tests |
| [Code conventions](typescript/conventions.md) | Braces always; strict types; errors as typed codes; no direct console; patches with a reason, pinned. | ESLint, review |

### Mobile apps ([mobile/](mobile))

| Practice | In one line |
| --- | --- |
| [Releases and the stores](mobile/releases.md) | Version from the tag, build number from the run; a throwaway signing keychain; the artefact checked; promotion by hand. |
| [CI for iOS and Android](mobile/ci.md) | Every platform built before every release; Xcode and toolchains pinned from the project's own files. |
| [Devices and walks](mobile/devices.md) | Never test on a device with real data; done means walked on each platform; field crashes become tests. |
| [Lifecycle and screens](mobile/lifecycle.md) | Modals close on background; single-flight actions; explain before a permission; no layout jumps. |

Coming: `web/` (from the dorhawk.software website). The roadmap lists it.

## What's in the repository

```text
common/            practices for any project
ui/                UI design practices, for any toolkit
rust/              Rust practices, and templates/ to copy into a project
typescript/        TypeScript and JavaScript practices
mobile/            iOS and Android app practices
guides/            how to use all of it, step by step
examples/          a project that uses everything (CI runs the kit on it)
scripts/, hooks/   the kit: synced into a project's .practices/
bin/install        puts the kit into a project, or updates it
templates/         files install copies into any project (update workflow, settings)
actions/           composite GitHub Actions (rust-setup, rust-lint, ...)
.github/workflows/ reusable workflows (rust-ci, practices-update, ...)
plugin/            the Claude Code plugin (skills)
.claude-plugin/    the plugin's marketplace entry
tests/             the kit's own tests and this repository's gate
.githooks/         runs tests/check before each commit
CHANGELOG.md       what changed, per release
ROADMAP.md         what's planned and what's missing
FROM-*.md          what each project brought in, and what's left
CLAUDE.md          how agents work on this repository
```

## The kit, in a project

`bin/install` puts the scripts in the project's `.practices/bin/`. The
same files then run in three places:
- Claude Code's hook, after every cargo build;
- the git hooks, before every commit;
- CI.

Whatever passes locally passes in CI. What the project owns stays its
own: `.practices/config`, its coverage floors and size allowlist, and its
own extra checks in `.practices/lint.d/`.

| Script | Checks |
| --- | --- |
| `lint` | Rust: rustfmt, clippy (all lints errors), cargo deny, cargo machete; JS/TS: `tsc --noEmit` and the npm scripts in `NPM_CHECKS`; then the checks below and `lint.d/` |
| `check-private` | no client names, hosts or addresses in files or messages (fails closed) |
| `check-size` | 700 lines per Rust, TS or JS file, 1000 per test file, a shrinking allowlist |
| `check-unsafe` | `unsafe` only where the project allows it |
| `check-coverage` | tests, measured, against per-crate per-OS floors; never lowered |
| `check-commit-msg` | no session IDs or session links in commit messages |
| `disk-guard` | cargo's build disk keeps a free-space floor |
| `claude-hook` | Claude Code's PostToolUse hook: disk-guard, then the gate after cargo builds and tests, and after npm or npx tests, tsc and ESLint |
| `install-hooks` | points git at `.practices/hooks` (pre-commit, commit-msg) |
| `prune-caches`, `sweep-target` | keep GitHub's 10 GB cache budget for what jobs restore |

In CI, one call does it:

```yaml
jobs:
  rust:
    uses: dorhawk-software/practices/.github/workflows/rust-ci.yml@v1
    permissions: { contents: read, actions: write }
    with: { gpui: true }
    secrets: { private-terms: "${{ secrets.PRIVATE_TERMS }}" }
```

For Claude Code, the plugin brings skills to adopt the kit, fix what the
gate finds, and fold in another project:

```text
/plugin marketplace add dorhawk-software/practices
/plugin install practices@dorhawk-practices
```

## Versions

Tags follow semantic versioning: `v1.2.3`, plus a moving `v1` for the
newest 1.x. Projects reference actions and workflows `@v1` and get
non-breaking changes. Each project pulls kit updates itself, weekly, as a
pull request ([how it works](guides/how-it-works.md#updates-each-project-pulls-its-own)). Changes are in [CHANGELOG.md](CHANGELOG.md).

## Licence

MIT ([LICENSE](LICENSE)). Copy anything into anything.
