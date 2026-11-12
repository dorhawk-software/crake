# An identity, in iterations, through the whole app

How to give a product its identity, and how to carry it into every
screen. It is written to be followed step by step, by a person or a coding
agent. The rules for the drawing itself (outlines, stroke ends, type
sizes) are in [visual-craft.md](visual-craft.md).

The lessons come from three passes in one day (2026-11-11): Obálka (a
bland app, pushed far), Crake (a strong identity, polished) and tern (a
strong identity, first wrongly limited to its logo). Product details here
are labelled examples. The rules hold for any product.

The work runs in three iterations, each agreed with the product's owner
before the next starts:

1. **Establish the identity**: research, facts, one physical idea, its
   objects ([below](#iteration-1-establish-the-identity)).
2. **Redesign a few parts** toward it, show them, iterate until agreed
   ([below](#iteration-2-a-few-parts-shown-iterated-agreed)).
3. **Redesign the whole app** against the agreed spec, re-audit, iterate,
   polish ([below](#iteration-3-the-whole-app-re-audited-and-polished)).

## Identity first, before the first screen

**What.** Ideally the identity is researched before anything is built,
and the app is polished from its first screen. A redesign after the app
exists should not be needed. The research covers who the app is for, what
it does and for which process, what inspires it, and what the market
already looks like.

**Why.** All three passes of 2026-11-11 polished apps that already
shipped screens. Each one meant redrawing work that was already built and
tested. The owner: "in an ideal world, the polish after an app is already
done should NOT be necessary at all". Products differ too much for one
look: Shrike and Crake are different kinds of product, aimed at
different processes.

**Apply.** Before the first screen of a new product, write a one-page
identity brief:
1. **Audience.** Who uses it, how often, in what setting and light, with
   what at stake (their money, their mail, their network).
2. **The job and the process.** What the app does, step by step, from
   the user's side. The identity should fit this process, not a
   category.
3. **Inspiration.** The references the owner holds up, and why each one.
   Look at them before drawing.
4. **The market.** Five to ten products in the same space. Note the
   category's defaults, so you can avoid looking like all of them.
5. **The idea and its facts.** The physical idea (see the bar below), and
   the facts about it, gathered from real sources.
6. **Tone.** How serious the product must be, with one sentence on what
   would go too far.

Show the brief to the owner and agree on it. Then design the first
screens with it. For an existing product, write the brief first anyway:
it is iteration 1.

**Check.** The project has the brief, and the owner agreed to it before
the first designed screen.

## Get the facts right

**What.** When the identity borrows from the real world, draw the real
thing. Research what makes it recognisable, and keep those features as
the drawing gets simpler.

**Why.** Tern's logo was a stock bird icon. The pass redrew it from what
a tern really looks like: a black cap that runs into a long, sharp bill,
long narrow wings, and the forked tail, kept for the flying bird. The
owner "really loved" that it went for the facts. Obálka's printed-matter
objects (an envelope's window, a postmark, a stamp) worked for the same
reason: real postal objects people recognise.

**Apply.**
1. Look up the real thing in reliable sources: field guides, museum
   collections, real objects, photographs.
2. List its two or three identifying features: the ones someone would
   name first.
3. Keep those features at every size the mark ships at. Simplify
   everything else.
4. Check the drawing against a photograph, then at 16 px.

**Check.** The design doc names the identifying features and shows the
drawing at its smallest size with them still visible.

## Iteration 1: establish the identity

The identity brief above, then the two rules below: the bar, and how far
to push.

### The bar: serious, with one physical idea

**What.** An identity pass gives a product its own look and feel: drawn
icons, a material or a physical metaphor, considered buttons and sections,
and one or two signature motions. It stays serious. It is playful but never
gamified, because people trust these apps with their mail, money or
network.

**Why.** The owner judged three apps against two references: a sibling
app with a strong look, and the company website (an envelope, a cellophane
window). Next to them the apps "lacked identity". One proposal then went
too far: dates drawn as rubber stamps on a government mail app's inbox
rows. The owner called it "taking it too far". A serious product that
looks like a toy loses trust.

**Apply.**
1. Name the references the owner holds up, and look at them first.
2. Find the product's one physical idea in its own name or job. Examples:
   printed matter for a postal app (envelope, window, stamp, postmark); a
   line the product keeps to, for a VPN that routes traffic; a seabird
   that brings mail home, for a mail client.
3. List the objects that idea gives you. Give each object exactly one
   job. Example: the envelope's window shows who it's from; the postmark
   shows when it came.
4. Put identity into those few objects only, never into every piece of
   data. A date stays a date.
5. Test each idea against "would I trust this app with my bank's
   letters?". If not, cut it.

**Check.** You can name the idea in one sentence and list its objects,
each with its one job. No object appears without a job.

### Scale of change follows the identity; scope is always the whole app

**What.** Judge how much identity the product already has before you
propose anything. A bland product gets pushed far. A product with a
working identity keeps its look and tokens and gets precise polish, plus
one or two objects that grow out of what is already there. Either way,
you review every screen.

**Why.** Crake already had a strong look: a line along its left edge. The
pass kept the look and grew that line into a timeline with marks. The
owner praised it for "staying true to the product", with "merely very
good polishes". Tern also had a defined design, so the pass first redrew
only its bird (logo, icon, tray, two empty states). The owner called that
"very naive": a defined design can still be improved. The full review
then found real gaps. The identity was missing from the screens used most
often. Boxes were creeping in. One flow looked like another app. The
canvas had drifted from the code.

**Apply.**
1. Rate the product: bland, partial, or strong. Write one sentence on why.
2. Bland: propose a new idea and its objects (see the bar above).
3. Partial or strong: keep the palette, type, tokens and voice. Find what
   is already there that can carry more meaning. Grow one or two objects
   out of it.
4. In every case, run the whole process below on every screen. "Polish
   only" sets how big the changes are, not whether you look.
5. Finding nothing to change is a valid result. Skipping the look is not.

**Check.** The proposal states the rating and lists every screen that
was reviewed, with a verdict for each.

## Iteration 2: a few parts, shown, iterated, agreed

**What.** Apply the identity to a few parts first: the logo and the
screens that define the product. Show them to the owner, iterate, and
agree. The agreed parts become the spec for the rest of the app.

**Why.** This is how the three passes actually went: the identity, then
a few screens, shown and iterated until the owner agreed. A few parts
are enough to agree on the idea and the polished spec. They are not
enough to call the product done; that is iteration 3.

**Apply.**
1. Survey every screen and board first. Read each one's source, not only
   its picture. Note where the identity is strong, generic, or missing.
2. Run an independent critique of the whole surface before proposing:
   two isolated reviewers, one doing a design review and one running an
   automated detector. Neither sees the other's output.
3. Verify every factual claim in the critique against the code before you
   report it. Reviewers get facts wrong. On tern, a reviewer said three
   undo windows disagreed. They were one window caught at three moments.
   Another said a flow looked like another app. The app already framed
   it correctly; only the canvas lagged.
4. Draw two or three variants for each open choice, in place, with real
   context. Real context means the real header, the real status bar, real
   copy, both themes. The owner judges spacing from it.
5. Recommend one variant per choice and say why in one line.
6. Ask with concrete options. Ask at most four questions at a time.
7. After the pick, remove the other variants and their labels from the
   canvas ([design-before-build.md](design-before-build.md)).
8. Polish the picked design: a second critique, then fixes, then look at
   it again.
9. Repeat steps 4 to 8 until the owner agrees the parts are right.
   Expect several rounds.

**Check.** Every choice on the canvas has exactly one design left. The
critique report and its verdicts are saved where the next pass can read
them.

## Iteration 3: the whole app, re-audited and polished

**What.** Only once iteration 2 is agreed, redesign the whole
application against that spec. Redraw everything that falls short. Show
it to the owner and take their tweaks. The pass is done when the whole
app matches the spec, not when the showcase screens do.

**Why.** The owner, after the three passes: the redesigned parts show
"the idea of how we want to take the product further". What must follow
is an audit of "the entire application, its all sections, clickity-
clacketies, modals, animations, processes", then iterating and
re-auditing until it is polished.

**Apply.**
1. Build an inventory from the code, not from the canvas. List every
   route, window, view, dialog and popover.
2. For each, list its states: empty, loading, partial, full, error,
   offline, read-only, first run, signed out.
3. List every control and its states: rest, hover, pressed, focus,
   selected, disabled, busy.
4. List every transient surface: toasts, menus, tooltips, confirmations,
   notifications, the tray or menu-bar icon, the app icon at every size.
5. List every process end to end: onboarding, sign-in, adding an account,
   sending, syncing, undo, deleting, settings changes, quitting.
6. List every motion: transitions, progress, empty-state scenes, and what
   reduced motion shows instead.
7. Walk the list in both themes and in every shipped language, including
   a pseudo-locale for long strings.
8. Grade each item against the spec: matches, drifts, or misses the
   identity. Use the rules in [visual-craft.md](visual-craft.md).
9. Redraw what drifts or misses. Put it on the canvas with variants when
   the choice is open. Ask the owner. Strip the losers.
10. Fix canvas-versus-code drift as you find it: stale states, leftovers,
    features the canvas still calls "soon".
11. Build the agreed designs one to one, then capture the app and compare
    it with the canvas ([design-before-build.md](design-before-build.md)).
12. Re-audit the built app with a fresh critique. Iterate with the owner
    until the critique and the owner find nothing left to fix.

**Check.** The inventory exists as a checklist, and every line is marked
matches or fixed. The owner has seen every redrawn item. The last
critique of the built app found nothing new.

## Where the product's mark appears

**What.** The product's own drawn mark (a mascot, an emblem) appears only
at meaningful moments. Those are usually the moments its idea is about.
Everywhere else the interface stays quiet.

**Why.** Tern's bird stands for bringing mail home. It was there for
fetching mail, but absent at inbox zero (the moment people remember,
by the peak-end rule) and when mail is sent. Inbox zero showed a joke
line instead: "Go touch some grass." The owner picked the bird landing on
the shore for inbox zero, and a small bird carrying the letter in the
sending countdown. Nothing else got the bird.

**Apply.**
1. List the moments the idea is about. For a mail client: mail arriving,
   mail leaving, everything handled.
2. Give the mark those moments, and only those.
3. Let the copy go quiet where the mark speaks. Prefer a calm line in the
   product's voice to a joke. A joke next to a bank statement reads wrong.
4. Use one motion per moment, played once. Then rest. Loop only while
   something is actually working.

**Check.** You can list every place the mark appears, and each one is one
of the idea's moments.

## Before you show anything

**What.** A short pass over every page you are about to show the owner.

**Why.** The owner zooms in. They found leftover thin lines, a stroke
with one round end and one square end, a stray word printed on a board,
and a designer's note inside a mock's progress card. Each one costs trust
in the whole proposal.

**Apply.**
1. Look at the rendered page, not the source. Zoom into every mark.
2. Count the borders and boxes. Remove each one that tone, shadow or
   spacing could replace.
3. Check the top 200 px of each screen for dead space, and the bottom of
   each board.
4. Search the page for leftovers: placeholders, `[N]`, notes to yourself,
   text in the wrong language, states the code no longer has.
5. Check every claim in captions against the code.

**Check.** The handoff message says these checks ran.
