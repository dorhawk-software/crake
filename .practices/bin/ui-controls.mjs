// check-ui's control checks: ui-audit.mjs calls controlAudit at every render.
// No packages.
//
// - stack (ui/visual-craft.md, "Alike buttons stacked become one block"): two
//   buttons with a surface of the same look (fill, corners, ink, type, height),
//   one under the other in one column, less than 24 px apart with nothing drawn
//   between. They read as one button twice. One block (a hairline element
//   between its rows) or a word between passes. Three or more alike in a run
//   are a list (rows of one kind), left to review. Options of one question
//   (radio, tab, option, checkbox, switch, menu item) are left to their own rules.
// - menu (ui/pickers.md, "A time turns on a wheel; a menu holds a handful"): a
//   <select>, listbox or menu with more than 12 choices. A time or a list built
//   from data turns on a wheel.
//
// - stripe (ui/visual-craft.md, "A box has no stripe down one side"): a box
//   (an element with its own ground) with a border on one side 2 px or more
//   while the others have none (or under 1 px), or an inset shadow that draws
//   one (inset 4px 0 0). A generic callout's accent, not the product's own
//   sign. A hairline passes; so does an underline with no ground behind it.
//
// Why: one iPhone app, 25 Nov 2026. Its Focus card had "Close all until 04:00"
// and "Until a date…" alike, one under the other, and the owner read one button
// twice; its lock's end time came from a menu of 96 quarter hours; and its
// lock's warning was a grey box with a 4 px stripe down its left edge.

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
    if (!e || e.closest("template, symbol, defs, [hidden], noscript, head, [aria-hidden=true], [inert]")) return false;
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
  const add = (rule, e, text, key) => out.push({ rule, text, where: where(e), y: Math.round(e.getBoundingClientRect().top + scrollY), key: key || label(e) + "|" + words(e) });
  const alpha = (c) => { const m = c.match(/rgba?\(([^)]+)\)/); if (!m) return c === "transparent" ? 0 : 1; const p = m[1].split(/[ ,/]+/).filter(Boolean); return p.length > 3 ? parseFloat(p[3]) : 1; };

  // stack
  const OPTIONS = "[role=radio], [role=tab], [role=option], [role=checkbox], [role=switch], [role=menuitem], [role=menuitemradio], [role=radiogroup] *, [role=tablist] *";
  const surfaced = (e) => {
    const s = cs(e);
    return alpha(s.backgroundColor) > 0.05 || s.backgroundImage !== "none" || (parseFloat(s.borderTopWidth) > 0 && alpha(s.borderTopColor) > 0.05);
  };
  const look = (e) => { const s = cs(e); return [s.backgroundColor, s.backgroundImage, s.borderTopLeftRadius, s.borderBottomLeftRadius, s.borderTopWidth, s.borderTopColor, s.color, s.fontSize, s.fontWeight, Math.round(e.getBoundingClientRect().height / 2)].join("|"); };
  const buttons = [...document.querySelectorAll("button, [role=button], a[href]")]
    .filter((e) => visible(e) && !e.matches(OPTIONS) && surfaced(e));
  // Pairs first; a run of three or more alike is a list, and is left to review.
  const pairs = [], next = new Map(), prev = new Map();
  for (const a of buttons) {
    const ra = a.getBoundingClientRect();
    for (const b of buttons) {
      if (a === b || a.contains(b) || b.contains(a)) continue;
      const rb = b.getBoundingClientRect();
      const gap = rb.top - ra.bottom;
      if (gap < -0.5 || gap >= 24 || Math.abs(ra.left - rb.left) > 2 || Math.abs(ra.width - rb.width) > 2) continue;
      if (look(a) !== look(b)) continue;
      next.set(a, b); prev.set(b, a); // a run counts alike neighbours, whatever is drawn between
      // Anything drawn between them, in their column: a hairline, a word.
      let between = false;
      if (gap > 0.5) {
        for (const e of document.querySelectorAll("body *")) {
          if (e === a || e === b || e.contains(a) || e.contains(b) || a.contains(e) || b.contains(e) || !visible(e)) continue;
          const r = e.getBoundingClientRect();
          if (r.bottom <= ra.bottom + 0.25 || r.top >= rb.top - 0.25 || r.right <= ra.left || r.left >= ra.right) continue;
          if (r.height < 0.5 && r.width < 0.5) continue;
          between = true;
          break;
        }
      }
      if (between) continue;
      pairs.push([a, b, gap]);
    }
  }
  for (const [a, b, gap] of pairs) {
    {
      if (prev.has(a) || next.has(b) || allowNode(a, "stack") || allowNode(b, "stack")) continue;
      add("stack", b, `"${words(a)}" and "${words(b)}" look alike, one under the other ${Math.round(Math.max(gap, 0))} px apart with nothing between: they read as one button twice.`, label(a) + "|" + words(a) + "|" + words(b));
    }
  }

  // menu
  const lists = [...document.querySelectorAll("select, [role=listbox], [role=menu]")];
  for (const l of lists) {
    if (!(l.tagName === "SELECT" ? !l.closest("[hidden], [aria-hidden=true], [inert]") && cs(l).display !== "none" : visible(l))) continue;
    if (allowNode(l, "menu")) continue;
    const n = l.tagName === "SELECT" ? l.querySelectorAll("option").length : l.querySelectorAll("[role=option], [role=menuitem], [role=menuitemradio], [role=menuitemcheckbox]").length;
    if (n <= 12) continue;
    add("menu", l, `${label(l)} offers ${n} choices in a menu; a time or a list built from data turns on a wheel.`);
  }
  // stripe
  const SIDES = ["Top", "Right", "Bottom", "Left"];
  for (const e of document.querySelectorAll("body *")) {
    if (!visible(e) || allowNode(e, "stripe")) continue;
    const s = cs(e), r = e.getBoundingClientRect();
    // A box: its own ground. A heading's underline or a tab's selected edge has none.
    if (r.width < 24 || r.height < 16 || !(alpha(s.backgroundColor) > 0.05 || s.backgroundImage !== "none")) continue;
    const w = SIDES.map((x) => (s["border" + x + "Style"] === "none" || alpha(s["border" + x + "Color"]) < 0.05 ? 0 : parseFloat(s["border" + x + "Width"])));
    const thick = w.map((x, i) => (x >= 2 && w.every((y, j) => j === i || y < 1) ? i : -1)).filter((i) => i >= 0);
    let side = thick.length === 1 ? SIDES[thick[0]].toLowerCase() : null, px = side ? w[thick[0]] : 0;
    if (!side && s.boxShadow !== "none") {
      for (const sh of s.boxShadow.split(/,(?![^(]*\))/)) {
        if (!/inset/.test(sh) || alpha(sh) < 0.05) continue;
        const n = (sh.replace(/rgba?\([^)]*\)/, "").match(/-?\d+(\.\d+)?px/g) || []).map(parseFloat);
        const [x = 0, y = 0, blur = 0, spread = 0] = n;
        if (blur === 0 && spread === 0 && (Math.abs(x) >= 2) !== (Math.abs(y) >= 2)) {
          side = Math.abs(x) >= 2 ? (x > 0 ? "left" : "right") : (y > 0 ? "top" : "bottom");
          px = Math.abs(x) >= 2 ? Math.abs(x) : Math.abs(y);
          break;
        }
      }
    }
    if (!side) continue;
    add("stripe", e, `${label(e)} "${words(e)}" draws a ${px} px stripe down its ${side} side, an accent no other box wears.`);
  }
  return out;
}

export async function controlAudit(s) {
  const r = await s("Runtime.evaluate", { expression: `(${page.toString()})()`, returnByValue: true });
  if (r.exceptionDetails) throw new Error(`the control checks failed in the page: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
  return r.result.value;
}
