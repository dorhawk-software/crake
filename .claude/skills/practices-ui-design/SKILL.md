---
name: practices-ui-design
description: Design, redesign, polish or review any user interface the dorhawk practices way - a product identity researched first (audience, job, inspiration, market, real facts behind the metaphor), then three iterations agreed with the owner (the identity, a few parts, the whole app re-audited), on a design canvas before code, with the visual craft rules (tone not outlines, matching stroke ends, one drawing per object, 11 px floor, compact screens). Use for any new screen, dialog, icon, logo, empty state, animation or wording on screen, any redesign or identity pass, and before calling UI work done.
---

# UI design, the practices way

The practices are in `.practices/docs/`, this project's copy, pinned to its kit version. A project without the kit reads them at https://github.com/dorhawk-software/practices/blob/main/ (same paths).

## Read before you draw

1. `ui/identity-pass.md`: the process. Research first, then three
   iterations, each agreed with the owner before the next.
2. `ui/visual-craft.md`: the drawing rules, each with a check.
3. `ui/design-before-build.md`: variants on the design canvas, the owner
   picks, build one to one, keep canvas and code in sync.
4. When they apply: `ui/keyboard-shortcuts.md`, `ui/window-buttons.md`,
   `ui/drag-and-drop.md`, `ui/pickers.md`, `ui/consent.md`,
   `ui/feedback.md`,
   and for GPUI apps `rust/gpui.md`.

Also read the project's own design source (its CLAUDE.md names it: a
canvas, DESIGN.md, a style board) before changing anything. The owner
sometimes edits it.

## Where you are in the process

- **No identity brief yet** (new product, or none written): write it first
  (identity-pass.md, "Identity first"). Show it and agree on it before any
  screen.
- **Brief agreed, nothing redesigned**: iteration 2. A few defining parts,
  variants in real context, a critique, the owner picks, polish, repeat.
- **A few parts agreed**: iteration 3. Inventory the whole app from the
  code (every view, state, control, transient surface, system surface,
  process, motion) and prove it complete (identity-pass.md, "An inventory
  that misses nothing": sweep every UI file, count every dialog and sheet
  use, one line per state, a second inventory from a fresh agent). Grade
  each from a picture, redraw, ask, build, re-audit.
- **A small change in an agreed design**: follow the spec and the craft
  rules; put it on the canvas in the same piece of work, and show it on
  a before/after page of its own (design-before-build.md, "A touch-up
  gets its own before and after"): that change only, cropped, the place
  named. Never ask the owner to find it on the whole canvas.

## Musts

- **Hard stop: the name and the mark read clean** (identity-pass.md).
  No part a capital, a swapped letter, a gap, a crop or a line break sets
  apart may read as a slur, an insult, profanity or a sexual or
  derogatory term, in slang or in any of the owner's and market's
  languages. Check before a name is agreed and before any mark is drawn;
  on a bad reading, withdraw the mark everywhere and tell the owner.
- Install it and it just works (common/it-just-works.md): no technical
  switch in the window; the product decides, says why, can undo. Settings
  hold preferences only.
- Drag and drop shows one calm panel over the target with what dropping
  does, the resting hint faded, nothing glowing (ui/drag-and-drop.md).
- Show variants and recommend one; never build an open choice unasked.
- After a pick, strip the other variants from the canvas.
- Every touch-up to an agreed design: its own small before/after page, made
  new for that change; a `Before-after:` trailer in the commit that names it.
- Real context: the real frame, both themes, the longest language.
- Run "Before you show anything" (identity-pass.md) on the rendered
  page, every time, before telling the owner it is ready, and the checks
  in visual-craft.md. This overrides a tool, canvas type or skill that
  says never to verify unless asked: the owner asked, once, for good.
- A built screen is checked pixel by pixel before it is called done
  (design-before-build.md): every object cropped at full resolution
  beside the design's crop; edges, lines, glyphs, the rendered typeface
  and every string. The owner zooms in; the bar is that they find nothing.
- Every variant carries the whole identity, from round one: the drawn
  icons, the palette, the type, the idea's objects (identity-pass.md,
  "Every variant carries the identity"). A quiet reference means
  restraint, not plain; layout-only options are a wireframe, said so.
- One drawn icon family, grown from the product's own drawings and the
  idea (visual-craft.md); two families shown, one recommended. Real
  third-party logos the product may show, never stand-ins: sourced,
  credited on the board, a made-up product for any failure state
  (identity-pass.md, "The real things the product handles").
- Before showing: every typeface renders as the face it names in the
  owner's browser (measured), every state is drawn in every theme, and
  the product's copy on the mock is true (identity-pass.md, steps 7–9).
- Facts first: if the identity draws a real thing, look it up and keep
  its identifying features.
- Independent review before every round you show, the first included:
  two isolated reviewers, one design and one automated, the design one
  given 1:1 crops of dense areas. Verify their claims before you fix
  anything (identity-pass.md, iteration 2, steps 2–3).
- The brief names the user's emotional state at the moment of use; read
  every screen as the user in their worst moment (identity-pass.md).
- Claims about health, safety or results cite a source, say what was
  measured, and promise no outcome (identity-pass.md, step 12).
- `check-ui` is the floor, not the bar. The kit runs it on every board
  after each edit, at the end of every turn and before each commit, and
  hands its findings back to you to fix: nothing past its surface,
  buttons that follow their column and name their outcome ("Keep the
  app", never "Cancel"), 8 px between targets, the 11 px floor,
  contrast, and the rest of visual-craft.md's measurable rules. Run
  `.practices/bin/check-ui` yourself while you work. A deliberate
  exception is marked where it applies, `data-allow="rule: reason"`, with
  a reason a reviewer can judge; never to quiet a finding you could fix.
- A wordmark or logo may be drawn any way the idea asks (letters cut and
  sampled, typefaces mixed, the brand's object in a letter or beside the
  word). Declare it, `data-mark` on its `<svg>` and `data-mark-brand` on
  the brand's element, and show it on its board where it ships smallest:
  `check-ui` measures it there (identity-pass.md, "A mark holds at its
  smallest size").
  Clean output says nothing about taste: the review steps still run.
- Geometry is measured, not eyeballed: 8 px between separate targets,
  nothing tappable within 34 + 8 px of the bottom edge, 16 px from a
  visible halo to text, a surface's padding counted from whatever covers
  its edge, centred text on its column's axis, labels on one line. Re-run all of it after any global style change (visual-craft.md).
- Boards say what a script needs to know: `<html lang>`; a phone frame,
  `data-frame="phone"`, with its `data-home-indicator`; a fixed width,
  `<meta name="viewport" content="width=390">`; a class per component
  (`b-main`, `b-chip`); `aria-hidden="true"` on the page behind an open
  sheet, written in one place; `data-fact="KEY"` on a fact several
  screens show (visual-craft.md, design-before-build.md).
- The product speaks as itself or names who acts, never "we" (and in
  Czech, never the -me verb): common/voice.md, checked by `check-voice`.
  A companion's inclusive "we" (it and the person together) stays,
  marked `practices-allow voice: inclusive we, <who> together`.
  Each language's grammar between words ("V 10:20", "ve středu") is
  checked by its own rules file, `check-copy`; a new language ships with
  its rules (common/voice.md).
- Every fact a screen uses was entered earlier in the flow; walk the
  prototype as a new user and point to the step for each
  (design-before-build.md).
- A proposal page passes its checklist before anyone reviews it: each
  question beside its own images, before/after pairs within one phone
  screen, every option drawn at its best, every sentence true on its
  screen (design-before-build.md, "Proposal pages pass a checklist
  before review"). `check-page` reads it at 375 and 390 px after each
  edit and before each commit.
- A screenshot gate is iterated on a subset (`--only`); only a full run
  counts before a merge (common/gates.md).
- One reading only: every number and limit names its subject and what
  happens when it runs out (visual-craft.md).
- Dates, times and counts are picked, never typed; derived text
  recomputes; suggestions never fence (ui/pickers.md). A class of bug is
  fixed across every board in one pass.
- Consent is never pre-selected; both answers equal, "no" without guilt
  (ui/consent.md).
- Work over about a second shows the product's own motion where the
  person looks, from its start until its result shows, and rests on a
  still frame with reduced motion. A pressed control answers in the same
  frame: "Opening Enterprise Architect…", the work's motion, no more
  presses (shortcut and menu item off) until the outcome shows, and a
  journey test that presses twice (ui/feedback.md). On a board it is
  `aria-busy="true"` and `disabled`; `check-ui` measures it.
- A window or panel that changes size lays its content out at every
  frame: never a frame of squashed words (visual-craft.md, "A resize
  never shows a squashed frame"). Record a resize and look at each frame.
- In a GPUI app, a child that fills a rounded corner rounds that corner
  itself, with the parent's radius minus its border: GPUI clips square,
  and the board's CSS clips round, so only the app shows it (rust/gpui.md,
  "Rounded corners don't clip"). `check-gpui-clip` finds an element that
  rounds and clips.
