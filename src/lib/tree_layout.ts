// 树感知布局（v3.0 布局专项）
//
// 依据：Engram 的图在数学上是森林——每个节点最多一个 parent、无环（实测 5 个工作区：
// perf1500/D:\TA/G:\ta/G:\engram/test-data 的 multiParentNodes=0、cycles=0）。
// 树在正确布局下边交叉理论值为 0，且有 O(n) 确定性算法；用力导向物理去逼近它是纯浪费
// （实测 1500 节点：力导向 361997 次交叉、80 帧后仍以 26521px/帧漂移；本模块 1.4ms、0 交叉、完全静止）。
//
// 本模块是**纯函数**：同输入必得同输出（无 Math.random、无时间依赖、无副作用）。
// 这是工程约束而非洁癖——力导向版曾用非种子 Math.random 打破重合节点，导致同一张图
// 每次跑出来的布局都不一样，截图/回归都无法比对。
//
// 双模式：
//   layered —— 分层整洁树（Reingold–Tilford / Buchheim 一脉）：层内水平排布、父节点居中于子节点。
//              长宽比贴近"链式阅读"，适合 ≤ 视口预算的中小图。
//   radial  —— 径向整洁树（Shneiderman 一脉）：子树占角度扇区、扇区大小 ∝ 叶子数，
//              层半径线性递增。1500 节点实测包围盒 1056×1132（分层是 49036×672，73:1），
//              大图 fit 后节点 17.9px vs 分层 0.82px —— 大图只有径向可用。
//
// 另有 maxRadius 安全阀：节点数超过阈值时按比例压缩半径，避免超大图把世界撑到不可用的尺寸。
// 注意它换来的是"能看"，不是"能读"——真正的可读性由渲染层的深度裁剪（渐进披露）负责，
// 因为屏幕像素存在物理下限：实测可视可读预算约 150–300 节点。

import type { ChainSnapshot, NodeType } from './types';

/** 布局模式 */
export type LayoutMode = 'layered' | 'radial';

export interface TreeLayoutOptions {
  mode: LayoutMode;
  /** 层间距（相邻层之间的中心距，px） */
  levelGap: number;
  /** 同层相邻节点的最小中心距（px，会与节点直径取大值） */
  siblingGap: number;
  /** radial 模式的基础环间距（px）：第 d 层半径 = d × ringGap，再按每环拥挤度放大 */
  ringGap: number;
  /** 世界半径上限（px）：超过则整体等比压缩（大图安全阀，0 = 不限） */
  maxRadius: number;
}

export const DEFAULT_LAYOUT_OPTIONS: TreeLayoutOptions = {
  mode: 'radial',
  levelGap: 96,
  siblingGap: 46,
  ringGap: 96,
  maxRadius: 6000,
};

export interface TreeLayoutResult {
  /**
   * 坐标数组，**按 snapshot.nodes 的原索引对齐**（不是按可见节点紧凑索引）。
   * 被 visible 裁掉的节点对应位置为 undefined。
   *
   * 踩坑记录：最初返回的是"紧凑索引"数组（长度 = 可见节点数），调用方却拿
   * `snap.nodes` 的原索引去取值 —— 索引错位导致一部分可见节点拿不到坐标、
   * 在渲染层被当作"不该显示"而隐藏（实测：36 个可见节点只显示了 14 个）。
   * 现在按原索引对齐，`result.positions[i]` 与 `snapshot.nodes[i]` 一一对应。
   */
  positions: ({ x: number; y: number } | undefined)[];
  /** 显示直径，同样按 snapshot.nodes 原索引对齐 */
  sizes: (number | undefined)[];
  /** 节点 id → 深度（根为 0）；用于渐进披露的深度裁剪 */
  depth: Map<string, number>;
  /** 实际使用的模式（自动模式下可能与请求不同） */
  mode: LayoutMode;
  /** 节点 id → 直接子节点数（渐进披露的折叠角标用） */
  childCount: Map<string, number>;
  /** 计算耗时（ms），供性能浮层展示 */
  ms: number;
}

/**
 * 显示直径：度越多越大（与 App.svelte:nodeSize 同式，平方根缓增 + 封顶，Obsidian 风格）。
 * 注意这里按**可见边**的度计算——渐进披露裁剪后半径会随之变小，避免"看不见的连接"撑着大圆点。
 */
export function nodeDisplaySize(degree: number): number {
  return 14 + Math.min(Math.sqrt(degree), 6) * 4;
}

/** 自动模式：可见节点数决定形态。≤300 用分层（贴合视口的链式阅读），否则径向（唯一能压住规模的形态）。 */
export function pickMode(visibleCount: number): LayoutMode {
  return visibleCount <= 300 ? 'layered' : 'radial';
}

/**
 * 共享预处理（layered() 与 estimateLayeredSize() 用同一套递推，保证估算不跑偏）：
 * 后序累加子树占位宽度 ext[]、BFS 深度 depth[]。
 */
function computeExtents(P: Prepared, siblingGap: number): { ext: Float64Array; depth: Int32Array; maxDepth: number } {
  const N = P.ids.length;
  const { children, roots, sizes } = P;
  const ext = new Float64Array(N);
  const depth = new Int32Array(N).fill(-1);

  for (const r of roots) {
    depth[r] = 0;
    const q: number[] = [r];
    let h = 0;
    while (h < q.length) {
      const cur = q[h++];
      for (const c of children[cur]) {
        if (depth[c] < 0) { depth[c] = depth[cur] + 1; q.push(c); }
      }
    }
  }
  for (let i = 0; i < N; i++) if (depth[i] < 0) depth[i] = 0;

  // 后序遍历（迭代式，避免深链递归爆栈）
  const order: number[] = [];
  const visited = new Uint8Array(N);
  for (const r of roots) {
    const stack = [r];
    while (stack.length) {
      const i = stack[stack.length - 1];
      if (visited[i]) { stack.pop(); order.push(i); continue; }
      visited[i] = 1;
      for (let k = children[i].length - 1; k >= 0; k--) stack.push(children[i][k]);
    }
  }
  for (const i of order) {
    const ch = children[i];
    if (ch.length === 0) { ext[i] = sizes[i]; continue; }
    let w = 0;
    for (let k = 0; k < ch.length; k++) {
      w += ext[ch[k]];
      if (k > 0) w += siblingGap;
    }
    ext[i] = Math.max(w, sizes[i]);
  }
  let maxDepth = 0;
  for (let i = 0; i < N; i++) if (depth[i] > maxDepth) maxDepth = depth[i];
  return { ext, depth, maxDepth };
}

/**
 * 预估分层布局的世界尺寸（宽 × 高）。
 *
 * 为什么需要：分层整洁树的宽度 ∝ 叶子总数，浅宽树展开后是极扁的细长条
 * （1500 节点实测 12000×118px）。塞进 16:9 视口后节点只有 3.3px —— 形同不可见。
 * 所以"节点数少就用分层"这条规则不够，还得看 fit 之后还读不读得出来（见 preferLayered）。
 */
export function estimateLayeredSize(
  snapshot: ChainSnapshot,
  visible: Set<string> | null,
  siblingGap: number,
  levelGap: number,
): { w: number; h: number; meanSize: number } {
  const P = prepare(snapshot, visible);
  if (P.ids.length < 3) return { w: Infinity, h: Infinity, meanSize: 14 };
  const { ext, maxDepth } = computeExtents(P, siblingGap);
  let totalW = 0;
  for (const r of P.roots) totalW += ext[r] + siblingGap * 2;
  let sum = 0;
  for (const s of P.sizes) sum += s;
  return { w: totalW, h: Math.max(maxDepth * levelGap, 1), meanSize: sum / P.ids.length };
}

/** 形态回退阈值：分层布局在视口里 fit 后，节点屏幕直径低于此值就改走径向 */
export const MIN_READABLE_NODE_PX = 9;

/** 分层布局的最小世界宽度（防止小图被"高度极矮"除出一个虚高的 zoom） */
const MIN_LAYERED_WIDTH = 1200;

/**
 * 分层 or 径向？判据不是"扁平"本身，而是**fit 到参考视口后节点还剩几个像素**。
 *
 * 走过的弯路：一开始用"宽高比 > 8 就回退"。结果 80 节点的真实图（3399×384，宽高比 8.85）
 * 被误判回退——而它恰恰是分层的理想场景（fit 后节点 11.8px、层间距直观可读）。
 * 扁平本身不是问题，"fit 完看不见"才是问题，所以直接算可读性。
 *
 * @param size 分层布局的估算世界尺寸（estimateLayeredSize 的返回值）
 */
export function preferLayered(
  size: { w: number; h: number; meanSize?: number },
  viewportW = 1600,
  viewportH = 900,
  minNodePx = MIN_READABLE_NODE_PX,
): boolean {
  if (!isFinite(size.w) || !isFinite(size.h) || size.w <= 0) return true;   // 太小，一律分层
  const w = Math.max(size.w, MIN_LAYERED_WIDTH);
  const h = Math.max(size.h, 1);
  const zoom = Math.min((viewportW - 60) / w, (viewportH - 120) / h);
  // 用**实际平均直径**而非"最小叶节点 14px"估算：小图的节点往往比 14px 大不少
  // （D:\TA 均值约 19px），用 14px 会把"其实读得清"的图误判成要回退（实测被误伤过）。
  const mean = size.meanSize && size.meanSize > 0 ? size.meanSize : 14;
  return mean * zoom >= minNodePx;
}

interface Prepared {
  ids: string[];          // 参与布局的节点 id（按 snapshot.nodes 顺序，紧凑）
  snapIndex: Int32Array;  // 紧凑索引 k → snapshot.nodes 的原索引（回填坐标用）
  index: Map<string, number>;
  children: number[][];   // 子节点索引（紧凑索引，按输入顺序，稳定）
  parent: Int32Array;     // 父节点索引（-1 = 根）
  roots: number[];        // 森林的根（按输入顺序）
  degree: Int32Array;     // 可见度（去重后的无向度）
  sizes: number[];
}

/** 建邻接结构与度。parent 指向不存在/自环的边按"根"处理（与后端校验一致，此处只做防御）。 */
function prepare(snapshot: ChainSnapshot, visible: Set<string> | null): Prepared {
  const all = snapshot.nodes;
  const ids: string[] = [];
  const snapIdx: number[] = [];
  const index = new Map<string, number>();
  for (let i = 0; i < all.length; i++) {
    const n = all[i];
    if (visible && !visible.has(n.id)) continue;
    index.set(n.id, ids.length);
    ids.push(n.id);
    snapIdx.push(i);
  }
  const N = ids.length;
  const snapIndex = Int32Array.from(snapIdx);
  const children: number[][] = Array.from({ length: N }, () => []);
  const parent = new Int32Array(N).fill(-1);
  const deg = new Int32Array(N);
  const seenPair = new Set<number>();

  for (const e of snapshot.edges) {
    const p = index.get(e.parent);
    const c = index.get(e.child);
    if (p === undefined || c === undefined || p === c) continue;
    const key = p * N + c;
    if (seenPair.has(key)) continue;   // 重复边只算一次（防止度虚高 + 子树重复挂载）
    seenPair.add(key);
    children[p].push(c);
    parent[c] = p;                     // 多父时后者覆盖（当前数据不存在，防御性处理）
    deg[p]++;
    deg[c]++;
  }

  // 根：无父节点者。若 manifest.root 在其中，排到最前（保证主链优先铺开）
  const roots: number[] = [];
  const rootId = snapshot.manifest?.root;
  const rootIdx = rootId !== undefined ? index.get(rootId) : undefined;
  if (rootIdx !== undefined && parent[rootIdx] === -1) roots.push(rootIdx);
  for (let i = 0; i < N; i++) if (parent[i] === -1 && i !== rootIdx) roots.push(i);

  const sizes = new Array<number>(N);
  for (let i = 0; i < N; i++) sizes[i] = nodeDisplaySize(deg[i]);

  return { ids, snapIndex, index, children, parent, roots, degree: deg, sizes };
}

/**
 * 布局主入口。
 * @param snapshot 图快照（节点 + 边）
 * @param visible  可见节点 id 集合；null = 全部可见。位置按**该集合**计算（裁剪后重排）
 * @param opts     布局参数
 */
export function computeTreeLayout(
  snapshot: ChainSnapshot,
  visible: Set<string> | null,
  opts: TreeLayoutOptions,
): TreeLayoutResult {
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();
  const P = prepare(snapshot, visible);
  const total = snapshot.nodes.length;
  const N = P.ids.length;
  // 关键：数组按 snapshot.nodes 的**原索引**长度分配（见 TreeLayoutResult.positions 的踩坑记录），
  // 只有可见且参与布局的索引有值，其余留 undefined
  const pos: ({ x: number; y: number } | undefined)[] = new Array(total);
  const outSizes: (number | undefined)[] = new Array(total);
  const depth = new Map<string, number>();
  const childCount = new Map<string, number>();
  P.ids.forEach((id, k) => {
    pos[P.snapIndex[k]] = { x: 0, y: 0 };
    outSizes[P.snapIndex[k]] = P.sizes[k];
  });
  for (const id of P.ids) childCount.set(id, 0);
  for (let i = 0; i < N; i++) childCount.set(P.ids[i], P.children[i].length);

  if (N === 0) return { positions: pos, sizes: outSizes, depth, mode: opts.mode, childCount, ms: 0 };
  if (N === 1) {
    depth.set(P.ids[0], 0);
    return { positions: pos, sizes: outSizes, depth, mode: opts.mode, childCount, ms: elapsed(t0) };
  }

  // 深度（BFS，按 roots 逐棵；forest 的每棵根 depth=0）
  const depthArr = new Int32Array(N).fill(-1);
  for (const r of P.roots) {
    depthArr[r] = 0;
    const q: number[] = [r];
    let h = 0;
    while (h < q.length) {
      const cur = q[h++];
      for (const c of P.children[cur]) {
        if (depthArr[c] < 0) {
          depthArr[c] = depthArr[cur] + 1;
          q.push(c);
        }
      }
    }
  }
  for (let i = 0; i < N; i++) depth.set(P.ids[i], Math.max(depthArr[i], 0));

  // 内部用紧凑数组布局（O(N)），最后再按原索引回填
  const compact: { x: number; y: number }[] = new Array(N);
  for (let i = 0; i < N; i++) compact[i] = { x: 0, y: 0 };

  const mode = opts.mode;
  if (mode === 'layered') layered(P, compact, opts);
  else radial(P, compact, opts);

  // 居中：包围盒中心移到原点（视口 fit/center 的基准稳定，避免"图整体偏在右下"）
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  for (const p of compact) {
    if (p.x < minX) minX = p.x;
    if (p.x > maxX) maxX = p.x;
    if (p.y < minY) minY = p.y;
    if (p.y > maxY) maxY = p.y;
  }
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  for (const p of compact) { p.x -= cx; p.y -= cy; }

  // 安全阀：世界半径上限（大图兜底）
  //
  // 语义是"兜底"而非"整形"，所以必须守住最小缩放——否则一个 12000px 宽的分层树
  // 会被压成 6000px 半径（缩放 0.175），层间距 96px → 17px，形态直接被毁（实测踩过）。
  // 宁可让极宽的世界溢出上限，也不把它压成一条线：溢出的处理交给 mode 回退（见下）与
  // 渲染层的深度裁剪，而不是在这里均匀缩小。
  const MIN_SAFETY_SCALE = 0.35;
  if (opts.maxRadius > 0) {
    let maxR = 0;
    for (const p of compact) maxR = Math.max(maxR, Math.hypot(p.x, p.y));
    if (maxR > opts.maxRadius) {
      const k = Math.max(opts.maxRadius / maxR, MIN_SAFETY_SCALE);
      for (const p of compact) { p.x *= k; p.y *= k; }
    }
  }

  // 回填到原索引（此时才把坐标交给调用方）
  for (let i = 0; i < N; i++) pos[P.snapIndex[i]] = compact[i];

  return { positions: pos, sizes: outSizes, depth, mode, childCount, ms: elapsed(t0) };
}

function elapsed(t0: number): number {
  const now = typeof performance !== 'undefined' ? performance.now() : Date.now();
  return Math.round((now - t0) * 100) / 100;
}

// ───────────────────────── layered：分层整洁树 ─────────────────────────
// 后序算子树占位宽度 → 前序按左边界摆放 → 父节点回中到子节点中点。
// 单次遍历，O(n)，无迭代收敛，无随机。
function layered(P: Prepared, pos: { x: number; y: number }[], opts: TreeLayoutOptions): void {
  const { children, roots, sizes } = P;
  const { ext } = computeExtents(P, opts.siblingGap);

  // 前序：给定子树左边界 left 与深度 d，摆下整棵子树
  const place = (root: number, left: number, d: number) => {
    const st: { node: number; left: number; depth: number }[] = [{ node: root, left, depth: d }];
    while (st.length) {
      const { node, left: l, depth: dd } = st.pop()!;
      const ch = children[node];
      pos[node].y = dd * opts.levelGap;
      if (ch.length === 0) {
        pos[node].x = l + sizes[node] / 2;
        continue;
      }
      // 子节点块居中于子树占位内
      let blockW = 0;
      for (let k = 0; k < ch.length; k++) {
        blockW += ext[ch[k]];
        if (k > 0) blockW += opts.siblingGap;
      }
      let cur = l + (ext[node] - blockW) / 2;
      let firstMid = 0;
      let lastMid = 0;
      for (let k = 0; k < ch.length; k++) {
        const c = ch[k];
        st.push({ node: c, left: cur, depth: dd + 1 });
        const mid = cur + ext[c] / 2;
        if (k === 0) firstMid = mid;
        lastMid = mid;
        cur += ext[c] + opts.siblingGap;
      }
      pos[node].x = (firstMid + lastMid) / 2;   // 父节点居中于首末子节点
    }
  };

  // 森林：每棵独立摆（左边界从 0 起），再按各棵宽度横向拼接，避免互相压在一起
  const R0: number[] = [];
  const R1: number[] = [];
  for (const r of roots) {
    const lo = 0;
    place(r, lo, 0);
    R0.push(lo);
    R1.push(lo + ext[r]);
  }
  let cursor = 0;
  for (let k = 0; k < roots.length; k++) {
    const shift = cursor - R0[k];
    if (shift !== 0) shiftSubtree(children, roots[k], pos, shift, 0);
    cursor += R1[k] - R0[k] + opts.siblingGap * 2;   // 森林各棵之间留更大间隙
  }
}

/** 横向平移一棵子树（森林拼接用；纯几何位移，不改结构） */
function shiftSubtree(
  children: number[][],
  root: number,
  pos: { x: number; y: number }[],
  dx: number,
  dy: number,
): void {
  const st = [root];
  while (st.length) {
    const i = st.pop()!;
    pos[i].x += dx;
    pos[i].y += dy;
    for (const c of children[i]) st.push(c);
  }
}

// ───────────────────────── radial：径向整洁树 ─────────────────────────
// 自底向上算叶子数 → 按叶子权重切分角度扇区 → 逐层定半径。
// 子树各占不相交扇区 ⇒ 边全落在自己扇区内 ⇒ 交叉数恒为 0（树结构下）。
function radial(P: Prepared, pos: { x: number; y: number }[], opts: TreeLayoutOptions): void {
  const N = P.ids.length;
  const { children, roots, sizes } = P;
  const leaves = new Int32Array(N);
  const depthArr = new Int32Array(N);
  const angle = new Float64Array(N);

  // 后序：叶子数 + 深度
  const order: number[] = [];
  const visited = new Uint8Array(N);
  for (const r of roots) {
    const stack = [r];
    while (stack.length) {
      const i = stack[stack.length - 1];
      if (visited[i]) { stack.pop(); order.push(i); continue; }
      visited[i] = 1;
      for (let k = children[i].length - 1; k >= 0; k--) stack.push(children[i][k]);
    }
  }
  for (let i = 0; i < N; i++) depthArr[i] = -1;
  for (const r of roots) depthArr[r] = 0;
  // 深度必须**独立预计算**：早前把"设子节点深度"塞在叶子数归并循环里，
  // 而该循环对叶子是 continue 分支 —— 叶节点深度永远停在 -1，兜底成 0，
  // 导致所有叶子被放到圆心、径向布局整体塌陷（实测包围盒 188px）。
  for (const r of roots) {
    const q: number[] = [r];
    let h = 0;
    while (h < q.length) {
      const cur = q[h++];
      for (const c of children[cur]) {
        if (depthArr[c] < 0) {
          depthArr[c] = depthArr[cur] + 1;
          q.push(c);
        }
      }
    }
  }
  for (const i of order) {
    const ch = children[i];
    if (ch.length === 0) { leaves[i] = 1; continue; }
    let s = 0;
    for (const c of ch) s += leaves[c];
    leaves[i] = s;
  }
  // 深度兜底（理论上 BFS 已覆盖；防御悬空/不可达）
  for (let i = 0; i < N; i++) if (depthArr[i] < 0) depthArr[i] = 0;

  // 前序：把 [a0, a1) 扇区按叶子数分配给子节点
  const spread = (root: number) => {
    const st: { node: number; a0: number; a1: number }[] = [{ node: root, a0: 0, a1: Math.PI * 2 }];
    while (st.length) {
      const { node, a0, a1 } = st.pop()!;
      const ch = children[node];
      if (!ch.length) continue;
      let total = 0;
      for (const c of ch) total += leaves[c];
      let a = a0;
      for (const c of ch) {
        const span = (a1 - a0) * (leaves[c] / Math.max(total, 1));
        angle[c] = a + span / 2;
        st.push({ node: c, a0: a, a1: a + span });
        a += span;
      }
    }
  };

  // 多层同环安全间距：按该环的节点数/叶子数估算弧长需求，反推最小半径
  const maxDepth = depthArr.reduce((m, d) => Math.max(m, d), 0);
  const needPerLevel = new Float64Array(maxDepth + 1);
  const countPerLevel = new Int32Array(maxDepth + 1);
  for (let i = 0; i < N; i++) {
    const d = depthArr[i];
    countPerLevel[d]++;
    needPerLevel[d] += sizes[i] + opts.siblingGap;
  }

  for (const r of roots) {
    angle[r] = 0;
    spread(r);
  }

  const radiusOfLevel = new Float64Array(maxDepth + 1);
  radiusOfLevel[0] = 0;
  for (let d = 1; d <= maxDepth; d++) {
    // 弧长需求 → 半径；再与"上一环 + ringGap"取大值保证层间不塌
    const arcNeed = countPerLevel[d] > 1 ? needPerLevel[d] / (Math.PI * 2) : 0;
    radiusOfLevel[d] = Math.max(radiusOfLevel[d - 1] + opts.ringGap, arcNeed);
  }

  for (let i = 0; i < N; i++) {
    const d = depthArr[i];
    const r = radiusOfLevel[d];
    pos[i].x = Math.cos(angle[i]) * r;
    pos[i].y = Math.sin(angle[i]) * r;
  }
}
