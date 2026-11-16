// The UI gate's measuring half: renders each HTML board in headless Chrome
// at every width and theme, and measures what ui/visual-craft.md asks.
// check-ui runs it; read that script for the settings. No packages: it
// talks to Chrome over --remote-debugging-pipe (any Node 18+).
//
//   node ui-audit.mjs --chrome PATH --widths "1100 1440" --themes "light dark"
//        --targets SELECTOR --states "focus hover" --min-text 11 --gap 8
//        --halo 16 FILE...
//
// Exit 0: no findings. 1: findings (printed, grouped). 2: it couldn't
// measure (no Chrome, a page that didn't load): never "clean".
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { inflateSync } from "node:zlib";
import { markAudit } from "./ui-mark.mjs";

const opts = { widths: "1100 1440", themes: "light dark", targets: "button, a[href], [role=button], input, select, textarea, summary",
  states: "focus hover active pressed", "min-text": "11", gap: "8", halo: "16", chrome: "",
  "mark-stroke": "1", "mark-brand": "10", "mark-small": "24" };
const files = [];
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i++) {
  if (argv[i].startsWith("--")) opts[argv[i].slice(2)] = argv[++i];
  else files.push(argv[i]);
}
const die = (msg) => { console.error(`check-ui: ${msg}`); process.exit(2); };
if (!files.length) die("no boards given");
if (!opts.chrome || !existsSync(opts.chrome)) die(`no Chrome at "${opts.chrome}". Fix: install Google Chrome or Chromium, or set UI_CHROME in .practices/config.`);

// --- Chrome over a pipe: fd 3 to Chrome, fd 4 from it, JSON messages ending in \0.
const profile = mkdtempSync(join(tmpdir(), "check-ui-"));
const args = ["--headless=new", "--remote-debugging-pipe", "--disable-gpu", "--hide-scrollbars", "--no-first-run",
  "--no-default-browser-check", "--allow-file-access-from-files", "--force-color-profile=srgb", `--user-data-dir=${profile}`];
if (process.env.CI) args.push("--no-sandbox"); // CI runners' kernels refuse Chrome's sandbox; the pages are the project's own
const chrome = spawn(opts.chrome, [...args, "about:blank"], { stdio: ["ignore", "ignore", "pipe", "pipe", "pipe"] });
let chromeErr = "";
chrome.stderr.on("data", (d) => { chromeErr = (chromeErr + d).slice(-2000); });
const cleanup = () => { try { chrome.kill(); } catch {} try { rmSync(profile, { recursive: true, force: true }); } catch {} };
process.on("exit", cleanup);
const toChrome = chrome.stdio[3], fromChrome = chrome.stdio[4];
let id = 0, buf = "";
const waiting = new Map(), events = [];
fromChrome.on("data", (d) => {
  buf += d;
  let at;
  while ((at = buf.indexOf("\0")) >= 0) {
    const msg = JSON.parse(buf.slice(0, at));
    buf = buf.slice(at + 1);
    if (msg.id && waiting.has(msg.id)) { waiting.get(msg.id)(msg); waiting.delete(msg.id); } else events.push(msg);
  }
});
chrome.on("exit", (code) => { if (waiting.size) die(`Chrome stopped (exit ${code}): ${chromeErr.trim().split("\n").pop()}`); });
const send = (method, params = {}, sessionId) => new Promise((ok, fail) => {
  const i = ++id;
  const timer = setTimeout(() => fail(new Error(`${method}: no answer in 60 s`)), 60000);
  waiting.set(i, (m) => { clearTimeout(timer); m.error ? fail(new Error(`${method}: ${m.error.message}`)) : ok(m.result); });
  toChrome.write(JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) }) + "\0");
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// --- What runs in the page. Returns { findings, measured } for one render.
function audit(cfg) {
  const out = [];
  const measured = { contrastMeasured: 0, contrastSkipped: 0, pending: [] };
  const RULES = ["contain", "align", "gap", "clip", "text-size", "ring", "shadow", "halo", "inset", "centre", "state", "icon", "outcome", "same-look", "contrast", "mark", "ink"];
  const isSvgInner = (e) => e instanceof SVGElement && e.tagName.toLowerCase() !== "svg";
  const cs = (e) => getComputedStyle(e);
  const hiddenByOpacity = (e) => { for (let a = e; a && a.nodeType === 1; a = a.parentElement) if (+cs(a).opacity === 0) return true; return false; };
  const visible = (e) => {
    if (e.closest("template, symbol, defs, clipPath, mask, pattern, marker, [hidden], noscript, head")) return false;
    const s = cs(e);
    if (s.display === "none" || s.visibility !== "visible") return false;
    const r = e.getBoundingClientRect();
    return r.width > 0.5 && r.height > 0.5 && !hiddenByOpacity(e);
  };
  // data-allow="rule: reason; rule: reason" on an element or an ancestor.
  const allowNode = (e, rule) => {
    for (let a = e; a && a.nodeType === 1; a = a.parentElement) {
      const v = a.getAttribute && a.getAttribute("data-allow");
      if (v && v.split(";").some((p) => p.split(":")[0].trim() === rule)) return a;
    }
    return null;
  };
  const label = (e) => {
    const t = e.tagName.toLowerCase();
    const c = typeof e.className === "string" && e.className.trim() ? "." + e.className.trim().split(/\s+/).join(".") : "";
    return t + (e.id ? "#" + e.id : "") + c;
  };
  const words = (e) => (e.textContent || "").replace(/\s+/g, " ").trim().slice(0, 40);
  const heads = [...document.querySelectorAll("h1, h2, h3")];
  const where = (e) => {
    let h = null;
    for (const x of heads) if (x.compareDocumentPosition(e) & Node.DOCUMENT_POSITION_FOLLOWING || x.contains(e)) h = x; else break;
    const win = e.closest("[id]");
    return [h ? `under "${words(h)}"` : null, win && win !== e ? `in #${win.id}` : null].filter(Boolean).join(", ");
  };
  const y = (e) => Math.round(e.getBoundingClientRect().top + scrollY);
  const add = (rule, e, text, key) => out.push({ rule, text, where: where(e), y: y(e), key: key || label(e) + "|" + words(e) });
  const px = (v) => Math.round(v * 10) / 10;

  // The allow marks themselves: a known rule and a reason, always.
  for (const e of document.querySelectorAll("[data-allow]")) {
    for (const part of e.getAttribute("data-allow").split(";")) {
      if (!part.trim()) continue;
      const [rule, ...rest] = part.split(":");
      const reason = rest.join(":").trim();
      if (!RULES.includes(rule.trim())) add("allow", e, `${label(e)} allows "${rule.trim()}", which isn't a rule (${RULES.join(", ")}).`);
      else if (reason.split(/\s+/).filter(Boolean).length < 3) add("allow", e, `${label(e)} allows "${rule.trim()}" without a reason of at least three words.`);
    }
  }

  // A surface: an ancestor that draws something (a fill, an edge, a shadow) or clips.
  const surfaceCache = new Map();
  const draws = (a) => {
    if (surfaceCache.has(a)) return surfaceCache.get(a);
    const s = cs(a);
    const r = s.display !== "contents" && (s.backgroundColor !== "rgba(0, 0, 0, 0)" || s.backgroundImage !== "none" ||
      (parseFloat(s.borderTopWidth) > 0 && s.borderTopStyle !== "none") || (parseFloat(s.borderLeftWidth) > 0 && s.borderLeftStyle !== "none") ||
      s.boxShadow !== "none" || s.overflowX !== "visible" || s.overflowY !== "visible");
    surfaceCache.set(a, r);
    return r;
  };
  const surfaceOf = (e) => { for (let a = e.parentElement; a && a !== document.body && a !== document.documentElement; a = a.parentElement) if (draws(a)) return a; return null; };
  const scrolls = (a) => /auto|scroll/.test(cs(a).overflowX + cs(a).overflowY);
  // A surface that clips hides what runs past it: clip reports that, once, and one mark allows it (1.6.1;
  // a phone drawn mid-scroll and marked for clip was still reported by contain).
  const clips = (a) => /hidden|clip/.test(cs(a).overflowX + cs(a).overflowY);
  const outside = (r, p) => ({ left: p.left - r.left, right: r.right - p.right, top: p.top - r.top, bottom: r.bottom - p.bottom });

  const all = [...document.body.querySelectorAll("*")].filter((e) => !isSvgInner(e) && visible(e));

  // contain: nothing runs outside the surface it is drawn on.
  const spills = new Set();
  for (const e of all) {
    const s = surfaceOf(e);
    if (!s || scrolls(s) || clips(s)) continue;
    const d = outside(e.getBoundingClientRect(), s.getBoundingClientRect());
    const sides = Object.entries(d).filter(([, v]) => v > 0.5);
    if (!sides.length) continue;
    spills.add(e);
    if (spills.has(e.parentElement) && surfaceOf(e.parentElement) === s) continue; // its parent is the one reported
    if (allowNode(e, "contain")) continue;
    add("contain", e, `${label(e)} "${words(e)}" runs ${sides.map(([k, v]) => `${px(v)} px past the ${k}`).join(", ")} of ${label(s)}.`);
  }

  // clip: a box that hides what doesn't fit, cutting real content.
  for (const e of all) {
    const s = cs(e);
    if (!/hidden|clip/.test(s.overflowX + s.overflowY) || e.tagName === "svg" || e.tagName === "IMG") continue;
    const dx = e.scrollWidth - e.clientWidth, dy = e.scrollHeight - e.clientHeight;
    const cutX = /hidden|clip/.test(s.overflowX) && dx > 1 && s.textOverflow !== "ellipsis";
    const cutY = /hidden|clip/.test(s.overflowY) && dy > 1 && s.webkitLineClamp === "none";
    if (!(cutX || cutY) || allowNode(e, "clip")) continue;
    add("clip", e, `${label(e)} "${words(e)}" cuts off ${[cutX ? `${dx} px across` : null, cutY ? `${dy} px down` : null].filter(Boolean).join(" and ")} of its content.`);
  }

  // Text runs, measured once for several checks.
  const texts = [];
  const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n; (n = walk.nextNode()); ) {
    if (!n.textContent.trim()) continue;
    const e = n.parentElement;
    if (!e || !visible(e) || e.closest("style, script, title")) continue;
    const range = document.createRange();
    range.selectNodeContents(n);
    const r = range.getBoundingClientRect();
    if (r.width < 0.5 || r.height < 0.5) continue;
    texts.push({ n, e, r, lines: [...range.getClientRects()].filter((x) => x.width > 0.5) });
  }

  // Covered things (under a sheet, behind a window) aren't seen: hit-test each line's middle.
  // Everything else is measured at scroll 0, where page and viewport coordinates agree.
  // Both axes: a board wider than the window has things past its right edge, where elementFromPoint
  // finds nothing and everything read as uncovered (Obálka's 3150 px board, 1.6.4).
  const coveredAt = (e, q) => {
    window.scrollTo(Math.max(0, q.left + q.width / 2 - innerWidth / 2), Math.max(0, q.top + q.height / 2 - innerHeight / 2));
    const hit = document.elementFromPoint(q.left + q.width / 2 - scrollX, q.top + q.height / 2 - scrollY);
    return !!hit && !(hit === e || e.contains(hit) || hit.contains(e));
  };
  for (const t of texts) t.covered = t.lines.length > 0 && coveredAt(t.e, t.lines[0]);
  window.scrollTo(0, 0);

  // How much transforms shrink an element on the screen: the scales of it and its ancestors, from
  // `transform` and `scale`. Not its drawn box over offsetWidth/Height: those round to whole pixels,
  // so an unscaled 19.6 px badge read as 0.98 (Gentes, 1.6.2).
  const drawnScale = (e) => {
    let k = 1;
    for (let a = e; a instanceof Element; a = a.parentElement) {
      const s = cs(a);
      if (s.transform && s.transform !== "none") {
        const m = new DOMMatrixReadOnly(s.transform);
        k *= Math.min(Math.hypot(m.a, m.b), Math.hypot(m.c, m.d));
      }
      if (s.scale && s.scale !== "none") { const [x, y = x] = s.scale.split(" ").map(parseFloat); k *= Math.min(x, y); }
    }
    return Math.min(k, 1);
  };
  // text-size: nothing anyone reads under the floor (11 px).
  for (const { e } of texts) {
    let size = parseFloat(cs(e).fontSize);
    if (e instanceof SVGElement) { const m = e.getScreenCTM && e.getScreenCTM(); if (m) size *= Math.hypot(m.a, m.b); }
    else size *= drawnScale(e); // a scaled preview draws its type smaller than its font-size says (1.6.1)
    if (size < cfg.minText - 0.05 && !allowNode(e, "text-size")) add("text-size", e, `"${words(e)}" is ${px(size)} px, under ${cfg.minText}.`, label(e) + "|" + px(size));
  }

  // Box shadows, parsed: [{inset, x, y, blur, spread, alpha}].
  const shadows = (s) => {
    if (!s || s === "none") return [];
    return s.split(/,(?![^(]*\))/).map((layer) => {
      const alpha = (() => { const m = layer.match(/rgba?\(([^)]+)\)/); if (!m) return 1; const p = m[1].split(/[ ,/]+/).filter(Boolean); return p.length > 3 ? parseFloat(p[3]) : 1; })();
      const nums = layer.replace(/rgba?\([^)]+\)/, "").match(/-?[\d.]+px/g)?.map(parseFloat) || [];
      return { inset: /inset/.test(layer), x: nums[0] || 0, y: nums[1] || 0, blur: nums[2] || 0, spread: nums[3] || 0, alpha };
    });
  };
  
  for (const e of all) {
    for (const l of shadows(cs(e).boxShadow)) {
      if (l.alpha === 0) continue;
      // ring: a meaningful ring belongs to outline, which no global shadow layer can erase.
      if (l.blur === 0 && l.x === 0 && l.y === 0 && l.spread >= 1) {
        if (!allowNode(e, "ring")) add("ring", e, `${label(e)} draws a ${px(l.spread)} px ring with box-shadow${l.inset ? " (inset)" : ""}.`);
        continue;
      }
      if (l.inset || l.blur === 0) continue;
      // shadow: depth falls away from the light (above), and a zero-offset halo is decoration.
      if (allowNode(e, "shadow")) continue;
      if (l.x === 0 && l.y === 0) add("shadow", e, `${label(e)} has a zero-offset halo (blur ${px(l.blur)} px): decoration, not depth.`);
      else if (l.y < 0) add("shadow", e, `${label(e)}'s shadow falls up (y ${px(l.y)} px), toward the light.`);
      else if (l.y < l.blur / 4) add("shadow", e, `${label(e)}'s shadow (${px(l.x)} ${px(l.y)} ${px(l.blur)} px) is nearly a halo: it spreads almost as far up, toward the light, as down.`);
    }
  }

  // Targets: separate things a person clicks.
  const targets = [...document.querySelectorAll(cfg.targets)].filter((e) => visible(e) && !e.closest("[aria-hidden=true]"));
  const outer = targets.filter((e) => !targets.some((o) => o !== e && o.contains(e)));
  // A segmented control says so by its role: its options are one control (1.6.1).
  const SEGMENTED = "[role=tablist], [role=radiogroup]";
  const segment = (e) => e.closest(SEGMENTED);

  // align: a row of buttons follows its column (ui/visual-craft.md, "Buttons follow the column").
  const rowsSeen = new Set();
  for (const t of outer) {
    const row = t.parentElement, col = row && row.parentElement;
    if (!col || rowsSeen.has(row)) continue;
    rowsSeen.add(row);
    const kids = [...row.children].filter(visible);
    if (!kids.every((k) => outer.includes(k)) || [...row.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) continue; // a row of buttons, nothing else
    // A bar of its own (it draws a fill or an edge) or a row placed by hand (absolute) is its buttons'
    // column: they follow its padding (1.6.1: every phone board's bottom bar, padded like the screen's
    // text, read as "16 px in" from the phone's frame).
    const own = draws(row) || /absolute|fixed/.test(cs(row).position);
    const ccs = cs(own ? row : col);
    if (!own && /flex|grid/.test(ccs.display) && !/column/.test(ccs.flexDirection)) continue; // beside the text, not under it
    const text = own ? [] : [...col.children].filter((c) => c !== row && visible(c) && words(c) && !outer.includes(c) && !c.querySelector(cfg.targets));
    if ((!own && !text.length) || allowNode(row, "align")) continue; // no text to line up with
    const rs = kids.map((k) => k.getBoundingClientRect());
    const left = Math.min(...rs.map((r) => r.left)), right = Math.max(...rs.map((r) => r.right));
    const cr = (own ? row : col).getBoundingClientRect();
    const cl = cr.left + parseFloat(ccs.paddingLeft) + parseFloat(ccs.borderLeftWidth), cr2 = cr.right - parseFloat(ccs.paddingRight) - parseFloat(ccs.borderRightWidth);
    const rtl = ccs.direction === "rtl";
    const fromLead = rtl ? cr2 - right : left - cl, fromTrail = rtl ? left - cl : cr2 - right;
    const atLead = Math.abs(fromLead) <= 2, atTrail = Math.abs(fromTrail) <= 2, n = kids.length;
    const names = kids.map((k) => `"${words(k)}"`).join(", ");
    // Centred by text-align, or by a flex column that centres its items (1.6.1: a lock screen's
    // centred heading and its button 24 px in from each side read as off the leading edge).
    const centred = (c) => { const t = cs(c); return t.textAlign === "center" || (/flex/.test(t.display) && /column/.test(t.flexDirection) && t.alignItems === "center"); };
    if (text.length && text.every(centred)) {
      const off = (left + right) / 2 - (cl + cr2) / 2;
      if (!(atLead && atTrail) && Math.abs(off) > 3) add("align", row, `the buttons ${names} sit ${px(Math.abs(off))} px off the centre of a centred column.`);
    } else if (n === 1 && atLead && atTrail && cr2 - cl > 480) {
      add("align", row, `the lone button ${names} stretches across a ${px(cr2 - cl)} px column; one button keeps its own width (full width only at phone width, 480 px or less).`);
    } else if (n === 1 && !atLead && !atTrail) {
      add("align", row, `the lone button ${names} sits ${px(fromLead)} px in from the column's leading edge: put it at the leading edge, or at the trailing edge if it moves the person on.`);
    } else if (n > 1 && !atLead) {
      add("align", row, `the row ${names} starts ${px(fromLead)} px in from the column's leading edge, where its text starts: start it there, or fill the column with equal parts.`);
    }
  }

  // gap: 8 px between separate targets, edge to edge, across and down.
  const rects = outer.map((e) => [e, e.getBoundingClientRect()]);
  for (let i = 0; i < rects.length; i++) for (let j = i + 1; j < rects.length; j++) {
    const [a, p] = rects[i], [b, q] = rects[j];
    if (Math.abs(p.top - q.top) > 400) continue;
    const dx = Math.max(q.left - p.right, p.left - q.right), dy = Math.max(q.top - p.bottom, p.top - q.bottom);
    const d = Math.max(dx, dy);
    if (d >= cfg.gap - 0.05) continue;
    if (segment(a) && segment(a) === segment(b)) continue; // options of one segmented control
    const na = allowNode(a, "gap"), nb = allowNode(b, "gap");
    if ((na && na.contains(b)) || (nb && nb.contains(a))) continue;
    add("gap", a, `${label(a)} "${words(a)}" and ${label(b)} "${words(b)}" are ${px(d)} px apart${d < 0 ? " (they overlap)" : ""}; separate targets need ${cfg.gap}.`, label(a) + "|" + words(a) + "|" + words(b));
  }

  // halo: a ring or glow keeps 16 px from text that isn't its own.
  const notTargetText = texts.filter(({ e, covered }) => !covered && !e.closest(cfg.targets));
  for (const e of all) {
    const s = cs(e);
    let reach = 0;
    // a ring (outline of 2 px or more) or a glow (zero offset, blurred); a hairline is an edge, not a ring
    if (s.outlineStyle !== "none" && parseFloat(s.outlineWidth) >= 2) reach = parseFloat(s.outlineWidth) + (parseFloat(s.outlineOffset) || 0);
    for (const l of shadows(s.boxShadow)) if (!l.inset && l.x === 0 && l.y === 0 && l.blur > 0 && l.alpha > 0) reach = Math.max(reach, l.spread + l.blur / 2);
    if (reach <= 0 || allowNode(e, "halo")) continue;
    const r = e.getBoundingClientRect();
    const hidden = coveredAt(e, r);
    window.scrollTo(0, 0);
    if (hidden) continue;
    const ring = { left: r.left - reach, right: r.right + reach, top: r.top - reach, bottom: r.bottom + reach };
    let worst = null;
    for (const t of notTargetText) {
      if (e.contains(t.e) || t.e.contains(e)) continue;
      for (const q of t.lines) {
        const dx = Math.max(q.left - ring.right, ring.left - q.right), dy = Math.max(q.top - ring.bottom, ring.top - q.bottom);
        const d = Math.max(dx, dy);
        if (d < cfg.halo - 0.05 && (!worst || d < worst.d)) worst = { d, t };
      }
    }
    if (worst) add("halo", e, `${label(e)}'s ring reaches within ${px(worst.d)} px of "${words(worst.t.e)}"; a visible edge keeps ${cfg.halo} px from text.`);
  }

  // inset: what lies over a surface's edge takes room from its padding. From the covering edge to the
  // first text keeps the surface's smallest padding on its other sides (Obálka's letter, tucked 10 px
  // under its envelope with a padding of 14, showed 4 px above its subject and 14 below, 1.6.5).
  // A cover is a filled box. A drawing's box isn't what it paints (Obálka's postmark, whose box reaches
  // over the stamp beside it), and a box that only clips paints nothing.
  const fills = (c) => {
    const s = cs(c);
    if (s.backgroundImage !== "none") return true;
    const m = s.backgroundColor.match(/[\d.]+/g);
    return !!m && (m.length < 4 || +m[3] > 0);
  };
  const drawn = all.filter(fills).map((c) => ({ c, q: c.getBoundingClientRect() }));
  const onTop = (c, x, yy) => {
    window.scrollTo(Math.max(0, x - innerWidth / 2), Math.max(0, yy - innerHeight / 2));
    const hit = document.elementFromPoint(x - scrollX, yy - scrollY);
    return !!hit && (hit === c || c.contains(hit));
  };
  const SIDES = [["top", "bottom", "left", "right", 1], ["bottom", "top", "left", "right", -1], ["left", "right", "top", "bottom", 1], ["right", "left", "top", "bottom", -1]];
  for (const s of all) {
    if (!draws(s) || scrolls(s) || allowNode(s, "inset")) continue;
    const own = texts.filter((t) => !t.covered && s.contains(t.e) && surfaceOf(t.e) === s);
    if (!own.length) continue;
    const r = s.getBoundingClientRect(), st = cs(s);
    const pad = { top: parseFloat(st.paddingTop), bottom: parseFloat(st.paddingBottom), left: parseFloat(st.paddingLeft), right: parseFloat(st.paddingRight) };
    for (const [lo, hi, a0, a1, k] of SIDES) {
      const v = (x) => k * x;
      const want = Math.min(...SIDES.map(([side]) => side).filter((side) => side !== lo).map((side) => pad[side]));
      let worst = null;
      for (const { c, q } of drawn) {
        if (c === s || c.contains(s) || s.contains(c)) continue;
        if (!(v(q[lo]) <= v(r[lo]) + 0.5 && v(q[hi]) > v(r[lo]) + 0.5 && v(q[hi]) < v(r[hi]))) continue;
        const from = Math.max(q[a0], r[a0]), to = Math.min(q[a1], r[a1]);
        if (to - from < 1) continue;
        let room = null, line = null;
        for (const t of own) for (const l of t.lines) {
          if (l[a0] >= q[a1] || l[a1] <= q[a0]) continue;
          const d = v(l[lo]) - v(q[hi]);
          if (room === null || d < room) { room = d; line = t; }
        }
        if (room === null || room >= want - 1) continue;
        const mid = (from + to) / 2, along = (r[lo] + q[hi]) / 2;
        if (!(a0 === "left" ? onTop(c, mid, along) : onTop(c, along, mid))) continue;
        if (!worst || room < worst.room) worst = { c, room, line, over: k * (q[hi] - r[lo]) };
      }
      window.scrollTo(0, 0);
      if (worst) add("inset", s, `${label(worst.c)} lies ${px(worst.over)} px over ${label(s)}'s ${lo} edge, leaving ${px(worst.room)} px to "${words(worst.line.e)}"; the room from a covering edge to text keeps the surface's padding, ${px(want)} px.`, label(s) + "|" + words(s) + "|" + lo);
    }
  }

  // centre: a centred block sits on its column's axis.
  for (const e of all) {
    const s = cs(e);
    if (s.textAlign !== "center" || e.closest(cfg.targets) || ![...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) continue;
    const p = e.parentElement, ps = cs(p);
    if (!p || /inline/.test(s.display) || (ps.display.includes("flex") && !ps.flexDirection.includes("column")) || ps.display.includes("grid")) continue;
    if (cs(p).textAlign === "center") continue; // the parent centres too: the parent is measured
    const pr = p.getBoundingClientRect(), r = e.getBoundingClientRect();
    const axis = pr.left + parseFloat(ps.paddingLeft) + (pr.width - parseFloat(ps.paddingLeft) - parseFloat(ps.paddingRight)) / 2;
    const off = r.left + r.width / 2 - axis;
    if (Math.abs(off) > 3 && !allowNode(e, "centre")) add("centre", e, `centred "${words(e)}" sits ${px(Math.abs(off))} px ${off < 0 ? "left" : "right"} of its column's axis.`);
  }

  // icon: every main button carries one from the family.
  for (const e of outer) {
    if (!/^(BUTTON)$/.test(e.tagName) && !e.matches("[role=button]")) continue;
    if (!words(e) || e.querySelector("svg, img, [class*=icon]") || segment(e) || allowNode(e, "icon")) continue; // a tab is no main button
    add("icon", e, `the button "${words(e)}" has no icon.`);
  }

  // outcome: a button's label says what happens, never a word that fits either answer.
  // English, and Czech (1.6.1: a Czech board's "Zrušit" and "Ano" passed): zrušit and storno (cancel), ano, ne,
  // potvrdit (confirm), přijmout (accept), odmítnout (decline), přerušit (abort).
  const vague = /^(cancel|ok|okay|yes|no|confirm|submit|accept|decline|proceed|abort|dismiss|zrušit|storno|ano|ne|potvrdit|přijmout|odmítnout|přerušit)$/iu;
  for (const e of outer) {
    if (!/^(BUTTON)$/.test(e.tagName) && !e.matches("[role=button], input[type=button], input[type=submit]")) continue;
    const said = (e.tagName === "INPUT" ? e.value : words(e)).replace(/[.!…]+$/, "").trim();
    if (!vague.test(said) || allowNode(e, "outcome")) continue;
    add("outcome", e, `the button "${said}" names no outcome: it could mean either answer to the question it sits under.`);
  }

  // same-look: like controls keep one look (same tag and classes, same size and type).
  const groups = new Map();
  const stateRe = new RegExp(`^(${cfg.states.split(/\s+/).join("|")})$`);
  for (const e of outer) {
    if (allowNode(e, "same-look") || (e.tagName === "A" && !(typeof e.className === "string" && e.className.trim()))) continue; // a plain link takes its sentence's size
    // A button with no words (an icon button) is its own kind, and its type is nobody's to read (1.6.1:
    // a header's back arrow was "another look" of the labelled button beside it).
    const wordless = !words(e);
    const key = e.tagName + "." + (typeof e.className === "string" ? e.className.trim().split(/\s+/).filter((c) => c && !stateRe.test(c)).sort().join(".") : "") + (wordless ? " (icon only)" : "") + (e.getAttribute("role") ? `[role=${e.getAttribute("role")}]` : ""); // a tab is a kind of its own
    const s = cs(e);
    const look = [Math.round(e.getBoundingClientRect().height), wordless ? "-" : s.fontSize, wordless ? "-" : s.fontWeight, s.borderTopLeftRadius, s.paddingTop].join(" / ");
    if (!groups.has(key)) groups.set(key, new Map());
    const g = groups.get(key);
    if (!g.has(look)) g.set(look, e);
  }
  for (const [key, g] of groups) {
    if (g.size < 2) continue;
    const looks = [...g.entries()];
    add("same-look", looks[1][1], `${key.toLowerCase()} comes in ${g.size} looks (height / size / weight / radius / top padding): ${looks.map(([l]) => l).join(" vs ")}; first at "${words(looks[0][1])}", other at "${words(looks[1][1])}".`, key);
  }

  // state: a focus or hover state never moves its content.
  for (const cls of cfg.states.split(/\s+/).filter(Boolean)) {
    for (const e of document.querySelectorAll("." + CSS.escape(cls))) {
      if (!visible(e) || allowNode(e, "state")) continue;
      const parts = [e, ...e.querySelectorAll("*")].filter((x) => !isSvgInner(x));
      const before = parts.map((x) => x.getBoundingClientRect());
      e.classList.remove(cls);
      const after = parts.map((x) => x.getBoundingClientRect());
      e.classList.add(cls);
      let moved = 0;
      before.forEach((b, i) => { moved = Math.max(moved, Math.abs(b.left - after[i].left), Math.abs(b.top - after[i].top), Math.abs(b.width - after[i].width), Math.abs(b.height - after[i].height)); });
      if (moved > 0.5) add("state", e, `${label(e)} "${words(e)}" moves or resizes its content by ${px(moved)} px in its .${cls} state.`);
    }
  }

  // contrast: 4.5:1 (3:1 for large text) on solid ground; text over images or gradients is counted as not measured.
  const rgba = (c) => { const m = c.match(/rgba?\(([^)]+)\)/); if (!m) return null; const p = m[1].split(/[ ,/]+/).filter(Boolean).map(parseFloat); return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1]; };
  const over = (top, under) => { const a = top[3]; return [0, 1, 2].map((i) => top[i] * a + under[i] * (1 - a)).concat(1); };
  const lum = (c) => { const f = (v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]); };
  for (const { e, lines, covered } of texts) {
    if (covered || e instanceof SVGElement || e.closest("[disabled], [aria-disabled=true]") || allowNode(e, "contrast")) continue;
    const layers = [];
    let ground = null, opacity = 1, image = false;
    for (let a = e; a; a = a.parentElement) {
      const s = cs(a);
      opacity *= +s.opacity;
      if (s.backgroundImage !== "none") { image = true; break; }
      const c = rgba(s.backgroundColor);
      if (c && c[3] > 0) { if (c[3] >= 1) { ground = c; break; } layers.push(c); }
    }
    if (!ground && !image) ground = [255, 255, 255, 1]; // nothing painted under it: the canvas's white
    const fgRaw = rgba(cs(e).color);
    if (!ground) { // a gradient or an image: measured against the pixels behind it, after this
      if (!fgRaw) { measured.contrastSkipped++; continue; }
      const st = cs(e), sz = parseFloat(st.fontSize);
      measured.pending.push({ fg: [fgRaw[0], fgRaw[1], fgRaw[2], fgRaw[3] * opacity], large: sz >= 24 || (sz >= 18.66 && +st.fontWeight >= 700),
        lines: lines.map((q) => [q.left, q.top, q.width, q.height]), label: label(e), words: words(e), where: where(e), y: y(e) });
      continue;
    }
    let bg = ground;
    for (let i = layers.length - 1; i >= 0; i--) bg = over(layers[i], bg);
    const fg0 = rgba(cs(e).color);
    if (!fg0) { measured.contrastSkipped++; continue; }
    const fg = over([fg0[0], fg0[1], fg0[2], fg0[3] * opacity], bg);
    const [l1, l2] = [lum(fg), lum(bg)].sort((a, b) => b - a);
    const ratio = (l1 + 0.05) / (l2 + 0.05);
    const s = cs(e), size = parseFloat(s.fontSize), large = size >= 24 || (size >= 18.66 && +s.fontWeight >= 700);
    const need = large ? 3 : 4.5;
    measured.contrastMeasured++;
    const hex = (c) => "#" + c.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");
    if (ratio < need - 0.005) add("contrast", e, `"${words(e)}" (${label(e)}) is ${Math.round(ratio * 100) / 100}:1, ${hex(fg)} on ${hex(bg)}; ${large ? "large " : ""}text needs ${need}:1.`, label(e) + "|" + words(e));
  }

  // ink: a glyph keeps one ink. Its fills (an offset plane) take the stroke's hue, or none (1.6.3:
  // a red bin on a blue plane fought; on the red button, its plane white over red, it was right).
  const hueOf = (c) => {
    const [r, g, b] = c.slice(0, 3).map((v) => v / 255), mx = Math.max(r, g, b), d = mx - Math.min(r, g, b);
    if (d < 0.12) return null; // a neutral: black, grey, white, the paper
    const h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return (h * 60 + 360) % 360;
  };
  const hexOf = (c) => "#" + c.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");
  const SHAPES = "path, rect, circle, ellipse, line, polyline, polygon";
  for (const svg of document.querySelectorAll("svg")) {
    if (!visible(svg) || allowNode(svg, "ink")) continue;
    const box = svg.getBoundingClientRect();
    if (box.width < 4 || box.width > 64 || box.height > 64) continue; // a glyph, not an illustration
    // A <use> draws its symbol in a closed tree: copy the shapes in beside it, read them, take them out.
    const copies = [];
    for (const u of svg.querySelectorAll("use")) {
      const id = (u.getAttribute("href") || u.getAttribute("xlink:href") || "").replace(/^#/, "");
      const src = id && document.getElementById(id);
      if (!src) continue;
      const g = document.createElementNS("http://www.w3.org/2000/svg", "g");
      for (const a of ["class", "style", "fill", "stroke", "color"]) if (u.getAttribute(a)) g.setAttribute(a, u.getAttribute(a));
      for (const k of src.childNodes) g.appendChild(k.cloneNode(true));
      u.parentNode.insertBefore(g, u);
      copies.push(g);
    }
    const strokes = [], fills = [];
    for (const sh of svg.querySelectorAll(SHAPES)) {
      if (sh.closest("defs, symbol, mask, clipPath, pattern, marker")) continue;
      const s = cs(sh);
      if (s.display === "none" || s.visibility === "hidden") continue;
      const st = s.stroke !== "none" && parseFloat(s.strokeWidth) > 0 && +s.strokeOpacity > 0.05 && rgba(s.stroke);
      if (st && st[3] > 0.05) strokes.push(st);
      const fl = s.fill !== "none" && +s.fillOpacity > 0.05 && rgba(s.fill);
      if (fl && fl[3] > 0.05) fills.push(fl);
    }
    for (const g of copies) g.remove();
    let worst = null;
    for (const a of strokes) for (const b of [...strokes, ...fills]) {
      const ha = hueOf(a), hb = hueOf(b);
      if (ha === null || hb === null) continue;
      const apart = Math.min(Math.abs(ha - hb), 360 - Math.abs(ha - hb));
      if (apart > 45 && (!worst || apart > worst[2])) worst = [a, b, apart];
    }
    if (!worst) continue;
    const host = words(svg.closest("button, a, [role=button], li, label") || svg.parentElement);
    add("ink", svg, `the glyph${host ? ` in "${host}"` : ""} mixes two inks: ${hexOf(worst[0])} with ${hexOf(worst[1])}, ${Math.round(worst[2])}° apart.`, host + "|" + hexOf(worst[0]) + "|" + hexOf(worst[1]));
  }

  window.scrollTo(0, 0);
  return { findings: out, measured };
}

// What each rule is for, where it is written, and how to fix it (common/gates.md: every finding says how to fix it).
const RULE = {
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
  contrast: ["Text passes contrast: 4.5:1, or 3:1 for large text", "Darken (or, in the dark theme, lighten) the text or its ground.", "A disabled control, which also says it is disabled"],
  mark: ["A mark holds at its smallest size", "Measure it where it ships smallest: every stroke 1 px or more, no leftover piece of a cut, the brand element 10 px or taller, and the brand touching the letters or a quarter of a stroke clear of them.", "A mark only ever shown large, a poster or a splash", "ui/identity-pass.md"],
  ink: ["A glyph keeps one ink", "Print the plane or fill as a faint tint of the stroke's colour, or a neutral (white or black at low alpha, the paper).", "A glyph meant to carry two colours (a flag)"],
  allow: ["Every exception has a written reason", "Write data-allow=\"rule: a reason of at least three words\".", "-"],
};


// --- Contrast on gradients and images: hide every glyph, screenshot the page,
// and measure each line of text against the pixels that were behind it.
function decodePng(buf) {
  let at = 8, w = 0, h = 0, type = 0;
  const idat = [];
  while (at < buf.length) {
    const len = buf.readUInt32BE(at), kind = buf.toString("ascii", at + 4, at + 8), data = buf.subarray(at + 8, at + 8 + len);
    if (kind === "IHDR") { w = data.readUInt32BE(0); h = data.readUInt32BE(4); type = data[9]; if (data[8] !== 8) throw new Error("PNG: 8-bit only"); }
    else if (kind === "IDAT") idat.push(data);
    at += 12 + len;
  }
  const bpp = type === 6 ? 4 : type === 2 ? 3 : 0;
  if (!bpp) throw new Error(`PNG colour type ${type} isn't RGB or RGBA`);
  const raw = inflateSync(Buffer.concat(idat)), stride = w * bpp, px = Buffer.alloc(h * stride);
  for (let yy = 0; yy < h; yy++) {
    const f = raw[yy * (stride + 1)], line = raw.subarray(yy * (stride + 1) + 1, (yy + 1) * (stride + 1)), o = yy * stride;
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? px[o + x - bpp] : 0, b = yy ? px[o - stride + x] : 0, c = yy && x >= bpp ? px[o - stride + x - bpp] : 0;
      let v = line[x];
      if (f === 1) v += a; else if (f === 2) v += b; else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      px[o + x] = v & 255;
    }
  }
  return { w, h, bpp, px };
}
const lum = (c) => { const f = (v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]); };
async function pixelContrast(s, result, width) {
  await s("Runtime.evaluate", { expression: `(() => { const st = document.createElement("style"); st.id = "__check_ui_hide"; st.textContent = "*, *::before, *::after { color: transparent !important; -webkit-text-fill-color: transparent !important; text-shadow: none !important; text-decoration-color: transparent !important; caret-color: transparent !important }"; document.head.append(st); })()` });
  const { cssContentSize } = await s("Page.getLayoutMetrics");
  const height = Math.ceil(cssContentSize.height), tile = 4000, tiles = [];
  for (let top = 0; top < height; top += tile) {
    const { data } = await s("Page.captureScreenshot", { format: "png", captureBeyondViewport: true, clip: { x: 0, y: top, width, height: Math.min(tile, height - top), scale: 1 } });
    tiles.push({ top, img: decodePng(Buffer.from(data, "base64")) });
  }
  const pixel = (x, yy) => {
    const t = tiles[Math.floor(yy / tile)];
    if (!t) return null;
    const { w, h, bpp, px } = t.img, ly = yy - t.top;
    if (x < 0 || x >= w || ly < 0 || ly >= h) return null;
    const o = ly * w * bpp + x * bpp;
    return [px[o], px[o + 1], px[o + 2]];
  };
  for (const p of result.measured.pending) {
    const ratios = [];
    for (const [lx, ly, lw, lh] of p.lines) {
      const sx = Math.max(1, Math.floor(lw / 24)), sy = Math.max(1, Math.floor(lh / 6));
      for (let yy = Math.ceil(ly); yy < ly + lh; yy += sy) for (let x = Math.ceil(lx); x < lx + lw; x += sx) {
        const bg = pixel(x, yy);
        if (!bg) continue;
        const fg = [0, 1, 2].map((i) => p.fg[i] * p.fg[3] + bg[i] * (1 - p.fg[3]));
        const [l1, l2] = [lum(fg), lum(bg)].sort((a, b) => b - a);
        ratios.push((l1 + 0.05) / (l2 + 0.05));
      }
    }
    if (!ratios.length) { result.measured.contrastSkipped++; continue; }
    ratios.sort((a, b) => a - b);
    const ratio = ratios[Math.floor(ratios.length * 0.05)]; // the worst 5 %: a stray anti-aliased pixel doesn't decide
    const need = p.large ? 3 : 4.5;
    result.measured.contrastMeasured++;
    if (ratio < need - 0.005) result.findings.push({ rule: "contrast", where: p.where, y: p.y, key: p.label + "|" + p.words,
      text: `"${p.words}" (${p.label}) is ${Math.round(ratio * 100) / 100}:1 against the pixels behind it (a gradient or a drawing); ${p.large ? "large " : ""}text needs ${need}:1.` });
  }
  delete result.measured.pending;
}

// --- Run every board at every width and theme.
const runs = [], marks = new Map();
try {
  const { targetInfos } = await send("Target.getTargets");
  const page = targetInfos.find((t) => t.type === "page");
  const { sessionId } = await send("Target.attachToTarget", { targetId: page.targetId, flatten: true });
  const s = (m, p) => send(m, p, sessionId);
  await s("Page.enable");
  await s("Runtime.enable");
  await s("Animation.enable");
  for (const file of files) {
    const url = pathToFileURL(resolve(file)).href;
    for (const theme of opts.themes.split(/\s+/).filter(Boolean)) for (const width of opts.widths.split(/\s+/).filter(Boolean).map(Number)) {
      await s("Emulation.setDeviceMetricsOverride", { width, height: 900, deviceScaleFactor: 1, mobile: false });
      await s("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: theme }, { name: "prefers-reduced-motion", value: "reduce" }] });
      await s("Animation.setPlaybackRate", { playbackRate: 0 }); // every run measures the same frame
      events.length = 0;
      await s("Page.navigate", { url });
      const t0 = Date.now();
      while (!events.some((e) => e.method === "Page.loadEventFired" && e.sessionId === sessionId)) {
        if (Date.now() - t0 > 30000) throw new Error(`${file} didn't load in 30 s`);
        await sleep(25);
      }
      await s("Runtime.evaluate", { expression: "document.fonts.ready.then(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))))", awaitPromise: true });
      const cfg = { targets: opts.targets, states: opts.states, minText: +opts["min-text"], gap: +opts.gap, halo: +opts.halo };
      const r = await s("Runtime.evaluate", { expression: `(${audit.toString()})(${JSON.stringify(cfg)})`, returnByValue: true, awaitPromise: true });
      if (r.exceptionDetails) throw new Error(`${file}: the audit failed in the page: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
      const result = r.result.value;
      // The marks once per board, on the first render, before contrast hides the text's colour.
      if (!marks.has(file)) marks.set(file, await markAudit(s, width, opts, decodePng));
      result.findings.push(...marks.get(file));
      if (result.measured.pending.length) await pixelContrast(s, result, width);
      runs.push({ file, theme, width, ...result });
    }
  }
} catch (err) {
  die(err.message);
}

// --- Report: grouped by rule, one line per fault with every render it showed in.
const grouped = new Map();
let measured = 0, skipped = 0;
for (const run of runs) {
  measured += run.measured.contrastMeasured;
  skipped += run.measured.contrastSkipped;
  for (const f of run.findings) {
    const k = `${run.file}|${f.rule}|${f.key}`;
    if (!grouped.has(k)) grouped.set(k, { ...f, file: run.file, seen: new Set(), copies: 0 });
    const g = grouped.get(k);
    const render = `${run.theme} ${run.width}`;
    if (!g.seen.has(render)) g.seen.add(render); else if (g.seen.size === 1) g.copies++;
  }
}
const perFile = runs.length / files.length;
const order = Object.keys(RULE);
const list = [...grouped.values()].sort((a, b) => order.indexOf(a.rule) - order.indexOf(b.rule) || a.file.localeCompare(b.file) || a.y - b.y);
let last = "";
for (const g of list) {
  if (g.rule !== last) {
    last = g.rule;
    const [rule, fix, deliberate] = RULE[g.rule];
    const n = list.filter((x) => x.rule === g.rule).length;
    console.log(`\n[${g.rule}] ${n} finding${n > 1 ? "s" : ""}. Rule: ${rule} (.practices/docs/${RULE[g.rule][3] || "ui/visual-craft.md"}).`);
    console.log(`  Fix: ${fix}`);
    if (deliberate !== "-") console.log(`  Deliberate (${deliberate})? Mark the element or its group: data-allow="${g.rule}: <the reason>".`);
  }
  const seen = g.seen.size === perFile ? "" : ` [${[...g.seen].join(", ")}]`;
  const copies = g.copies ? ` (and ${g.copies} more like it)` : "";
  console.log(`  - ${g.file}, ${g.y} px down: ${g.text}${copies}${g.where ? " " + g.where : ""}${seen}`);
}
const n = grouped.size;
console.log(`\ncheck-ui: ${files.length} board${files.length > 1 ? "s" : ""}, ${runs.length} renders (${opts.themes} × ${opts.widths} px): ${n ? n + " finding" + (n > 1 ? "s" : "") : "no findings"}.`);
console.log(`check-ui: contrast measured on ${measured} text runs${skipped ? `; ${skipped} could not be measured` : ""}.`);
cleanup();
process.exit(n ? 1 : 0);
