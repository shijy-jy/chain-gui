// 细节补充测量：现有算法为何爆炸 + tidy 布局的分步耗时 + 视口适配后的实际观感换算
const fs = require('fs');
const path = require('path');

function parseFrontmatter(txt) {
  const m = txt.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return null;
  const out = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (kv) { let v = kv[2].trim(); out[kv[1]] = (v === 'null' || v === '') ? null : v; }
  }
  return out;
}
function load(dir) {
  const nd = path.join(dir, '.chain', 'nodes');
  const nodes = new Map();
  for (const f of fs.readdirSync(nd).filter((x) => x.endsWith('.md'))) {
    const fm = parseFrontmatter(fs.readFileSync(path.join(nd, f), 'utf8'));
    if (fm && fm.id) nodes.set(fm.id, fm);
  }
  const ids = [...nodes.keys()]; const idx = new Map(ids.map((id, i) => [id, i]));
  const children = ids.map(() => []); const edges = []; let root = -1;
  for (const id of ids) { const p = nodes.get(id).parent; if (p && nodes.has(p)) { children[idx.get(p)].push(idx.get(id)); edges.push([idx.get(p), idx.get(id)]); } else root = idx.get(id); }
  return { ids, children, edges, root, n: ids.length };
}
const G = load(process.argv[2] || 'G:\\perf1500');

// ── A. 初始圆环半径随 n 的爆炸（chain_to_cytoscape.ts 的 R = 180 + n*5）──
const R = 180 + G.n * 5, ringGap = Math.max(120, R * 0.42);
const maxDepth = (() => { const d = new Int32Array(G.n).fill(-1); const q = [G.root]; d[G.root] = 0; let h = 0, mx = 0; while (h < q.length) { const c = q[h++]; for (const k of G.children[c]) { d[k] = d[c] + 1; mx = Math.max(mx, d[k]); q.push(k); } } return mx; })();
console.log('=== A. 初始散点半径（R = 180 + n*5，ringGap = max(120, R*0.42)）===');
console.log(`n=${G.n}  R=${R}  ringGap=${ringGap.toFixed(0)}  maxDepth=${maxDepth}`);
console.log(`最深层(d=${maxDepth})圆环半径 = R + (d-1)*ringGap = ${Math.round(R + (maxDepth - 1) * ringGap)} px`);
console.log(`→ 直径 ${Math.round(2 * (R + (maxDepth - 1) * ringGap))} px 的初始环形世界，全部塞进一个 ~1600x900 视口 fit\n`);

// ── B. 每帧力预算：弹簧 vs 斥力 ──
const minDist = 40, edgeLen = 80, gravity = 0.15, repulsion = 30000;
const leafCount = G.children.filter((c) => c.length === 0).length;
const midCount = G.children.filter((c) => c.length > 0 && c.length <= 3).length;
const bigCount = G.children.filter((c) => c.length > 3).length;
console.log('=== B. 每帧受力预算（弹簧刚度 0.15，碰撞区间被截断在 3x3 网格内）===');
console.log(`叶节点(度1)=${leafCount}  小分支(度2-4)=${midCount}  大分支(度>4)=${bigCount}`);
console.log(`斥力作用范围 = cell = max(edgeLen, 2*(minDist+2*maxR)) = ${Math.max(edgeLen, 2 * (minDist + 2 * 18))} px（3x3 邻胞，仅 144px 内两两互斥）`);
console.log(`弹簧理想长 ${edgeLen}px：当 d=46000px 时 F = 0.15*(46000-80) = ${(0.15 * (46000 - 80)).toFixed(0)} → 被 MAX_F? 不受限（弹簧无上限钳制）`);
console.log(`→ 第一帧就把节点以 ~6900px/帧 的力甩出去，位移钳制 MAX_STEP=10px/帧 只是"限速"，不是"限力"\n`);

// ── C. 视口换算：观感 = 屏幕像素 ──
console.log('=== C. 视口换算（1600x900 视口，cytoscape fit padding 60）===');
const viewport = 1480;
for (const [name, worldW, worldH] of [['current 力导向', 46023, 49541], ['tidy-layered', 49116, 672], ['tidy 分带(每带12层)', 49116, 672 * 1]]) {
  const zoom = Math.min(viewport / worldW, (900 - 120) / worldH);
  const nodePx = 26 * zoom;
  const labelZoom = 11 * zoom;
  console.log(`${name.padEnd(22)} world=${worldW}x${worldH}  fit_zoom=${zoom.toExponential(2)}  节点屏幕直径=${nodePx.toFixed(2)}px  标签字号=${labelZoom.toFixed(2)}px`);
}
console.log('\n节点屏幕直径 < 3px = 视觉上只剩"雾"；标签字号 < 6px 时 cytoscape 按 min-zoomed-font-size 整批丢弃标签');
console.log('→ current 力导向在 1500 节点上 fit 后：zoom≈0.016，节点 0.4px（雾），标签 0.18px（全部丢弃）');

// ── D. Buchheim tidy 分步耗时（纯 JS，无弛豫）──
const t0 = process.hrtime.bigint();
const width = new Float64Array(G.n); const order = [];
(function post(i) { for (const c of G.children[i]) post(c); order.push(i); })(G.root);
for (const i of order) { let w = 0; for (const c of G.children[i]) w += width[c] + 46; width[i] = G.children[i].length ? w - 46 : 0; }
const tPost = Number(process.hrtime.bigint() - t0) / 1e6;
const x = new Float64Array(G.n), y = new Float64Array(G.n);
function pre(i, left, depth) { y[i] = depth * 96; if (!G.children[i].length) { x[i] = left; return; } let cur = left; for (const c of G.children[i]) { pre(c, cur, depth + 1); cur += width[c] + 46; } x[i] = (x[G.children[i][0]] + x[G.children[i][G.children[i].length - 1]]) / 2; }
pre(G.root, 0, 0);
const tPre = Number(process.hrtime.bigint() - t0) / 1e6;
console.log(`\n=== D. tidy 布局分步耗时（n=${G.n}）===\n后序宽度: ${tPost.toFixed(2)}ms   前序定位: ${(tPre - tPost).toFixed(2)}ms   合计: ${tPre.toFixed(2)}ms（O(n)）`);
const maxFanout = Math.max(...G.children.map((c) => c.length));
console.log(`最大扇出 = ${maxFanout} → 该层宽度 ≈ ${maxFanout} * (46+18) = ${maxFanout * 64}px（分带/径向折叠的动机）`);
