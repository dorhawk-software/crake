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
  code (every view, state, control, transient surface, process, motion),
  grade each against the spec, redraw, ask, build, re-audit.
- **A small change in an agreed design**: follow the spec and the craft
  rules; put it on the canvas in the same piece of work.

## Musts

- Show variants and recommend one; never build an open choice unasked.
- After a pick, strip the other variants from the canvas.
- Real context: the real frame, both themes, the longest language.
- Run "Before you show anything" (identity-pass.md) on every page you
  show, and the checks in visual-craft.md.
- Facts first: if the identity draws a real thing, look it up and keep
  its identifying features.
