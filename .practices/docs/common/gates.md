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

**Check.** Break the check's config once, while the canary is added: the
canary fails the job. Then undo the break, and say so in the commit
([below](#a-new-check-is-broken-once-then-trusted)).

## A rule a machine can check runs where the agent can't skip it

**What.** Every rule a script can decide is enforced by one: after each
edit the agent makes, before the agent's turn ends, before each commit,
and in CI. The script's findings go back to the agent as an error to fix.
Only judgement (taste, wording, whether an icon is ugly) stays with
reviewers.

**Why.** On 2026-11-15 an agent called a design board ready. Five
seconds later the owner saw a button sticking 3 px out of its panel. The
rule was written, and the practices were strict, but every check ran only
when the agent chose to run it, with scripts it wrote itself. Those
measured height and not width, so they passed. An agent's diligence
varies from run to run; a hook doesn't.

**Apply.**
1. Write the check as a script in the kit (`scripts/`), not as advice in a
   Check paragraph. Its message follows
   [the contract below](#every-finding-says-how-to-fix-it).
2. Run it from the hooks the agent can't skip:
   - `claude-hook` after Bash, Edit and Write (Claude Code's
     PostToolUse), where exit 2 hands the findings to the agent;
   - `claude-hook` on Stop and SubagentStop, so the agent can't end its
     turn, or tell the person it's done, while a check fails;
   - `hooks/pre-commit`, and CI for what got past both.

   A slow gate on Stop (minutes of simulator work) skips while its own
   lock is held, since a run is already under way. It also skips when
   nothing it checks changed since its last clean run. On 2026-11-21
   Scops' Stop hook ran its five-minute pixel match at every turn's end,
   while a sub-agent was still building the same screens. It repeated a
   finding already being fixed, and queued simulator runs behind the
   sub-agent's.
3. Wire the hooks in `.claude/settings.json` from the kit's template.
   `check-hooks` fails a commit whose settings drop them.
4. A deliberate exception is written where it applies, with its reason
   (for UI boards, `data-allow="rule: reason"`), and a mark without a
   reason is itself a finding.
5. List the rule in [ENFORCEMENT.md](../ENFORCEMENT.md): gate, partial,
   todo or review.

**Check.** `tests/enforcement` fails when a practice has a rule that
ENFORCEMENT.md doesn't classify. Every gate has a planted fault in
`tests/run`, and was broken once while it was added
([below](#a-new-check-is-broken-once-then-trusted)).

## A new check is broken once, then trusted

**What.** This is what mutation testing means here, and all it means. The
owner, 2026-11-18: "a mutation testing is only done when incorporating a
new rule, and the mutation validates during this addition of that single
rule that the rule catches it, after mutation verifies it works, the
mutation is no longer present and the script with its test (a quick one)
is taken as true". So a check is broken on purpose once, while it is
added or changed, and its test must fail on its own case. Then the break
is undone, and the commit says how it was proven. No break is stored,
and nothing reruns one.

**Why.** Two incidents. On 2026-11-15 an agent proved its new checks by
breaking them by hand, and reported every one "caught". None was: its
copies sat outside a git repository, so every run stopped before
checking, and the refusal counted as a kill. Its baseline read the exit
status of the wrong command. A break needs its guards. Then the breaks
were kept: 141 of them, each a full copy of the kit rerunning the whole
Chrome-backed `tests/run`. A run took about an hour, loaded the owner's
Mac, and ran again in twelve CI shards on every push. After its first
run it proved nothing new. The owner, 2026-11-18: "yea, those mutations
are bullshit", and "stop the local mutation tests, they are pointless".

**Apply.**
1. Plant the fault first: a case in the check's test that the check must
   catch, on its own rule.
2. Run that test unbroken: it passes. Without a green baseline, no
   failure means anything.
3. Break one line of the check, in the working tree, inside the
   repository. Not in a copy outside it: 2026-11-15's copies failed
   before checking.
4. Run the check's test, or only its part (`tests/run check-ui` here).
   It must fail with a line naming the check's own case. A failure for
   another reason (a crash, a refused start, a parse error) proves
   nothing.
5. Capture a command's exit status on the next line, into a variable.
6. Undo the break, and see `git diff` hold only the change you meant.
   The test passes again.
7. Say it in the commit, on a line of its own: `Proven: the gap check
   turned off; tests/run failed on gap.html`.
8. Keep nothing else: no list of breaks, no job that reruns them. The
   check and its quick test are taken as true.

**Check.** `check-proven`, run by `check-commit-msg` (the commit-msg
hook): a commit that adds or changes one of the project's checks
(`CHECK_FILES`) carries a `Proven:` line of three words or more. This
repository's own commit-msg hook sets `CHECK_FILES` to the kit's
scripts and hooks. Whether the break was really tried, and failed on its
own case, is review. Until 2026-11-18 this rule was "A test of a check
is proven by breaking the check": every check kept a mutant in
`tests/mutate`, and CI reran them all on every push.

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

## A subset iterates; only the full run gates

**What.** While working on a few screens, a slow gate may run on just
those (`--only A,B`). Its result says it was partial. Only a full run
counts before a merge, and the hooks always run the full set.

**Why.** In Scops, an iOS app, a change to a column view every screen
shares was meant for one screen (2026-11-22). It made another screen
fade its text at the standard size. Only the full screenshot match
caught it: 9 findings, a lead 5 pt off. A run of the screen being worked
on would have passed.

**Apply.**
1. Give a slow gate a way to run a few of its items by name. An unknown
   name fails, listing the names it knows.
2. A partial run says so in its result: "Only 3 of 51 boards: only a
   full run counts before a merge".
3. The commit hook and the Stop hook run the full set. Nothing is merged,
   or called matching, on a partial run.

**Check.** Review: a claim that rests on a partial run is in the
conversation. The hooks' full run is the gate.

## Cache by content, never by time

**What.** A slow gate reuses an earlier result only under a key made from
everything that result is made of. Never by a file's age or a timestamp.
A changed input gives a new key, and the work is done again.

**Why.** Scops' screenshot match took 8.8 min, 71 % of it shooting the
app on a simulator. The owner asked: "Is it necessary to shoot 51
screens when only like 2 change?" (2026-11-23). Keyed by content, a run
with the app unchanged takes 35 s and boots no simulator. A change to a
board's text alone is caught in 37 s.

**Apply.**
1. A design render's key: a hash of the board, every file it loaded (as
   the browser's network log records them; a web font by its versioned
   URL), the rendering script, and the browser's and system's versions.
   A web font that doesn't load fails the render, so a fallback face is
   never kept.
2. An app shot's key: a hash of the app as built and its test runner,
   every file, with code signatures and the linker's per-build UUID
   (`LC_UUID`) left out. Add the scenario, the device type, the runtime
   and its build, the text size, the appearance and the shooting code.
3. Hash what was built, not the sources. A comment or a touched file
   then costs nothing, and a changed resource can't be missed.
4. While no key changed, nothing boots: no simulator, no browser.
5. The cache lives outside the repository, on the build disk. An entry
   no run used for 14 days goes; `--fresh` redoes everything.

**Check.** Before the cache is trusted, prove it and say so in the commit:
- a design-only change is caught without the simulator (a board's word
  changed: 5 findings, no simulator booted);
- a one-line code change forces every shot again, and fails (a 1 pt
  padding: 51 screens shot, 2 findings);
- a kept file is byte-identical to the run it came from;
- two builds of the same tree give the same app hash (every source
  touched in between).

A hash that differs when nothing changed costs a reshoot, never a wrong
reuse. It happened once there, after a rebase.

## A slow gate prints each phase's time

**What.** A gate that takes minutes prints how long each phase took, and
the total. A claim about its speed quotes those lines.

**Why.** "Please time it, i would like to see some optimizations of the
process" (the owner, 2026-11-23). The phases showed where Scops' 8.8 min
went: shots 71 %, renders 16 %. Each change after that was measured, not
guessed: a render pass from 86 s to 10 s, a shot from 7.47 s to 1.26 s.

**Apply.** Print a line per phase (render, lock wait, build, shots,
match) and a total. The wait for a shared lock gets its own line: it is
another run's time. Quote a duration with where it was measured, as for
CI ([agent-working-rules.md](agent-working-rules.md#quote-measured-ci-timings)).

**Check.** todo: the kit's own slow gates (check-ui, check-ui-tests)
don't print their phases yet.

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

## A size limit is met by splitting, never by trimming

**What.** When a file at its size limit needs more, split it first.
Never shorten what users read, drop a plural form or pick another
implementation just so the change fits.

**Why.** Obálka's strings file sat exactly at its allowlist count. A
weekday under each day number came from the platform's calendar instead
of the app's strings ([26ff3d5](https://github.com/dorhawk-software/obalka/commit/26ff3d5)). A
freshness line folded "před 5 minutami" into "před 5 min". The gate
passed both times; the wording and the design paid.

**Apply.** Before adding to a file in `.practices/size-allowlist`, look
at its room. With none, split it in the same commit (a strings file by
feature, a screen by its parts), lower its count, then make the change.

**Check.** No commit that adds to a capped file also shortens user text
or drops a plural form; the reviewer reads the diff of that file.

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
