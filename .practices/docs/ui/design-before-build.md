# Design before build

New UI is designed before it is built. It goes on the design (a canvas or
a proposals page) first, with variants when the choice is open. It is built
only after the choice is made, and the build matches the design one to one.
The design is kept current with the app afterwards.

## Proposals with options, then build "as chosen"

**What.** A UI change starts as a proposal: one page showing the options
(A, B, …) in place, with a recommendation. The person who owns the product
picks one. The build commit says "as chosen", and the rejected directions
are recorded with why.

**Why.** Building first and asking after wastes whole rounds of work.
Crake keeps its proposals as HTML pages in `design/proposals/` (62 files).
For example, the proposal
[d1ccaa1](https://github.com/dorhawk-software/crake/commit/d1ccaa1)
became the build
[1a726d0](https://github.com/dorhawk-software/crake/commit/1a726d0),
"as chosen". Tern does the same on a shared design canvas: variants side
by side, built after the pick. Either form works. What matters is that the
choice happens before the code.

**Apply.**
1. Draw the options where they will live, next to their real neighbours.
2. Recommend one, and say why.
3. Ask, then build the chosen one.
4. Record rejected directions in the design doc (Crake: `design/DESIGN.md`)
   in the same commit, so they aren't proposed again.

**Check.** A UI commit points at its proposal or canvas page. The design
doc lists what was rejected.

## The design holds only agreed designs

**What.** After a pick, the losing variants and their labels come off the
canvas or are marked rejected. The canvas then shows what the app should
look like, nothing else.

**Why.** A canvas full of old options misleads the next reader, person or
agent, about what was decided. Tern strips variants after every pick.

**Apply.** In the same session as the pick, delete the other variants, or
move them to a "rejected" page with their reasons.

**Check.** Every screen on the canvas matches a built screen or an
approved, unbuilt one.

## Keep the design in sync with the code

**What.** Every UI change made in code also goes onto the design, and the
other way round. Once a change is built, its mockup is replaced by a
capture of the app.

**Why.** Crake's design audit had to put two windows back to their
approved screens after they drifted
([6b26f7b](https://github.com/dorhawk-software/crake/commit/6b26f7b)). A
"before/after" built from an older mockup once looked older than the
"before", and confused the review.

**Apply.** Treat the design file as part of the change: same session, and
the same commit where it lives in the repo.

**Check.** In review of a UI commit: is the design updated?

## Read the design's source, never a picture

**What.** Values for a build (colours, spacing, sizes, radii, copy) come from the design's source: its code, its tokens, its inspector. A screenshot is only for checking the result.

**Why.** Obálka's owner, 2026-07-14: never derive the design from a picture. A port read from renders had "reinterpreted" the design in many places: a card's fill, a subtitle's missing field, an accent colour.

**Apply.** Open the design file's source and copy its literals. When the design tool has an API or a file the agent can read, use that.

**Check.** Every value in the change can be found in the design's source.

## When the agent can't edit the design: a prompt and a sync-back list

**What.** If the design tool is one the agent can't or mustn't edit, design changes go to the owner as a ready-to-paste prompt, with a status ("sent", "returned <date>", "what came back"). Differences the build had to make are listed for the design to take back.

**Why.** Obálka's design lives in a tool the owner edits; the agent fixes the code to match and hands design changes over as prompts. Its sync-back lists kept the design and the app from drifting apart.

**Apply.** A `design-prompts.md` and a `design-sync-back.md` beside the feature record. Here, "keep the design in sync" means: the design is updated in the same piece of work, by whoever may edit it.

**Check.** Every open sync-back item has a prompt sent for it.

## Match one to one, verified by screenshot

**What.** The build matches the design's spacing, sizes, colours and copy.
It is checked by capturing the app and putting it next to the design
before anyone is shown the result.

**Why.** "Looks about right" drifts. A demo mode that shows the design's
sample data makes exact comparison possible (Crake's `CRAKE_DEMO`, tern's
`TERN_FAKE` and `TERN_SHOW`). See [testing.md](../common/testing.md).

**Apply.** Open the app in demo mode and capture the window. Compare it
side by side with the design, and fix the differences before showing it.
Capture sparingly: screen-portal captures flash the user's screen, so read
a window's own pixels where you can.

**Check.** The handoff includes the side-by-side capture.

## Review a proposal in its place, with fresh eyes

**What.** Before a proposal is shown, it gets an independent critique (in
Crake, two isolated assessments plus a confirmation pass after the fixes).
The proposal is also answered against six questions about where it will
live:
1. Who needs it, and how often? Debug tools go out of everyday view.
2. What sits next to it? Two actions that answer the same question
   confuse.
3. What moves when its state changes? A label that changes width shifts
   its neighbours.
4. How does the user see that it's working? Progress needs its own look
   and motion.
5. Does the product already have a sign for this? Reuse it rather than a
   generic spinner.
6. What personal data could it carry? Say plainly what is in it, and let
   the user read it first.

**Why.** The assessments checked proposals against the design rules, not
against their surroundings. The product owner then had to point out each
of these six for one feature (2026-10-19). The critique also caught real
faults before they reached anyone: a clipped mark, and type too small to
read.

**Apply.** Put the six questions into every review prompt, and answer
them yourself first. Show before and after side by side.

**Check.** The proposal page or message answers the six questions.
