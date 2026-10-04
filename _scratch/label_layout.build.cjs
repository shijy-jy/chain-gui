"use strict";
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);
var label_layout_exports = {};
__export(label_layout_exports, {
  DEFAULT_LABEL_METRICS: () => DEFAULT_LABEL_METRICS,
  LABEL_FONT_SIZES: () => LABEL_FONT_SIZES,
  estimateLabelBox: () => estimateLabelBox,
  selectVisibleLabels: () => selectVisibleLabels,
  truncateLabelText: () => truncateLabelText
});
module.exports = __toCommonJS(label_layout_exports);
const DEFAULT_LABEL_METRICS = {
  // 11px 可读性与占地之间是实测选出的平衡点：10px 在 165Hz 屏上发虚、11px 配 72px 换行宽度
  // 仍是"小徽标"而非原来 150px 的"一块板"。字号可通过 LABEL_FONT_SIZES 让用户调。
  fontSize: 11,
  lineHeight: 1.25,
  // 原来 150px 一行能塞 20 个汉字（视觉上就是一块板）；72px ≈ 7 个汉字/行
  maxWidth: 72,
  maxLines: 2,
  marginY: 5,
  outline: 2,
  pad: 1
};
const LABEL_FONT_SIZES = [9, 10, 11, 12, 13];
const OVERLAP_PAD = 6;
function charWidth(ch, fontSizePx) {
  const c = ch.codePointAt(0) ?? 0;
  const wide = c >= 4352 && c <= 4447 || c >= 11904 && c <= 42191 || c >= 44032 && c <= 55203 || c >= 63744 && c <= 64255 || c >= 65072 && c <= 65135 || c >= 65280 && c <= 65376 || c >= 65504 && c <= 65510;
  return wide ? fontSizePx : fontSizePx * 0.55;
}
function textWidth(text, fontSizePx) {
  let w = 0;
  for (const ch of text) w += charWidth(ch, fontSizePx);
  return w;
}
function countLines(text, maxW, fontSizePx) {
  if (maxW <= 0) return 1;
  let lines = 1;
  let cur = 0;
  for (const ch of text) {
    const cw = charWidth(ch, fontSizePx);
    if (cur + cw > maxW) {
      lines++;
      cur = cw;
    } else cur += cw;
  }
  return lines;
}
function estimateLabelBox(text, m, fontSizePx, measure) {
  const maxW = m.maxWidth * (fontSizePx / m.fontSize);
  let textW;
  let lines;
  if (measure) {
    textW = measure(text);
    lines = Math.min(m.maxLines, Math.max(1, Math.ceil(textW / Math.max(maxW, 1))));
  } else {
    textW = textWidth(text, fontSizePx);
    lines = Math.min(m.maxLines, countLines(text, maxW, fontSizePx));
  }
  const w = Math.min(maxW, textW);
  return {
    w: w + m.pad * 2 + m.outline * 2,
    h: lines * fontSizePx * m.lineHeight + m.pad * 2 + m.outline * 2,
    lines
  };
}
function truncateLabelText(text, capacityPx, fontSizePx, measure) {
  const width = measure ? measure(text) : textWidth(text, fontSizePx);
  if (width <= capacityPx) return text;
  const chars = [...text];
  const ell = "\u2026";
  const ellW = measure ? measure(ell) : charWidth(ell, fontSizePx);
  let w = ellW;
  let keep = 0;
  for (const ch of chars) {
    const cw = measure ? measure(ch) : charWidth(ch, fontSizePx);
    if (w + cw > capacityPx) break;
    w += cw;
    keep++;
  }
  return chars.slice(0, Math.max(keep, 1)).join("") + ell;
}
class ScreenGrid {
  constructor(cell) {
    this.cell = cell;
  }
  cells = /* @__PURE__ */ new Map();
  key(cx, cy) {
    return cx * 100003 + cy;
  }
  insert(b) {
    const x0 = Math.floor(b.x / this.cell), x1 = Math.floor((b.x + b.w) / this.cell);
    const y0 = Math.floor(b.y / this.cell), y1 = Math.floor((b.y + b.h) / this.cell);
    for (let gx = x0; gx <= x1; gx++) {
      for (let gy = y0; gy <= y1; gy++) {
        const k = this.key(gx, gy);
        const arr = this.cells.get(k);
        if (arr) arr.push(b);
        else this.cells.set(k, [b]);
      }
    }
  }
  query(b, padX = 0, padY = 0) {
    const x0 = Math.floor((b.x - padX) / this.cell), x1 = Math.floor((b.x + b.w + padX) / this.cell);
    const y0 = Math.floor((b.y - padY) / this.cell), y1 = Math.floor((b.y + b.h + padY) / this.cell);
    const out = [];
    const seen = /* @__PURE__ */ new Set();
    for (let gx = x0; gx <= x1; gx++) {
      for (let gy = y0; gy <= y1; gy++) {
        for (const it of this.cells.get(this.key(gx, gy)) ?? []) {
          if (!seen.has(it)) {
            seen.add(it);
            out.push(it);
          }
        }
      }
    }
    return out;
  }
}
const boxesOverlap = (a, b, pad) => a.x - pad < b.x + b.w && a.x + a.w + pad > b.x && a.y - pad < b.y + b.h && a.y + a.h + pad > b.y;
const circleHitsBox = (cx, cy, r, b, clearance) => {
  const nx = Math.max(b.x, Math.min(cx, b.x + b.w));
  const ny = Math.max(b.y, Math.min(cy, b.y + b.h));
  const dx = cx - nx, dy = cy - ny;
  const rr = r + clearance;
  return dx * dx + dy * dy < rr * rr;
};
function selectVisibleLabels(candidates, o) {
  const m = o.metrics ?? DEFAULT_LABEL_METRICS;
  const zoom = o.zoom;
  const margin = o.margin ?? 60;
  const clearance = o.nodeClearance ?? 2;
  const fs = o.renderedFontPxOverride ?? m.fontSize;
  if (o.minRenderedFontPx && o.minRenderedFontPx > 0 && fs < o.minRenderedFontPx) {
    return {
      visible: /* @__PURE__ */ new Set(),
      stats: {
        candidates: candidates.length,
        inViewport: 0,
        accepted: 0,
        rejectedNodeCover: 0,
        rejectedLabelOverlap: 0,
        rejectedBudget: 0,
        rejectedOffscreen: candidates.length,
        rejectedTooSmall: candidates.length,
        coverage: 0
      }
    };
  }
  const items = [];
  const nodeCircles = [];
  let offscreen = 0;
  for (const c of candidates) {
    const sx = c.x * zoom;
    const sy = c.y * zoom;
    const r = c.size * zoom / 2;
    nodeCircles.push({ x: sx - r, y: sy - r, w: r * 2, h: r * 2, cx: sx, cy: sy, r });
    if (!(sx >= -margin && sy >= -margin && sx <= o.viewportW + margin && sy <= o.viewportH + margin)) {
      offscreen++;
      continue;
    }
    const est = estimateLabelBox(c.text, m, fs, o.measureText);
    const bw = Math.min(est.w, m.maxWidth + (m.pad + m.outline) * 2) * zoom;
    const bh = est.h * zoom;
    const box = { x: sx - bw / 2, y: sy + r + m.marginY * zoom, w: bw, h: bh };
    items.push({ c, sx, sy, r, box });
  }
  items.sort((a, b) => {
    const fa = a.c.focused ? 1 : 0;
    const fb = b.c.focused ? 1 : 0;
    if (fa !== fb) return fb - fa;
    if (b.c.degree !== a.c.degree) return b.c.degree - a.c.degree;
    if (a.c.depth !== b.c.depth) return a.c.depth - b.c.depth;
    return a.c.id < b.c.id ? -1 : a.c.id > b.c.id ? 1 : 0;
  });
  const cell = Math.max(m.maxWidth + 24, m.maxLines * m.fontSize * m.lineHeight + 24, 64);
  const labelGrid = new ScreenGrid(cell);
  const nodeGrid = new ScreenGrid(cell);
  for (const n of nodeCircles) nodeGrid.insert(n);
  const areaBudget = o.viewportW * o.viewportH * (o.maxScreenAreaRatio ?? 0.14);
  const limit = o.maxLabels > 0 ? o.maxLabels : Number.POSITIVE_INFINITY;
  const visible = /* @__PURE__ */ new Set();
  let rejectNode = 0, rejectLabel = 0, rejectBudget = 0, area = 0;
  for (const it of items) {
    if (visible.size >= limit) {
      rejectBudget++;
      continue;
    }
    if (area + it.box.w * it.box.h > areaBudget) {
      rejectBudget++;
      continue;
    }
    let covers = false;
    for (const n of nodeGrid.query(it.box)) {
      if (n.cx === it.sx && n.cy === it.sy) continue;
      if (circleHitsBox(n.cx, n.cy, n.r, it.box, clearance)) {
        covers = true;
        break;
      }
    }
    if (covers) {
      rejectNode++;
      continue;
    }
    if (labelGrid.query(it.box, OVERLAP_PAD, OVERLAP_PAD).some((b) => boxesOverlap(it.box, b, OVERLAP_PAD))) {
      rejectLabel++;
      continue;
    }
    visible.add(it.c.id);
    area += it.box.w * it.box.h;
    labelGrid.insert(it.box);
  }
  return {
    visible,
    stats: {
      candidates: candidates.length,
      inViewport: items.length,
      accepted: visible.size,
      rejectedNodeCover: rejectNode,
      rejectedLabelOverlap: rejectLabel,
      rejectedBudget: rejectBudget,
      rejectedOffscreen: offscreen,
      rejectedTooSmall: 0,
      coverage: area / Math.max(o.viewportW * o.viewportH, 1)
    }
  };
}
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  DEFAULT_LABEL_METRICS,
  LABEL_FONT_SIZES,
  estimateLabelBox,
  selectVisibleLabels,
  truncateLabelText
});
