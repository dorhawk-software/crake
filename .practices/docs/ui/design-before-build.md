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

**Check.** The handoff names each object checked at 1:1 against its
design crop. The commit counts the owner's catches. The goal is zero.

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

**Why.** The assessments checked proposals against the design rules, not
against their surroundings. The product owner then had to point out each
of these six for one feature (2026-10-19). The critique also caught real
faults before they reached anyone: a clipped mark, and type too small to
read.

**Apply.** Put the six questions into every review prompt, and answer
them yourself first. Show before and after side by side.

**Check.** The proposal page or message answers the six questions.
