# Visual craft

The rules for drawing an interface that looks made, not generated. They
apply to any product and any toolkit. The process that uses them is in
[identity-pass.md](identity-pass.md). Entering dates, times and numbers
is in [pickers.md](pickers.md); asking for consent is in
[consent.md](consent.md); showing work under way and answering a press
is in [feedback.md](feedback.md).

Many of these rules are geometry: a gap, a distance from an edge, an
axis. A person reviewing a scaled-down board cannot see a 2 px gap, and
a script finds it in seconds. So each geometry rule's check is a
measurement, and the reviewers also get 1:1 crops of the dense areas.

The measurements run on their own: `.practices/bin/check-ui` renders
every design board (`UI_BOARDS`, `design/**/*.html` by default) in
headless Chrome, at each width and in light and dark. It runs after every
edit to a board and before the agent's turn ends (claude-hook), before
each commit (pre-commit), and in CI. Its findings go back to the agent,
grouped by rule, each with its fix. A deliberate exception is marked in
the board, `data-allow="rule: the reason"`, and a mark without a reason
is a finding too. What it checks, and what is still a reviewer's, is in
[ENFORCEMENT.md](../ENFORCEMENT.md). A board drawn at a fixed width (a
390 px phone) says so, `<meta name="viewport" content="width=390">` or a
`canvas.json` beside it, and renders at that width only. A board with no
dark rules renders in one theme. The words on the boards are checked too,
for their language's grammar, by `check-copy`
([common/voice.md](../common/voice.md#each-languages-grammar-is-checked)).

Each rule names the incident behind it. Product details are labelled
examples.

## Tone, not outlines

**What.** Separate things with tone, shadow, type and spacing, not with
borders and boxes. Anything at rest gets a step of tone. A line means
focus, and nothing else. Floating surfaces (menus, popovers) carry a
shadow, not a hairline. Lists stay unboxed. Nested cards are always
wrong.

**Why.** The owner warned that putting everything into boxes and borders
is an AI tell, and design review tools flag it. Tern's audit found 1 px
rings on resting tabs, secondary buttons, time pills, per-message actions
and palette chips, and a hairline around every popover. The owner took
the tone-only version of all of them.

**Apply.**
1. Resting control on the page background: fill it one tone step darker
   than the background. On a white sheet: one step darker than white.
2. Selected: a strong fill (ink, or the accent), never a ring.
3. Focus: a 2 px line in the ink or accent colour. This is the only
   ring.
4. Keep a ring only where it carries meaning: an unticked checkbox,
   today's date in a calendar.
5. Floating surface: a two-layer shadow (a tight one plus a soft one), no
   hairline. In the dark theme, also lift its fill one step above the
   ground, so its edge holds where shadows are weak.
6. Before showing a design, count every border. Justify each one that
   remains, or replace it.
7. Draw a ring that carries meaning (today, focus, the selected item)
   with a property no global layer owns: `outline`, not `box-shadow`.
   Scops' depth layer (2026-11-15) set `box-shadow` on every button. It
   erased the ring that marked today in the date picker, and nobody saw
   it until a screenshot.
8. After adding a global layer (depth, shadows, a theme), check every
   meaningful mark it could touch: today's date, focus, the selected
   item.

**Check.** `check-ui` (ring) flags every ring drawn with `box-shadow`.
Search the styles for borders: each one left is a focus ring, a checkbox,
or has a written reason. After a global layer lands, a capture of each
meaningful mark still shows it.

## Every stroke end matches

**What.** Every end of a drawn mark uses the same cap as its family,
usually round, at every zoom. Connecting lines are drawn under the marks,
never over them.

**Why.** On Crake's timeline, a branch had a flat end where it left the
line and a round end at its tick. The line's segment was drawn over the
branch's round cap. The owner zoomed in and caught it: both ends must be
round, "to match the roundness" of the logo.

**Apply.**
1. Set the cap and join once for the whole icon family (for example
   round and round), and use it on every path.
2. Draw connecting lines first and marks on top.
3. Zoom to 400 % on every mark before showing it. Look at each end.

**Check.** A zoomed capture of each mark, with every end the same shape.

## One object, one drawing

**What.** An object that appears in several places is drawn the same way
everywhere: the same proportions, line weight, corner radius and details.
If it moves from one place to another on screen, it must stay the same
thing.

**Why.** Tern's bird carries a letter onto a pile while mail downloads.
The carried letter was the bird's own icon letter: almost square, with a
5 px line. The pile was flat envelopes with a 1.6 px line. Once the
letter landed it looked like a different object. The owner: it "looks
weird, like the brought envelope is designed completely different from
the rest". The fix drew one envelope, in a 3.5 px round line, for both
places.

**Apply.**
1. List every object that appears in more than one place or that moves
   between places.
2. Give each one drawing, in one source file, at one line weight in
   pixels. Choose a weight that sits well next to the marks it travels
   with.
3. When an object scales, keep its line weight in pixels. Do not let the
   line scale with it.
4. Animate the object itself from place to place. Never swap it for a
   different drawing at the hand-off.

**Check.** Put every appearance of the object side by side at its real
size. They read as the same thing.

## A drawing over an object reaches its edges

**What.** Folds, seams, tints and sheens drawn over an object span the
object's whole box, not the room left inside its padding. A line that
runs to a corner ends on the corner.

**Why.** Obálka's envelope draws two folds from its bottom corners. The
drawing sat inside the envelope with a width and height of 100 %. In
React Native those resolve inside the padding. So each fold started 16
to 18 px in from its corner, on every envelope in the app. Tests passed
and the shape looked right at a glance. The owner saw it on a phone: the
creases were "borked".

**Apply.**
1. Put the drawing in its own layer, pinned to all four edges of the
   object (absolute, 0 on every side), outside the content's padding.
2. Size the drawing to that layer. Never size it from the content box.
3. Give the drawing the object's own outline in its coordinates: the
   corners in the path are the object's corners.
4. Check this on every object that carries a drawing, not just the one
   that showed the fault. One shared component fixed three screens.

**Check.** A full-resolution crop of each corner where a line ends. The
line meets the corner, with no gap. A test asserts the drawing's layer
has no padding and sits at 0 on every side.

## A line weight per family

**What.** Marks shown together share a line weight. A mascot drawn at
64 px and the 16 px icons beside it are different families. Each family
has its own weight, chosen on purpose.

**Why.** The envelope incident above. A thin object next to a heavy
mascot reads as a wisp. A heavy object on a thin pile reads as a foreign
body.

**Apply.** Write down each family and its weight: icons (for example
2 px at 24), the mascot, the objects the mascot handles. Check new marks
against the list.

**Check.** The design doc lists the families and their weights.

## A drawn icon set, grown from the idea

**What.** Every icon in the product comes from one drawn family. The
family grows from what the product already draws (its site, its app
icon, its printed material) and from the objects the idea is about. Stock
icons, or a mix of sets, are a placeholder, not a design.

**Why.** A proposal's screens kept the mark rare, as the identity pass
asks, and used system symbols everywhere else. The owner's verdict: "Where
are some icons? That adds identity to the app." With the mark held back,
the icons are the part of the identity the user sees on every screen.

**Apply.**
1. Facts first: collect the product's existing drawings and note their
   line weight, caps, corners and colour. Keep what identifies them.
2. List every icon the screens need, from the inventory, not from memory.
3. Pick the family's rules: grid (for example 24), weight (for example
   2 px), caps and joins, fill or line, the one colour and when it changes.
   Give the idea's object (the thing the mark carries or hands over) a
   place inside the icons that are about it.
4. Draw at least two families, each over the whole list, and recommend
   one. Show each at its real sizes, in both themes, and in a real screen.
5. Third-party marks (bank, card or store logos) are not redrawn. Use the
   real ones the product already has the right to show, each on its own
   neutral tile, and say where they came from.
6. Use icons generously. Every settings row, list item and main button
   carries one from the family. On Scops (2026-11-14) the owner asked
   for icons everywhere, settings first.
7. Keep one map from action to icon in code, with no default. Its lookup
   fails loudly when a label has no icon, so a new button without one
   breaks the build, not the review. On Scops (2026-11-16) `check-ui`
   found 189 buttons without an icon. Most were quiet text buttons,
   written by hand 16 times in three weights. The fix was one `ghost()`
   component and that map.
8. A single choice is a `role="radiogroup"`, not a row of buttons: chips,
   month and year grids, day grids, quick times, method cards. It is the
   right meaning for a screen reader, and `check-ui` treats the group as
   one control, not as many main buttons without icons.

**Check.** No icon on any screen is a system or stock symbol unless the
design doc says why. Each icon of the family, side by side at 24 and at
48, reads as one hand. The weight is in the families list above. A
list row or button without an icon has a written reason. `check-ui` (icon)
flags every button with words and no icon, unless it is marked
`data-allow="icon: …"`. The options of a segmented control
(`role="tablist"` or `role="radiogroup"`) are no main buttons and aren't
flagged.

## A glyph keeps one ink

**What.** A glyph is printed in one ink. Its second colour, an offset
plane or a fill, is a tint of the stroke's own hue, or a neutral (white,
grey, the paper). Two hues never meet in one small glyph.

**Why.** Obálka's offset glyphs print a pale blue plane under every
stroke. Under a black stroke it reads as print. When the bin on a
neutral "Smazat" button turned red, its blue plane stayed, and the
owner saw at once that the red and blue fought (2026-11-16). On the red
danger button the same bin looked right, because its plane there was
white over red: one hue.

**Apply.**
1. When a glyph takes a colour (danger, success, a link's blue), its
   plane takes a faint tint of that colour, not the family's default.
   Obálka's danger bin: the danger red at 20 % (light) and 26 % (dark).
2. On a filled button the plane is white or black at low alpha, so it
   takes on the button's colour.
3. Give each coloured plane a theme token beside the ink it belongs to,
   so a new colour can't forget its plane.
4. A two-colour drawing that is a mark, not a glyph (the logo, a flag),
   is filled, not stroked, and isn't held to this.

**Check.** At 18 to 48 px, every glyph reads as one colour family.
`check-ui` (ink) flags a glyph of 64 px or less whose stroke and any fill
are two distinct hues (each with chroma of at least 0.12, more than 45°
apart), including symbols drawn through `<use>`. A glyph that is meant
to carry two colours is marked `data-allow="ink: …"`.

## Depth and texture from a concept

**What.** A mobile design has depth and, where the identity allows,
texture. The depth comes from one concept that belongs to the product.
It never comes from generic shadows or from another product's signature.

**Why.** The owner found Scops, a baby-sleep app, "very flat, without
detail" (2026-11-14). The first depth round looked alike in all three
directions. The chosen one was Scops' own: the owl's night lamp lights
the screen. The owner rejected split-flap clock tiles, because they are
the signature of Shrike, another Dorhawk product.

**Apply.**
1. Name the concept first, from the product's own story. Scops: the
   owl's lamp. Obálka: sheets of paper with weight.
2. Let the concept decide the light and the layers:
   - which way the light falls;
   - what glows, what is raised and what is pressed in;
   - what overlaps what (the owl sits over the edge of a card).
3. Make controls feel touchable: a raised primary button, a pressed-in
   selected state.
4. Keep texture quiet: a fine grain over the screen, and a material
   detail on the mascot (the owl's feathers).
5. Show two or three depth directions on the same screens. Make them
   different enough to compare at a glance.
6. Never borrow another product's signature element. Compare with the
   references before you show anything.
7. Shadows fall away from the light. Scops' green buttons had a glow
   that blurred 8 px above their top edge. The owner asked whether the
   soft top and sharp bottom were intended. They were not.
8. A glow is not depth. Scops' lit beads had an amber glow with no
   offset (2026-11-16). It gave 32 shadow findings and 20 halo findings,
   and it fogged the label "hotovo 5 min" beside it. Now a lit bead shines
   by its own gradient, with a small shadow that falls down. It still
   reads as lit, and nothing around it is fogged.
9. A bottom sheet casts no shadow upward. Light comes from above, so a
   raised sheet's shadow falls behind it, where nobody sees it. Separate
   the sheet by tone, the scrim and a 1 px top highlight
   (`inset 0 1px 0`). Scops' sheets cast `0 -18px 50px` upward.
10. A ring that means something (focus, the selected item) is an
    `outline`, never a `box-shadow` of `0 0 0 1px`.

**Check.**
- Each depth effect is explained by the concept in one sentence.
- Nothing on screen is recognisable as another product's signature.
- No glow or drop shadow reaches above the top edge of something lit
  from above. There it reads as a blurred edge. `check-ui` (shadow)
  flags a zero-offset halo, a shadow that falls up, and one whose offset
  is under a quarter of its blur; a glow the concept explains is marked
  `data-allow="shadow: …"`.
- Text still passes contrast on every raised and pressed surface.

## The system's own sign for the system's own act

**What.** A control that stands for something the operating system does
shows the sign that system uses for it. Unlocking shows Face ID's face on
an iPhone and a fingerprint on Android. Sharing shows each system's share
sign. The sign is drawn in the product's own icon family, not taken from
a stock set.

**Why.** Obálka's lock screen board drew the Face ID face, on an iPhone
mock-up. Built as it was, Android phones that unlock with a fingerprint
showed a face (2026-11-12). The owner's rule: the platform's own sign
reads as native. The first fix took a stock fingerprint, which no longer
matched the board's face; both are now drawn in one frame at one weight
(1068694).

**Apply.**
1. List the controls that stand for a system act: unlock, share, open in
   another app, add to the calendar, the system's settings.
2. For each, draw one glyph per system, in the product's family: the same
   frame, line weight and stroke ends. The system changes what is drawn,
   never how.
3. Choose the glyph in one place, by the platform, with the platform as
   an argument so a test can ask for each.
4. A board drawn for one system shows that system's sign; note the other
   system's on the canvas, so the design holds both.

**Check.** A test renders each such glyph for every platform and expects
different drawings in the same frame. Walk the screen on each platform
and look at the sign.

## Compact, not breathy

**What.** Content or the main control starts right under the window's
header. Decoration takes only the room it needs and overlaps the chrome
instead of pushing content down. Boards and proposal pages end where
their content ends.

**Why.** Drafts kept coming out "breathy". One inbox proposal's decorative
flap took about a fifth of the screen before the first control. The owner
has corrected this on three products. These are tools people open to get
something done.

**Apply.**
1. Look at the top 200 px of every screen. Anything there that isn't
   content or a control needs a reason.
2. Use the smaller of two spacing steps when unsure.
3. Measure the content's height and size the board to it.

**Check.** No band of empty space above the first content, and none at
the bottom of a board.

## Nothing runs past its surface

**What.** Every button, label and line sits inside the surface it is
drawn on: the panel, the card, the window. Nothing is cut off by a box
too small for it. This holds at every width the board is shown at, and
in both themes.

**Why.** The Windows-app runner's folder panel (2026-11-15) got an icon
on "Allow All of Documents", as the icon rule asks. The row grew 3 px
wider than the panel, and "Not now" stuck out past its left edge. The
agent called the board ready. Its own overflow script measured only
height, and its gap check only left to right, so both passed. The owner
saw it in five seconds. The rule was written, under "Pixel by pixel",
but no script measured it.

**Apply.**
1. Let a row of buttons wrap, or make its surface wide enough for the
   longest label in the longest language.
2. After adding anything to a row (an icon, a word), measure the row
   again at every width.
3. A badge or tooltip that hangs over an edge on purpose, or a list
   drawn mid-scroll, is marked in the board with its reason:
   `data-allow="contain: the count badge sits over the corner it numbers"`.
4. A board's CSS clips to rounded corners; GPUI clips to a rectangle. In
   a GPUI app, a child in a rounded corner rounds that corner itself
   ([rust/gpui.md](../rust/gpui.md#rounded-corners-dont-clip)).

**Check.** `.practices/bin/check-ui` measures every element against the
surface it is drawn on, and every clipping box against its content, at
each width and theme. A surface that clips is measured once, by clip, so
one `data-allow="clip: …"` covers a screen drawn mid-scroll. It runs after every edit to a board, before the
agent's turn ends, and before each commit.

## A button label stays on one line

**What.** A button, chip or tab keeps its label on one line
(`white-space: nowrap`). If the label doesn't fit, shorten it, give the
button its own row, or make it a list row. Content meant to stack (a card,
a list row with a subtitle) is a flex column.

**Why.** Adding icons on Scops (2026-11-16) pushed "Připojit se k rodině,
která už Scops má" (join a family that already uses Scops) onto two lines.
"Další dítě" (another child) wrapped beside a hint. Nothing overflowed:
the box just grew, so no check saw it. A one-line control that wraps reads
as broken.

**Apply.**
1. Give every button, chip and tab `white-space: nowrap`.
2. A long label is shortened first: the verb and its object.
3. A label that stays long gets the button its own row, full width at
   phone width, or becomes a list row.
4. A target with two lines on purpose (a title and a subtitle) stacks
   them in a flex column.

**Check.** `check-ui` (wrap) flags a target (a button, a tab, a radio, a
switch, a menu item) whose text runs onto a second line, unless it sits in
a flex column inside the target. A card-sized target whose text is a
paragraph on purpose is marked `data-allow="wrap: …"`.

## Buttons follow the column

**What.** A row of buttons lines up with the text above it. In a column
that starts at the leading edge (the left, in left-to-right text):
- several buttons start at that edge too, or fill the column's whole
  width as equal parts, so the row reads as deliberate;
- one forward action (Continue, Next) may sit at the trailing edge,
  because readers of left-to-right text take the right side as "go on";
- a single button never stretches across a wide surface. On a phone
  (480 px or narrower), a full-width main button is the platform's own
  pattern and stays.

In a centred column, the buttons are centred too.

**Why.** The Windows-app runner's folder panel (2026-11-15) set its
title, its text and its footnote at the left edge, and only the row of
three buttons at the right. The owner: everything is aligned left, "and
then buttons, and only the buttons, row is aligned to the right, like
what?" The macOS step beside it, one Continue at the right under left
text, read as right: that side means "continue". Always stretching is no
answer either: one button across a whole panel looks odd.

**Apply.**
1. Look at the column's edge first: where do the title and the text
   start? The row starts there.
2. Several actions: start them at the leading edge, the main action
   first and the escape (Cancel, Not now) last, or give them equal
   columns across the full width.
3. One action that moves on: the trailing edge is fine. One action of
   another kind: the leading edge.
4. The platform's own surfaces (a macOS alert, a system sheet) are drawn
   as the platform draws them, and marked
   `data-allow="align: drawn as macOS draws its alert"`.

**Check.** `check-ui` (align) finds each row of buttons and the column it
sits in. In a leading-aligned column, it flags a row of several buttons
that doesn't start at the column's edge (unless it fills the column), a
lone button that is neither at an edge nor at phone width, and a lone
button stretched across a surface wider than 480 px. In a centred column
(`text-align: center`, or a flex column that centres its items), it flags
a row off the centre. A row that is a surface of its own (a bottom bar
with a fill) or is placed by hand (`position: absolute`) is its buttons'
column: they follow its padding.

## Every button names its outcome

**What.** Each button says what happens when it's pressed, in the words
of the outcome, not of the operation. The way out of a question names
what stays: "Keep the app" beside "Remove", "Keep editing" beside
"Discard", "Keep copying" beside "Stop copying". Never a bare "Cancel",
"OK", "Yes", "No", "Confirm" or "Submit": each could mean either answer.

**Why.** The Windows-app runner asked whether to remove an app, with
"Remove" and "Cancel" (2026-11-15). The owner: "Remove" says what
will happen, but "Cancel" could just as well mean cancel the app you're
removing. The operation is a cancel; what matters to the person is the
outcome, and that the app stays.

**Apply.**
1. Read the question, then each button alone, as an answer to it. A
   button that only makes sense once you know which side it's on is
   wrong.
2. The action: its verb and object ("Remove", "Save and remove",
   "Empty Trash").
3. The way out: what is kept or goes on ("Keep the app", "Keep
   editing", "Not now" when the question comes back later).
4. A surface the platform draws (a macOS alert, a system sheet) keeps
   the platform's words, marked
   `data-allow="outcome: drawn as macOS draws its alert"`.

**Check.** `check-ui` (outcome) flags every button whose whole label is
a word that names no outcome: Cancel, OK, Okay, Yes, No, Confirm,
Submit, Accept, Decline, Proceed, Abort, Dismiss, and in Czech Zrušit,
Storno, Ano, Ne, Potvrdit, Přijmout, Odmítnout, Přerušit. The words are
language data, each language's `vagueButtons` in `copy-rules/<lang>.mjs`
(Spanish: Aceptar, Cancelar, Sí, No…), read for the language the button
is in. The label is the accessible name when there is one: in a weekday
picker, "Ne" with `aria-label="Neděle"` is Sunday, not "no" (Scops,
2026-11-16).

## Separate targets keep their distance

**What.** Two separate tappable things are at least 8 px apart, edge to
edge. Three cases are exceptions, because they read as one control:
- a segmented control;
- a grid of like cells (calendar days);
- list rows inside one card, divided by lines.

A row of separate actions is not an exception. It becomes a list: the
whole row is the target, with "Edit ›" as its label.

**Why.** Scops' name step (2026-11-15) stacked three "Upravit" (Edit)
tiles 2 px apart. The owner: they look crammed together. It had been
there since round 1. The automated audit checked size, contrast and
overflow, but never the gap between targets. The reviewers saw boards
scaled to a third, where 2 px is invisible. The depth layer made it
worse: it turned flat chips into raised tiles, and raised tiles need air.

**Apply.**
1. Keep at least 8 px between separate targets. Give raised buttons
   more.
2. For a repeated action on rows, make the row the button: a divider
   between rows, the action as quiet text at the end.
3. After any global style change (depth, shadows), measure the spacing
   again.

**Check.** `check-ui` (gap) measures the edge-to-edge gap between every
pair of targets, across and down. It flags anything under 8 px, except
the three cases above, marked `data-allow="gap: …"`. A segmented
control that says so by its role (`role="tablist"` or
`role="radiogroup"`) needs no mark, nor does a group of toggles that
each say whether they're pressed (`role="group"`, every option
`aria-pressed`: a weekday picker). The targets of a page behind an open
sheet aren't measured: it is `aria-hidden` or `inert`
([below](#a-sheets-page-is-inert-and-the-board-says-so)). The reviewers
get 1:1 crops of dense areas, not only scaled-down sheets.

## Spacing is measured from what you see

**What.** Spacing is measured to the visible edge of an object: a glow,
a halo, a shadow, a disc behind a mascot. It is not measured to the
bounding box. When a global layer makes an object look bigger, every
spacing around it is checked again.

**Why.** Scops added a lamp disc behind its owl (2026-11-15). It reached
the top of the owl's drawing, where there used to be empty space. On 22
screens the disc came within 3 to 13 px of the text above. The owner saw
it on the first night: the gap above the halo looked smaller than
anywhere else. One change caused it, not 22 separate mistakes.

The same holds the other way round: what lies over a surface's edge
takes room from its padding. Obálka tucked its letter 10 px under the
envelope it comes out of (2026-11-16). The letter kept its padding of
14, so only 4 to 6 px showed above the subject against 14 below the
last line. The owner: "the message title seems to be kinda hidden under
the blue top bar".

**Apply.**
1. Build room for effects into the object itself. Scops' owl has room
   above the disc in its own viewBox, so every screen gained the gap at
   once.
2. Keep at least 16 px between a visible edge and the nearest text.
3. When something covers a surface's edge, pad that side by the overlap
   plus the padding the other sides keep: Obálka's letter has 24 at the
   top, the 10 under the envelope and 14.

**Check.** `check-ui` (halo) measures every ring of 2 px or more and
every glow to the ink of the nearest text that isn't its own. The ink,
not the line box: the line box holds the font's whole descent, and digits
have none. Scops' lamp digits "03:48" read as 1.9 px into a glow whose
visible gap was 18 px (2026-11-16). `check-ui` (inset) measures, on every
filled surface with text, each filled box painted over one of its edges:
from that box's edge to the nearest text must be at least the surface's
smallest padding on its other sides. The painted extent of drawings (a
disc behind a mascot) is still a project's own script
([ENFORCEMENT.md](../ENFORCEMENT.md)).

## Centred means on the axis

**What.** A centred line of text sits on the centre axis of its column.
Centred lines inside a box that hugs one side are not centred. In a
column where everything else is left-aligned, a lone centred line is
wrong too: it follows the column.

**Why.** On Scops' first-night screen (2026-11-15), "Děláte to z
lásky…" ("You do this out of love…") had centred lines in a narrower box
hugging the left edge. It looked "as if it wanted to be in the middle,
but somehow isn't". Everything else on that screen was left-aligned.

**Apply.**
1. A centred block takes its column's full width, or is centred in it.
2. In a left-aligned column, align every line to the left.

**Check.** `check-ui` (centre) flags centred text blocks whose box is
more than 3 px off the column's axis. It skips labels inside targets,
whose icon offsets them on purpose.

## Rank what the screen says

**What.** Text that carries different facts is split and ranked, not run
together. What a person looks for leads. What can wait goes quieter,
shorter, or behind a tap. Nothing is dropped: the full fact stays one tap
away, and a screen reader always hears it whole.

**Why.** On Obálka, a data-box service sends a self-employed sender as one
string: "Jan Novák - podnikající fyzická osoba". The envelope printed it
as one heading. The legal form was as loud as the name and took three
lines of a 120 px block. The owner asked for the name first, and a rule:
too much at once confuses, so detail comes when it is needed, and what
shows when is chosen with care (obalka 556e19e).

**Apply.**
1. For each text, list the facts in it: a name, a legal form, an ID, a
   date, a place.
2. Rank them by what the reader is looking for on this screen. A list row
   needs the name. A detail may need the ID.
3. Give the first fact the strong role. Put the rest on a quieter line, in
   a smaller role and a softer colour.
4. Shorten what readers know by its short form ("PFO", not "podnikající
   fyzická osoba"). A tap, a tooltip or a long press shows it in full.
5. Never drop a fact to save room. The screen reader's label says it in
   full, and one tap or one level deeper shows it.
6. Rank by what tells things apart, not by habit. Obálka's address keeps
   the town bold and the street quiet: the town tells three Jan Nováks
   apart.
7. Decide what shows where, and when, on purpose. Write the ranking down
   in the design doc, with the reason.

**Check.** No text block mixes facts at one weight. For each quieter or
shortened fact, show where the full one is: the tap, the next screen, the
spoken label.

## A shortcut view says it is a shortcut

**What.** A view that shows only part of a range names the part ("next
two weeks") and offers the whole ("Full month ›").

**Why.** Scops (2026-11-15) offered a one-off event's next 14 days as two
rows. The owner asked whether it grows on tap. Nothing said there was
more.

**Apply.** Give every partial view a title that names its part, and an
action that opens the whole.

**Check.** For every list, strip or grid that shows part of a range, the
screen names the part and links the whole.

## A number says what it counts

**What.** A bare number reads as the value of the nearest label. So
every number on screen says what it counts ("Takes 2.1 GB", "3 fixes"),
and never sits right after a label or control it doesn't belong to.
Put the number with its own subject, or give it its own noun.

**Why.** The Windows-app runner's window footer (2026-11-13) read
"Technical log 2.1 GB". The 2.1 GB was the app's size on disk, but it
sat after the log link, so it read as a 2.1 GB log. The owner: "this
kinda seems as if the technical log was 2.1 GB large, which is a
nonsense, obviously". Two reviewers and the designer had passed it,
because each knew what the number meant.

**Apply.**
1. For every number on screen, cover what you know and read only its
   neighbours: what does it count?
2. Give it a noun or a verb when the neighbour is wrong: "Takes 2.1 GB",
   "640 MB in all", "3 fixes".
3. Keep actions apart from facts: a link or a button looks like one, and
   a fact is not glued to it. A divider or space between unrelated items.

**Check.** Read each number aloud with the word before it. The sentence
is true.

## One reading only

**What.** Every word, number and control on a screen has one meaning. A
limit says what it limits and what happens when it runs out.

**Why.** After a failed nap, Scops (2026-11-14) told the parent: "try
again, at most 20 minutes". The owner could not tell if the 20 minutes
meant playtime or the second attempt, and had to ask. That is the cost:
a tired parent at 14:30 has no one to ask.

**Apply.**
1. Read each line as a newcomer. Write down every meaning it can have.
   More than one: rewrite it.
2. A number names its subject and its consequence: "if he isn't asleep
   in 20 minutes, end the attempt", not "at most 20 minutes".
3. A counter says what it counts against: "12 min, of the 45 he has to
   fall asleep", not "12 min of 45".
4. A setting reads as a sentence: "If he isn't asleep, end after
   45 min", not "Longest: 45 min".
5. Fix the wording everywhere it appears: the guide, the button, the
   counter and the setting.

**Check.** Show each screen to someone who hasn't seen it. They explain
every number and limit in their own words, with no follow-up question.

## A counting mark keeps one scale

**What.** A mark that counts something (one tick is one minute) has one
scale everywhere. When the marks don't fit, show fewer, never denser.

**Why.** Scops (2026-11-14) draws minutes as ticks, and later as beads.
Squeezing them to fit made the same mark mean something different on
each screen.

**Apply.**
1. Write down each counting mark and its unit in the design doc.
2. When a count doesn't fit, show the last marks and a number for the
   rest. Never shrink the marks.

**Check.** Every appearance of a counting mark, side by side, has the
same size and spacing per unit.

## Decoration never half-hides what you read or click

**What.** A row, a label or a control is either whole or clearly
leaving (a fade at a scroll edge, the way the system does it). It is
never cut by an illustration in front of it, and text never sits on a
drawing's mid-tones. A playful overlap reads as a rendering fault.

**Why.** The Windows-app runner's sidebar (2026-11-13) let app rows
scroll behind the nest's front rim, "like eggs settling in". It was the
owner's own idea and read well in words; drawn, the owner called it
"horrible": "Drop an installer" was barely visible on the nest's ochre,
and the half-tucked app icon looked "like almost being clipped". Two
reviewers had passed it.

**Apply.**
1. Where content scrolls under a drawn shape, end the list above the
   shape instead, with the platform's own fade.
2. Put text on a flat surface of the window, not on an illustration; if
   it must sit on one, give it its own solid plate and check 4.5:1.
3. Before recommending an idea that came as words, draw it at the real
   size, mid-scroll and at rest, and judge the picture, not the idea.

**Check.** Screenshot the scroll at several offsets. No frame shows a
row partly covered by a drawing. `check-ui` (contrast) measures every
label against the exact pixels behind it, drawings and gradients
included, and skips text that something opaque covers.

## Nothing anyone reads under 11 px

**What.** Text that carries information is at least 11 px: dates, tags,
counts, key hints. Monospace is for data and measurements, not for
decoration.

**Why.** An automated scan of tern's canvas found about twenty labels at
9.5 to 10.5 px, among them date headers, row dates, status tags and key
hints. Uppercase monospace at that size blurs on a 1x screen, and those
labels carry real information.

**Apply.** Set an 11 px floor in the type scale. Smaller sizes may only
draw glyphs that sit inside a shape, like an avatar's initials.

**Check.** `check-ui` (text-size) flags every text run under 11 px at the
size it is drawn: SVG text through its transform, and HTML type through
any `transform: scale()` around it (a scaled preview). A glyph inside a shape is marked
`data-allow="text-size: …"`.

## One anatomy per job

**What.** Each job has one component with one anatomy. Every toast looks
the same. Every secondary button looks the same. Variation lives in the
words, not the shape.

**Why.** Tern had two toasts: an ink pill for "done", and a white card
with a progress bar for "sending". Same job, two looks. The owner picked
the ink pill for all toasts; the sending one counts down in words.

**Apply.**
1. For each component, list every place it is used.
2. If two places use different anatomies for the same job, pick one and
   redraw the other.
3. Write the anatomy down: parts, order, sizes, the action's place.
4. Name each component in the markup with a class: `b-main`, `b-sub`,
   `b-ghost`, `b-chip`, `b-day`, `b-day-adj`, `b-val`, `b-row`. A variant
   that is meant to differ (a muted day of the next month) gets its own
   class, and the design doc gives its reason. The design doc's list of
   components and the classes are the same list.

**Why the classes.** `check-ui` (same-look) compares like with like:
the same tag, classes and role. Scops' buttons had inline styles and no
classes (2026-11-16), so every button on a screen was compared with
every other: the main button, the child switcher, a calendar day, a
chip. That gave 71 findings, mostly noise around a few real ones: "Utišil
se" in 800 weight beside "Jít za ním hned" in 700, in one row; inputs 56
px high with 18 px type on one screen and 52 and 16 on the next.

**Check.** The design doc lists each component's anatomy, and the canvas
shows no second version. `check-ui` (same-look) flags like targets (the
same tag, classes and role) of different height, type, radius or padding.
A button with no words (an icon button) is a kind of its own, compared
by height, radius and padding only. When the like targets have no class,
the finding asks for the component's name first.

## A choice changes everything it owns

**What.** When a choice changes, everything that belongs to it changes
too: the fill, the text, the icon colours and any field filled from it.
One kind of control has one look, before and after a click.

**Why.** In the Scops prototype (2026-11-15), picking "Guests" swapped
the button fills, but the icons kept their old colours. A light icon sat
on a light chip, and the Name field still said "Swimming". The owner saw
it as a contrast bug. In the year and month picker, the year buttons
came out flat after a click while the months kept their depth. The
prototype wrote the colour back in a form the depth layer no longer
recognised. The years were also a different size (48 by 17 px, not 52
by 16). The owner asked why two similar things looked different. There
was no reason.

**Apply.**
1. Colour icons inside toggles with `currentColor`, and the accent with
   a variable the toggle sets.
2. List every field a choice fills, and update each one on change.
3. Build like controls from one component: the years and months in a
   picker are the same button.
4. In a prototype, click every option in every group. Compare the
   clicked prototype with the board, not only the untouched one.

**Check.** A test clicks each option. It checks the contrast of its
icon, the value of each dependent field, and that its size and style
match its siblings'. On boards, `check-ui` (state) flags a focus, hover,
active or pressed state that moves or resizes what's inside it.

## Show it in real context

**What.** Proposals are drawn inside the real frame: the real window
header (with the system's own window buttons on macOS), a phone's real
status bar, the real neighbours, real copy, both themes, and the longest
shipped language.

**Why.** On a phone app, the owner asked for real status-bar icons before
choosing, to judge the spacing. On tern, a flow drawn as loose cards
looked like "another app". In the app it already sat in the right frame,
and the drawing misled the review.

**Apply.**
1. Copy the frame from the live design or the app. Never redraw it from
   memory.
2. A new product has no live design yet. Take the frame from the
   platform's own design kit (Apple's design resources, Material's
   kit), and say which one on the board. Scops' status bars were drawn
   from memory (2026-11-14), because nothing else was at hand.
3. For a phone, the frames include the status bar and Dynamic Island,
   the lock screen with a Live Activity, and Android's status bar with
   an ongoing notification.
4. Show a pseudo-locale for the longest strings.

**Check.** Every proposal shows its frame and both themes. The board
names where its frame came from.

## Both safe areas belong to the frame

**What.** The phone frame reserves the system's areas at the top and at
the bottom, in one shared place:
- top: the status bar and the Dynamic Island;
- bottom: the home indicator, 34 px on an iPhone.

Content and buttons end above the bottom area, with at least 8 px to
spare. A screen too dense to fit scrolls, with its main action pinned
above the home indicator. It is never squeezed into the reserved area.

**Why.** The owner noticed that Scops' "Pokračovat k volbě metody"
(Continue to the method) sat almost on the bottom edge (2026-11-15). A
search found 21 of 85 screens with a button 16 to 28 px from the bottom,
inside the home indicator's area. There were three causes:
1. **The frame was half done.** The generator drew the top safe area in
   one shared place. The bottom one was drawn only inside the tab bar.
   Screens without a tab bar had no home indicator, and each chose its
   own bottom padding (14 to 28 px).
2. **The boards don't scroll.** A fixed 844 px board pushes the designer
   to make everything fit. Dense screens were squeezed instead of
   scrolling.
3. **The checks looked at the wrong edge.** The audit caught overflow
   outside the board, not intrusion into the system's areas. The
   reviewers saw scaled-down sheets without a home indicator.

**Apply.**
1. One frame component draws both safe areas on every phone board. It
   always draws the home indicator, tab bar or not.
2. Its layout reserves the bottom area: the content's bottom padding is
   the safe area plus at least 8 px.
3. If content doesn't fit, it scrolls. The main action is pinned above
   the safe area, with a short fade over the scrolling content.
4. Bottom sheets add the safe area to their bottom padding too.

**Check.** `check-ui` (safe) measures every target in a phone frame,
marked `data-frame="phone"`. It flags anything that ends less than 34 + 8
px above the frame's bottom, or that starts under the status bar (54 px).
A tab bar (`nav` or `role="tablist"`) sits on the safe area itself, so it
needs the 34 px only. What scrolls inside the frame may pass under the
home indicator. A frame that draws no home indicator (marked
`data-home-indicator`) is a finding. So is a board drawn at a phone's
width (480 px or less) with no frame marked. Other sizes go on the
frame: `data-safe-top="24" data-safe-bottom="48"`.

## A sheet's page is inert, and the board says so

**What.** While a sheet or dialog is open, the page behind it takes no
taps and no screen reader reads it: it is `inert` in the product. A board
that draws an open `role="dialog"` says the same, with
`aria-hidden="true"` on everything drawn before the dialog in its screen.

**Why.** Scops' screens with an open sheet produced dozens of "targets
overlap" findings (2026-11-16). They were buttons of the page behind the
scrim. A real app makes that page inert; the boards didn't say so, so a
check couldn't tell the page from the sheet. Marking them `inert` on the
boards brought false contrast findings instead (1.9:1 under the scrim):
Chrome's hit test skips inert elements, so the scrim inside the page no
longer covered its text. `check-ui` now skips both.

**Apply.**
1. In the product, set `inert` on the page while a sheet is open, and
   clear it when the sheet closes.
2. On a board, mark everything drawn before the dialog in its screen
   `aria-hidden="true"`. Do it in one place: the board writer (Scops'
   `finish()`), never by hand per screen.
3. A dialog that isn't modal (a popover over a live page) is marked
   `data-allow="sheet: …"`.

**Check.** `check-ui` (sheet) flags an open dialog (`role="dialog"`,
`role="alertdialog"`, `<dialog open>`) while a target drawn before it in
its screen (its `data-frame`, or its parent) is still live. The targets
and text of an `aria-hidden` or `inert` page aren't measured.

## Words in the product's voice

**What.** Screens speak in the product's own voice. Dialogs use little
text and a button that names and does the action. Serious moments get
calm words, not jokes. Who speaks (the app, never "we", except a
companion's inclusive "we") is in [common/voice.md](../common/voice.md).

**Why.** Tern's first-run lines carry its idea ("Bring your mail home.",
"Only you hold the key."). Its inbox zero carried a meme, which a review
flagged as wrong next to a bank statement. The owner chose "All home.
Nothing waits on you."

**Apply.**
1. Write buttons as verbs with their object: "Empty Trash", not "OK".
2. Keep body text to one or two short lines.
3. Read every line aloud next to the most serious message the product
   handles. If it jars, rewrite it.
4. Translate it, and check the translation for the same tone.
5. In languages with gender and case, use full forms that agree, never
   a neutral shortcut. Write "She woke up and is crying", not "Waking".
6. Decline names. Guess the forms ("Elišce" from "Eliška"), and let the
   user correct them.
7. The user's own gender sets past tenses and adjectives. Czech: "jste
   ho uložila / uložil / uložili" (you put him down).

**Why the full forms.** The practices knew "the longest language", but
not inflection. On Scops (2026-11-14) the owner asked for the harder
full forms on purpose: they read as care.

**Check.** No "OK" or "Yes" buttons (`check-ui` outcome, see
[Every button names its outcome](#every-button-names-its-outcome)). No
jokes on screens that can show sensitive content. Every screen that
mentions a person is on the board in every gender. The grammar between
words that a script can decide ("V 10:20", "ve středu") is
`check-copy`'s, per language
([common/voice.md](../common/voice.md#each-languages-grammar-is-checked)).

## Answer the next question where it arises

**What.** When a step can fail or leave the user unsure, the screen
where it happens says what to do next. Not an FAQ, not a help page.

**Why.** After a failed nap in Scops (2026-11-15), the owner had to ask
what to do when the baby is sleepy again ten minutes later. The owner:
"not in an FAQ, but right where they act". The fix put three steps on
the "didn't fall asleep" screen. Ten minutes later, the app asks "Is he
sleepy again?".

**Apply.**
1. For every action, write the question the user will ask right after
   it.
2. Answer it on that screen, in two or three steps, with the next action
   as a button.
3. If the answer depends on time ("in ten minutes"), the app comes back
   and asks. The user doesn't have to remember.

**Check.** Walk each flow and ask "what now?" after every action. The
screen answers it.

## Motion with a reason

**What.** A motion tells a small story tied to the product's idea, plays
at the moment it is about, then rests. Every motion has a reduced-motion
form.

**Why.** Tern's bird pecks pebbles at the shore on an empty list, and
carries a letter like a fish while mail downloads. Both are tied to its
idea, so they read as serious and specific. A generic spinner would
have said nothing.

**Apply.**
1. Use the product's own mark or objects instead of a generic spinner
   wherever it waits. Every wait over about a second shows it, and a
   pressed control shows it too ([feedback.md](feedback.md)).
2. Play one-off moments once. Loop only while work is actually
   happening.
3. Give every motion a still frame for reduced motion that still makes
   sense.

A resize is motion too: its frames never show squashed words
([below](#a-resize-never-shows-a-squashed-frame)).

**Check.** `check-ui` (motion) renders every board with reduced motion,
and flags any animation that still loops there. A loop that is the
content itself is marked `data-allow="motion: …"`. Every other check
measures the same reduced-motion render. Whether every screen still
reads correctly with reduced motion, and whether a motion tells the
product's story, is review.

## A resize never shows a squashed frame

**What.** When a window or a panel changes size, its content is laid out
at the new size for every frame it shows. No frame shows content scaled
to a size it wasn't laid out for: words never stretch or squash, even
for a tenth of a second.

**Why.** Gentes' Settings window shrinks to fit each tab. The owner,
2026-11-18: "I noticed i opened Gerntes settings and went to the Privacy
tab (which is not as tall as the first tab), and the contents were
squashed before they re-rendered and text was looking good again, as if
something was rendered first at 150 px (an exampple), the squashed to
like 80 px and then re-rendered at 80 px to be nice". Then: "make this
last re-rendering issue another rule and check pls". The window changed
size in code, and macOS scaled the last picture it had into the new
bounds until the app drew again, a frame or two later. Gentes f5c8854
pins that picture to the window's top left while it resizes: cropped,
never scaled.

**Apply.**
1. Resize without the system's frame animation, or lay the content out
   at every frame of it.
2. Follow what the platform does with tabs. macOS Settings windows
   animate their height between tabs, with the content laid out at every
   frame; a fixed-size window is the other way.
3. Animate a panel's size (its height, a clip, a crossfade), never a
   scale of its content. A scale that keeps the shape (a popover
   appearing) is fine.
4. Record the resize frame by frame (a screen recording, or animations
   slowed down) and look at each frame. The words keep one size
   throughout.

**Check.** `check-ui` (squash) flags an element holding words that is
drawn scaled out of its shape (`scaleY(.7)`), and one that plays a
`@keyframes` rule that would scale it so, under any media. A drawing
whose squash is its idea, with no words in it, isn't measured. The app's
own resize is review: record it and look at every frame.
