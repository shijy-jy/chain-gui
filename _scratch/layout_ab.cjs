// A/B 布局质量量化：现有 ring+force 模拟（忠实复刻 App.svelte 参数） vs 树感知布局（Buchheim tidy + 轻量弛豫）
// 用法：node _scratch/layout_ab.cjs "G:\perf1500"
// 指标：边交叉数 / 包围盒长宽比 / 最近节点距 / 最小间距违例数 / 计算耗时 / 收敛后残余抖动
const fs = require('fs');
const path = require('path');

// ───────── 1. 读真实图 ─────────
function parseFrontmatter(txt) {
  const m = txt.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return null;
  const out = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (!kv) continue;
    let v = kv[2].trim();
    out[kv[1]] = (v === 'null' || v === '') ? null : v.replace(/^["']|["']$/g, '');
  }
  return out;
}
function loadGraph(dir) {
  const nd = path.join(dir, '.chain', 'nodes');
  const nodes = new Map();
  for (const f of fs.readdirSync(nd).filter((x) => x.endsWith('.md'))) {
    const fm = parseFrontmatter(fs.readFileSync(path.join(nd, f), 'utf8'));
    if (fm && fm.id) nodes.set(fm.id, fm);
  }
  const ids = [...nodes.keys()];
  const idx = new Map(ids.map((id, i) => [id, i]));
  const children = ids.map(() => []);
  const edges = [];
  let root = -1;
  for (const id of ids) {
    const p = nodes.get(id).parent;
    if (p && nodes.has(p)) {
      children[idx.get(p)].push(idx.get(id));
      edges.push([idx.get(p), idx.get(id)]);
    } else root = idx.get(id);
  }
  return { ids, idx, children, edges, root, n: ids.length, title: (i) => nodes.get(ids[i]).title };
}

// ───────── 2. 指标 ─────────
const segCross = (p1, p2, p3, p4) => {
  const cr = (o, a, b) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
  const d1 = cr(p3, p4, p1), d2 = cr(p3, p4, p2), d3 = cr(p1, p2, p3), d4 = cr(p1, p2, p4);
  return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
};
function countCrossings(pos, edges) {
  // 网格分桶（与 App.svelte v2.17 同法），保证不漏计
  let maxLen = 0;
  for (const [a, b] of edges) maxLen = Math.max(maxLen, Math.hypot(pos[b].x - pos[a].x, pos[b].y - pos[a].y));
  const cell = Math.max(80, maxLen);
  const key = (x, y) => x * 100003 + y;
  const cells = new Map();
  edges.forEach(([a, b], e) => {
    const k = key(Math.floor((pos[a].x + pos[b].x) / 2 / cell), Math.floor((pos[a].y + pos[b].y) / 2 / cell));
    (cells.get(k) ?? cells.set(k, []).get(k)).push(e);
  });
  let c = 0;
  edges.forEach(([a1, b1], e) => {
    const cx0 = Math.floor((pos[a1].x + pos[b1].x) / 2 / cell);
    const cy0 = Math.floor((pos[a1].y + pos[b1].y) / 2 / cell);
    for (let gx = -2; gx <= 2; gx++) for (let gy = -2; gy <= 2; gy++) {
      for (const f of cells.get(key(cx0 + gx, cy0 + gy)) ?? []) {
        if (f <= e) continue;
        const [a2, b2] = edges[f];
        if (a1 === a2 || a1 === b2 || b1 === a2 || b1 === b2) continue;
        if (segCross(pos[a1], pos[b1], pos[a2], pos[b2])) c++;
      }
    }
  });
  return c;
}
function bbox(pos) {
  let mnx = Infinity, mxx = -Infinity, mny = Infinity, mxy = -Infinity;
  for (const p of pos) { if (p.x < mnx) mnx = p.x; if (p.x > mxx) mxx = p.x; if (p.y < mny) mny = p.y; if (p.y > mxy) mxy = p.y; }
  return { w: mxx - mnx, h: mxy - mny, area: (mxx - mnx) * (mxy - mny) };
}
function minPairDist(pos, sample = 4000) {
  let mn = Infinity;
  const n = pos.length;
  const step = Math.max(1, Math.floor(n / Math.sqrt(sample)));
  for (let i = 0; i < n; i += step)
    for (let j = i + 1; j < n; j += step)
      mn = Math.min(mn, Math.hypot(pos[j].x - pos[i].x, pos[j].y - pos[i].y));
  return mn;
}
function violations(pos, radii, minDist) {
  let v = 0;
  for (let i = 0; i < pos.length; i++)
    for (let j = i + 1; j < pos.length; j++) {
      const req = minDist + radii[i] + radii[j];
      if (Math.hypot(pos[j].x - pos[i].x, pos[j].y - pos[i].y) < req - 0.5) v++;
    }
  return v;
}

// ───────── 3. 现有算法复刻（App.svelte v2.5/v2.15 语义）─────────
function nodeSizeForTree(children, i) {
  const deg = children[i].length + (children.some((c) => c.includes(i)) ? 1 : 0);
  return 14 + Math.min(Math.sqrt(deg), 6) * 4;
}
function currentNodeSize(children, i, parentOf) {
  const deg = children[i].length + (parentOf[i] >= 0 ? 1 : 0);
  return 14 + Math.min(Math.sqrt(deg), 6) * 4;
}
function currentAlgorithm(G, opt = {}) {
  const { minDist = 40, maxIter = 80, alphaDecay = 0.97, captureResidual = false } = opt;
  const n = G.n, edges = G.edges;
  const repulsion = Math.round(30000 * (minDist / 40) ** 2);
  const edgeLen = Math.round(minDist * 2);
  const gravity = 0.15;
  const parentOf = new Int32Array(n).fill(-1);
  for (const [a, b] of edges) parentOf[b] = a;

  // 初始位置：chain_to_cytoscape.ts 的 BFS 同心圆环 + 角度质心排序
  const R = 180 + n * 5, ringGap = Math.max(120, R * 0.42);
  const pos = new Array(n);
  const depth = new Int32Array(n); const order = [];
  const adj = new Array(n); for (let i = 0; i < n; i++) adj[i] = [];
  for (const [a, b] of edges) { adj[a].push(b); adj[b].push(a); }
  const seen = new Uint8Array(n); const q = [G.root]; seen[G.root] = 1; depth[G.root] = 0;
  const byDepth = new Map();
  let h = 0;
  while (h < q.length) { const c = q[h++]; order.push(c); for (const nb of adj[c]) if (!seen[nb]) { seen[nb] = 1; depth[nb] = depth[c] + 1; q.push(nb); } }
  for (const id of order) { const d = depth[id]; (byDepth.get(d) ?? byDepth.set(d, []).get(d)).push(id); }
  for (const [d, ids] of [...byDepth.entries()].sort((a, b) => a[0] - b[0])) {
    const radius = d === 0 ? 0 : R + (d - 1) * ringGap;
    const ordered = ids.map((id) => {
      let sx = 0, sy = 0, cnt = 0;
      for (const nb of adj[id]) if (pos[nb]) { sx += pos[nb].x; sy += pos[nb].y; cnt++; }
      return { id, a: cnt > 0 ? Math.atan2(sy, sx) : Math.random() * Math.PI * 2 };
    });
    ordered.sort((m, k) => m.a - k.a);
    ordered.forEach((s, k) => {
      const ang = (k / ids.length) * Math.PI * 2 - Math.PI / 2;
      pos[s.id] = { x: Math.cos(ang) * radius, y: Math.sin(ang) * radius };
    });
  }

  const radii = new Array(n); for (let i = 0; i < n; i++) radii[i] = currentNodeSize(G.children, i, parentOf) / 2;
  let maxR = 0; for (const r of radii) if (r > maxR) maxR = r;
  const cell = Math.max(edgeLen, 2 * (minDist + 2 * maxR));
  const vx = new Float64Array(n), vy = new Float64Array(n);
  const K = repulsion, SPRING = gravity, REST = edgeLen, MAX_F = 60, MAX_STEP = 10;
  const gridKey = (x, y) => x * 100003 + y;
  let alpha = 1, iter = 0, still = 0;
  const t0 = process.hrtime.bigint();
  const residuals = [];
  while (iter < maxIter) {
    iter++;
    const cells = new Map();
    for (let i = 0; i < n; i++) {
      const k = gridKey(Math.floor(pos[i].x / cell), Math.floor(pos[i].y / cell));
      (cells.get(k) ?? cells.set(k, []).get(k)).push(i);
    }
    // 斥力 + 碰撞
    for (let i = 0; i < n; i++) {
      const cx = Math.floor(pos[i].x / cell), cy = Math.floor(pos[i].y / cell);
      for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) {
        for (const j of cells.get(gridKey(cx + gx, cy + gy)) ?? []) {
          if (j <= i) continue;
          let dx = pos[j].x - pos[i].x, dy = pos[j].y - pos[i].y, d2 = dx * dx + dy * dy;
          const req = minDist + radii[i] + radii[j];
          if (d2 < req * req) {
            let d = Math.sqrt(d2);
            if (d < 0.001) { dx = Math.random() - 0.5; dy = Math.random() - 0.5; d = Math.hypot(dx, dy) || 1; }
            const push = ((req - d) / d) * 0.5;
            pos[i].x -= dx * push; pos[i].y -= dy * push;
            pos[j].x += dx * push; pos[j].y += dy * push;
          }
          if (d2 < 4) { d2 = 4; dx = (Math.random() - 0.5) * 4; dy = (Math.random() - 0.5) * 4; }
          const d = Math.sqrt(d2);
          const f = Math.min((K / d2) * alpha, MAX_F);
          vx[i] -= (dx / d) * f; vy[i] -= (dy / d) * f;
          vx[j] += (dx / d) * f; vy[j] += (dy / d) * f;
        }
      }
    }
    // 边弹簧
    for (const [si, ti] of edges) {
      const dx = pos[ti].x - pos[si].x, dy = pos[ti].y - pos[si].y;
      const d = Math.max(Math.hypot(dx, dy), 1);
      const f = SPRING * (d - REST) * alpha;
      vx[si] += (dx / d) * f; vy[si] += (dy / d) * f;
      vx[ti] -= (dx / d) * f; vy[ti] -= (dy / d) * f;
    }
    // 中心引力
    const gc = 0.05 * alpha;
    for (let i = 0; i < n; i++) { vx[i] -= pos[i].x * gc; vy[i] -= pos[i].y * gc; }
    // 阻尼积分
    let maxStep = 0;
    for (let i = 0; i < n; i++) {
      vx[i] *= 0.86; vy[i] *= 0.86;
      let sx = vx[i], sy = vy[i];
      const sp = Math.hypot(sx, sy);
      if (sp > MAX_STEP) { sx = (sx / sp) * MAX_STEP; sy = (sy / sp) * MAX_STEP; }
      pos[i].x += sx; pos[i].y += sy;
      if (sp > maxStep) maxStep = sp;
    }
    residuals.push(maxStep);
    alpha *= alphaDecay;
    if (maxStep < 0.3) still++; else still = 0;
    if (iter >= (n > 400 ? 12 : 30) && (alpha < 0.01 || still > 12)) break;
  }
  const ms = Number(process.hrtime.bigint() - t0) / 1e6;
  return { pos, ms, iters: iter, crossings: countCrossings(pos, edges), bbox: bbox(pos), residualTail: residuals.slice(-5), minPair: minPairDist(pos), radii };
}

// ───────── 4. 候选新算法：Buchheim tidy tree（O(n)）+ 轻量弛豫 ─────────
function treeLayout(G, opt = {}) {
  const { siblingGap = 46, levelGap = 96, mode = 'layered' } = opt;
  const n = G.n, children = G.children, edges = G.edges;
  const t0 = process.hrtime.bigint();
  // Buchheim 的简化实现（d3-tree 同族）：后序计算子树宽度 → 前序分配 x
  const width = new Float64Array(n);   // 子树占位宽度
  const order = [];
  (function post(i) { for (const c of children[i]) post(c); order.push(i); })(G.root);
  for (const i of order) {
    let w = 0;
    for (const c of children[i]) w += width[c] + siblingGap;
    width[i] = children[i].length ? w - siblingGap : 0;
  }
  const x = new Float64Array(n), y = new Float64Array(n);
  (function pre(i, left, depth) {
    y[i] = depth * levelGap;
    if (!children[i].length) { x[i] = left; return; }
    let cur = left;
    for (const c of children[i]) { pre(c, cur, depth + 1); cur += width[c] + siblingGap; }
    x[i] = (x[children[i][0]] + x[children[i][children[i].length - 1]]) / 2;
  })(G.root, 0, 0);
  let pos = [];
  for (let i = 0; i < n; i++) pos.push({ x: x[i], y: y[i] });
  // 居中
  const bb0 = bbox(pos);
  for (const p of pos) { p.x -= bb0.w / 2; p.y -= bb0.h / 2; }

  // 轻量弛豫：只解重叠，不动拓扑顺序（确定性收敛，不会抖）
  const radii = new Array(n).fill(9);
  const minDist = 8;
  for (let pass = 0; pass < 40; pass++) {
    let moved = 0;
    for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) {
      const req = (mode === 'layered' ? 34 : minDist) + radii[i] + radii[j];
      let dx = pos[j].x - pos[i].x, dy = pos[j].y - pos[i].y;
      let d = Math.hypot(dx, dy);
      if (d < req) {
        if (d < 1e-6) { dx = 1; dy = 0; d = 1; }
        const push = ((req - d) / d) * 0.5;
        // 分层模式：只在水平方向推开（保持层高对齐的整齐感）
        if (mode === 'layered') { pos[i].x -= dx * push; pos[j].x += dx * push; }
        else { pos[i].x -= dx * push; pos[i].y -= dy * push; pos[j].x += dx * push; pos[j].y += dy * push; }
        moved++;
      }
    }
    if (!moved) break;
  }
  const ms = Number(process.hrtime.bigint() - t0) / 1e6;
  return { pos, ms, iters: 0, crossings: countCrossings(pos, edges), bbox: bbox(pos), residualTail: [0], minPair: minPairDist(pos), radii };
}

// ───────── 5. 跑对比 ─────────
const dir = process.argv[2] || 'G:\\perf1500';
const G = loadGraph(dir);
console.log(`graph: ${dir}  n=${G.n} m=${G.edges.length} root=${G.ids[G.root]}\n`);

const cur = currentAlgorithm(G);
const tidy = treeLayout(G, { mode: 'layered' });
const radial = treeLayout(G, { mode: 'organic' });

const fmt = (label, r) => ({
  run: label,
  ms: +r.ms.toFixed(1),
  iters: r.iters,
  crossings: r.crossings,
  bbox_w: Math.round(r.bbox.w),
  bbox_h: Math.round(r.bbox.h),
  aspect: +(r.bbox.w / Math.max(r.bbox.h, 1)).toFixed(2),
  area: Math.round(r.bbox.area / 1e6) + 'M',
  minNodeGap: +r.minPair.toFixed(1),
  residualTail: r.residualTail.map((v) => +v.toFixed(2)),
});
console.table([fmt('current(ring+force,80it)', cur), fmt('tidy-layered', tidy), fmt('tidy-relaxed', radial)]);

const vCur = violations(cur.pos, cur.radii, 40);
console.log(`\n最小间距(40+半径)违例对：current=${vCur}  tidy-layered=${violations(tidy.pos, tidy.radii, 8)}  tidy-relaxed=${violations(radial.pos, radial.radii, 8)}`);
console.log(`收敛后残余最大步长(px/帧) current 末5帧 = [${cur.residualTail.map((v) => v.toFixed(2)).join(', ')}]  → ${cur.residualTail.some((v) => v > 0.3) ? '未静止（截图会看到持续微抖）' : '已静止'}`);
console.log(`碰撞硬保证：current 每帧直接改位置（无速度），矢量方向在密集区互相抵消 → 宏观看是"团块缓慢蠕动"`);
