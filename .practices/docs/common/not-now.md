# Not now: deferrals are written down

Whatever you decide not to do right now is written down where it will be
found again, in the same commit as the decision. That covers a gap, a
stub, an untried case, a follow-up and an open question.

## Write it down in the same commit

**What.** Each deferral goes to `docs/engineering/not-now.md` (its detail)
with one line in CLAUDE.md's "Not now" list, or into a note next to the
code it concerns. A doc comment on a stub isn't enough. Neither is a
mention in chat or in a pull request. Done items leave the list.

**Why.** In Crake, a Windows and macOS stub returned a constant `false`
with nothing pointing back to it. The rule followed
([6267399](https://github.com/dorhawk-software/crake/commit/6267399),
asked for 2026-10-14): "whenever we decide not to do something right now,
it must be written down so we know about it and get back to it." Crake's
not-now.md then held the idea for this very repository, which is how it
wasn't lost.

**Apply.** At the end of each piece of work, list what was deferred or left
unverified. Record each item before committing:

```markdown
- **<What isn't done> (<date>).** Why not now. What it would take.
  What would make it urgent.
```

**Check.** In review: does the commit body's "deferred" list match new
entries in not-now.md? Does the CLAUDE.md "Not now" list have a line per
item?

## Fail loudly instead of a silent stub

**What.** Code that isn't done says so when it runs: an error that names
the gap, or a `not_here(why)` in tests. It never returns a plausible
default.

**Why.** A stub that returns `false` or `Ok(())` passes every test and
looks finished. Crake's scenario suite has a system that can't run a
scenario say `not_here`, never pass quietly
([b32b88e](https://github.com/dorhawk-software/crake/commit/b32b88e)).
The shared version counts those skips (see [testing.md](testing.md)).

**Apply.** Prefer `bail!("<feature> isn't built on <platform> yet: see
not-now.md")` to a default value. For tests, use a counted skip.

**Check.** Grep for `todo!`, `unimplemented!` and constant-returning stubs
behind `#[cfg]`. Each one needs a not-now entry.

## Decided "left as it is" is also written down

**What.** A deliberate decision to keep a limitation stays in not-now.md,
marked "decided, left as it is (<date>)", with the reason and what would
reopen it.

**Why.** Without it, the same question comes back every few weeks and is
argued again from scratch. Crake's "IPv6-only hosts on a network without
IPv6" entry records the decision, why, and the trigger that would change
it.

**Apply.** Use the same entry shape and add "Decided: left as it is".

**Check.** Before proposing a change, search not-now.md for it.
