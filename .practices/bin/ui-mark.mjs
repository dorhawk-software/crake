// check-ui's mark check (ui/identity-pass.md, "A mark holds at its smallest
// size"): ui-audit.mjs calls markAudit once per board, with its Chrome
// session, its settings (--mark-stroke, --mark-brand, --mark-small) and
// its PNG decoder. No packages.

// --- The mark: a product's wordmark or logo (<svg data-mark>, its brand element
// [data-mark-brand]), measured at the smallest size the board shows it. Drawn
// alone and large on a flat ground, twice (with and without the brand), and
// read from the pixels: what a cut, a font or a sample leaves at that size.
function edt(ink, w, h) {
  // Exact Euclidean distance from each ink pixel to the nearest pixel that isn't (Felzenszwalb).
  const INF = 1e12, d = new Float64Array(w * h), n = Math.max(w, h);
  const f = new Float64Array(n), o = new Float64Array(n), v = new Int32Array(n), z = new Float64Array(n + 1);
  const pass = (len, get, set) => {
    for (let q = 0; q < len; q++) f[q] = get(q);
    let k = 0; v[0] = 0; z[0] = -Infinity; z[1] = Infinity;
    for (let q = 1; q < len; q++) {
      let x;
      while ((x = ((f[q] + q * q) - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k])) <= z[k]) k--;
      k++; v[k] = q; z[k] = x; z[k + 1] = Infinity;
    }
    k = 0;
    for (let q = 0; q < len; q++) { while (z[k + 1] < q) k++; o[q] = (q - v[k]) ** 2 + f[v[k]]; }
    for (let q = 0; q < len; q++) set(q, o[q]);
  };
  for (let i = 0; i < w * h; i++) d[i] = ink[i] ? INF : 0;
  for (let x = 0; x < w; x++) pass(h, (q) => d[q * w + x], (q, val) => { d[q * w + x] = val; });
  for (let y = 0; y < h; y++) pass(w, (q) => d[y * w + q], (q, val) => { d[y * w + q] = val; });
  for (let i = 0; i < w * h; i++) d[i] = Math.sqrt(d[i]);
  return d;
}
function components(mask, w, h) {
  const seen = new Uint8Array(w * h), out = [];
  for (let i = 0; i < w * h; i++) {
    if (!mask[i] || seen[i]) continue;
    const stack = [i]; seen[i] = 1;
    let area = 0, x0 = w, x1 = 0, y0 = h, y1 = 0;
    while (stack.length) {
      const j = stack.pop(), x = j % w, y = (j - x) / w;
      area++; x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y);
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        const xx = x + dx, yy = y + dy, k = yy * w + xx;
        if (xx >= 0 && yy >= 0 && xx < w && yy < h && mask[k] && !seen[k]) { seen[k] = 1; stack.push(k); }
      }
    }
    out.push({ area, x0, x1, y0, y1 });
  }
  return out;
}
export async function markAudit(s, width, opts, decodePng) {
  const ev = async (expression) => {
    const r = await s("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(`the mark check failed in the page: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
    return r.result.value;
  };
  const groups = await ev(`(() => {
    const allowed = (e) => { for (let a = e; a && a.nodeType === 1; a = a.parentElement) { const v = a.getAttribute("data-allow"); if (v && v.split(";").some((p) => p.split(":")[0].trim() === "mark")) return true; } return false; };
    const by = {};
    [...document.querySelectorAll("svg[data-mark]")].forEach((e, i) => {
      const r = e.getBoundingClientRect();
      if (r.width < 1 || r.height < 1 || getComputedStyle(e).visibility !== "visible" || e.closest("template, symbol, defs, [hidden]") || allowed(e)) return;
      const k = e.getAttribute("data-mark") || e.querySelector("use")?.getAttribute("href") || "the mark";
      e.setAttribute("data-check-ui-mark", i);
      (by[k] ||= []).push({ i, w: r.width, h: r.height, y: Math.round(r.top + scrollY) });
    });
    return by;
  })()`);
  const out = [], BG = [255, 0, 255]; // magenta: no product's ink, so every other pixel is the mark
  const shot = async (i, H, mode) => {
    const box = await ev(`(() => {
      document.getElementById("__cu_mk")?.remove();
      const st = document.getElementById("__cu_mkst") || document.head.appendChild(Object.assign(document.createElement("style"), { id: "__cu_mkst" }));
      st.textContent = ${JSON.stringify(mode)} === "letters" ? "[data-mark-brand] { visibility: hidden !important }" : "";
      const src = document.querySelector('[data-check-ui-mark="${i}"]'), r = src.getBoundingClientRect(), cs = getComputedStyle(src);
      const wrap = document.createElement("div");
      wrap.id = "__cu_mk";
      wrap.style.cssText = "position:fixed;left:0;top:0;z-index:2147483647;background:#FF00FF;padding:12px;margin:0;line-height:0;box-sizing:content-box";
      for (const p of cs) if (p.startsWith("--")) wrap.style.setProperty(p, cs.getPropertyValue(p));
      wrap.style.color = cs.color;
      const c = src.cloneNode(true), W = r.width * ${H} / r.height;
      c.removeAttribute("data-check-ui-mark");
      c.setAttribute("width", W); c.setAttribute("height", ${H});
      c.style.cssText = "display:block;margin:0;max-width:none;max-height:none;width:" + W + "px;height:${H}px";
      wrap.append(c); document.body.append(wrap);
      const b = wrap.getBoundingClientRect();
      return [b.width, b.height];
    })()`);
    await ev("new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))");
    const { data } = await s("Page.captureScreenshot", { format: "png", clip: { x: 4, y: 4, width: Math.floor(box[0]) - 8, height: Math.floor(box[1]) - 8, scale: 2 } }); // inside the ground's edge
    return decodePng(Buffer.from(data, "base64"));
  };
  const minStroke = +opts["mark-stroke"], minBrand = +opts["mark-brand"], small = +opts["mark-small"];
  const r1 = (v) => Math.round(v * 10) / 10;
  for (const [k, list] of Object.entries(groups)) {
    const least = list.reduce((a, b) => (b.h < a.h ? b : a));
    const add = (text, key) => out.push({ rule: "mark", text, where: `(data-mark "${k}")`, y: least.y, key: k + "|" + key });
    if (least.h > small) add(`"${k}" is shown no smaller than ${r1(least.h)} px. Show it where it ships smallest (${small} px or less: a title bar, a menu, an About window), so it is measured there.`, "small");
    // As large as the window allows, drawn at twice the pixels.
    const H = Math.max(40, Math.min(300, Math.floor((width - 40) * least.h / least.w)));
    const full = await shot(least.i, H, "full"), letters = await shot(least.i, H, "letters");
    const { w, h, bpp } = full, scale = least.h / (H * 2); // a pixel here, in px at the smallest size
    const ink = new Uint8Array(w * h), brand = new Uint8Array(w * h);
    let brandAny = false;
    for (let j = 0; j < w * h; j++) {
      const o = j * bpp;
      const off = (img) => Math.max(...[0, 1, 2].map((c) => Math.abs(img.px[o + c] - BG[c])));
      const diff = Math.max(...[0, 1, 2].map((c) => Math.abs(full.px[o + c] - letters.px[o + c])));
      ink[j] = off(letters) > 96 ? 1 : 0;
      if (diff > 96) { brand[j] = 1; brandAny = true; }
    }
    // The strokes: the distance to the edge, read on the ridge (where it is largest across).
    const d = edt(ink, w, h), thin = new Uint8Array(w * h), widths = [];
    for (let y = 1; y < h - 1; y++) for (let x = 1; x < w - 1; x++) {
      const j = y * w + x, v = d[j];
      if (!ink[j] || v < 2.5) continue;
      let ridge = true;
      for (let dy = -1; dy <= 1 && ridge; dy++) for (let dx = -1; dx <= 1; dx++) if (d[j + dy * w + dx] > v + 1e-9) { ridge = false; break; }
      if (!ridge) continue;
      widths.push(2 * v - 1);
      if ((2 * v - 1) * scale < minStroke) thin[j] = 1;
    }
    widths.sort((a, b) => a - b);
    const stroke = widths.length ? widths[Math.floor(widths.length / 2)] : 0;
    const thinParts = components(thin, w, h).filter((c) => c.area >= 3);
    if (thinParts.length) {
      let least2 = Infinity, at = 0;
      for (const c of thinParts) for (let y = c.y0; y <= c.y1; y++) for (let x = c.x0; x <= c.x1; x++) {
        const j = y * w + x;
        if (thin[j] && 2 * d[j] - 1 < least2) { least2 = 2 * d[j] - 1; at = x; }
      }
      add(`"${k}" thins to ${r1(least2 * scale)} px at its smallest size here (${r1(least.h)} px tall), ${Math.round(100 * at / w)}% across: under ${minStroke} px a stroke renders as grey fuzz. Cut through a stroke where it runs full width, not along a curve; or keep the mark larger.`, "thin");
    }
    // A cut's leftover: a small piece standing apart that isn't a dot.
    const parts = components(ink, w, h).filter((c) => c.area > 4);
    if (parts.length > 1 && stroke) for (const c of parts) {
      const bw = c.x1 - c.x0 + 1, bh = c.y1 - c.y0 + 1;
      const dotLike = c.area / (bw * bh) > 0.6 && Math.max(bw, bh) / Math.min(bw, bh) < 1.6;
      if (c.area < 1.5 * stroke * stroke && !dotLike)
        add(`a piece of "${k}" stands apart, ${Math.round(100 * (c.x0 + c.x1) / 2 / w)}% across, ${r1(bw * scale)} × ${r1(bh * scale)} px at its smallest: a cut's leftover reads as a mistake. Move the cut, or the stroke, so it takes the piece whole.`, "fragment");
    }
    if (brandAny) {
      let y0 = h, y1 = -1;
      for (let j = 0; j < w * h; j++) if (brand[j]) { const y = Math.floor(j / w); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
      const bh = (y1 - y0 + 1) * scale;
      if (bh < minBrand) add(`the brand element of "${k}" ([data-mark-brand]) is ${r1(bh)} px tall at the mark's smallest size here (${r1(least.h)} px): under ${minBrand} px nobody can tell what it is. Make it larger in the mark, or the mark larger.`, "brand");
      // Touching is a choice; nearly touching reads as a mistake.
      const notInk = new Uint8Array(w * h);
      for (let j = 0; j < w * h; j++) notInk[j] = ink[j] ? 0 : 1;
      const toInk = edt(notInk, w, h);
      let gap = Infinity;
      for (let j = 0; j < w * h; j++) if (brand[j] && !ink[j]) gap = Math.min(gap, toInk[j]);
      if (stroke && gap > 1.5 && gap < 0.25 * stroke)
        add(`the brand element of "${k}" comes within ${r1(gap / stroke)} of a stroke's width of the letters: nearly touching reads as a mistake. Let it touch, or keep a quarter of a stroke clear.`, "touch");
    }
  }
  await ev(`(() => { document.getElementById("__cu_mk")?.remove(); document.getElementById("__cu_mkst")?.remove(); document.querySelectorAll("[data-check-ui-mark]").forEach((e) => e.removeAttribute("data-check-ui-mark")); })()`);
  return out;
}
