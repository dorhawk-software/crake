# Commits

A commit says what changed for the person who uses or works on the project,
and why, in plain words. The log is the project's incident record. Write
each body as if someone will read it a year from now to learn why the code
is the way it is, because someone will.

## The subject is the outcome

**What.** One line, under about 70 characters. It says what the user or
developer sees now, in plain words: "Refuse tunnels that can't be up
together", "A tunnel that stops by itself says so, and why". There is no
`feat:` or `fix:` prefix. Infrastructure work may name its area first:
`CI:`, `Release:`, `Tests:`, `Scripts:`.

**Why.** Crake's 564 subjects read as a changelog of behaviour. A prefix
says what kind of change it was, but the sentence after it still has to
say what changed. For about a week Crake used area prefixes on everything
(`crake:`, `ipv6(3/n):`). Outcome sentences replaced them because they
read better in `git log --oneline` and in release notes.

**Apply.** Before committing, ask "what does someone notice now?" and
write that. Name a file or function only when the change is purely
internal.

**Check.** In review, read `git log --oneline` aloud. Every line should
make sense to someone who hasn't seen the diff.

## The body says why, with numbers, and how it was proven

**What.** A few short paragraphs:
- what was wrong, with the incident's numbers ("the .deb took 17 min, not
  7");
- why this change fixes it;
- how it was proven ("the test failed on the old code and passes now",
  "each check fails under its mutation");
- what was found along the way;
- what was deliberately left for later, and where that is written down.

**Why.** Crake has no `git revert` commits. Every course change is a
forward fix whose body explains the earlier mistake. Example:
[a31caa9](https://github.com/dorhawk-software/crake/commit/a31caa9) took
CI from 35 minutes back down, and its body is the only place the reasoning
behind "caches saved by main only" lives.

**Apply.** Paste the measurement, not an adjective. Name the test that
proves the change. Point at `docs/engineering/not-now.md` for anything
deferred (see [not-now.md](not-now.md)).

**Check.** In review: can someone tell, from the body alone, why this was
needed and how we know it works?

## Co-Authored-By yes, session IDs never

**What.** Commits made with a coding agent end with a `Co-Authored-By:`
trailer. They never carry a session ID, a session URL or a
`Claude-Session:` trailer. The same goes for pull request descriptions.

**Why.** A session link in a public log points at a private conversation.
It's noise for every reader, and it may expose what the session discussed.
This has been a rule since Crake's first commit
([0b63fbb](https://github.com/dorhawk-software/crake/commit/0b63fbb):
"Never, under any circumstances"). In tern, 44 commits had picked up
`Claude-Session:` trailers before any check existed.

**Apply.** Install the hooks once per clone: `.practices/bin/install-hooks`
(sets `core.hooksPath=.practices/hooks`).

**Check.** The `commit-msg` hook runs `.practices/bin/check-commit-msg`.
It refuses `Claude-Session:` trailers and claude.ai session URLs, and it
runs the private-terms check on the message. `.practices/bin/lint` fails
when the hooks aren't installed.

## Stable IDs survive a rewritten history

**What.** Subjects and docs cite stable IDs from the project's own records (a spec number, a requirement, a task) next to commit hashes: "Keep the password date in step (030 FR-008)".

**Why.** Obálka squashed its history before going public. Every commit hash its specs cited died; the spec, requirement and task IDs survived ([754d5ca](https://github.com/dorhawk-software/obalka/commit/754d5ca)).

**Apply.** Give each feature record an ID. Put it in brackets at the end of the subject when the commit implements part of it.

**Check.** `git log --oneline | grep -c '([0-9]\{3\}'` grows with the features.

## Trailers come last

**What.** `Co-Authored-By:` and any other trailer are the last lines of the message, after one blank line. Nothing follows them: no sign-off block, no signature.

**Why.** Git reads trailers only from the final paragraph. Obálka's messages ended with the owner's name and address after the trailer, so `git interpret-trailers --parse` found no trailer in any of its 26 commits. The author field already carries the identity. The owner chose to drop the block (2026-11-12).

**Apply.** End the body, a blank line, then the trailers.

**Check.** `git log -1 --format=%B | git interpret-trailers --parse` prints the `Co-Authored-By` line.

## One commit identity per repository

**What.** Each repository sets its own `user.email` (and `user.name`), so
every commit there carries the same identity, whatever the machine's global
setting is.

**Why.** Work and personal accounts live on the same machine. A global
default leaks the wrong address into a public log, and you can't take it
back without rewriting history. Set in Crake from its first commit
([0b63fbb](https://github.com/dorhawk-software/crake/commit/0b63fbb)).

**Apply.**

```sh
git config user.email "<the project's address>"
git config user.name  "<name>"
```

Write the address the project uses in its CLAUDE.md "Git rules", so an
agent doesn't guess.

**Check.** `git log --format=%ae | sort -u` prints one line.

## Pushing is stated, not assumed

**What.** CLAUDE.md says whether an agent may push, when ("once work is
done and tested"), and how (which credential, which account).

**Why.** An agent that pushes without asking publishes work that hasn't
been looked at. An agent that never pushes leaves finished work sitting
locally overnight. Crake's permission came with a date (asked for
2026-10-11) and with the exact push command, because the machine's active
GitHub account was a different one and pushes failed with 403.

**Apply.** Add a line under "Git rules" in CLAUDE.md: who may push, after
what, with which command. If accounts must be switched, say "switch only
for the command, then back".

**Check.** In review: does CLAUDE.md answer "may I push?" without anyone
having to ask?

## Release commits are just the version

**What.** A release commit's subject is `<Product> X.Y.Z`. Its body is
that version's changelog section. Its diff touches only version files and
the changelog's heading.

**Why.** The release script writes it
([a7186e1](https://github.com/dorhawk-software/crake/commit/a7186e1)), so
releases are easy to find (`git log --grep '^Crake '`) and never mixed
with code changes. See [releases.md](releases.md).

**Apply.** Never commit a release by hand; run the release script.

**Check.** `git show --stat vX.Y.Z` lists only version files and
CHANGELOG.md.
