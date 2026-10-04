// 径向（radial tidy）vs 分层（layered tidy）vs 现有力导向：紧凑度 / 交叉 / 屏幕可读性 / 局部展开观感
const fs = require('fs'); const path = require('path');
function pf(t) { const m = t.match(/^---\r?\n([\s\S]*?)\r?\n---/); if (!m) return null; const o = {}; for (const l of m[1].split(/\r?\n/)) { const kv = l.match(/^([A-Za-z_][\w-]*):\s*(.*)$/); if (kv) { const v = kv[2].trim(); o[kv[1]] = (v === 'null' || v === '') ? null : v; } } return o; }
function load(dir) {
  const nd = path.join(dir, '.chain', 'nodes'); const nodes = new Map();
  for (const f of fs.readdirSync(nd).filter((x) => x.endsWith('.md'))) { const fm = pf(fs.readFileSync(path.join(nd, f), 'utf8')); if (fm && fm.id) nodes.set(fm.id, fm); }
  const ids = [...nodes.keys()], idx = new Map(ids.map((id, i) => [id, i]));
  const children = ids.map(() => []), edges = []; let root = -1;
  for (const id of ids) { const p = nodes.get(id).parent; if (p && nodes.has(p)) { children[idx.get(p)].push(idx.get(id)); edges.push([idx.get(p), idx.get(id)]); } else root = idx.get(id); }
  return { ids, children, edges, root, n: ids.length };
}
const segCross = (p1, p2, p3, p4) => { const cr = (o, a, b) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x); const d1 = cr(p3, p4, p1), d2 = cr(p3, p4, p2), d3 = cr(p1, p2, p3), d4 = cr(p1, p2, p4); return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0)); };
function crossings(pos, edges) {
  let mx = 0; for (const [a, b] of edges) mx = Math.max(mx, Math.hypot(pos[b].x - pos[a].x, pos[b].y - pos[a].y));
  const cell = Math.max(80, mx), key = (x, y) => x * 100003 + y, cells = new Map();
  edges.forEach(([a, b], e) => { const k = key(Math.floor((pos[a].x + pos[b].x) / 2 / cell), Math.floor((pos[a].y + pos[b].y) / 2 / cell)); (cells.get(k) ?? cells.set(k, []).get(k)).push(e); });
  let c = 0;
  edges.forEach(([a1, b1], e) => { const cx0 = Math.floor((pos[a1].x + pos[b1].x) / 2 / cell), cy0 = Math.floor((pos[a1].y + pos[b1].y) / 2 / cell);
    for (let gx = -2; gx <= 2; gx++) for (let gy = -2; gy <= 2; gy++) for (const f of cells.get(key(cx0 + gx, cy0 + gy)) ?? []) {
      if (f <= e) continue; const [a2, b2] = edges[f]; if (a1 === a2 || a1 === b2 || b1 === a2 || b1 === b2) continue;
      if (segCross(pos[a1], pos[b1], pos[a2], pos[b2])) c++; } });
  return c;
}
const bbox = (pos) => { let mnx = 1e18, mxx = -1e18, mny = 1e18, mxy = -1e18; for (const p of pos) { mnx = Math.min(mnx, p.x); mxx = Math.max(mxx, p.x); mny = Math.min(mny, p.y); mxy = Math.max(mxy, p.y); } return { w: mxx - mnx, h: mxy - mny }; };
function minGap(pos) { let m = 1e18; for (let i = 0; i < pos.length; i++) for (let j = i + 1; j < pos.length; j++) m = Math.min(m, Math.hypot(pos[j].x - pos[i].x, pos[j].y - pos[i].y)); return m; }

function leafCount(G, i) { let s = 0; (function w(k) { if (!G.children[k].length) { s++; return; } for (const c of G.children[k]) w(c); })(i); return s; }
function subtreeSize(G, i) { let s = 1; for (const c of G.children[i]) s += subtreeSize(G, c); return s; }

// 径向 tidy：每个子树占一个角度扇区，扇区大小 ∝ 叶子数（Shneiderman 式，零交叉）
function radialLayout(G, layerGap = 96) {
  const t0 = process.hrtime.bigint();
  const n = G.n; const ang = new Float64Array(n); const depth = new Int32Array(n);
  const leaves = new Int32Array(n);
  const order = []; (function post(i, d) { depth[i] = d; for (const c of G.children[i]) post(c, d + 1); leaves[i] = G.children[i].length ? G.children[i].reduce((s, c) => s + leaves[c], 0) : 1; order.push(i); })(G.root, 0);
  ang[G.root] = 0;
  (function spread(i, a0, a1) { const ch = G.children[i]; if (!ch.length) return; const tot = ch.reduce((s, c) => s + leaves[c], 0); let a = a0; for (const c of ch) { const span = (a1 - a0) * (leaves[c] / tot); ang[c] = a + span / 2; spread(c, a, a + span); a += span; } })(G.root, 0, Math.PI * 2);
  const pos = [];
  for (let i = 0; i < n; i++) { const r = depth[i] * layerGap; pos.push({ x: Math.cos(ang[i]) * r, y: Math.sin(ang[i]) * r }); }
  const ms = Number(process.hrtime.bigint() - t0) / 1e6;
  return { pos, ms, crossings: crossings(pos, G.edges), bbox: bbox(pos), minGap: minGap(pos), kind: 'radial' };
}
// 分层 tidy（同前一脚本）
function layeredLayout(G, siblingGap = 46, levelGap = 96) {
  const t0 = process.hrtime.bigint(); const n = G.n; const width = new Float64Array(n);
  const order = []; (function post(i) { for (const c of G.children[i]) post(c); order.push(i); })(G.root);
  for (const i of order) { let w = 0; for (const c of G.children[i]) w += width[c] + siblingGap; width[i] = G.children[i].length ? w - siblingGap : 0; }
  const x = new Float64Array(n), y = new Float64Array(n);
  (function pre(i, left, d) { y[i] = d * levelGap; if (!G.children[i].length) { x[i] = left; return; } let cur = left; for (const c of G.children[i]) { pre(c, cur, d + 1); cur += width[c] + siblingGap; } x[i] = (x[G.children[i][0]] + x[G.children[i][G.children[i].length - 1]]) / 2; })(G.root, 0, 0);
  const pos = []; for (let i = 0; i < n; i++) pos.push({ x: x[i], y: y[i] });
  const bb = bbox(pos); for (const p of pos) { p.x -= bb.w / 2; p.y -= bb.h / 2; }
  const ms = Number(process.hrtime.bigint() - t0) / 1e6;
  return { pos, ms, crossings: crossings(pos, G.edges), bbox: bbox(pos), minGap: minGap(pos), kind: 'layered' };
}
// 局部展开子图（root + N 层）：检验"渐进披露"下的屏幕可读性
function subgraph(G, levels) {
  const keep = new Set();
  (function walk(i, d) { if (d > levels) return; keep.add(i); for (const c of G.children[i]) walk(c, d + 1); })(G.root, 0);
  return keep;
}

for (const dir of [process.argv[2] || 'G:\\perf1500', 'G:\\ta']) {
  const G = load(dir);
  const rad = radialLayout(G), lay = layeredLayout(G);
  console.log(`\n########## ${dir}  n=${G.n} ##########`);
  console.log(`tidy 计算耗时: layered=${lay.ms.toFixed(2)}ms  radial=${rad.ms.toFixed(2)}ms   （均 O(n)，单帧可完成，无需动画收敛）`);
  console.log(`边交叉: layered=${lay.crossings}  radial=${rad.crossings}   （树在正确布局下理论值 = 0）`);
  const VW = 1600, VH = 900;
  for (const r of [lay, rad]) {
    const zoom = Math.min((VW - 60) / r.bbox.w, (VH - 120) / r.bbox.h);
    console.log(`${r.kind.padEnd(8)} bbox=${Math.round(r.bbox.w)}x${Math.round(r.bbox.h)}  aspect=${(r.bbox.w / r.bbox.h).toFixed(2)}  全图fit zoom=${zoom.toExponential(2)}  节点屏幕=${(26 * zoom).toFixed(2)}px  最小节点间距=${r.minGap.toFixed(0)}px`);
  }
  // 渐进披露：只看前 N 层的实际观感（子图需重映射索引）
  for (const lv of [1, 2, 3, 4, 5]) {
    const keep = subgraph(G, lv);
    const keepArr = [...keep];
    const remap = new Map(keepArr.map((old, i) => [old, i]));
    const sub = {
      n: keepArr.length,
      ids: keepArr.map((i) => G.ids[i]),
      root: remap.get(G.root),
      children: keepArr.map((old) => G.children[old].filter((c) => keep.has(c)).map((c) => remap.get(c))),
      edges: G.edges.filter(([a, b]) => keep.has(a) && keep.has(b)).map(([a, b]) => [remap.get(a), remap.get(b)]),
    };
    const l2 = layeredLayout(sub);
    const zoom = Math.min((VW - 60) / l2.bbox.w, (VH - 120) / l2.bbox.h);
    console.log(`  展开 ${lv} 层: 可见 ${keep.size} 节点 (占 ${(100 * keep.size / G.n).toFixed(0)}%)  zoom=${zoom.toFixed(2)}  节点屏幕=${(26 * zoom).toFixed(1)}px  标签字号=${(11 * zoom).toFixed(1)}px`);
  }
}
