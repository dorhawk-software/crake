# Start a new project

Everything a new repository gets on its first day, in order, so the rules
hold from the first commit instead of being adopted later. This page is
for a project in any language. What each language adds (its build, lints,
coverage and CI job) is step 7, on the language's own page.

Adopting the rules later costs far more: see
[adopt in a Rust project](adopt-in-a-rust-project.md) for what that takes.

## 1. The repository and who commits

```sh
mkdir acme-app && cd acme-app
git init -b main
git config user.email you@your-domain.example   # this repository's identity, set here
git config user.name "Your Name"
```

- One commit identity per repository, set in the repository's own config,
  not taken from whatever the machine's global one is
  ([common/commits.md](../common/commits.md)).
- Decide now whether the repository is public. A public one needs the
  privacy setup in step 3 before anything else is written. A private one
  pays for CI minutes (step 5).
- Write down who may push, and when ("once work is done and tested"), in
  CLAUDE.md. An agent never assumes it.

## 2. CLAUDE.md: rules and pointers

CLAUDE.md is read whole by every agent request, so it holds the rules and a
one-line pointer per area. The detail goes to `docs/engineering/`
([common/claude-md.md](../common/claude-md.md)).

```markdown
# Acme App

One paragraph: what it is, who it's for, what it never does.

## Git rules
- Commit as the repository's configured identity.
- No session IDs or session links in commits (the commit-msg hook refuses them).
- Push once work is done and tested.

## Read first
- docs/engineering/: the detail behind each section here.

## Practices (github.com/dorhawk-software/practices)

The shared practices, a line per rule:
@.practices/rules.md

This project's own settings:
- (Its coverage floors, size allowlist, CI budget: only what differs.)
```

The `@.practices/rules.md` line loads every shared rule into each session
(step 4's `bin/install` adds it when it's missing). Don't copy shared
rules into CLAUDE.md: the copy goes stale when the practices change.

Every rule records where it came from and when: "(asked for 2026-11-11)".
An agent can then tell a hard rule from a past choice.

## 3. Privacy, before the first real name

Even a private repository may go public one day, and examples get copied.

```sh
printf 'CLAUDE.local.md\ndev/private-terms\n' >>.gitignore
mkdir -p dev
printf '# names never to commit (clients, hosts, addresses), one PCRE pattern per line\n' >dev/private-terms
```

- Code, tests, demo data and docs use made-up names (ACME, Globex, Initech),
  `*.example` and `*.internal` hosts, and documentation addresses
  (192.0.2.x, 198.51.100.x, 203.0.113.x).
- What you learn on real systems goes in `CLAUDE.local.md`, which git
  ignores.
- The hooks (step 4) refuse any commit, or commit message, that matches
  `dev/private-terms`
  ([common/privacy-public-repos.md](../common/privacy-public-repos.md)).
- Nothing private at all? Set `PRIVATE_TERMS=none` in `.practices/config`
  instead. The check never decides that on its own.

## 4. The git hooks

The kit's language-neutral part guards every commit, in any language:

```sh
git clone https://github.com/dorhawk-software/practices ~/src/practices
~/src/practices/bin/install .
.practices/bin/install-hooks
```

- **pre-commit:** no name from `dev/private-terms` in any file
  (`check-private`). With no terms file it fails rather than passing
  unchecked.
- **commit-msg:** no session IDs or session links, and no private names, in
  the message (`check-commit-msg`;
  [common/commits.md](../common/commits.md)).
- Neither runs a compiler or needs more than git and bash. When Rust files
  are committed, pre-commit also runs the Rust gate; when JavaScript or
  TypeScript files are, the JS/TS gate and the tests (step 7).
- Commit `.practices/` whole, with `.claude/skills/` and
  `.github/workflows/practices.yml`; `install-hooks` runs once per clone
  ([how it works](how-it-works.md)).
- `bin/install` also gives agents the rules (`.practices/rules.md`), the
  docs (`.practices/docs/`) and the skills (`.claude/skills/`). Restart
  the agent session afterwards: a running one doesn't load them.
- `.github/workflows/practices.yml` pulls new releases weekly as a pull
  request. Allow GitHub Actions to create pull requests in the
  repository's settings (Settings, Actions, General).

## 5. CI: decide what it costs

The rules hold on your machine first: the hooks are the real gate, and CI
checks again on a clean machine
([common/ci-caching.md](../common/ci-caching.md)).

- **Public repository:** standard runners are free. Add a job for each
  system you ship for.
- **Private repository:** minutes are a budget. One Linux job,
  `paths-ignore` for docs, and the local hooks as the gate. Other systems
  are tested on your own machines (or a self-hosted runner) until a
  release needs more.
- Secrets: `PRIVATE_TERMS` (the same patterns as `dev/private-terms`) so CI
  checks names too. Without it, CI says it skipped the check.
- The workflow itself comes from your language's page (step 7).

## 6. CHANGELOG.md and not-now.md

```markdown
# Changelog

## Unreleased

### New
### Better
### Changed
### Fixed
### Underneath
```

Written by hand, for the people using the app, in its own voice. Every
user-facing change adds a line in the same commit, dependency updates too
([common/changelog.md](../common/changelog.md)). Releases follow
[common/releases.md](../common/releases.md).

```sh
mkdir -p docs/engineering
printf '# Not now\n\nWhat we decided not to do yet, and why. Done items leave the list.\n' >docs/engineering/not-now.md
```

From the first day: whatever is decided "not now" is written down here, or
next to the code, in the same commit ([common/not-now.md](../common/not-now.md)).

## 7. Your language's setup

| Language | Its page | What it adds |
| --- | --- | --- |
| Rust | [Start a Rust project](start-a-rust-project.md) | workspace, lints as errors, cargo deny, pinned toolchain, coverage floors, file size and unsafe checks, the CI workflows, the Claude Code hook |
| TypeScript / React Native | [typescript/lints.md](../typescript/lints.md#how-practicesbinlint-runs-it), then [mobile/](../mobile/ci.md) for an app | `tsc --noEmit` and the npm scripts in `NPM_CHECKS` as the gate, the tests on commit, file sizes; no setup page or CI workflow yet ([ROADMAP.md](../ROADMAP.md), phase 7) |
| Websites | not yet | comes from the dorhawk.software site; see [ROADMAP.md](../ROADMAP.md), phase 7 |

For a language with no page yet, steps 1 to 6 still apply whole. Its own
checks can go in `.practices/lint.d/` until the practices have them
([fold in a project](fold-in-a-project.md) is how a language gets added).

## 8. The first commit

```sh
git add -A
git commit -m "Acme App: <what it is, in one sentence>" -m "<why it exists, what it does first>"
```

Both hooks read it: the files for private names, the message for session
links and private names ([common/commits.md](../common/commits.md)).

## Checklist

Every project:

- [ ] `git config user.email` set in this repository
- [ ] Public or private decided; push permission written in CLAUDE.md
- [ ] CLAUDE.md with rules and pointers; `docs/engineering/` exists
- [ ] `.gitignore` has `CLAUDE.local.md` and `dev/private-terms`
- [ ] `dev/private-terms` exists (or `PRIVATE_TERMS=none` in `.practices/config`)
- [ ] `bin/install` run; `.practices/` committed; `.practices/bin/install-hooks` run
- [ ] CI's cost decided: free public runners, or one Linux job with `paths-ignore`
- [ ] `PRIVATE_TERMS` secret set in the repository
- [ ] `CHANGELOG.md` with Unreleased and the five kinds
- [ ] `docs/engineering/not-now.md`
- [ ] The first commit goes through both hooks

Then your language's checklist, on its page (step 7).
