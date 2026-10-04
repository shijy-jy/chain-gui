// 生成布局对比 SVG：现有算法 vs 树感知布局，同数据同视口
// 用法：node _scratch/make_layout_svg.cjs "G:\perf1500" > _shots/layout-compare.svg
const fs = require('fs'); const path = require('path');
function pf(t) { const m = t.match(/^---\r?\n([\s\S]*?)\r?\n---/); if (!m) return null; const o = {}; for (const l of m[1].split(/\r?\n/)) { const kv = l.match(/^([A-Za-z_][\w-]*):\s*(.*)$/); if (kv) { const v = kv[2].trim(); o[kv[1]] = (v === 'null' || v === '') ? null : v; } } return o; }
function load(dir) {
  const nd = path.join(dir, '.chain', 'nodes'); const nodes = new Map();
  for (const f of fs.readdirSync(nd).filter((x) => x.endsWith('.md'))) { const fm = pf(fs.readFileSync(path.join(nd, f), 'utf8')); if (fm && fm.id) nodes.set(fm.id, fm); }
  const ids = [...nodes.keys()], idx = new Map(ids.map((id, i) => [id, i]));
  const children = ids.map(() => []), edges = []; let root = -1;
  for (const id of ids) { const p = nodes.get(id).parent; if (p && nodes.has(p)) { children[idx.get(p)].push(idx.get(id)); edges.push([idx.get(p), idx.get(id)]); } else root = idx.get(id); }
  return { ids, children, edges, root, n: ids.length, titles: ids.map((i) => nodes.get(i).title) };
}
const G = load(process.argv[2] || 'G:\\perf1500');

// ── 现有算法（复刻）──
function current(G, maxIter = 80) {
  const minDist = 40, n = G.n, edges = G.edges;
  const repulsion = 30000, edgeLen = 80, gravity = 0.15;
  const R = 180 + n * 5, ringGap = Math.max(120, R * 0.42);
  const pos = new Array(n); const adj = Array.from({ length: n }, () => []);
  for (const [a, b] of edges) { adj[a].push(b); adj[b].push(a); }
  const depth = new Int32Array(n).fill(-1); const q = [G.root]; depth[G.root] = 0; let h = 0;
  while (h < q.length) { const c = q[h++]; for (const nb of adj[c]) if (depth[nb] < 0) { depth[nb] = depth[c] + 1; q.push(nb); } }
  const byDepth = new Map(); for (let i = 0; i < n; i++) { const d = depth[i]; if (!byDepth.has(d)) byDepth.set(d, []); byDepth.get(d).push(i); }
  for (const [d, ids] of [...byDepth.entries()].sort((a, b) => a[0] - b[0])) {
    const radius = d === 0 ? 0 : R + (d - 1) * ringGap;
    ids.forEach((id, k) => { const ang = (k / ids.length) * Math.PI * 2 - Math.PI / 2; pos[id] = { x: Math.cos(ang) * radius, y: Math.sin(ang) * radius }; });
  }
  const radii = new Array(n); for (let i = 0; i < n; i++) radii[i] = (14 + Math.min(Math.sqrt(adj[i].length), 6) * 4) / 2;
  const cell = Math.max(edgeLen, 2 * (minDist + 2 * 18));
  const vx = new Float64Array(n), vy = new Float64Array(n);
  const gk = (x, y) => x * 100003 + y;
  let alpha = 1;
  for (let it = 0; it < maxIter; it++) {
    const cells = new Map();
    for (let i = 0; i < n; i++) { const k = gk(Math.floor(pos[i].x / cell), Math.floor(pos[i].y / cell)); if (!cells.has(k)) cells.set(k, []); cells.get(k).push(i); }
    for (let i = 0; i < n; i++) {
      const cx = Math.floor(pos[i].x / cell), cy = Math.floor(pos[i].y / cell);
      for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) for (const j of cells.get(gk(cx + gx, cy + gy)) || []) {
        if (j <= i) continue;
        let dx = pos[j].x - pos[i].x, dy = pos[j].y - pos[i].y, d2 = dx * dx + dy * dy;
        const req = minDist + radii[i] + radii[j];
        if (d2 < req * req) { let d = Math.sqrt(d2) || 1; const push = ((req - d) / d) * 0.5; pos[i].x -= dx * push; pos[i].y -= dy * push; pos[j].x += dx * push; pos[j].y += dy * push; }
        if (d2 < 4) { d2 = 4; dx = (Math.random() - 0.5) * 4; dy = (Math.random() - 0.5) * 4; }
        const d = Math.sqrt(d2), f = Math.min((repulsion / d2) * alpha, 60);
        vx[i] -= (dx / d) * f; vy[i] -= (dy / d) * f; vx[j] += (dx / d) * f; vy[j] += (dy / d) * f;
      }
    }
    for (const [si, ti] of edges) { const dx = pos[ti].x - pos[si].x, dy = pos[ti].y - pos[si].y, d = Math.max(Math.hypot(dx, dy), 1), f = gravity * (d - edgeLen) * alpha; vx[si] += (dx / d) * f; vy[si] += (dy / d) * f; vx[ti] -= (dx / d) * f; vy[ti] -= (dy / d) * f; }
    const gc = 0.05 * alpha;
    for (let i = 0; i < n; i++) { vx[i] -= pos[i].x * gc; vy[i] -= pos[i].y * gc; vx[i] *= 0.86; vy[i] *= 0.86;
      let sx = vx[i], sy = vy[i]; const sp = Math.hypot(sx, sy); if (sp > 10) { sx = sx / sp * 10; sy = sy / sp * 10; } pos[i].x += sx; pos[i].y += sy; }
    alpha *= 0.97;
  }
  return pos;
}
// ── 径向树布局 ──
function radial(G, layerGap = 96) {
  const n = G.n, ang = new Float64Array(n), depth = new Int32Array(n), leaves = new Int32Array(n);
  (function post(i, d) { depth[i] = d; for (const c of G.children[i]) post(c, d + 1); leaves[i] = G.children[i].length ? G.children[i].reduce((s, c) => s + leaves[c], 0) : 1; })(G.root, 0);
  (function spread(i, a0, a1) { const ch = G.children[i]; if (!ch.length) return; const tot = ch.reduce((s, c) => s + leaves[c], 0); let a = a0; for (const c of ch) { const span = (a1 - a0) * leaves[c] / tot; ang[c] = a + span / 2; spread(c, a, a + span); a += span; } })(G.root, 0, Math.PI * 2);
  const pos = []; for (let i = 0; i < n; i++) { const r = depth[i] * layerGap; pos.push({ x: Math.cos(ang[i]) * r, y: Math.sin(ang[i]) * r }); }
  return pos;
}
function bbox(pos) { let a = 1e18, b = -1e18, c = 1e18, d = -1e18; for (const p of pos) { a = Math.min(a, p.x); b = Math.max(b, p.x); c = Math.min(c, p.y); d = Math.max(d, p.y); } return { x: a, y: c, w: b - a, h: d - c }; }

const cur = current(G), rad = radial(G);
// 每个图的深度（配色用）
function computeDepth(gr) { const d = new Int32Array(gr.n).fill(-1); d[gr.root] = 0; const q = [gr.root]; let h = 0; while (h < q.length) { const c = q[h++]; for (const k of gr.children[c]) { d[k] = d[c] + 1; q.push(k); } } return d; }
G.depth = computeDepth(G);
const types = ['goal', 'design', 'task', 'verification', 'note'];
const colors = { goal: '#a78bfa', design: '#60a5fa', task: '#22d3ee', verification: '#34d399', note: '#94a3b8' };

function panel(gr, pos, x, y, w, h, title, sub, opt = {}) {
  const bb = bbox(pos);
  const s = Math.min(w / bb.w, h / bb.h) * 0.94;
  const cx = bb.x + bb.w / 2, cy = bb.y + bb.h / 2;
  const tx = (p) => x + w / 2 + (p.x - cx) * s;
  const ty = (p) => y + h / 2 + (p.y - cy) * s;
  const r = Math.max(0.35, 13 * s);
  const edgeW = Math.max(0.12, 1.2 * s);
  const parts = [];
  parts.push(`<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="#0d1117" stroke="#21262d"/>`);
  const es = gr.edges.map(([a, b]) => `<line x1="${tx(pos[a]).toFixed(2)}" y1="${ty(pos[a]).toFixed(2)}" x2="${tx(pos[b]).toFixed(2)}" y2="${ty(pos[b]).toFixed(2)}"/>`).join('');
  parts.push(`<g stroke="#58a6ff" stroke-opacity="${opt.edgeOpacity || 0.35}" stroke-width="${edgeW.toFixed(3)}">${es}</g>`);
  const nodes = [];
  for (let i = 0; i < gr.n; i++) {
    const t = typeOf(gr, i);
    nodes.push(`<circle cx="${tx(pos[i]).toFixed(2)}" cy="${ty(pos[i]).toFixed(2)}" r="${r.toFixed(2)}" fill="${colors[t]}" fill-opacity="0.85"/>`);
  }
  parts.push(`<g>${nodes.join('')}</g>`);
  parts.push(`<text x="${x + 14}" y="${y + 26}" fill="#e6edf3" font-family="Segoe UI,system-ui,sans-serif" font-size="17" font-weight="600">${title}</text>`);
  parts.push(`<text x="${x + 14}" y="${y + 48}" fill="#8b949e" font-family="Segoe UI,system-ui,sans-serif" font-size="12.5">${sub}</text>`);
  return parts.join('');
}
// 类型：按深度启发式（根=goal，浅层 design/task，叶=note）——仅用于配色区分
function typeOf(gr, i) { const d = gr.depth[i]; if (d === 0) return 'goal'; if (d === 1) return 'design'; if (gr.children[i].length === 0) return 'note'; return d <= 3 ? 'task' : 'verification'; }

// 只画"前 2 层"的对照（渐进披露视角）
const keep = new Set(); (function w(i, d) { if (d > 2) return; keep.add(i); for (const c of G.children[i]) w(c, d + 1); })(G.root, 0);
const subIdx = new Map([...keep].map((o, i) => [o, i]));
const subG = {
  n: keep.size, ids: [...keep].map((i) => G.ids[i]), root: subIdx.get(G.root),
  children: [...keep].map((o) => G.children[o].filter((c) => keep.has(c)).map((c) => subIdx.get(c))),
  edges: G.edges.filter(([a, b]) => keep.has(a) && keep.has(b)).map(([a, b]) => [subIdx.get(a), subIdx.get(b)]),
};
const curSub = current(subG, 400), radSub = radial(subG);
subG.depth = computeDepth(subG);

const W = 1680, H = 1040, PAD = 16, PW = (W - PAD * 3) / 2, PH = (H - 56 - PAD * 3) / 2;
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<rect width="${W}" height="${H}" fill="#010409"/>
<text x="${PAD + 4}" y="30" fill="#e6edf3" font-family="Segoe UI,system-ui,sans-serif" font-size="21" font-weight="700">Engram 节点布局对比 · ${G.n} 节点真实基准（G:\\perf1500）</text>
<text x="${PAD + 4}" y="50" fill="#8b949e" font-family="Segoe UI,system-ui,sans-serif" font-size="13">同样数据、同样视口比例；上排=现有环形散点+力导向，下排=树感知径向布局</text>
${panel(G, cur, PAD, 62, PW, PH, '现有算法 · 全图 1500 节点', 'fit 后节点仅 0.41px → 用户看到的就是这团"雾"；361 997 次边交叉；80 帧后仍以 26 521px/帧漂移', { edgeOpacity: 0.3 })}
${panel(G, rad, PAD * 2 + PW, 62, PW, PH, '径向树布局 · 全图 1500 节点', '包围盒 1056×1132（现有 46023×49541）；0 次边交叉；1.4ms 算完；fit 后节点 17.9px', { edgeOpacity: 0.4 })}
${panel(subG, curSub, PAD, 62 + PH + PAD, PW, PH, '现有算法 · 只展开 2 层（44 节点）', '小图上也在漂移：末帧残余 200–300px/帧；80 节点真实图有 206 次交叉、48 对间距违例', { edgeOpacity: 0.45 })}
${panel(subG, radSub, PAD * 2 + PW, 62 + PH + PAD, PW, PH, '径向树布局 · 只展开 2 层（44 节点）', '0 交叉、0 间距违例、完全静止；标签字号 10.2px 可读（现有 0.17px 被整批丢弃）', { edgeOpacity: 0.5 })}
</svg>`;
process.stdout.write(svg);
