# CLAUDE.md and the engineering docs

CLAUDE.md holds a project's rules plus a one-line summary and pointer for
each area. The detail lives in `docs/engineering/<topic>.md`. Each rule
says where it came from and when, and every change in behaviour keeps a
dated note of what came before.

## Rules and pointers in CLAUDE.md; detail in docs/engineering

**What.** CLAUDE.md is short:
- what the project is;
- git rules;
- what to read first;
- architecture in brief;
- the rules for lints, testing and releases;
- one line per area, pointing at its file.

Detail goes to `docs/engineering/`: milestones.md, not-now.md, testing.md,
releases.md, platforms.md, packaging.md, tried-by-people.md. When a
section grows, its detail moves there word for word. The moved file says
where it came from. Nothing is dropped. New detail goes into those files
from the start.

**Why.** Every agent request carries CLAUDE.md whole. Crake's had grown to
48k characters. The split
([2f62e38](https://github.com/dorhawk-software/crake/commit/2f62e38),
asked for 2026-10-20) took it to 23k, and nothing was lost because the
moves were verbatim. Agents read the detail when they work in that area.

**Apply.**
1. Keep CLAUDE.md under a size you set (Crake: 150k characters as a hard
   ceiling; split well before it).
2. When a section passes about a screen, move its body to
   `docs/engineering/<topic>.md`. Head the moved file with "Moved from
   CLAUDE.md on <date>". Leave the rule plus "Detail:
   **docs/engineering/<topic>.md**".
3. Update comments in code that pointed at the old place.

**Check.** `wc -c CLAUDE.md`. In review: does this addition belong in
CLAUDE.md (a rule) or in a docs file (detail)?

## A handoff file is not a CLAUDE.md

**What.** A long HANDOFF.md that mixes state, decisions, done work, next
steps and deferrals is split three ways: rules go to CLAUDE.md, history to
milestones.md, and deferrals to not-now.md.

**Why.** Tern's HANDOFF.md reached 35k characters, and its "Known gaps and
next steps" part ran to about 330 lines of done, next and deferred work
mixed together. An agent reading it can't tell a standing rule from a note
about last week.

**Apply.** Move each part where it belongs. If the handoff is still
useful, keep it as "where we are", a page at most, pointing at the rest.

**Check.** Every line in CLAUDE.md is either a rule or a pointer.

## Every rule records its origin

**What.** A rule says who decided it and when, in a word or two:
"(asked for 2026-10-14)", "(decided 2026-10-13)", "option A, chosen".

**Why.** An agent has to tell a hard rule from a past preference. "Never
lower a coverage floor" and "the panel opens upward" carry different
weight, and a date shows how fresh a decision is. In Crake, every such
rule carries its date, so none is ever re-litigated by accident.

**Apply.** When you write down a decision, add the date and whether it was
asked for, decided together, or chosen from options. Never put a person's
name in a public repo (see [privacy-public-repos.md](privacy-public-repos.md)).

**Check.** In review: does each new "always" or "never" have an origin?

## The owner's words, quoted and dated

**What.** A feature record starts with an input line: the owner's words, quoted, with the date. Decisions inside it say which option was picked and when.

**Why.** Obálka's specs each begin with an "Input" line, for example: Owner, 2026-11-11, "can it detect this situation". Months later, an agent can tell what was asked from what was guessed.

**Apply.** `**Input**: Owner, <date>: "<their words>"` at the top of each spec, issue or milestone entry. Roles only in a public repository.

**Check.** Every feature record has an input line.

## Known gaps beside the rule, not a reworded rule

**What.** When the product falls short of a written principle, a dated "known gap" note goes under the principle. The principle keeps its wording. Claiming less than the product does is as wrong as claiming more.

**Why.** Obálka's constitution had drifted from what shipped: it called a sign-in method "unavailable" while the app offered it. Its amendments now record gaps under each principle, dated, and the principle stays as decided.

**Apply.** `> Known gap (<date>): <what falls short>; <what closes it>.` A gap leaves once it's closed.

**Check.** Each principle's known gaps are dated and none is older than its plan.

## "Until <date>" notes keep the old behaviour visible

**What.** When behaviour changes, the current statement keeps a short
dated note of what it was and why it changed. For example: "No answer just
then … (until 2026-10-16 'Probably down.', which read as a verdict on a
host that answered fine a moment later)."

**Why.** Without the note, someone "fixes" the wording back, or wonders
why the obvious approach wasn't taken. Crake's CLAUDE.md and docs carry
about ten of these. Each one stopped a repeat of a mistake.

**Apply.** Write `(until YYYY-MM-DD <old>, which <why it was wrong>)` next
to the rule or in the code comment.

**Check.** In review of a behaviour change: is the old behaviour recorded
where the new one is described?

## Milestones say what was built and how it was proven

**What.** `docs/engineering/milestones.md` has one part per milestone:
what it built, how it is proven (the tests and what they show), and what
real systems revealed. Read the milestone's part before changing that
area.

**Why.** "It's built" and "it's proven on a real system" are different
claims. Crake's milestones doc (about 1,500 lines) is where each claim's
evidence lives, so a later change knows what it must keep working.

**Apply.** End each milestone by writing its part: built, proven by,
found on real systems, deferred (with a pointer to not-now.md).

**Check.** In review of a feature: does its milestone part name the test
that proves it?

## Tried by real people: a checklist per platform

**What.** `docs/engineering/tried-by-people.md` lists what someone has
actually used the software for on a real machine (not a VM made for
tests). Each item is ticked with the date and a role, never a name, plus
what was found and fixed.

**Why.** Tests prove the mechanics. Real machines prove use. Nearly every
macOS fix in Crake came from a colleague's real Mac, not from CI
([5a7971f](https://github.com/dorhawk-software/crake/commit/5a7971f),
asked for 2026-10-14).

**Apply.** List the supported systems and the everyday tasks on each. Tick
them as people try them. When a real machine finds a bug, add a test on
the system where it appeared.

**Check.** Before a release that claims a platform: how many of its items
are ticked?
