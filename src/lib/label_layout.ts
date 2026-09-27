// 标签布局纯函数：在屏幕空间预算内挑选"该显示哪些标签"，保证**不互相压字、不盖住节点**
//
// 背景（用户反馈「节点一多，文字把节点全部挡住」）：
//   原样式 11px 字号 + text-max-width:150px + 换行 + text-margin-y:8 + 2px 描边
//   → 一个 26px 的圆点配一块最宽 150px、最高可达 80px 的标签板；节点一多就互相盖住、也盖住节点本身。
//   光把字号调小治不了本：密度高的区域必须**少显示**标签，而不是"显示得更小"。
//
// 本模块的三个约束（按优先级）：
//   1. 不盖住任何节点（含自己的锚点邻居）——这是"观感"的第一杀手
//   2. 标签之间不重叠（留 3px 呼吸）
//   3. 总覆盖面积不超过视口的一定比例（默认 14%），避免整屏都是字
// 在满足约束的前提下，按重要性（度 → 深度 → id）贪心放入，保证枢纽节点优先被标出。
//
// 纯函数、可离线回归（tools/label_verify.cjs）：同输入同输出，不依赖 cytoscape。

export interface LabelCandidate {
  id: string;
  x: number;          // 世界坐标（节点中心）
  y: number;
  size: number;       // 节点直径（世界单位）
  text: string;       // 显示文本（已含类型前缀）
  degree: number;     // 优先级主键
  depth: number;      // 优先级次键（浅层更重要）
}

export interface LabelMetrics {
  /** 字号（屏幕 px；cytoscape 的 font-size 即屏幕 px，不随 zoom 缩放） */
  fontSize: number;
  /** 行高倍数 */
  lineHeight: number;
  /** 文本最大宽度（屏幕 px，换行阈值） */
  maxWidth: number;
  /** 最多行数（超出由 fitLabelText 截断加省略号） */
  maxLines: number;
  /** 文本与节点的间距（屏幕 px） */
  marginY: number;
  /** 描边宽度（屏幕 px） */
  outline: number;
  /** 标签内边距（屏幕 px） */
  pad: number;
}

export const DEFAULT_LABEL_METRICS: LabelMetrics = {
  // 11px 可读性与占地之间是实测选出的平衡点：10px 在 165Hz 屏上发虚、11px 配 72px 换行宽度
  // 仍是"小徽标"而非原来 150px 的"一块板"。字号可通过 LABEL_FONT_SIZES 让用户调。
  fontSize: 11,
  lineHeight: 1.25,
  // 原来 150px 一行能塞 20 个汉字（视觉上就是一块板）；72px ≈ 7 个汉字/行
  maxWidth: 72,
  maxLines: 2,
  marginY: 5,
  outline: 2,
  pad: 1,
};

/** 可选字号档（UI 滑条用；越小越省空间，越大越易读） */
export const LABEL_FONT_SIZES = [9, 10, 11, 12, 13] as const;

export interface LabelBudgetOptions {
  metrics?: LabelMetrics;
  /** 屏幕上最多同时显示多少个标签（0 = 不限，按剩余空间尽力放） */
  maxLabels: number;
  /** 标签总覆盖面积 / 视口面积 的上限（默认 0.14） */
  maxScreenAreaRatio?: number;
  /** 世界坐标 → 屏幕坐标的缩放（= cytoscape zoom） */
  zoom: number;
  /** 视口尺寸（屏幕 px） */
  viewportW: number;
  viewportH: number;
  /** 视口外扩边距（屏幕 px）：略超出视口也算可见，避免平移时标签闪烁 */
  margin?: number;
  /** 标签与**非锚点**节点圆之间的最小间隙（屏幕 px）；0 = 不检查节点遮挡 */
  nodeClearance?: number;
  /**
   * 渲染字号下限（屏幕 px，对应 cytoscape 的 min-zoomed-font-size）：
   * 低于它 cytoscape 根本不画标签，此时不该占用标签预算（否则统计与画面不一致）。
   * 0 = 不检查。
   */
  minRenderedFontPx?: number;
  /**
   * 标签**实际**的屏幕字号（px）。调用方按 1/zoom 反算世界字号以实现"屏幕恒字号"，
   * 反算结果可能被上限截断，因此实际值未必等于 metrics.fontSize —— 装箱判定必须用实际值，
   * 否则标签框尺寸估错、拒绝/接受判断与实际画面不符。
   */
  renderedFontPxOverride?: number;
}

export interface LabelBudgetResult {
  /** 应当显示标签的节点 id */
  visible: Set<string>;
  stats: {
    candidates: number;
    inViewport: number;
    accepted: number;
    rejectedNodeCover: number;
    rejectedLabelOverlap: number;
    rejectedBudget: number;
    rejectedOffscreen: number;
    /** 因"渲染字号过小、cytoscape 不画"而整体不显示的候选数 */
    rejectedTooSmall: number;
    coverage: number;
  };
}

type Box = { x: number; y: number; w: number; h: number };

const OVERLAP_PAD = 3;   // 相邻标签之间留一点呼吸，避免"字贴字"

/** 单个字符宽度（屏幕 px）：CJK/全角 ≈ 1em，其余 ≈ 0.55em */
function charWidth(ch: string, fontSizePx: number): number {
  const c = ch.codePointAt(0) ?? 0;
  const wide =
    (c >= 0x1100 && c <= 0x115f) ||
    (c >= 0x2e80 && c <= 0xa4cf) ||
    (c >= 0xac00 && c <= 0xd7a3) ||
    (c >= 0xf900 && c <= 0xfaff) ||
    (c >= 0xfe30 && c <= 0xfe6f) ||
    (c >= 0xff00 && c <= 0xff60) ||
    (c >= 0xffe0 && c <= 0xffe6);
  return wide ? fontSizePx : fontSizePx * 0.55;
}

function textWidth(text: string, fontSizePx: number): number {
  let w = 0;
  for (const ch of text) w += charWidth(ch, fontSizePx);
  return w;
}

function countLines(text: string, maxW: number, fontSizePx: number): number {
  if (maxW <= 0) return 1;
  let lines = 1;
  let cur = 0;
  for (const ch of text) {
    const cw = charWidth(ch, fontSizePx);
    if (cur + cw > maxW) { lines++; cur = cw; } else cur += cw;
  }
  return lines;
}

/**
 * 估算标签的屏幕包围盒（宽 × 高）。
 * 不调 canvas measureText 是为了保持纯函数（可在 Node 里回归）；对装箱判定精度足够。
 */
export function estimateLabelBox(
  text: string,
  m: LabelMetrics,
  fontSizePx: number,
): { w: number; h: number; lines: number } {
  const maxW = m.maxWidth * (fontSizePx / m.fontSize);
  const lines = Math.min(countLines(text, maxW, fontSizePx), m.maxLines);
  const w = Math.min(maxW, textWidth(text, fontSizePx));
  return {
    w: w + m.pad * 2 + m.outline * 2,
    h: lines * fontSizePx * m.lineHeight + m.pad * 2 + m.outline * 2,
    lines,
  };
}

/** 截断文本使其在 maxLines 行内放下（超出加省略号）：标签高度有上限，避免长标题撑成 3–4 行盖住邻居 */
export function fitLabelText(text: string, m: LabelMetrics, fontSizePx: number): string {
  const maxW = m.maxWidth * (fontSizePx / m.fontSize);
  const capacity = maxW * m.maxLines;
  if (textWidth(text, fontSizePx) <= capacity) return text;
  const chars = [...text];
  let w = charWidth('…', fontSizePx);
  let keep = 0;
  for (const ch of chars) {
    const cw = charWidth(ch, fontSizePx);
    if (w + cw > capacity) break;
    w += cw;
    keep++;
  }
  return chars.slice(0, keep).join('') + '…';
}

/** 屏幕空间均匀网格（标签框与节点圆各一个） */
class ScreenGrid<T extends { x: number; y: number; w: number; h: number }> {
  private cells = new Map<number, T[]>();
  constructor(private cell: number) {}
  private key(cx: number, cy: number): number { return cx * 100003 + cy; }
  insert(b: T): void {
    const x0 = Math.floor(b.x / this.cell), x1 = Math.floor((b.x + b.w) / this.cell);
    const y0 = Math.floor(b.y / this.cell), y1 = Math.floor((b.y + b.h) / this.cell);
    for (let gx = x0; gx <= x1; gx++) {
      for (let gy = y0; gy <= y1; gy++) {
        const k = this.key(gx, gy);
        const arr = this.cells.get(k);
        if (arr) arr.push(b); else this.cells.set(k, [b]);
      }
    }
  }
  query(b: Box, padX = 0, padY = 0): T[] {
    const x0 = Math.floor((b.x - padX) / this.cell), x1 = Math.floor((b.x + b.w + padX) / this.cell);
    const y0 = Math.floor((b.y - padY) / this.cell), y1 = Math.floor((b.y + b.h + padY) / this.cell);
    const out: T[] = [];
    const seen = new Set<T>();
    for (let gx = x0; gx <= x1; gx++) {
      for (let gy = y0; gy <= y1; gy++) {
        for (const it of this.cells.get(this.key(gx, gy)) ?? []) {
          if (!seen.has(it)) { seen.add(it); out.push(it); }
        }
      }
    }
    return out;
  }
}

const boxesOverlap = (a: Box, b: Box, pad: number): boolean =>
  a.x - pad < b.x + b.w && a.x + a.w + pad > b.x && a.y - pad < b.y + b.h && a.y + a.h + pad > b.y;

/** 圆（屏幕）与矩形（屏幕）是否相交 */
const circleHitsBox = (cx: number, cy: number, r: number, b: Box, clearance: number): boolean => {
  const nx = Math.max(b.x, Math.min(cx, b.x + b.w));
  const ny = Math.max(b.y, Math.min(cy, b.y + b.h));
  const dx = cx - nx, dy = cy - ny;
  const rr = r + clearance;
  return dx * dx + dy * dy < rr * rr;
};

/**
 * 挑选屏幕上应显示的标签集合。
 *
 * 为什么不用"zoom 阈值一刀切"：同一 zoom 下不同区域密度差异极大
 * （径向布局内环稀、外环密），阈值只能照顾一头。装箱判定直接以"屏幕上真的不撞"为准。
 *
 * 复杂度：O(n log n) 排序 + 网格邻近查询；1500 节点量级为个位数毫秒。
 */
export function selectVisibleLabels(
  candidates: LabelCandidate[],
  o: LabelBudgetOptions,
): LabelBudgetResult {
  const m = o.metrics ?? DEFAULT_LABEL_METRICS;
  const zoom = o.zoom;
  const margin = o.margin ?? 60;
  const clearance = o.nodeClearance ?? 2;
  // 实际渲染字号（屏幕 px）：调用方按 1/zoom 反算世界字号以实现"屏幕恒字号"，
  // 反算可能被上限压低，所以装箱必须用实际值，否则标签框估错、判定与画面不符
  const fs = o.renderedFontPxOverride ?? m.fontSize;

  // 0) 实际渲染字号低于可读下限时标签不画：整体返回空集，
  //    避免"统计说有标签、画面却空白"的口径不一致
  if (o.minRenderedFontPx && o.minRenderedFontPx > 0 && fs < o.minRenderedFontPx) {
    return {
      visible: new Set<string>(),
      stats: {
        candidates: candidates.length,
        inViewport: 0,
        accepted: 0,
        rejectedNodeCover: 0,
        rejectedLabelOverlap: 0,
        rejectedBudget: 0,
        rejectedOffscreen: candidates.length,
        rejectedTooSmall: candidates.length,
        coverage: 0,
      },
    };
  }

  // 1) 世界 → 屏幕 + 视口预筛
  interface Item { c: LabelCandidate; sx: number; sy: number; r: number; box: Box; }
  const items: Item[] = [];
  const nodeCircles: { x: number; y: number; w: number; h: number; cx: number; cy: number; r: number }[] = [];
  let offscreen = 0;
  for (const c of candidates) {
    const sx = c.x * zoom;
    const sy = c.y * zoom;
    const r = (c.size * zoom) / 2;
    // 节点圆本身也在屏幕网格里，用于"标签不许盖住节点"判定
    nodeCircles.push({ x: sx - r, y: sy - r, w: r * 2, h: r * 2, cx: sx, cy: sy, r });
    if (!(sx >= -margin && sy >= -margin && sx <= o.viewportW + margin && sy <= o.viewportH + margin)) {
      offscreen++;
      continue;
    }
    const est = estimateLabelBox(c.text, m, fs);
    // 标签锚点：节点下方居中（与 cytoscape text-valign:bottom 一致）
    const box: Box = { x: sx - est.w / 2, y: sy + r + m.marginY, w: est.w, h: est.h };
    items.push({ c, sx, sy, r, box });
  }
  // 诊断口径：offset 计数即"被视口筛掉的"，便于与调用方对账

  // 2) 优先级：度高的先放（枢纽节点最需要被标出）→ 浅层优先 → id 稳定排序
  items.sort((a, b) => {
    if (b.c.degree !== a.c.degree) return b.c.degree - a.c.degree;
    if (a.c.depth !== b.c.depth) return a.c.depth - b.c.depth;
    return a.c.id < b.c.id ? -1 : a.c.id > b.c.id ? 1 : 0;
  });

  // 3) 网格：cell 取"最大标签框"量级，保证邻近查询不漏
  const maxBoxW = m.maxWidth + (m.pad + m.outline) * 2;
  const maxBoxH = m.maxLines * fs * m.lineHeight + (m.pad + m.outline) * 2;
  const cell = Math.max(maxBoxW, maxBoxH, 64);
  const labelGrid = new ScreenGrid<Box & { x: number; y: number; w: number; h: number }>(cell);
  const nodeGrid = new ScreenGrid<{ x: number; y: number; w: number; h: number; cx: number; cy: number; r: number }>(cell);
  for (const n of nodeCircles) nodeGrid.insert(n);

  const areaBudget = o.viewportW * o.viewportH * (o.maxScreenAreaRatio ?? 0.14);
  const limit = o.maxLabels > 0 ? o.maxLabels : Number.POSITIVE_INFINITY;

  const visible = new Set<string>();
  let rejectNode = 0, rejectLabel = 0, rejectBudget = 0, area = 0;

  for (const it of items) {
    if (visible.size >= limit) { rejectBudget++; continue; }
    if (area + it.box.w * it.box.h > areaBudget) { rejectBudget++; continue; }

    // 3a) 不许盖住任何**非自身**节点圆（自身节点在标签上方，天然不冲突）
    let covers = false;
    for (const n of nodeGrid.query(it.box)) {
      if (n.cx === it.sx && n.cy === it.sy) continue;   // 自己的锚点
      if (circleHitsBox(n.cx, n.cy, n.r, it.box, clearance)) { covers = true; break; }
    }
    if (covers) { rejectNode++; continue; }

    // 3b) 标签之间不重叠
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
      coverage: area / Math.max(o.viewportW * o.viewportH, 1),
    },
  };
}
