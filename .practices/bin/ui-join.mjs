// check-ui's join check (ui/visual-craft.md, "A joined line breaks between
// its parts"): ui-audit.mjs calls joinAudit at every render, since where a
// line breaks depends on the width. No packages.
//
// Parts joined on one line by a separator (" · ", " | ") break between
// parts, never at the separator. Each separator's line is measured against
// the nearest character before and after it, across elements, inside the
// block that holds the line; on another line from either, it is a finding.
// A separator with nothing on one side (a leading mark) joins nothing.
//
// Why: an iPhone app's Today, 25 Nov 2026. "tento týden · celkem 18" was
// allowed two lines and broke after the dot: "tento týden ·" over "celkem
// 18". The English fitted, so nothing looked wrong in the screenshots.

// --- What runs in the page: the findings for one render.
function page() {
  const out = [], cs = (e) => getComputedStyle(e);
  const allowNode = (e, rule) => {
    for (let a = e; a && a.nodeType === 1; a = a.parentElement) {
      const v = a.getAttribute("data-allow");
      if (v && v.split(";").some((p) => p.split(":")[0].trim() === rule)) return a;
    }
    return null;
  };
  const visible = (e) => {
    if (!e || e.closest("template, symbol, defs, clipPath, mask, pattern, marker, [hidden], noscript, head, style, script, title, [aria-hidden=true], [inert]")) return false;
    for (let a = e; a && a.nodeType === 1; a = a.parentElement) if (+cs(a).opacity === 0) return false;
    const s = cs(e), r = e.getBoundingClientRect();
    return s.display !== "none" && s.visibility === "visible" && r.width > 0.5 && r.height > 0.5;
  };
  const label = (e) => {
    const c = typeof e.className === "string" && e.className.trim() ? "." + e.className.trim().split(/\s+/).join(".") : "";
    return e.tagName.toLowerCase() + (e.id ? "#" + e.id : "") + c;
  };
  const words = (e) => (e.textContent || "").replace(/\s+/g, " ").trim().slice(0, 40);
  const heads = [...document.querySelectorAll("h1, h2, h3")];
  const where = (e) => {
    let h = null;
    for (const x of heads) if (x.compareDocumentPosition(e) & Node.DOCUMENT_POSITION_FOLLOWING || x.contains(e)) h = x; else break;
    const win = e.closest("[id]");
    return [h ? `under "${words(h)}"` : null, win && win !== e ? `in #${win.id}` : null].filter(Boolean).join(", ");
  };
  const add = (e, text) => out.push({ rule: "join", text, where: where(e), y: Math.round(e.getBoundingClientRect().top + scrollY), key: label(e) + "|" + words(e) });

  const SEP = /[ \n\t](·|\|)(?=[ \n\t])|[ \n\t]·|·(?=[ \n\t])/g;
  const charRect = (n, i) => { const r = document.createRange(); r.setStart(n, i); r.setEnd(n, i + 1); return [...r.getClientRects()].find((x) => x.width > 0.1) || null; };
  const sameLine = (a, b) => Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > Math.min(a.height, b.height) / 2;
  // The block that holds the line: up through inline elements, and through any that hold only the separator.
  const blockOf = (e) => {
    let c = e;
    while (c.parentElement && c !== document.body && (/^(inline|contents)$/.test(cs(c).display) || !/[^\s·|]/.test(c.textContent))) c = c.parentElement;
    return c;
  };
  // The nearest drawn character before (dir -1) or after (+1) offset i of node n, inside root.
  const neighbour = (root, n, i, dir) => {
    const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    w.currentNode = n;
    for (let node = n, k = i + dir; node; node = dir < 0 ? w.previousNode() : w.nextNode(), k = dir < 0 ? (node ? node.textContent.length - 1 : 0) : 0) {
      if (node !== n && !visible(node.parentElement)) continue;
      const tx = node.textContent;
      for (; k >= 0 && k < tx.length; k += dir) if (!/\s/.test(tx[k])) { const r = charRect(node, k); if (r) return r; }
    }
    return null;
  };
  // What a sheet or a window covers isn't seen: hit-test the separator's middle.
  // Scrolled into view first, then measured again: elementFromPoint takes the window's coordinates.
  const covered = (e, n, at) => {
    const q0 = charRect(n, at);
    window.scrollTo(Math.max(0, scrollX + q0.left - innerWidth / 2), Math.max(0, scrollY + q0.top - innerHeight / 2));
    const q = charRect(n, at);
    if (!q) return false;
    const hit = document.elementFromPoint(q.left + q.width / 2, q.top + q.height / 2);
    return !!hit && !(hit === e || e.contains(hit) || hit.contains(e));
  };

  const flagged = new Set();
  const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n; (n = walk.nextNode()); ) {
    const tx = n.textContent;
    if (!/[·|]/.test(tx)) continue;
    const e = n.parentElement;
    if (!visible(e) || allowNode(e, "join")) continue;
    for (const m of tx.matchAll(SEP)) {
      const at = m.index + m[0].indexOf(m[0].trim());
      const root = blockOf(e);
      if (flagged.has(root)) break;
      const dot = charRect(n, at);
      if (!dot) continue;
      const before = neighbour(root, n, at, -1), after = neighbour(root, n, at, 1);
      if (!before || !after) continue;
      if (sameLine(before, dot) && sameLine(dot, after)) continue;
      if (covered(e, n, at)) continue;
      flagged.add(root);
      add(root, `"${words(root)}" breaks at its "${tx[at]}": the separator ${sameLine(before, dot) ? "ends a line" : "starts a line"}.`);
    }
  }
  window.scrollTo(0, 0);
  return out;
}

export async function joinAudit(s) {
  const r = await s("Runtime.evaluate", { expression: `(${page.toString()})()`, returnByValue: true });
  if (r.exceptionDetails) throw new Error(`the join check failed in the page: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
  return r.result.value;
}
