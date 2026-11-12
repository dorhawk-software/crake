# Gates that can fail

A check that can't fail protects nothing, and a check whose findings
nobody can act on gets switched off. These rules make every gate (lint,
scanners, audits, licence lists) prove it works and say how to fix what it
finds. They apply to any language.

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka)
and [Crake](https://github.com/dorhawk-software/crake).

## Prove the gate can fail

**What.** Every scanner or check has a canary: a planted bad input that
the check must catch, run next to the real check. If the canary passes,
the job fails.

**Why.** Obálka's secret scanner went green while checking nothing: its
action ignored an unknown input and dropped the custom rules, and its
stock rules missed a planted crash-reporting token. In Crake, a privacy
check let every commit through on macOS until 2026-10-26.

**Apply.**
1. Write the canary as a fake value of the exact shape the check looks for.
2. Run it with the same binary and config as the real scan. Obálka's
   canary installed a different version, so it proved a different tool.
3. Pin scanners and their actions by commit, like any third-party action
   ([ci-caching.md](ci-caching.md#pin-third-party-actions-by-commit)).

**Check.** Break the check's config on a branch: the canary fails the job.

## Every finding says how to fix it

**What.** A check's message says what it found, the fix, where the system
grows if no fix fits, and why the rule exists.

**Why.** Obálka's design-system lint rules (spec 028) took 750 off-scale
values to zero in a day. Every message has the same four parts, so an
agent or a person fixes the finding from the message alone, instead of
switching the rule off.

**Apply.** A message contract, tested:
```text
Spacing 13 isn't on the scale.
Fix: use space.md (12) or space.lg (16).
Not here? Add the step to the theme and DESIGN.md.
Why: one spacing scale keeps screens consistent.
```

**Check.** A test asserts each rule's whole message.

## A rule is an error, or it's off with a reason

**What.** No warnings. Each rule is an error, or it's turned off with a
comment giving the count of hits and the reason. A disable comment in code
says why.

**Why.** Warnings are noise: Obálka had 211 that nobody acted on, hiding
ten stale disables, until an audit on 2026-09-09.

**Apply.** See [typescript/lints.md](../typescript/lints.md) and
[rust/lints.md](../rust/lints.md) for each language's settings.

**Check.** The config has no `warn` level.

## The gate visits every file

**What.** A gate covers every file of the kinds it checks, including dev
scripts. A file it can't parse fails the gate.

**Why.** Five of Obálka's scripts were parse errors the linter never
visited; it reported zero.

**Apply.** Once, plant an error in each kind of file and run the gate.
Count the files it reports checking against `git ls-files`.

**Check.** The gate's file count matches the repository's.

## A ratchet for a rule you can't meet yet

**What.** When a new rule has many findings, commit a baseline of counts.
The gate fails when a count rises, and also when it falls until the
baseline is lowered. At zero, delete the baseline; the rule is a plain
error.

**Why.** Obálka's design lint started with a baseline and reached zero the
same day. Failing on a fall keeps the baseline honest: a fix that isn't
recorded would let the count climb back.

**Apply.** The size allowlist (`.practices/size-allowlist`) is the same
idea for file lengths. Use it for any countable rule.

**Check.** Fix one finding: the gate asks you to lower the baseline.

## Audit only what ships; re-check every accepted advisory

**What.** A dependency audit blocks on advisories in code that ships, not
in build tools. An accepted advisory carries a written argument and a
check that runs each time and fails once the argument stops holding.

**Why.** Plain `npm audit` failed on build tools that never ship, so people
learned to ignore it. Obálka's audit compares each advisory with the
shipped copy: a vulnerable parser was the CLI's old copy, not the app's.
It prints what it can't see (native and vendored code) on every run.

**Apply.**
1. Get the shipped list from the build itself (a bundle's source map),
   not from the package manager's tree.
2. Store each accepted advisory with its argument and a `voidWhen` check.
3. Print the blind spots.

**Check.** Move a vulnerable package into the shipped list: the audit
fails.

## Licence notices from what ships; anything unknown fails

**What.** The app's licence notices list exactly what ships, taken from
the build. Components the tool can't see (native libraries, fonts,
vendored code) are listed by hand. Anything it can't resolve is an error.

**Why.** Two typefaces shipped with no notice in Obálka. The package
manager listed 717 packages; the bundle's source map showed 170 really
ship. A generated file, committed and diffed in CI, catches drift.

**Apply.** Generate, commit, and check in CI that regenerating changes
nothing. Keep each licence's text in the repository: a licence with no
text there fails, which works as an allow-list.

**Check.** Add a dependency without regenerating: CI fails.
