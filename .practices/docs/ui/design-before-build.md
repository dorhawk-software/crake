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

## The owner answers one question at a time

**What.** Open questions go to the owner in the agent's question dialog,
a few per round, one per step. Each step stands alone: the page's link,
the board's name, the options in plain words, the recommendation first.
The page with the boards is for looking; the answers come from the dialog.

**Why.** On 2026-11-21 two Scops proposal pages held 22 numbered
questions. The owner asked: "can we do a questionnaire of the gazzilions
questinons and we will do them one by one?" In the dialog all 22 were
answered in six short rounds. Later a step named its board but not the
page's link. The dialog covers the chat that held the link, and the owner
wrote: "where is the link to the design? i cannot find it anywhere".

**Apply.**
1. Number the questions on the page, so each step can name its place.
2. Ask in the dialog, up to four steps a round, grouped by topic.
3. Each step carries the page's link and the board's name, two to four
   options, and "(Recommended)" on the first.
4. A question with one proposal ("OK as proposed?") still offers two
   answers: as proposed, or what should differ.
5. Save each answer to a file as it arrives, so a restart loses nothing.
   Quote the owner's free-text answers word for word in the decision
   record.

**Check.** Review: the questions live in the agent's dialog, which no
script sees. The proposal page's numbered questions are what a script
could count.

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

## A touch-up gets its own before and after

**What.** A change to a design that is already agreed gets its own small
page: that one change only, before beside after, cropped to the object
that changed. It names where to find it, on the canvas and in the app.
The page is made new for each change, never a reused overview.

**Why.** Obálka's design has 24 boards and about 70 phone screens. A
touch-up there (a postmark's waves that ran through its date) was
published as a new version of the whole canvas. The owner, 2026-11-16:
"otherwise it is very difficult to see in 70 screens where to look and
what has changed". The same fix on its own page, the app and the board
each as a before/after pair, was accepted at a glance.

**Apply.**
1. Before you change the board, keep a copy of it. In git, the last
   commit is the copy.
2. Render the board before and after at the same size and device pixel
   ratio, and crop both to the same box around the changed object. Do
   the same for the app's capture, where it is built.
3. One page: before on the left, after on the right, the place named
   above each pair (the board's title or a word to search for, the
   screen's path in the app), and the values that changed in one line.
   Both themes when the change differs between them.
4. Publish it beside the design and link it in the message that asks
   the owner to look. Several touch-ups get several pages.

**Check.** A commit that changes an existing board in `UI_BOARDS` carries
a `Before-after:` trailer naming the page (a path in the repository or an
https link). A change that doesn't alter the look says
`Before-after: none, <why, three words or more>`. `check-commit-msg`
refuses the commit otherwise (the commit-msg hook). A design kept outside
the repository, on a canvas, isn't seen by it: there, the reviewer asks
for the page. Whether the page shows that one change only is judgement.

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

Eyes accept a line drawn twice as wide. Obálka's compose stamp has a
1 pt translucent frame; Android drew it 6 px wide on a 2.625x screen,
where an opaque one is 3. Side by side it looked right; measured, it
was not ([530f8da](https://github.com/dorhawk-software/obalka/commit/530f8da)).

**Apply.** Open the app in demo mode and capture the window. Compare it
side by side with the design, and fix the differences before showing it.
Capture sparingly: screen-portal captures flash the user's screen, so read
a window's own pixels where you can.

Then measure. Render the board at the device's pixel ratio and read rows
of pixels across the same parts of both: line weights, insets, heights.
Any image tool that dumps a row of pixels will do (ffmpeg's `crop` to
`rawvideo`, for one).

Then make the comparing a gate. Scops' screenshot match renders each
board and shoots each built screen at the same size, then compares them
object by object, on every commit. A gate that slow needs the rules in
[gates.md](../common/gates.md) to stay usable: iterate on a subset, gate
on the whole; cache by content; time every phase. Its shots follow
[the rule below](#a-screenshot-gate-launches-the-app-once).

**Check.** The handoff includes the side-by-side capture and the measured
line weights and insets of each new object.

## Pixel by pixel: the owner's eye is the bar

**What.** Before a built screen is called done, every object on it is
checked at the device's full resolution, cropped beside the same crop of
the design. The bar is that the owner finds nothing. Matching the layout
is not enough: edges, lines, glyphs, the typeface and every string are
checked one by one.

**Why.** On 2026-11-13 Obálka's owner found five faults on screens the
agent had walked, captured and called matching:

- an envelope's creases poked past its rounded corners;
- a 4 px blue rim showed above a flap that should reach the top edge;
- the search header drew a hairline its board does not draw;
- the search field had an outline and no glyph, where the board drew a
  glyph and no outline;
- every text field showed the system's Roboto, not the product's face.

Each one was visible in a capture the agent had already looked at, but
only shrunk to 800 px. The owner asked for "extreme attention to detail"
and "pixel perfection" from then on (2026-11-13). All five were fixed in
one commit, each with a test
([561d039](https://github.com/dorhawk-software/obalka/commit/561d039)).

The design can be wrong too: the board had the same crease overshoot. The
code was fixed and the board went on the sync-back list.

A fix can overshoot as well. The first one took the hairline off every
header, because the agent read the shared header class and not the
inline style each board sets on it; every board but the search's draws
the line. It went back the next day, with a test for each side
([a37862e](https://github.com/dorhawk-software/obalka/commit/a37862e)).
Check a fix against every design that shows the part, not only the one
where the fault was found.

**Apply.**
1. Capture at the device's full resolution. Crop each object to its own
   picture at 1:1 or larger. Never judge from a thumbnail.
2. Crop the same object from the design, rendered at the device's pixel
   ratio, and put the two side by side.
3. Check each object against this list:
   - **Edges.** Nothing runs past a rounded corner or a clip. A fill
     that should reach an edge reaches it, with no rim of what is under it.
   - **Lines.** Each hairline, outline and divider is in the design. Each
     one the design draws is there.
   - **Glyphs.** Each icon in the design is there, from the same family,
     in the same ink.
   - **Type.** The face that actually renders, letter by letter. Fields,
     placeholders and buttons fall back to the system font when no face
     is set.
   - **Words.** Each string appears once, in the design's wording and
     punctuation.
   - **Both themes**, and the longest language.
4. A walk that forces a state with a temporary code swap can show things
   the app never does. Say so before showing that capture, and restore
   the swap.
5. A fault in the design itself is fixed in the code and goes on the
   sync-back list ([above](#when-the-agent-cant-edit-the-design-a-prompt-and-a-sync-back-list)).
6. Each fault the owner finds becomes a test that fails without the fix
   ([testing.md](../common/testing.md), "A guard test per incident,
   proven able to fail"). A new kind of fault becomes an item in step 3.
7. Measure text line by line: each line's ink width and left edge, not
   only the paragraph's box. Type tuned to the design's ink (a weight
   and a tracking per style) sets each line's tracking from the width
   the browser gives that line.

A paragraph's box hides a line. Scops' pixel match measured each text's
box. A note's first line came out 1.0 pt narrower than the board's and
passed, because its second line was the widest. On 2026-11-21 a new
board's sentence had its narrow line widest: 1.3 pt short, a finding.
The cause was the type tuning: one tracking per text style, fitted over
all its strings. A line with another mix of letters missed by up to a
point. Tracking set per line, from the line's own browser width, made
all 29 boards match.

**Check.** The handoff names each object checked at 1:1 against its
design crop. The commit counts the owner's catches. The goal is zero.

## A screenshot gate launches the app once

**What.** A gate that shoots many screens of an app launches it once.
Each later screen is switched in place, under a debug-only flag, and
rebuilt as a launch would build it. A shot waits for the app's own ready
mark, never a fixed sleep. A guard compares a switched screen with a
launched one, and is proven able to fail.

**Why.** Scops, an iOS app, shot each screen with a fresh launch and a
2 s sleep. Over 59 shots a shot took 7.47 s: the launch 4.28 s, waiting
for a button 1.06 s, the sleep 2.00 s, the screenshot 0.09 s
(2026-11-23). The owner: "Yes, keep all 51, make shots fast". Switched in
place, a shot takes 1.26 s. A full run after a code change fell from
8.8 min to about 3 min.

**Apply.**
1. The switch lives in debug builds only, behind a launch flag. Scops
   posts a Darwin notification per scenario.
2. It rebuilds what a launch builds: the model emptied and seeded again,
   and a new identity for the root view, so no view keeps its own state.
3. Each scenario's screen publishes a ready element once it is up
   (`ready-<scenario>`, on the cover while one is shown). The test waits
   for it, then fails naming the scenario (15 s there).
4. It then takes screenshots until two in a row agree, with a limit
   that fails loudly. A capture taken mid-draw differs from the next.
5. Before shooting, the test checks the device's text size and
   appearance, and fails naming both. A wrong setting is then one
   failure, not a finding on every screen.
6. The guard: for pairs that leave state of different kinds behind (a
   view's own state, a cover, the model's), shoot B launched, and A
   launched then switched to B. Compare them below the status bar.
7. Allow 2 levels a channel, no more. Surfaces blurred in a Canvas
   dither differently from run to run, and between a fresh process and
   a used one. State left behind differs by hundreds of levels.

**Check.** The guard runs with every set of shots, and was broken once:
with the switch keeping the view tree, it failed with 1,386,033 pixels
off, worst 236 levels. Of 59 switched screens, 9 matched their launched
shots exactly, 44 within 1 level and 6 within 2. The match's findings
were the same either way: none. `check-shots` compares a live switch
with a fresh start the same way, without the tolerance
([mobile/devices.md](../mobile/devices.md#a-live-switch-draws-what-a-fresh-start-draws)).

## A value the scale doesn't have is a question for the owner

**What.** An agreed board may use a spacing, size or radius the project's
scale lacks. Then ask the owner: add the step to the scale, or move the
board to the nearest step. Never put the value where the lint doesn't
look.

**Why.** Obálka's empty-inbox board spaced its envelope 56 and 34 from
its neighbours; the scale has neither. The first build put both in a
theme file, which the spacing lint skips. Another agreed board drew the
same layout at 48 and 32: one layout, four numbers. Hidden values are
how a scale stops being one.

**Apply.** List each off-scale value with its board and its nearest
steps, and ask once. Record the answer where it lives: a new step in the
theme and the design system, or the board moved on the canvas. The same
layout on several boards gets one set of values.

**Check.** Every number in a lint-exempt folder is a named step, or an
object's own geometry with its board named beside it.

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

Every option, the recommended one above all, is also read against the
spec's text. An option that contradicts the spec says so, and says how
the spec would change.

**Why.** The assessments checked proposals against the design rules, not
against their surroundings. The product owner then had to point out each
of these six for one feature (2026-10-19). The critique also caught real
faults before they reached anyone: a clipped mark, and type too small to
read.

On 2026-11-21 a Scops review page recommended a restart button where the
spec puts the restart in settings. Another board drew it at 12:10; the
spec allows it "in the evening only". The agent's own pass missed both.
The independent reviewer, given the spec's sections, quoted the lines.

**Apply.** Put the six questions into every review prompt, and answer
them yourself first. Give the reviewer the spec's sections the proposal
touches, and ask it to quote the line each option follows or changes.
Show before and after side by side.

**Check.** The proposal page or message answers the six questions, and
quotes the spec line any option changes.

## Proposal pages pass a checklist before review

**What.** Before a reviewer or the owner sees a proposal page, its
author goes through a checklist and fixes what it finds. `check-page`
runs the part a script can. The review is then for judgement.

**Why.** Scops' proposal pages went through three review rounds before
the owner saw them (2026-11-20 to 23). Each round found what a list or a
script could have: an option's letter alone at a line's end ("C /
rozhodne"), a table that scrolled sideways at 375 px, an A about
2,100 px from its B, a 10.4 MB page. The owner chose a checklist the
author meets first (2026-11-23): the loop "greatly reduces the ability
to iterate and move the design forward".

**Apply.** Run `check-page` on the page, then `check-ui` and
`check-copy`. Then go through the list:
- **Layout, as the owner reads it on a phone.**
  - Each question sits beside its own images: the question, its boards
    and the recommendation in one numbered section.
  - Each before/after pair fits one 812 px screen at 375 px: crop to the
    band that changes, and put the pair side by side.
  - Crops are cut at an explicit offset, at the device's 2x, and drawn
    at most at half their pixel width, each linked to its whole board.
    The page stays around 1 MB (WebP). macOS' `sips -c` crops around the
    centre whatever offset it is given: crop a decoded PNG instead.
  - No one-letter word ends a line where the language ties it, an
    option's letter included; nothing scrolls sideways at 375 or 390 px.
  - `<!doctype html>` first, and `<meta charset="utf-8">`.
- **Options, a fair choice.**
  - Every option is drawn, at its best. The owner can't judge a sheet
    nobody drew.
  - Rivals sit over the same screen and state, the option's letter in
    the caption.
  - Each "before" is the alternative being decided.
- **Content, true and per the spec.**
  - Every sentence is true on each screen it would show on.
  - Each row's label promises what it records: name the event it writes.
  - A recommendation follows the spec, or says what it changes
    ([above](#review-a-proposal-in-its-place-with-fresh-eyes)).
- **Each language's rules.** Every form the copy can take (gender, case,
  the person addressed), any number of people or items, and the
  product's own names and words, as its copy rules say
  ([voice.md](../common/voice.md#each-languages-grammar-is-checked)).
- **Order.** Ask which rows a screen offers before how its words name
  them. Each question says which earlier answers it depends on.

**Check.** `check-page` reads each page in `UI_PAGES` (by default
`design/proposals/` and `design/changes/`) at each width in
`PAGE_WIDTHS` (375 and 390 px): `sideways`, `one-letter-end` (in a
language whose copy rules tie one-letter words), `image-width`,
`image-pixels`, `doctype`, `charset`, `lang`. It runs from claude-hook
after edits and on Stop, and from pre-commit. The rest of the list is
the author's, and the reviewer's.

## Every fact a screen uses was entered earlier

**What.** When a screen relies on data ("start Wednesday, after
swimming"), the flow that reaches it contains the step where the person
gave that data, or where the app got it, with consent. A fact two screens
show comes from one source, so they never disagree.

**Why.** Scops' onboarding step "Kdy začít" (when to start) recommended
"Wednesday evening, after swimming", from the week's events (2026-11-16).
But onboarding never asked for events: they lived only in the Týden
(week) tab. Every board was right on its own; the flow wasn't. The same
screen said "three days with no morning events after Wednesday", while
Týden had a vaccination on Friday at 9:15: two screens disagreeing about
one fact.

**Apply.**
1. For each screen, list the facts it shows or decides from. Beside each
   one, write the step that supplies it: the person's input, the phone's
   calendar, a default.
2. Walk the flow in the prototype as a new user. A fact with no earlier
   step means a missing screen, not a copy fix.
3. The demo data is one story. A fact shown on two screens (an event, a
   time, a name) comes from one source in the generator, never typed
   twice. Mark it where it is shown, `data-fact="swim-time"`.
4. A step that collects data used later may be skipped ("Teď nic pevného
   nemáme", nothing fixed yet). The later screen then has a designed
   state for no data.

**Check.** `check-copy` (fact) compares every element marked
`data-fact="KEY"` across all the boards, per language: one fact, one
wording. A board changed alone is still compared with all the others.
Whether a step supplies each fact is the click-through walk: for each
screen that recommends something, the reviewer asks "where did this come
from?" and points to the step.

## Every control records an event or only navigates

**What.** Each button or choice on a board either records an event the
spec's event table lists, or only moves to another screen. A choice that
changes what happens next and records nothing is a finding.

**Why.** An event-sourced app rebuilds its state from what was recorded,
and only recorded events reach a second device. Scops' audit of
2026-11-21 found choices that recorded nothing: keeping the evening at
20:50, declining a phase offer, adding a waking nobody tapped. Each was
forgotten on restart, and the other parent's phone never learned it.

**Apply.**
1. Give each control on a board its event, or mark it as navigation.
2. A choice with no event in the spec is a question for the owner:
   which event records it.
3. The spec's event table names the buttons that write each event.

**Check.** todo: a check reads every button on the boards and matches its
label to the spec's event table or a navigation mark; a button left over
fails.

## Cross each new event with each state before asking

**What.** Before the first question of a round, cross each new event or
control with each state it can happen in. Every pair the spec doesn't
answer is a question in that same round.

**Why.** Scops' audit of 2026-11-21 asked the owner 22 questions. Writing
the answers into the spec found 8 more, those answers found 4, and those
found 3. Each was a pair nobody had crossed: undoing a pause during a
wait, then during a visit, then during an extra visit. Four rounds of the
owner's time went on what one table would have shown.

**Apply.**
1. List the states (screens or engine modes) and the new events, as a
   table.
2. In each cell write what the spec says happens, or "undefined".
3. Ask about every undefined cell in the round, grouped by event.
   Recommend the answer of the nearest decided cell.
4. A reference engine fails loudly on an undefined pair, naming it,
   instead of guessing ([not-now.md](../common/not-now.md), "Fail loudly
   instead of a silent stub").

**Check.** todo: in a project with a reference engine, a test enumerates
every state and event pair and fails on one the engine doesn't define.
Whether the questions cover the table is review.
