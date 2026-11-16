# Drag and drop

How an app shows that it will take what is being dragged over it. One
pattern, taken from tern and named by the owner (2026-11-13) "the
optimal representation of drag and drop, most understandable".

## One calm panel says what will happen

**What.** While something droppable is held over a drop target, one
panel covers the whole target and says, in words, what dropping will
do. Nothing else on screen lights up or moves.

- **The panel:** the target's full area, inset a few points from its
  edges, with a large radius (about 20 px). A 2 px dashed line in the
  ink at about 45%. The fill is the product's soft accent, nearly solid
  where the content behind is busy (about 90%, with a light blur), so the
  window behind is a quiet hint. Where the content behind is sparse, half
  strength is enough (tern's message area).
- **In the middle, stacked:** the action's icon in a 52 px circle of the
  surface colour (white in light; the raised surface in dark), a short
  verb phrase in the display face at about 20 px ("Drop to attach",
  "Drop to add it"), and one quiet line at 13 px saying the consequence
  or the scope ("Anywhere in the message"; "Gentes reads the installer
  before running anything").
- **The resting hint goes:** the dashed "Attach or drop files" box, or
  the "Drop an installer · ⌘O" label, fades to nothing while the panel
  shows, so the same promise isn't made twice.
- **The panel covers the content, not the system's chrome.** It starts
  below the window buttons and title row and ends above the window's
  edge; the window buttons stay visible and nothing is cut in half at
  the panel's edge.
- **Not:** glows, light-up strips, a coloured inset ring on a sidebar,
  bouncing, a second label next to the first. Only the panel shows.

**Why.** Tern (the email client) shows "Drop to attach · Anywhere in the
message" on such a panel over the compose window while files are held
over it. Gentes (the Windows-app runner) first lit up a strip under its
app list and a ring around the sidebar, with the words squeezed into
the strip. The owner, 2026-11-13: "the light up strip is ugly … see how
tern handles drag and drop, much more gracefully and prettier design,
copy it over". Copied at tern's 50% fill, Gentes' busy window showed
through and fought the words; at about 90% with a blur, it read.

**Apply.**
1. Decide the target: the area that accepts the drop, often the whole
   content of the window. One target per drop kind.
2. Draw the panel and its three parts in the product's palette and
   faces, light and dark.
3. Write the verb phrase as what happens ("Drop to attach"), and the
   line under it as the scope or the consequence, never the instruction
   repeated.
4. Fade every resting drop hint while the panel shows.
5. Drop the file into the real thing at the end: the panel goes, and the
   result appears where the user will look for it.

**Check.** Render the drag state over the busiest real screen, in both
themes. The words read at a glance against what is behind, nothing but
the panel has changed, the panel cuts no text at its edge, and the
resting hint is gone.
