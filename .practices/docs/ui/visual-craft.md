# Visual craft

The rules for drawing an interface that looks made, not generated. They
apply to any product and any toolkit. The process that uses them is in
[identity-pass.md](identity-pass.md).

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

**Check.** Search the styles for borders and inset rings. Each one left
is a focus ring, a checkbox, or has a written reason.

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

**Check.** Search the styles for font sizes under 11. Each hit is a glyph
inside a shape.

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

**Check.** The design doc lists each component's anatomy, and the canvas
shows no second version.

## Show it in real context

**What.** Proposals are drawn inside the real frame: the real window
header (with the system's own window buttons on macOS), a phone's real
status bar, the real neighbours, real copy, both themes, and the longest
shipped language.

**Why.** On a phone app, the owner asked for real status-bar icons before
choosing, to judge the spacing. On tern, a flow drawn as loose cards
looked like "another app". In the app it already sat in the right frame,
and the drawing misled the review.

**Apply.** Copy the frame from the live design or the app. Never redraw
it from memory. Show a pseudo-locale for the longest strings.

**Check.** Every proposal shows its frame and both themes.

## Words in the product's voice

**What.** Screens speak in the product's own voice. Dialogs use little
text and a button that names and does the action. Serious moments get
calm words, not jokes.

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

**Check.** No "OK" or "Yes" buttons. No jokes on screens that can show
sensitive content.

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
   wherever it waits.
2. Play one-off moments once. Loop only while work is actually
   happening.
3. Give every motion a still frame for reduced motion that still makes
   sense.

**Check.** Turn on reduced motion. Every screen still reads correctly.
