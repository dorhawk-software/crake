// check-ui's motion checks (ui/feedback.md; ui/visual-craft.md, "Motion
// with a reason"): ui-audit.mjs calls motionAudit once per board, after its
// first render, which is drawn with prefers-reduced-motion: reduce.
//
// - motion: with reduced motion, nothing loops on. An infinite animation
//   still running is a finding.
// - busy: a control a board marks aria-busy="true" (pressed, its work under
//   way) takes no presses: it, or the control inside it, is disabled or
//   aria-disabled="true", and neither shows a pointer cursor. Its words end
//   in "…" (U+2026), never "...": "Opening Enterprise Architect…".
// - working: with motion allowed (the board loaded again), something
//   inside each busy control moves.
// - squash: words drawn at a size they weren't laid out for: an element
//   holding text scaled out of its shape (scaleY(.7)), now or by a
//   @keyframes rule it plays (ui/visual-craft.md, "A resize never shows a
//   squashed frame").
//
// Why: the Windows-app runner, 2026-11-18. It showed a still egg for the
// minutes it spent getting an app running, and its Open button gave no sign
// for the 10 to 30 s an app took to show, so it could be pressed again and
// again. No rule said either; nothing measured it.

// --- What runs in the page: { findings, busy } for one phase ("reduce" or "moving").
function page(cfg, phase) {
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
    const s = cs(e), r = e.getBoundingClientRect();
    return s.display !== "none" && s.visibility === "visible" && r.width > 0.5 && r.height > 0.5;
  };
  const label = (e) => {
    const c = typeof e.className === "string" && e.className.trim() ? "." + e.className.trim().split(/\s+/).join(".") : "";
    return e.tagName.toLowerCase() + (e.id ? "#" + e.id : "") + c;
  };
  const said = (e) => (e.innerText || e.textContent || "").replace(/\s+/g, " ").trim();
  const where = (e) => { const w = e.parentElement && e.parentElement.closest("[id]"); return w ? `in #${w.id}` : ""; };
  const add = (rule, e, text, kind = "") => out.push({ rule, text, where: where(e), y: Math.round(e.getBoundingClientRect().top + scrollY), key: label(e) + "|" + said(e).slice(0, 40) + "|" + kind });
  const running = document.getAnimations().filter((a) => a.playState === "running" && a.effect && a.effect.target);

  if (phase === "reduce") {
    // motion: a loop with reduced motion on. One finding per element, however many loops it runs.
    const seen = new Set();
    for (const a of running) {
      const t = a.effect.target;
      if (seen.has(t) || a.effect.getComputedTiming().iterations !== Infinity || !visible(t) || allowNode(t, "motion")) continue;
      seen.add(t);
      add("motion", t, `${label(t)} "${said(t).slice(0, 40)}" still loops${a.animationName ? ` (${a.animationName})` : ""} with reduced motion on.`);
    }
    // squash: words drawn at a size they weren't laid out for, as in a resize scaled instead of laid out
    // (the runner's Settings window, 2026-11-18). A scale that keeps the shape (a popover's scale-in)
    // isn't measured; one that changes it squashes the words. Drawn now, or by a @keyframes rule.
    const px = (v) => Math.round(v * 100) / 100, flagged = new Set();
    const holdsText = (e) => visible(e) && said(e).length > 0;
    const squashed = (k) => !!k && Math.abs(k[0] - k[1]) > 0.02 * Math.max(k[0], k[1]);
    const scaleOf = (transform, scale) => {
      if (transform && transform !== "none") { try { const m = new DOMMatrixReadOnly(transform); const k = [Math.hypot(m.a, m.b), Math.hypot(m.c, m.d)]; if (squashed(k)) return k; } catch {} }
      if (scale && scale !== "none") { const [x, y = x] = scale.split(/\s+/).map(parseFloat); return [x, y]; }
      return null;
    };
    for (const e of document.body.querySelectorAll("*")) {
      if (e instanceof SVGElement && e.tagName.toLowerCase() !== "svg") continue;
      const k = scaleOf(cs(e).transform, cs(e).scale);
      if (!squashed(k) || !holdsText(e) || allowNode(e, "squash")) continue;
      flagged.add(e);
      add("squash", e, `${label(e)} "${said(e).slice(0, 40)}" is drawn scaled ${px(k[0])} × ${px(k[1])}: its words are squashed, not laid out at that size.`, "frame");
    }
    const rules = [], walk = (list) => { for (const r of list) { rules.push(r); if (r.cssRules) walk(r.cssRules); } };
    for (const sh of document.styleSheets) { try { walk(sh.cssRules); } catch {} } // a sheet from another origin can't be read
    const squashing = new Map();
    for (const r of rules) if (r instanceof CSSKeyframesRule) for (const f of r.cssRules) { const k = scaleOf(f.style.transform, f.style.scale); if (squashed(k)) squashing.set(r.name, k); }
    for (const r of rules) {
      const name = r instanceof CSSStyleRule && (r.style.animationName || "").split(",").map((n) => n.trim()).find((n) => squashing.has(n));
      if (!name) continue;
      let els = [];
      try { els = document.querySelectorAll(r.selectorText); } catch {}
      for (const e of els) {
        if (flagged.has(e) || !holdsText(e) || allowNode(e, "squash")) continue;
        flagged.add(e);
        const k = squashing.get(name);
        add("squash", e, `${label(e)} "${said(e).slice(0, 40)}" plays @keyframes ${name}, which scales it ${px(k[0])} × ${px(k[1])}: its words show squashed until the next layout.`, "keyframes");
      }
    }
  }
  const busy = [...document.querySelectorAll("[aria-busy=true]")].filter(visible);
  for (const e of busy) {
    const name = said(e);
    if (phase === "moving") {
      // working: the work shows it's happening, inside the control that was pressed.
      if (!running.some((a) => e.contains(a.effect.target)) && !allowNode(e, "working"))
        add("working", e, `${label(e)} "${name.slice(0, 40)}" is busy, and nothing in it moves with motion allowed.`);
      continue;
    }
    if (allowNode(e, "busy")) continue;
    // busy: it takes no more presses. The control is the busy element, or the first one inside it; a busy
    // region with nothing to press (a list loading) is only asked for its words.
    const p = e.matches(cfg.targets) ? e : [...e.querySelectorAll(cfg.targets)].find(visible);
    if (p) {
      const off = p.matches(":disabled") || [e, p].some((x) => x.getAttribute("aria-disabled") === "true");
      if (!off) add("busy", e, `${label(e)} "${name.slice(0, 40)}" is busy and can still be pressed: it isn't disabled or aria-disabled="true".`, "off");
      const hand = [e, p].find((x) => cs(x).cursor === "pointer");
      if (hand) add("busy", e, `${label(e)} "${name.slice(0, 40)}" is busy and ${hand === e ? "it" : label(hand) + " inside it"} still shows a pointer cursor.`, "cursor");
    }
    if (/\.\.\.$/.test(name)) add("busy", e, `${label(e)} "${name.slice(0, 40)}" ends in three dots: use … (U+2026).`, "dots");
    else if (!name.endsWith("…")) add("busy", e, `${label(e)} "${name.slice(0, 40)}" is busy and doesn't say so: name what is happening, ending in … ("Opening Enterprise Architect…").`, "words");
  }
  return { findings: out, busy: busy.length };
}

// The board is loaded with reduced motion on. reload() loads it again with motion allowed: only when a
// control on it is busy, so a board without one costs no second load.
export async function motionAudit(s, cfg, reload) {
  const ev = async (phase) => {
    const r = await s("Runtime.evaluate", { expression: `(${page.toString()})(${JSON.stringify(cfg)}, ${JSON.stringify(phase)})`, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(`the motion check failed in the page: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
    return r.result.value;
  };
  const still = await ev("reduce");
  if (!still.busy) return still.findings;
  await reload();
  return [...still.findings, ...(await ev("moving")).findings];
}
