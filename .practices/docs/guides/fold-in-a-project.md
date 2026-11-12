# Fold in a project

How a project's hard-won practices come into this repository. It's the
method used on Crake (564 commits, a month of work) in November 2026, written
so it can be run again on Obálka (a React Native app) and on the
dorhawk.software website.

The idea: **the commit history knows why each practice exists.** A rule
without its incident is an opinion. So the audit reads the files *and* the
git history, and every practice that comes over carries the incident that
taught it.

## 1. Audit, in seven parts

Six areas, each audited on its own (in parallel, by agents or by people),
plus one source only the agent has.

| # | Area | What to cover |
| --- | --- | --- |
| 1 | CI | workflows, caching, runners, matrices, secrets, timeouts, update bots, cost |
| 2 | Lint and verification | linter config, custom checks, hooks, what runs where and how long it takes |
| 3 | Testing | the kinds of test, harnesses, fixtures, demo modes, coverage, flakes |
| 4 | Release and packaging | changelog, versioning, release scripts, stores and packages, signing |
| 5 | Commit-history conventions | message style, rules the user stated (with dates), bug classes and their guards, reverts |
| 6 | Code conventions | layout, error handling, logging, platform code, dependencies, performance |
| 7 | The agent's memory | the rules the user gave the agent while working (`~/.claude/projects/<project>/memory/`) |

Part 7 matters: in Crake, about 15 working rules (low CPU priority for
builds, the disk floor, "release only on a green main", "keep going
overnight") existed only there. They became
[common/agent-working-rules.md](../common/agent-working-rules.md).

### The brief for each area

Give each auditor the same frame, with only the area changed:

```text
CONTEXT: <path> is <what the project is>: <N> commits, <dates>, <public or
private>. The owner wants its development practices extracted into a public
"practices" repository: common conventions, language-specific conventions,
and reusable CI. The next project to adopt them is <name>. Audit ONE area.

PRIVACY (hard rule): don't read CLAUDE.local.md or dev/private-terms. If a
file or commit reveals a client, a real host, a non-documentation address
or a person's name, never quote it: write "[private]". Your output feeds a
public repository.

METHOD: read the current files AND the git history for your area
(git log --follow -p -- <file>, git log -S, git log --grep, git show).
For each piece, find out why it exists, what broke, what was tried and
reverted, and the rules the user stated, with dates. Don't modify anything.

REPORT (at most ~1500 words, no file dumps). For each practice:
- what it is (path)
- why it exists (the incident, with a commit hash)
- generality: any project | <language>-generic | <framework> | project-only
- extraction form: doc convention | script | composite action |
  reusable workflow | config template | package/tool
- what would need parameterising
- maturity and known problems
End with a ranked "copy first" list.

YOUR AREA: <one row of the table above, with the files to start from>
```

### Put the findings together

One document for the person who decides:

- a table of what the project has, per area, with maturity and generality;
- the repository shape the findings call for;
- the first slice to copy, as a checklist;
- the gaps found in the source project's own harness (they must not be
  copied: in Crake, a privacy check that silently passed in CI);
- what stays in the source project, written up only as a pattern;
- the decisions for the person, each with a recommendation.

Nothing gets built until the person has decided.

## 2. Copy verbatim, in one commit

Copy the files of the first slice unchanged, with a note saying where they
came from (`FROM-CRAKE.md` here). One commit, nothing else in it.

**Why:** the next commit's diff then shows exactly what changed to make each
file general. A reviewer can see that nothing was lost.

## 3. Generalise, with tests

The next commit makes the files work for any project:

- project names, paths and lists become settings with defaults;
- every gap the audit found is fixed, each with a test that failed before;
- the scripts run on every system the projects use (bash 3.2 and BSD tools
  on macOS, GNU on Linux);
- every check fails closed: what it can't check is refused, never "clean".

`tests/run` is where the tests go. Test the checkers: a checker with a hole
lets through exactly what it exists to stop.

## 4. Prove it on the source project first

Switch the source project to the shared versions before anyone else uses
them. Its CI already catches what it catches; if the switch loses
anything, it shows there, on code that's known. Only then tag a release.

## 5. Adopt in the next project

Follow [adopt in a Rust project](adopt-in-a-rust-project.md) (or its
equivalent for the new language). Whatever the second project finds is
fixed here, with a test, and synced back to the first.

## Adding a new language

Mirror `rust/`:

```text
typescript/                 (from Obálka)
  conventions.md            layout, errors, logging, platform code
  lints.md                  ESLint/Biome config, type-check as an error, unused exports
  coverage.md               floors per package, never lowered
  react-native.md           the framework's own practices (like rust/gpui.md)
  templates/                config files a project copies
web/                        (from the dorhawk.software website)
  conventions.md  deploy.md  templates/
```

- Scripts for the language go in `scripts/` with a prefix only when they
  clash (`lint` stays the one entry point and runs what the project has:
  `Cargo.toml` → the Rust checks, `package.json` → the TypeScript ones).
- Shared actions get a language prefix: `actions/ts-setup`, `actions/ts-lint`.
- A practice that turns out to hold in every language moves to `common/`.
- A lesson about the interface (layout, platform conventions, the design
  process) goes to `ui/`, whatever the toolkit; its toolkit how-to stays in
  the language folder and links to it.
- Each new folder gets a line in the README's map and in
  [ROADMAP.md](../ROADMAP.md).
