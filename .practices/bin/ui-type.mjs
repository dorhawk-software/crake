// check-ui's type checks: ui-audit.mjs calls typeAudit once per board, with
// its Chrome session. No packages.
//
// - fallback (ui/identity-pass.md, "Every face draws the product's own
//   language"): text meant for a face the page loads is drawn in that face.
//   Chrome reports the fonts it really used for each element's own text
//   (CSS.getPlatformFontsForNode); a letter the face lacks is drawn by a
//   system font, unseen at a glance. Text whose stack starts with a system or
//   generic family (system-ui, -apple-system, sans-serif) means the system's
//   face, and isn't checked; a page that loads no web font isn't either.
// - tabular (ui/identity-pass.md, "Roles by meaning"): a time ("21:05",
//   "03:48") or a counter the page marks with data-number keeps its width as
//   it changes: its figures, measured in its own font and settings, are all
//   one width (ten 0s as wide as ten 1s), whether by tabular-nums or by the
//   face's design. tabular-nums on a face without tnum changes nothing.

// What runs in the page: every element with text of its own, its stack, and
// whether it is a time or a counter; each gets a data-practices-type index.
function collect() {
  const GENERIC = /^(system-ui|-apple-system|blinkmacsystemfont|sans-serif|serif|monospace|ui-sans-serif|ui-serif|ui-monospace|ui-rounded|cursive|fantasy|emoji|math)$/i;
  const loaded = new Set([...document.fonts].filter((f) => f.status === "loaded").map((f) => f.family.replace(/^["']|["']$/g, "").toLowerCase()));
  const allowed = (e, rule) => { for (let a = e; a && a.nodeType === 1; a = a.parentElement) { const v = a.getAttribute("data-allow"); if (v && v.split(";").some((p) => p.split(":")[0].trim() === rule)) return true; } return false; };
  const heads = [...document.querySelectorAll("h1, h2, h3")];
  const words = (e) => (e.textContent || "").replace(/\s+/g, " ").trim().slice(0, 40);
  const where = (e) => {
    let h = null;
    for (const x of heads) if (x.compareDocumentPosition(e) & Node.DOCUMENT_POSITION_FOLLOWING || x.contains(e)) h = x; else break;
    return h ? `under "${words(h)}"` : "";
  };
  const label = (e) => e.tagName.toLowerCase() + (typeof e.className === "string" && e.className.trim() ? "." + e.className.trim().split(/\s+/).join(".") : "");
  const out = [];
  let i = 0;
  for (const e of document.querySelectorAll("body *")) {
    if (e.closest("template, symbol, defs, [hidden], noscript, [aria-hidden=true] svg") || e instanceof SVGElement && e.tagName.toLowerCase() !== "text") continue;
    const own = [...e.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent).join("").replace(/\s+/g, " ").trim();
    if (!own) continue;
    const s = getComputedStyle(e), r = e.getBoundingClientRect();
    if (s.display === "none" || s.visibility !== "visible" || r.width < 0.5 || r.height < 0.5) continue;
    const first = s.fontFamily.split(",")[0].trim().replace(/^["']|["']$/g, "");
    const web = loaded.has(first.toLowerCase()) && !GENERIC.test(first);
    const number = e.hasAttribute("data-number") || /^\d{1,2}:\d{2}(:\d{2})?$/.test(own);
    let tabular = null;
    if (number && !allowed(e, "tabular")) {
      // Measured, never read from the CSS: tabular-nums on a face without a tnum feature changes
      // nothing. Ten 0s against ten 1s, in this element's font and its numeric settings.
      const probe = document.createElement("span");
      probe.style.cssText = "position: absolute; visibility: hidden; white-space: pre; letter-spacing: 0";
      e.appendChild(probe);
      probe.textContent = "0000000000"; const zero = probe.getBoundingClientRect().width;
      probe.textContent = "1111111111"; const one = probe.getBoundingClientRect().width;
      probe.remove();
      tabular = Math.abs(zero - one) < 0.05 ? true : { zero: Math.round(zero * 100) / 100, one: Math.round(one * 100) / 100 };
    }
    e.setAttribute("data-practices-type", String(i));
    out.push({ i: i++, own: own.slice(0, 40), first, web: web && !allowed(e, "fallback"), number, tabular, label: label(e), where: where(e), y: Math.round(r.top + scrollY) });
  }
  return { texts: out, webFonts: loaded.size };
}

export async function typeAudit(s) {
  const findings = [];
  const r = await s("Runtime.evaluate", { expression: `(${collect.toString()})()`, returnByValue: true });
  if (r.exceptionDetails) throw new Error(`the type check failed in the page: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
  const { texts, webFonts } = r.result.value;
  for (const t of texts) {
    if (t.number && t.tabular && t.tabular !== true) {
      findings.push({ rule: "tabular", text: `"${t.own}" (${t.label}) is a time or a count in proportional figures: ten 0s are ${t.tabular.zero} px, ten 1s ${t.tabular.one} px, so it jumps as it changes.`, where: t.where, y: t.y, key: t.label + "|" + t.own });
    }
  }
  if (webFonts && texts.some((t) => t.web)) {
    await s("DOM.enable");
    await s("CSS.enable");
    const { root } = await s("DOM.getDocument", { depth: 0 });
    const { nodeIds } = await s("DOM.querySelectorAll", { nodeId: root.nodeId, selector: "[data-practices-type]" });
    for (const nodeId of nodeIds) {
      const { attributes } = await s("DOM.getAttributes", { nodeId });
      const t = texts[+attributes[attributes.indexOf("data-practices-type") + 1]];
      if (!t || !t.web) continue;
      const { fonts } = await s("CSS.getPlatformFontsForNode", { nodeId });
      const other = fonts.filter((f) => !f.isCustomFont && f.glyphCount > 0);
      if (other.length) {
        const n = other.reduce((a, f) => a + f.glyphCount, 0);
        findings.push({ rule: "fallback", text: `"${t.own}" (${t.label}) asks for ${t.first}, but ${n} of its glyphs are drawn in ${other.map((f) => f.familyName).join(", ")}: the face lacks them.`, where: t.where, y: t.y, key: t.label + "|" + t.own });
      }
    }
    await s("CSS.disable");
    await s("DOM.disable");
  }
  await s("Runtime.evaluate", { expression: `document.querySelectorAll("[data-practices-type]").forEach((e) => e.removeAttribute("data-practices-type"))` });
  return findings;
}
