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
| [It just works](common/it-just-works.md) | Install it and it works: the product decides from signals, says why, and can undo; settings are preferences, never workarounds. |
| [Commits](common/commits.md) | The subject is the outcome; the body says why, with numbers and proof; never a session ID. |
| [Changelog](common/changelog.md) | Written by hand for users, in five kinds, in the same commit as the change. |
| [Releases](common/releases.md) | Only a green main is released, by one command; a rehearsal publishes nothing. |
| [The product's voice](common/voice.md) | The product speaks as itself or names who acts, never "we" (a companion's inclusive "we" is marked); each language's grammar checked by its own rules (`check-voice`, `check-copy`). |
| [Not now](common/not-now.md) | Whatever is deferred is written down in the same commit; stubs fail loudly. |
| [Testing](common/testing.md) | Nothing touches your machine; a journey suite is the regression suite; red before green; no retries, except a renderer that gave no answer, asked once more. |
| [CI and caching](common/ci-caching.md) | GitHub's 10 GB cache and private repos' minutes are budgets, spent on purpose. |
| [CLAUDE.md and docs](common/claude-md.md) | Rules and pointers in CLAUDE.md, detail in docs/engineering, every rule dated. |
| [Privacy in public repos](common/privacy-public-repos.md) | Made-up names, documentation addresses, and a denylist the hooks refuse. |
| [Working rules for coding agents](common/agent-working-rules.md) | Low CPU priority, a disk floor, rebuild before "restart", keep going on a plan; one simulator at a time until the machine is measured; no edits to a script while it runs. |
| [Gates that can fail](common/gates.md) | A canary proves each check works; findings say how to fix them; a subset iterates and only the full run gates; caches keyed by content; each phase timed; a size limit is met by splitting; audit and list licences from what ships. |
| [Crash reports and logs](common/crash-reports.md) | Consent before the SDK starts; every hook checked in the vendor's code; an allow-list scrubber; expected refusals unreported. |

### UI design ([ui/](ui))

| Practice | In one line |
| --- | --- |
| [Design before build](ui/design-before-build.md) | Proposals with options first, each page through a checklist before review (`check-page`); build what was chosen, one to one, measured and checked pixel by pixel, shot in one launch; off-scale values go to the owner; every fact a screen uses was entered earlier. |
| [Window buttons](ui/window-buttons.md) | macOS draws its own on the left and the header makes room; the app's own on the right elsewhere. |
| [Drag and drop](ui/drag-and-drop.md) | One calm panel over the target says what dropping does; the resting hint fades; nothing glows. |
| [Feedback while it works](ui/feedback.md) | Work over a second shows the product's own motion until its result shows, and rests with reduced motion; a pressed control says what is happening ("Opening…"), shows the motion and takes no more presses until the outcome shows. |
| [Keyboard shortcuts](ui/keyboard-shortcuts.md) | Bind the system's main modifier (⌘ on a Mac), and show shortcuts the way the system writes them. |
| [An identity pass](ui/identity-pass.md) | Research the identity first (audience and their emotional state, job, inspiration, market, facts); then three agreed iterations: the identity, a few parts, the whole app re-audited from an inventory proven complete. |
| [Visual craft](ui/visual-craft.md) | Tone, not outlines; depth from the product's own concept; one reading only; 8 px between targets; labels on one line; both safe areas in the frame; a sheet's page inert; compact; facts ranked; an 11 px floor; no frame of a resize shows squashed words. |
| [Dates, times and numbers](ui/pickers.md) | Picked, never typed, with what's derived recomputed; calendars jump to a year and show neighbouring days; suggestions never fence. |
| [Consent without pressure](ui/consent.md) | Never pre-selected; two equal answers, no guilt; what is sent, said next to them; one tap to change your mind. |

### Rust ([rust/](rust))

| Practice | In one line | Enforced by |
| --- | --- | --- |
| [Lints are errors](rust/lints.md) | Every rustc and clippy lint denied; deny, machete, size and unsafe checks; a pinned toolchain. | `.practices/bin/lint` |
| [Coverage floors](rust/coverage.md) | A floor per crate and OS that only rises; under it, the lines no test ran. | `.practices/bin/check-coverage` |
| [Code conventions](rust/conventions.md) | One workspace; unsafe in one crate; OS code behind small modules; errors as sentences. | review, `check-unsafe`, `check-size` |
| [GPUI desktop apps](rust/gpui.md) | Headless window tests, a journey kit, demo switches, frame timing, macOS window buttons, rounded corners GPUI doesn't clip, a patched GPUI kept current. | tests, CI, `check-gpui-clip` |

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
| [App Review](mobile/app-review.md) | Each rejection becomes a rule and a check before resubmitting; every binary carries the entitlements its frameworks need; one device family; "Continue", never "Allow", before a permission prompt; every screen fits an iPhone SE at large text; UI tests audited and in a pinned language. |
| [Lifecycle and screens](mobile/lifecycle.md) | Modals close on background; single-flight actions; explain before a permission; no layout jumps; text takes its container's width; every string names its face; a control fades as one piece. |

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
ENFORCEMENT.md     every rule, and the gate that enforces it (or why none can)
FROM-*.md          what each project brought in, and what's left
CLAUDE.md          how agents work on this repository
```

## The kit, in a project

`bin/install` puts the scripts in the project's `.practices/bin/`. The
same files then run in four places:
- Claude Code's hook, after every cargo build and every edit to a design
  board, where findings go back to the agent to fix;
- Claude Code's Stop hook, so an agent can't end its turn while a board
  it changed fails;
- the git hooks, before every commit;
- CI.

Every rule a script can check is enforced this way, so no agent can skip
it; [ENFORCEMENT.md](ENFORCEMENT.md) says where each rule stands.

Whatever passes locally passes in CI. What the project owns stays its
own: `.practices/config`, its coverage floors and size allowlist, and its
own extra checks in `.practices/lint.d/`.

| Script | Checks |
| --- | --- |
| `lint` | Rust: rustfmt, clippy (all lints errors), cargo deny, cargo machete; JS/TS: `tsc --noEmit` and the npm scripts in `NPM_CHECKS`; then the checks below and `lint.d/` |
| `check-private` | no client names, hosts or addresses in files or messages (fails closed) |
| `check-voice` | no "we" in the copy people see (`VOICE_FILES`), English and Czech; reads strings, not comments or keys |
| `voice-audit.py` | check-voice's reading half (python3, no packages); with `--strings`, every string for check-copy |
| `check-copy` | each language's grammar in the copy (boards and `VOICE_FILES`): "V 10:20", "ve středu", "al", "an hour"; copy that names no language, or a language with no rules file, fails; a fact (`data-fact`) reads the same on every board |
| `copy-audit.mjs` | check-copy's reading half (Node, no packages) |
| `copy-rules` | the rules per language, `<lang>.mjs` (cs, en, es): checks of the grammar between words, and the words no button may say alone; a project adds its own in `.practices/copy-rules/` |
| `check-permission-words` | no "Allow", "Enable" or "Turn on" on a button before a permission prompt, in the copy and the code (English, Czech, Spanish, Vietnamese); a string catalog's stale entries skipped |
| `permission-audit.py` | check-permission-words' reading half (python3, no packages; reads strings with voice-audit.py) |
| `check-xcode` | each Xcode target that ships: built for the declared device family, and the entitlement for every gated framework its sources import (Family Controls, HealthKit, HomeKit); reads `project.pbxproj`, no Xcode needed |
| `check-ios-artefact` | a built `.ipa`, `.app` or `.xcarchive`: each bundle's `UIDeviceFamily`, and each binary's linked frameworks (`otool -L`) against its signed entitlements (macOS) |
| `xcode-audit.py` | check-xcode's and check-ios-artefact's reading half (python3, no packages) |
| `check-ui-tests` | XCUITests that pin the app's language, run Xcode's accessibility audit, and run on an iPhone SE simulator at a large text size |
| `check-shots` | a screen shot after a live switch (theme, language, text size) against the same screen after a fresh start: every region that differs, with `--skip-top` for the status bar; `shots-diff.mjs` measures |
| `shots-diff.mjs` | check-shots' measuring half (Node, no packages) |
| `check-size` | 700 lines per Rust, TS or JS file, 1000 per test file, a shrinking allowlist |
| `check-unsafe` | `unsafe` only where the project allows it |
| `check-gpui-clip` | in a GPUI app, no element that both rounds its corners and clips its children (GPUI clips square); `// practices-allow clip: <reason>` for a clip kept on purpose |
| `gpui-clip-audit.py` | check-gpui-clip's reading half (python3, no packages): Rust read as builder chains, comments and strings skipped |
| `check-coverage` | tests, measured, against per-crate per-OS floors; never lowered |
| `check-commit-msg` | no session IDs or session links in commit messages; a commit that changes a design board names its before/after page; a commit that changes a check says how it was proven (`check-proven`) |
| `check-proven` | a commit that adds or changes one of the project's checks (`CHECK_FILES`) has a `Proven:` line: the check broken once, its case seen failing |
| `check-ui` | design boards (`design/**/*.html`), rendered in headless Chrome at each width, light and dark: nothing past its surface or cut off, buttons that follow their column and name their outcome, 8 px between targets, 11 px text, rings drawn with outline, shadows away from the light, 16 px from a ring to text's ink, centring, states that don't move content, icons on buttons, like controls alike, labels on one line, both safe areas of a phone frame, a page behind an open sheet marked inert, contrast against the pixels behind the text, and each mark (`data-mark`) at its smallest size: no stroke under 1 px, no leftover piece of a cut, the brand element 10 px or taller; a busy control (`aria-busy="true"`) disabled, without a pointer cursor, its words ending in "…" and something in it moving; nothing still looping with reduced motion; no words scaled out of their shape, now or by a `@keyframes` rule |
| `check-page` | proposal and review pages (`design/proposals/`, `design/changes/`), read at 375 and 390 px as the owner reads them on a phone: nothing scrolls sideways, no one-letter word ends a line where its language ties it (an option's letter too), no image past the screen or drawn wider than its pixels at 2x, the doctype, the charset and the language there |
| `page-audit.mjs` | check-page's measuring half (Node, no packages; drives Chrome with ui-chrome.mjs) |
| `ui-audit.mjs` | check-ui's measuring half (Node, no packages; drives Chrome over a pipe) |
| `ui-chrome.mjs` | check-ui's Chrome: started headless, spoken to over a pipe, its first page opened or retried, each board loaded under the media features asked for |
| `ui-board.mjs` | check-ui's tables: each rule's doc and fix, a board's own width, whether it has dark rules |
| `ui-pixels.mjs` | check-ui's pixels: Chrome's screenshots decoded, contrast measured on gradients and drawings |
| `ui-mark.mjs` | check-ui's mark check: each `svg[data-mark]` drawn alone and large, measured from its pixels at the smallest size the board shows |
| `ui-type.mjs` | check-ui's type checks: each text drawn in the face it asks for, never a fallback (Chrome's own report of the fonts it used), and times and counts in tabular figures |
| `ui-motion.mjs` | check-ui's motion check: busy controls, with reduced motion and again with motion allowed, loops that don't stop for reduced motion, and words scaled out of their shape |
| `check-hooks` | `.claude/settings.json` still runs `claude-hook` after edits and when a turn ends |
| `disk-guard` | cargo's build disk keeps a free-space floor |
| `claude-hook` | Claude Code's hooks. After Bash, Edit and Write: disk-guard; the lint gate after cargo builds and tests, and after npm or npx tests, tsc and ESLint; check-ui on any board that changed, check-page on any proposal page that changed, check-voice and check-copy on any copy that changed, check-permission-words on copy or code, check-xcode and check-ui-tests on an Xcode project, entitlements or Swift; check-ios-artefact after `xcodebuild archive` or `-exportArchive`. On Stop: the turn can't end while a changed file fails a gate |
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
