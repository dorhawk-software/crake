// check-ui's pixels: decoding Chrome's PNG screenshots (no packages), and
// contrast measured against the pixels behind text on gradients and
// images. ui-audit.mjs and ui-mark.mjs use it.
import { inflateSync } from "node:zlib";

// --- Contrast on gradients and images: hide every glyph, screenshot the page,
// and measure each line of text against the pixels that were behind it.
export function decodePng(buf) {
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
export async function pixelContrast(s, result, width) {
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
