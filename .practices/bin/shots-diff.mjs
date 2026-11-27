// check-shots' measuring half: the regions where two screenshots of the same screen differ.
// No packages: the PNG decoder is check-ui's (ui-pixels.mjs).
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, basename } from "node:path";
import { decodePng } from "./ui-pixels.mjs";

const die = (m) => { console.error(`check-shots: ${m}`); process.exit(2); };
const args = process.argv.slice(2);
// --skip-top PX leaves out the top rows (a status bar's clock); --only-top PX compares only the
// rows above PX: the band where a placeholder must reach as far as its page.
let skipTop = 0, onlyTop = 0;
while (args[0] === "--skip-top" || args[0] === "--only-top") {
  const n = Number(args[1]);
  if (!(n >= 0) || args[1] === undefined) die(`${args[0]} takes a number of pixels.`);
  if (args[0] === "--skip-top") skipTop = n; else onlyTop = n;
  args.splice(0, 2);
}
if (args.length !== 2) die("usage: check-shots [--skip-top PX] [--only-top PX] FRESH LIVE (two PNGs, or two folders of them)");
const RULE = onlyTop
  ? "A placeholder is drawn on the page's own edges (.practices/docs/ui/feedback.md)"
  : "A live switch draws what a fresh start draws (.practices/docs/mobile/devices.md)";
const FIX = onlyTop
  ? "Fix: the placeholder takes the page's edges: the same safe areas ignored, the same background and glow reaching under the status bar. Crop both shots at each region to see the edge."
  : "Fix: Find what the live switch leaves stale (a measured box, a cached style, a face) and redraw it; crop both shots at each region to see it.";
const [fresh, live] = args;
for (const p of [fresh, live]) if (!existsSync(p)) die(`no such file or folder: ${p}`);

const pairs = [];
if (statSync(fresh).isDirectory()) {
  if (!statSync(live).isDirectory()) die("FRESH is a folder, so LIVE must be one too.");
  const names = readdirSync(fresh).filter((n) => n.endsWith(".png")).sort();
  if (!names.length) die(`no PNGs in ${fresh}`);
  for (const n of names) {
    if (!existsSync(join(live, n))) die(`${n} is in ${fresh} but not in ${live}: shoot every screen both ways.`);
    pairs.push([n, join(fresh, n), join(live, n)]);
  }
} else pairs.push([basename(fresh), fresh, live]);

// A pixel differs when a channel moves by more than 32: anti-aliasing noise stays under it, a glyph does not.
const CELL = 16;
const findings = [];
for (const [name, a, b] of pairs) {
  const A = decodePng(readFileSync(a)), B = decodePng(readFileSync(b));
  if (A.w !== B.w || A.h !== B.h) { findings.push(`${name}: ${A.w}×${A.h} fresh, ${B.w}×${B.h} live: not the same screen size.`); continue; }
  const cw = Math.ceil(A.w / CELL), ch = Math.ceil(A.h / CELL), box = new Map();
  for (let y = skipTop; y < (onlyTop ? Math.min(onlyTop, A.h) : A.h); y++) {
    for (let x = 0; x < A.w; x++) {
      const i = (y * A.w + x) * A.bpp, j = (y * B.w + x) * B.bpp;
      if (Math.max(Math.abs(A.px[i] - B.px[j]), Math.abs(A.px[i + 1] - B.px[j + 1]), Math.abs(A.px[i + 2] - B.px[j + 2])) <= 32) continue;
      const c = Math.floor(y / CELL) * cw + Math.floor(x / CELL), r = box.get(c);
      if (r) { r.l = Math.min(r.l, x); r.r = Math.max(r.r, x); r.t = Math.min(r.t, y); r.b = Math.max(r.b, y); }
      else box.set(c, { l: x, r: x, t: y, b: y });
    }
  }
  // Cells that touch are one region.
  const seen = new Set();
  for (const start of box.keys()) {
    if (seen.has(start)) continue;
    const q = [start], reg = { l: Infinity, r: -1, t: Infinity, b: -1 };
    seen.add(start);
    while (q.length) {
      const c = q.pop(), r = box.get(c);
      reg.l = Math.min(reg.l, r.l); reg.r = Math.max(reg.r, r.r); reg.t = Math.min(reg.t, r.t); reg.b = Math.max(reg.b, r.b);
      const cx = c % cw, cy = Math.floor(c / cw);
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        const nx = cx + dx, ny = cy + dy, n = ny * cw + nx;
        if (nx >= 0 && ny >= 0 && nx < cw && ny < ch && box.has(n) && !seen.has(n)) { seen.add(n); q.push(n); }
      }
    }
    findings.push(`${name}: ${reg.r - reg.l + 1}×${reg.b - reg.t + 1} px differ at ${reg.l},${reg.t}.`);
  }
}

if (!findings.length) {
  console.log(`check-shots: ${pairs.length} screen${pairs.length === 1 ? "" : "s"}: ${onlyTop ? `the top ${onlyTop} px match` : "a live switch draws what a fresh start draws"}.`);
  process.exit(0);
}
console.log(`[shots] ${findings.length} finding${findings.length === 1 ? "" : "s"}. Rule: ${RULE}.`);
console.log(`  ${FIX}`);
for (const f of findings) console.log(`  - ${f}`);
process.exit(1);
