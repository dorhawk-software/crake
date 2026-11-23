// What check-ui knows before it renders a board: each rule's name in the
// docs, its fix and what a deliberate exception looks like (every finding
// says how to fix it, common/gates.md); a board's own width; and the probe
// that tells whether a board draws differently in the dark. ui-audit.mjs
// uses it.
import { existsSync, readFileSync } from "node:fs";
import { basename, dirname, join, relative } from "node:path";

// What each rule is for, where it is written, and how to fix it (common/gates.md: every finding says how to fix it).
export const RULE = {
  align: ["Buttons follow the column", "Start the row where the text starts (main action first, escape last), or fill the column with equal parts; one forward action may sit at the trailing edge.", "A surface the platform draws (a macOS alert)"],
  contain: ["Nothing runs past the surface it is drawn on", "Make the surface wider, let the row wrap, or shorten the words.", "A badge or tooltip that hangs over an edge on purpose"],
  clip: ["Nothing is cut off by a box too small for it", "Make the box fit its content, or let the content wrap.", "A list that scrolls, drawn mid-scroll"],
  gap: ["Separate targets keep their distance", "Keep 8 px between separate targets; make a repeated action the whole row.", "A segmented control, a grid of like cells, list rows inside one card"],
  "text-size": ["Nothing anyone reads under 11 px", "Use the 11 px floor of the type scale.", "A glyph inside a shape (an avatar's initials)"],
  ring: ["Tone, not outlines: a meaningful ring is drawn with outline", "Draw the ring with outline (and outline-offset), not box-shadow.", "A hairline the platform itself draws"],
  shadow: ["Depth and texture from a concept: shadows fall away from the light", "Give the shadow a downward offset of at least a quarter of its blur.", "Something the concept says glows (a lamp)"],
  halo: ["Spacing is measured from what you see: 16 px from a visible edge to text", "Move the text or the ringed object until 16 px separate them.", "A checkbox ring beside its own label"],
  inset: ["Spacing is measured from what you see: what covers an edge takes room from the padding", "Add the covered part to the padding on that side (the overlap plus the padding the other sides keep), or move the cover off the text.", "A badge that hangs over a card's corner on purpose"],
  centre: ["Centred means on the axis", "Let the centred block take its column's full width, or centre it in the column.", "-"],
  state: ["A choice changes everything it owns, and like controls keep one look", "Draw the state with outline, colour or a fill; never with padding, a border or a weight that changes size.", "-"],
  outcome: ["Every button names its outcome", "Say what happens: the action's verb and object (\"Remove\"), and for the way out, what stays (\"Keep the app\", \"Keep editing\").", "A surface the platform draws (a macOS alert), in the platform's words"],
  icon: ["A drawn icon set: every main button carries an icon", "Add the family's icon for the button's action.", "An escape button (Cancel), or two answers that must weigh the same"],
  "same-look": ["One anatomy per job: like controls keep one look", "Build both from one component, or give the different one its own class and reason.", "-"],
  safe: ["Both safe areas belong to the frame", "Draw both safe areas in the one phone frame; end content the safe area plus 8 px above the bottom; let a dense screen scroll, its main action pinned above the home indicator.", "A screen drawn mid-scroll, cut by its frame"],
  wrap: ["A button label stays on one line", "Shorten the label, give the button its own row, or make it a list row; stacked content sits in a flex column.", "A card-sized target whose text is a paragraph"],
  sheet: ["A sheet's page is inert, and the board says so", "Mark everything drawn before the dialog in its screen aria-hidden=\"true\" (inert in the product); the board's writer can do it in one place.", "A dialog that isn't modal (a popover)"],
  contrast: ["Text passes contrast: 4.5:1, or 3:1 for large text", "Darken (or, in the dark theme, lighten) the text or its ground.", "A disabled control, which also says it is disabled"],
  mark: ["A mark holds at its smallest size", "Measure it where it ships smallest: every stroke 1 px or more, no leftover piece of a cut, the brand element 10 px or taller, and the brand touching the letters or a quarter of a stroke clear of them.", "A mark only ever shown large, a poster or a splash", "ui/identity-pass.md"],
  ink: ["A glyph keeps one ink", "Print the plane or fill as a faint tint of the stroke's colour, or a neutral (white or black at low alpha, the paper).", "A glyph meant to carry two colours (a flag)"],
  fallback: ["Every face draws the product's own language", "Load the face's subset that has the letters (Google Fonts: the latin-ext subset for Czech), or choose a face that has them; check the spacing around marks like ď and ť by eye.", "A sign the face lacks on purpose, drawn by the system (an arrow)", "ui/identity-pass.md"],
  tabular: ["Roles by meaning: times and counts in tabular figures", "Set font-variant-numeric: tabular-nums on it (or its role's class), or give times the interface face, whose figures are tabular.", "A time set as a word, never changing or lined up (a title)", "ui/identity-pass.md"],
  busy: ["A press answers at once: a busy control takes no more presses and says what is happening", "Disable it (disabled, or aria-disabled=\"true\"), its shortcut and menu item too; drop the pointer cursor; name the work in progress form, ending in … (U+2026): \"Opening Enterprise Architect…\".", "A control whose press stops the work (Stop)", "ui/feedback.md"],
  working: ["Work shows it's happening: a busy control moves while it works", "Play the work's own motion inside the control (the product's mark, not a generic spinner), from the press until the result shows.", "A control the board draws busy, whose motion is drawn elsewhere on the screen", "ui/feedback.md"],
  motion: ["Motion with a reason: with reduced motion, nothing loops", "Stop the loop under @media (prefers-reduced-motion: reduce) and rest on a still frame that still makes sense; words in progress form (\"Installing…\") say it's working.", "A loop that is the content itself (a video the person started)"],
  squash: ["A resize never shows a squashed frame", "Animate the size and let the content lay out at every frame (or resize without the animation, or crossfade); never scale content to a size it wasn't laid out for.", "A drawing whose squash is its idea (a bouncing ball), with no words in it"],
  allow: ["Every exception has a written reason", "Write data-allow=\"rule: a reason of at least three words\".", "-"],
};


// --- A board's own width: canvas.json beside it, or its viewport meta. 0: none, it renders at UI_WIDTHS.
export function fixedWidth(file) {
  const canvas = join(dirname(file), "canvas.json");
  if (existsSync(canvas)) {
    let boards;
    try { boards = JSON.parse(readFileSync(canvas, "utf8")).boards || {}; } catch (err) { throw new Error(`${canvas} isn't JSON (${err.message}). Fix it, or remove it.`); }
    const b = boards[basename(file)] || boards[relative(dirname(canvas), file)];
    if (b && +b.w > 0) return +b.w;
  }
  const head = readFileSync(file, "utf8").slice(0, 20000);
  const meta = head.match(/<meta\b[^>]*name=["']?viewport["']?[^>]*>/i);
  const w = meta && meta[0].match(/content=["'][^"']*\bwidth\s*=\s*(\d+)/i);
  return w ? +w[1] : 0;
}
// Whether the board draws differently in the dark: a prefers-color-scheme rule, light-dark(), a dark
// color-scheme, or a script that asks. A sheet it can't read (another origin) counts as themed.
export const THEMED = `(() => {
  const has = (rules) => { for (const r of rules) {
    if ((r.media && /prefers-color-scheme/.test(r.media.mediaText)) || (r.conditionText && /prefers-color-scheme/.test(r.conditionText))) return true;
    if (r.cssText && /light-dark\\(|color-scheme:[^;}]*dark/.test(r.cssText)) return true;
    if (r.cssRules && has(r.cssRules)) return true; } return false; };
  for (const sh of document.styleSheets) { if (sh.media && /prefers-color-scheme/.test(sh.media.mediaText)) return true; try { if (has(sh.cssRules)) return true; } catch { return true; } }
  if (document.querySelector("source[media*=prefers-color-scheme], [style*=light-dark], [style*=color-scheme]")) return true;
  return [...document.scripts].some((x) => /prefers-color-scheme/.test(x.textContent));
})()`;
