// check-copy's reading half: the grammar between words, checked for the
// language each piece of copy is in (common/voice.md, "Each language's
// grammar is checked"), and one fact, one wording across the boards
// (ui/design-before-build.md, "Every fact a screen uses was entered
// earlier"). check-copy runs it; read that script for the settings.
//
//   node copy-audit.mjs [--lang LANG] [--rules DIR] [--context FILE]... FILE...
//
// Boards (.html) are read as written: the text of each block, with the
// lang of the element it sits in (<html lang="cs">, a <span lang="en">),
// the copy in aria-label, alt, title and placeholder, and an exception
// marked on the element or an ancestor, data-allow="copy: <reason>".
// Every other file is read by voice-audit.py --strings, as check-voice
// reads it: the language from the path (cs.json, values-cs/, cs.lproj/),
// the block (`cs: {`) or a Markdown page's front matter (`lang: cs`), an exception marked on the string's line or the one
// above, "practices-allow copy: <reason>". Copy that names no language
// takes --lang (COPY_LANG); with none, it is a finding: never a guess.
//
// The rules are copy-rules/<lang>.mjs beside this file, plus the project's
// own in --rules DIR (.practices/copy-rules): a language with copy and no
// rules file is a finding. An element marked data-fact="KEY" holds a fact
// several boards show (an event's time, a name): every board says it in
// the same words. --context names boards read only for their facts.
//
// Exit 0: no findings. 1: findings (grouped, each with its fix). 2: a file
// or a rules file it couldn't read: never "clean".
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

// --- The rules: { lang: { rules, vague, files } }, the kit's and the project's.
export async function loadRules(projectDir) {
  const langs = {};
  for (const dir of [join(here, "copy-rules"), projectDir ? resolve(projectDir) : ""].filter(Boolean)) {
    if (!existsSync(dir)) continue;
    for (const f of readdirSync(dir).filter((x) => /^[a-z]{2,3}\.mjs$/.test(x)).sort()) {
      const lang = f.slice(0, -4), path = join(dir, f);
      const mod = await import(pathToFileURL(path).href);
      const l = langs[lang] || (langs[lang] = { rules: [], vague: [], files: [] });
      (mod.rules || []).forEach((r, i) => {
        const lacks = ["id", "what", "fix"].filter((k) => typeof r[k] !== "string" || !r[k]);
        if (!(r.find instanceof RegExp) || !r.find.global) lacks.push("find (a RegExp with the g flag)");
        if (typeof r.check !== "function") lacks.push("check (a function)");
        if (lacks.length) throw new Error(`${path}: rule ${i + 1}${r.id ? ` (${r.id})` : ""} has no ${lacks.join(", ")}.`);
      });
      l.rules.push(...(mod.rules || []).map((r) => ({ ...r, lang })));
      l.vague.push(...(mod.vagueButtons || []));
      l.files.push(path);
    }
  }
  return { langs, vague: Object.fromEntries(Object.entries(langs).map(([k, v]) => [k, v.vague])) };
}

// --- A board, read as written. Strings are { line, text, chunks: [{ start, lang, allow, line }] }.
const VOID = new Set("area base br col embed hr img input link meta param source track wbr".split(" "));
const INLINE = new Set("a abbr b bdi bdo br cite code data dfn em font i kbd label mark q s samp small span strong sub sup time tspan u var wbr".split(" "));
const RAW = new Set(["script", "style", "template"]);
const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: "\"", apos: "'", nbsp: " ", ndash: "–", mdash: "—", hellip: "…", thinsp: " ", shy: "", lsquo: "‘", rsquo: "’", ldquo: "“", rdquo: "”", bdquo: "„", laquo: "«", raquo: "»" };
const decode = (s) => s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (all, e) =>
  e[0] === "#" ? String.fromCodePoint(e[1] === "x" || e[1] === "X" ? parseInt(e.slice(2), 16) : +e.slice(1)) : ENTITIES[e.toLowerCase()] ?? all);
const lineAtOf = (text) => {
  const starts = [0];
  for (let i = text.indexOf("\n"); i >= 0; i = text.indexOf("\n", i + 1)) starts.push(i + 1);
  return (at) => { let lo = 0, hi = starts.length - 1; while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid] <= at) lo = mid; else hi = mid - 1; } return lo + 1; };
};
const words = (s) => s.split(/\s+/).filter(Boolean).length;
const langOf = (v) => (v || "").trim().toLowerCase().split(/[-_]/)[0] || null;

function readBoard(text) {
  const lineAt = lineAtOf(text);
  const strings = [], facts = [], marks = [];
  const stack = [{ tag: "", lang: null, allow: null, fact: null }];
  let cur = null;
  const flush = () => { if (cur && /\p{L}{2}/u.test(cur.text)) strings.push(cur); cur = null; };
  const single = (s, lang, allow, line) => { if (/\p{L}{2}/u.test(s)) strings.push({ text: s, chunks: [{ start: 0, lang, allow, line }] }); };
  const tok = /<!--[\s\S]*?-->|<![^>]*>|<\?[^>]*>|<\/([A-Za-z][\w:-]*)\s*>|<([A-Za-z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>|[^<]+|</g;
  for (let m; (m = tok.exec(text)); ) {
    const [all, close, open, rest] = m;
    if (open) {
      const tag = open.toLowerCase(), line = lineAt(m.index), top = stack[stack.length - 1];
      const attrs = {};
      for (const a of rest.matchAll(/([^\s=/"']+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?/g)) attrs[a[1].toLowerCase()] = decode(a[2] ?? a[3] ?? a[4] ?? "");
      if (!INLINE.has(tag)) flush();
      const lang = "lang" in attrs ? langOf(attrs.lang) : top.lang;
      let allow = top.allow;
      for (const part of (attrs["data-allow"] || "").split(";")) {
        const [rule, ...why] = part.split(":");
        const reason = why.join(":").trim();
        if (!/^(copy|fact)$/.test(rule.trim())) continue;
        if (words(reason) < 3) marks.push({ line, rule: rule.trim(), reason });
        else if (rule.trim() === "copy") allow = reason;
      }
      for (const a of ["aria-label", "alt", "title", "placeholder"]) if (attrs[a]) single(attrs[a], lang, allow, line);
      if (tag === "input" && /^(button|submit|reset)$/i.test(attrs.type || "") && attrs.value) single(attrs.value, lang, allow, line);
      const closed = VOID.has(tag) || /\/\s*$/.test(rest);
      if (RAW.has(tag) && !closed) {
        const end = text.toLowerCase().indexOf(`</${tag}`, tok.lastIndex);
        tok.lastIndex = end < 0 ? text.length : end;
        continue;
      }
      if (!closed) stack.push({ tag, lang, allow, fact: attrs["data-fact"] ? { key: attrs["data-fact"], text: "", line, lang } : null });
    } else if (close) {
      const tag = close.toLowerCase();
      if (!INLINE.has(tag)) flush();
      const i = stack.map((e) => e.tag).lastIndexOf(tag);
      if (i > 0) for (const e of stack.splice(i)) if (e.fact) facts.push(e.fact);
    } else if (!all.startsWith("<!") && !all.startsWith("<?")) {
      const s = decode(all), top = stack[stack.length - 1];
      if (!cur) cur = { text: "", chunks: [] };
      cur.chunks.push({ start: cur.text.length, lang: top.lang, allow: top.allow, line: lineAt(m.index + (all.length - all.trimStart().length)) });
      cur.text += s;
      for (const e of stack) if (e.fact) e.fact.text += s;
    }
  }
  flush();
  return { strings, facts, marks };
}

// --- Everything else, read by voice-audit.py, as check-voice reads it.
function readCatalogues(paths) {
  if (!paths.length) return { strings: [], broken: [] };
  const r = spawnSync("python3", [join(here, "voice-audit.py"), "--strings", ...paths], { encoding: "utf8", maxBuffer: 1 << 28 });
  if (r.error || r.status !== 0) {
    console.error(`check-copy: couldn't read ${paths.length} copy file(s) with python3 ${join(here, "voice-audit.py")}: ${r.error ? r.error.message : r.stderr.trim()}`);
    process.exit(2);
  }
  return JSON.parse(r.stdout);
}

// --- Run.
async function main(argv) {
  const opts = { lang: "", rules: "" }, files = [], context = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--context") context.push(argv[++i]);
    else if (argv[i].startsWith("--")) opts[argv[i].slice(2)] = argv[++i] ?? "";
    else files.push(argv[i]);
  }
  if (!files.length) { console.error("check-copy: no files given"); process.exit(2); }
  let langs;
  try { ({ langs } = await loadRules(opts.rules)); } catch (err) { console.error(`check-copy: a copy-rules file is broken: ${err.message}`); process.exit(2); }
  const fallback = langOf(opts.lang);

  const findings = [], broken = [], facts = [];
  const unknown = new Map(), missing = new Map();
  const note = (rule, file, line, text) => findings.push({ rule, file, line, text });
  const pieces = []; // { file, text, chunks }
  const boards = files.filter((f) => /\.html?$/i.test(f)), others = files.filter((f) => !/\.html?$/i.test(f));
  for (const file of [...boards, ...context.filter((c) => !files.includes(c))]) {
    let text;
    try { text = readFileSync(file, "utf8"); } catch (err) { if (files.includes(file)) broken.push(`${file}: ${err.message}`); continue; }
    const read = readBoard(text);
    facts.push(...read.facts.map((f) => ({ ...f, file, lang: f.lang || fallback })));
    if (!files.includes(file)) continue;
    for (const mk of read.marks) note("allow", file, mk.line, `data-allow="${mk.rule}: ${mk.reason}" gives no reason of three words or more.`);
    for (const s of read.strings) pieces.push({ file, ...s });
  }
  const cat = readCatalogues(others);
  broken.push(...cat.broken);
  // A code catalogue (a .ts or .js file with language blocks) keeps its copy in them; outside them
  // are its ids, plural categories and format strings, code. 1.7.1 read Obálka's FAQ order and plural
  // rules as copy in no language.
  const blocks = new Set(cat.strings.filter((s) => s.lang && /\.[cm]?[jt]sx?$/.test(s.path)).map((s) => s.path));
  for (const s of cat.strings) {
    if (!s.lang && blocks.has(s.path)) continue;
    if (s.allow !== null && words(s.allow) < 3) { note("allow", s.path, s.line, `"practices-allow copy: ${s.allow}" gives no reason of three words or more.`); continue; }
    if (/\p{L}{2}/u.test(s.text)) pieces.push({ file: s.path, text: s.text, chunks: [{ start: 0, lang: langOf(s.lang), allow: s.allow, line: s.line }] });
  }

  for (const p of pieces) {
    const chunkAt = (i) => { let c = p.chunks[0]; for (const x of p.chunks) if (x.start <= i) c = x; return c; };
    const langAt = (c) => c.lang || fallback;
    for (const lang of new Set(p.chunks.map(langAt))) {
      if (!lang) { unknown.set(p.file, (unknown.get(p.file) || 0) + 1); continue; }
      if (!langs[lang]) { const k = `${lang}|${p.file}`; missing.set(k, (missing.get(k) || 0) + 1); continue; }
      for (const rule of langs[lang].rules) {
        rule.find.lastIndex = 0;
        for (const m of p.text.matchAll(rule.find)) {
          const c = chunkAt(m.index);
          if (langAt(c) !== lang || p.chunks.some((x) => x.allow && x.start < m.index + m[0].length && (x === c || x.start >= m.index))) continue;
          const said = rule.check(m);
          if (said) note(rule.id, p.file, c.line, said);
        }
      }
    }
  }
  for (const [file, n] of unknown) note("language", file, 0, `${n} piece${n > 1 ? "s" : ""} of copy name${n > 1 ? "" : "s"} no language, so no rules ran on ${n > 1 ? "them" : "it"}. Name it: <html lang="cs"> on a board, the locale in the path (cs.json, values-cs/, cs.lproj/) or the block (\`cs: {\`), "lang: cs" in a Markdown page's front matter; or set COPY_LANG in .practices/config to the language of copy that names none.`);
  for (const [k, n] of missing) {
    const [lang, file] = k.split("|");
    note("language", file, 0, `${n} piece${n > 1 ? "s" : ""} of copy in "${lang}", and no rules for it: add .practices/copy-rules/${lang}.mjs with its rules and vagueButtons (the kit's copy-rules/cs.mjs is the format). A language ships with its rules.`);
  }

  // One fact, one wording: every board that shows a fact says it the same way.
  const byKey = new Map();
  for (const f of facts) {
    const k = `${f.key}|${f.lang || ""}`;
    if (!byKey.has(k)) byKey.set(k, new Map());
    const said = f.text.replace(/\s+/g, " ").trim();
    const g = byKey.get(k);
    if (!g.has(said)) g.set(said, []);
    g.get(said).push(f);
  }
  for (const [k, g] of byKey) {
    if (g.size < 2) continue;
    const all = [...g.values()].flat();
    const mine = all.find((f) => files.includes(f.file));
    if (!mine) continue;
    const ways = [...g.entries()].map(([said, at]) => `"${said}" (${at.map((f) => `${f.file}:${f.line}`).slice(0, 3).join(", ")}${at.length > 3 ? ", …" : ""})`);
    note("fact", mine.file, mine.line, `the fact "${k.split("|")[0]}" reads ${g.size} ways: ${ways.join(" vs ")}.`);
  }

  const WHAT = {
    language: ["Each language's grammar is checked: copy names its language, and the language has rules", "-"],
    allow: ["Every exception has a written reason", "Write the reason: data-allow=\"copy: <three words or more>\", or practices-allow copy: <the reason>."],
    fact: ["One fact, one source: a fact several screens show reads the same on each", "Write the fact once, in the generator's or the prototype's data, and fill every board from it; a fact that really differs gets its own data-fact key."],
  };
  const order = ["language", "allow", "fact"];
  const ruleOf = (id) => Object.values(langs).flatMap((l) => l.rules).find((r) => r.id === id);
  findings.sort((a, b) => (order.indexOf(a.rule) + 1 || 99) - (order.indexOf(b.rule) + 1 || 99) || a.rule.localeCompare(b.rule) || a.file.localeCompare(b.file) || a.line - b.line);
  if (broken.length) {
    console.log("check-copy could not read these, so it can't say they are clean:");
    for (const b of broken) console.log(`  - ${b}`);
  }
  let last = "";
  for (const f of findings) {
    if (f.rule !== last) {
      last = f.rule;
      const n = findings.filter((x) => x.rule === f.rule).length, r = ruleOf(f.rule);
      const [what, fix] = WHAT[f.rule] || [`${r.what} (${r.lang})`, r.fix];
      console.log(`\n[${f.rule}] ${n} finding${n > 1 ? "s" : ""}. Rule: ${what} (.practices/docs/${f.rule === "fact" ? "ui/design-before-build.md" : "common/voice.md"}).`);
      if (fix !== "-") console.log(`  Fix: ${fix}`);
    }
    console.log(`  - ${f.file}${f.line ? ":" + f.line : ""} ${f.text}`);
  }
  if (findings.some((f) => ruleOf(f.rule)))
    console.log(`\n  Deliberate (a quoted name, a brand)? Mark it with its reason: data-allow="copy: <the reason>" on the board's element, or practices-allow copy: <the reason> on the string's line or the one above.`);
  const n = findings.length;
  console.log(`\ncheck-copy: ${files.length} file${files.length > 1 ? "s" : ""} (${Object.keys(langs).sort().join(", ")} rules): ${n || "no"} finding${n === 1 ? "" : "s"}.`);
  process.exit(broken.length ? 2 : n ? 1 : 0);
}

// Run when called, not when ui-audit.mjs imports loadRules. Both paths resolved: a temporary folder
// on macOS is /var/…, a link to /private/var/…, and a plain comparison skipped the audit and passed.
const self = (p) => { try { return realpathSync(p); } catch { return p; } };
if (process.argv[1] && self(process.argv[1]) === self(fileURLToPath(import.meta.url))) await main(process.argv.slice(2));
