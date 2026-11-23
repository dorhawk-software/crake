// check-page's measuring half: a proposal or review page, checked as its reader opens it, on a
// phone (ui/design-before-build.md, "Proposal pages pass a checklist before review"). check-page
// runs it; read that script for the settings.
//
//   node page-audit.mjs --chrome PATH [--widths "375 390"] [--lang LANG] [--copy-rules DIR] PAGE...
//
// At each width:
//   - sideways: nothing scrolls sideways (the document is no wider than the screen);
//   - one-letter-end: no one-letter word ends a line, in a language whose copy rules tie one-letter
//     words to the next (a rule with the id "one-letter-end" in copy-rules/<lang>.mjs: Czech), an
//     option's letter ("A", "B", "C") included; text in a language that names none takes --lang;
//   - image-width: no image runs past the screen;
//   - image-pixels: no image is drawn wider than its pixels at 2x (a blurred crop).
// Once per page: doctype, <!doctype html> on the first line; charset, <meta charset="utf-8">
// in the first 1024 bytes; lang, text that names no language while --lang is empty.
//
// Exit 0: no findings. 1: findings, grouped by rule, each rule with its fix. 2: a page or Chrome
// it couldn't use: never "clean".
//
// Why: Scops' proposal pages went through three review rounds before the owner saw them
// (2026-11-20 to 23). Each round found what a script finds: "C / rozhodne", an option's letter
// ending a line, and a table that scrolled sideways at 375 px.
import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { startChrome } from "./ui-chrome.mjs";
import { loadRules } from "./copy-audit.mjs";

const opts = { chrome: "", widths: "375 390", lang: "", "copy-rules": "" };
const files = [];
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i++) {
  if (argv[i].startsWith("--")) opts[argv[i].slice(2)] = argv[++i] ?? "";
  else files.push(argv[i]);
}
const die = (m) => { console.error(`check-page: ${m}`); process.exit(2); };
if (!files.length) die("no pages given");
if (!opts.chrome || !existsSync(opts.chrome)) die(`no Chrome at "${opts.chrome}". Fix: install Google Chrome or Chromium, or set UI_CHROME in .practices/config.`);
const widths = opts.widths.split(/\s+/).filter(Boolean).map(Number);
if (!widths.length || widths.some((w) => !(w > 0))) die(`PAGE_WIDTHS is "${opts.widths}": give widths in px, space separated ("375 390").`);

// The languages whose copy rules tie a one-letter word to the next.
let langs;
try { ({ langs } = await loadRules(opts["copy-rules"])); } catch (err) { die(`a copy-rules file is broken: ${err.message}`); }
const tie = Object.entries(langs).filter(([, l]) => l.rules.some((r) => r.id === "one-letter-end")).map(([k]) => k);

const RULE = {
  sideways: ["nothing scrolls sideways on a phone",
    "keep every block within the column: max-width: 100%, no fixed width over the screen's; a wide table goes in its own overflow-x: auto box."],
  "one-letter-end": ["no one-letter word ends a line where the language ties it, an option's letter included",
    "tie it to the next word with a no-break space: &nbsp; in HTML (\"C&nbsp;rozhodne\")."],
  "image-width": ["no image runs past the screen", "give the image max-width: 100% (and height: auto)."],
  "image-pixels": ["an image is drawn at most at half its pixel width (2x)",
    "crop at the device's 2x, and draw the crop at half its pixel width or less; link it to the whole picture."],
  doctype: ["the page starts with <!doctype html>", "put <!doctype html> on the first line: without it the page renders in quirks mode."],
  charset: ["the page says its charset", "put <meta charset=\"utf-8\"> first in <head>: without it letters like č and ř can show garbled."],
  lang: ["the page names its language", "set <html lang=\"…\">, or COPY_LANG in .practices/config: a line's end is checked by its language's rules."],
};

// In the page, at width W: every finding, as { rule, text }.
const PROBE = (W, tieLangs, fallback) => `(() => {
  const found = [], tie = new Set(${JSON.stringify(tieLangs)});
  const sw = document.documentElement.scrollWidth;
  if (sw > ${W}) found.push({ rule: "sideways", text: "the page is " + sw + " px wide" });
  let unnamed = 0;
  const words = [];
  const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n; (n = walk.nextNode());) {
    const p = n.parentElement;
    if (!p || p.closest("script, style, noscript, template") || !n.textContent.trim()) continue;
    const lang = ((p.closest("[lang]") || {}).lang || ${JSON.stringify(fallback)}).toLowerCase().split("-")[0];
    if (!lang) { unnamed++; continue; }
    for (const m of n.textContent.matchAll(/[^\\s]+/g)) {
      const r = document.createRange();
      r.setStart(n, m.index); r.setEnd(n, m.index + m[0].length);
      const q = r.getClientRects()[0];
      if (q && q.width > 0) words.push({ w: m[0], lang, top: q.top, bottom: q.bottom });
    }
  }
  if (unnamed) found.push({ rule: "lang", text: unnamed + " piece(s) of text name no language" });
  for (let i = 0; i < words.length - 1; i++) {
    const a = words[i], b = words[i + 1];
    if (tie.has(a.lang) && b.top >= a.bottom - 1 && /^[(„“"'‚«]?\\p{L}$/u.test(a.w))
      found.push({ rule: "one-letter-end", text: "\\"" + a.w + " / " + b.w + "\\" (" + a.lang + ")" });
  }
  for (const img of document.images) {
    const r = img.getBoundingClientRect(), src = img.getAttribute("src") || "", name = src.slice(0, 60);
    if (r.width < 1) continue;
    if (r.right > ${W} + 0.5) found.push({ rule: "image-width", text: name + " ends at " + Math.round(r.right) + " px" });
    const vector = /\\.svg(\\?|#|$)/i.test(src) || /^data:image\\/svg/i.test(src);
    if (!vector && img.naturalWidth && r.width * 2 > img.naturalWidth + 1)
      found.push({ rule: "image-pixels", text: name + " is " + img.naturalWidth + " px wide, drawn " + Math.round(r.width) + " px wide" });
  }
  return found;
})()`;

const { send, openPage, load, cleanup } = startChrome(opts.chrome, die);
const findings = [];
try {
  const session = await openPage();
  const s = (m, p) => send(m, p, session);
  await s("Page.enable");
  for (const page of files) {
    const path = resolve(page);
    let src;
    try { src = readFileSync(path, "utf8"); } catch { die(`can't read ${page}`); }
    const add = (rule, text, width) => findings.push({ rule, page, text, width });
    if (!/^\s*<!doctype html>/i.test(src)) add("doctype", "no <!doctype html> on the first line");
    if (!/<meta\s+charset=["']?utf-8/i.test(src.slice(0, 1024))) add("charset", 'no <meta charset="utf-8"> in its first 1024 bytes');
    for (const W of widths) {
      await s("Emulation.setDeviceMetricsOverride", { width: W, height: 812, deviceScaleFactor: 2, mobile: true });
      await load(session, pathToFileURL(path).href, {}, page);
      const r = await s("Runtime.evaluate", { expression: PROBE(W, tie, opts.lang), returnByValue: true });
      if (r.exceptionDetails) die(`${page} couldn't be measured: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
      for (const f of r.result.value) add(f.rule, f.text, W);
    }
  }
} catch (err) {
  die(err.message);
}
cleanup();

// One line per fault, with every width it showed at.
const grouped = new Map();
for (const f of findings) {
  const k = `${f.rule}|${f.page}|${f.text}`;
  if (!grouped.has(k)) grouped.set(k, { ...f, at: [] });
  if (f.width) grouped.get(k).at.push(f.width);
}
const order = Object.keys(RULE);
const list = [...grouped.values()].sort((a, b) => order.indexOf(a.rule) - order.indexOf(b.rule) || a.page.localeCompare(b.page));
let last = "";
for (const g of list) {
  if (g.rule !== last) {
    last = g.rule;
    const n = list.filter((x) => x.rule === g.rule).length;
    console.log(`\n[${g.rule}] ${n} finding${n > 1 ? "s" : ""}. Rule: ${RULE[g.rule][0]} (.practices/docs/ui/design-before-build.md, "Proposal pages pass a checklist before review").`);
    console.log(`  Fix: ${RULE[g.rule][1]}`);
  }
  console.log(`  - ${g.page}: ${g.text}${g.at.length ? ` [${g.at.join(", ")} px]` : ""}`);
}
const n = grouped.size;
console.log(`\ncheck-page: ${files.length} page${files.length > 1 ? "s" : ""} at ${widths.join(" and ")} px: ${n ? n + " finding" + (n > 1 ? "s" : "") : "no findings"}.`);
process.exit(n ? 1 : 0);
