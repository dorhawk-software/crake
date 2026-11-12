# Privacy in public repositories

Nothing about the people or companies you work for goes into a public
repository: not their names, hosts or addresses, in code, tests, demo data,
designs, docs or commit messages. What you learn on real systems is kept in
a git-ignored local file. A hook refuses the names you list, and the hook
fails closed.

## Made-up names and documentation addresses

**What.** Examples, tests and demo data use invented companies (ACME,
Globex, Initech, Nordbank), `*.example` and `*.internal` host names, and
the documentation address ranges: 192.0.2.0/24, 198.51.100.0/24,
203.0.113.0/24 (RFC 5737) and 2001:db8::/32 (RFC 3849).

**Why.** The project was written alongside client work, with real systems
at hand, so a real host name in a test is the easiest thing in the world
to type. One got into Crake's tests on day two and was replaced by a
made-up one
([bf59dbc](https://github.com/dorhawk-software/crake/commit/bf59dbc)).
Documentation ranges are never routed, so a test can't reach a real
machine by accident.

**Apply.** When a real system teaches you something, give it a made-up
name before it goes anywhere tracked. Keep a mapping table ("ACME = …") in
the local file, never in the repo.

**Check.** The private-terms check below. In review: is every company,
host and address in this diff obviously invented?

## Real findings live in a git-ignored local file

**What.** `CLAUDE.local.md` (git-ignored) holds what was found on real
systems: which real system "ACME" stands for, its quirks, and the commands
that reach it. CLAUDE.md and the code only ever say "ACME".

**Why.** Those findings are the most valuable debugging knowledge a project
has, and an agent needs them in its context. They must not be public.
Claude Code reads `CLAUDE.local.md` next to `CLAUDE.md`, so the agent sees
both while git sees one.

**Apply.**

```gitignore
CLAUDE.local.md
dev/
```

Put the split in CLAUDE.md's "Git rules": "What was learned on real
systems lives in CLAUDE.local.md (git-ignored); never copy its names into
committed files."

**Check.** `git check-ignore CLAUDE.local.md` prints the path.

## A denylist the hooks refuse, failing closed

**What.** `dev/private-terms` (git-ignored, one Perl regex per line,
case-insensitive) lists the real names, hosts and addresses.
`.practices/bin/check-private` refuses any staged, tracked or new file that
matches, and any commit message that matches. It runs in three places:
- the `pre-commit` hook;
- the `commit-msg` hook, through `check-commit-msg`;
- `.practices/bin/lint`, which the agent hook runs after every build.

**Why.** On macOS, Crake's first version of this check used GNU grep's
`-P`, which macOS doesn't have. It read "couldn't search" as "nothing
found", so every commit made on a Mac went through unchecked for days
([eb13f5b](https://github.com/dorhawk-software/crake/commit/eb13f5b)).
It now uses `git grep -P` (PCRE everywhere), and any error refuses the
commit.

A missing terms file used to mean "nothing to check" and exit 0. That made
the check a silent no-op in CI and on fresh clones. The shared version
fails when the file is missing. To declare "this project has no private
terms", create the file with only a comment in it. In CI, the
`actions/rust-lint` step writes the terms from a repository secret. In a
run without that secret (a fork, Dependabot), it says the check was skipped
instead of passing quietly.

**Apply.**

```sh
mkdir -p dev && printf '# one regex per line\n' > dev/private-terms
.practices/bin/install-hooks      # core.hooksPath=.practices/hooks
```

Add the patterns as you meet the names, and store the same list as the
repository secret for CI.

**Check.** `.practices/bin/check-private` exits non-zero on a test commit
that contains a listed name. The practices repo's own tests check that it
also fails when the terms file is missing or the search errors.

## People appear as roles

**What.** Docs, checklists and commit messages say "a colleague on a Mac",
"the user", "a tester", never a person's name.

**Why.** People who tried the software didn't agree to be named in a
public log. A role also says what matters (the platform, the setup) better
than a name does. Crake's tried-by-people checklist ticks each item with a
date and a role
([5a7971f](https://github.com/dorhawk-software/crake/commit/5a7971f)).

**Apply.** Write the role and what made their setup different.

**Check.** In review: does any line name a person who isn't the author?
