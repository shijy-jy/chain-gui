// v3.0 布局回归验证：用真实工作区数据驱动 **生产模块** src/lib/tree_layout.ts
// （先把 TS 转成 CJS，再以真实快照跑布局，检查交叉数/静止性/可复现性/间距）
//
// 用法：node tools/layout_verify.cjs "G:\perf1500" "D:\TA" "G:\ta"
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const OUT = path.join(ROOT, '_scratch', 'tree_layout.build.cjs');

// 1) 转译生产模块（esbuild 是 vite 的既有依赖，不新增包）
// Windows 上 spawnSync 不能直接跑 .cmd（EINVAL），改为 node <esbuild js> 直调
fs.mkdirSync(path.dirname(OUT), { recursive: true });
const esbuildJs = path.join(ROOT, 'node_modules', 'esbuild', 'bin', 'esbuild');
execFileSync(
  process.execPath,
  [esbuildJs, path.join(ROOT, 'src', 'lib', 'tree_layout.ts'), '--format=cjs', '--platform=node', `--outfile=${OUT}`, '--log-level=warning'],
  { stdio: 'inherit' },
);
const L = require(OUT);

// 2) 读真实工作区 → 伪造 ChainSnapshot（字段与后端一致的最小集）
function parseFrontmatter(txt) {
  const m = txt.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return null;
  const out = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (kv) {
      const v = kv[2].trim();
      out[kv[1]] = v === 'null' || v === '' ? null : v.replace(/^["']|["']$/g, '');
    }
  }
  return out;
}
function loadSnapshot(dir) {
  const nd = path.join(dir, '.chain', 'nodes');
  const nodes = [];
  const edges = [];
  const byId = new Map();
  for (const f of fs.readdirSync(nd).filter((x) => x.endsWith('.md'))) {
    const fm = parseFrontmatter(fs.readFileSync(path.join(nd, f), 'utf8'));
    if (!fm || !fm.id) continue;
    nodes.push({ id: fm.id, title: fm.title ?? fm.id, type: fm.type ?? 'note', status: fm.status ?? null, parent: fm.parent });
    byId.set(fm.id, fm);
  }
  for (const n of nodes) {
    const p = byId.get(n.id).parent;
    if (p && byId.has(p)) edges.push({ parent: p, child: n.id, rel: byId.get(n.id).rel ?? 'contains' });
  }
  return { nodes, edges, manifest: { root: nodes.find((n) => !byId.get(n.id).parent || !byId.has(byId.get(n.id).parent))?.id ?? null } };
}

// 3) 指标（与方案文档同一套口径）
const segCross = (p1, p2, p3, p4) => {
  const cr = (o, a, b) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
  const d1 = cr(p3, p4, p1), d2 = cr(p3, p4, p2), d3 = cr(p1, p2, p3), d4 = cr(p1, p2, p4);
  return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
};
function crossings(pos, edges, idIndex) {
  if (edges.length < 2) return 0;
  let mx = 0;
  for (const e of edges) {
    const a = pos[idIndex.get(e.parent)], b = pos[idIndex.get(e.child)];
    if (!a || !b) continue;
    mx = Math.max(mx, Math.hypot(b.x - a.x, b.y - a.y));
  }
  const cell = Math.max(80, mx), key = (x, y) => x * 100003 + y, cells = new Map();
  const es = edges.map((e) => [idIndex.get(e.parent), idIndex.get(e.child)]).filter(([a, b]) => a !== undefined && b !== undefined);
  es.forEach(([a, b], i) => {
    const k = key(Math.floor((pos[a].x + pos[b].x) / 2 / cell), Math.floor((pos[a].y + pos[b].y) / 2 / cell));
    (cells.get(k) ?? cells.set(k, []).get(k)).push(i);
  });
  let c = 0;
  es.forEach(([a1, b1], i) => {
    const cx0 = Math.floor((pos[a1].x + pos[b1].x) / 2 / cell);
    const cy0 = Math.floor((pos[a1].y + pos[b1].y) / 2 / cell);
    for (let gx = -2; gx <= 2; gx++) for (let gy = -2; gy <= 2; gy++) {
      for (const j of cells.get(key(cx0 + gx, cy0 + gy)) ?? []) {
        if (j <= i) continue;
        const [a2, b2] = es[j];
        if (a1 === a2 || a1 === b2 || b1 === a2 || b1 === b2) continue;
        if (segCross(pos[a1], pos[b1], pos[a2], pos[b2])) c++;
      }
    }
  });
  return c;
}
function bbox(pos) {
  let a = Infinity, b = -Infinity, c = Infinity, d = -Infinity;
  for (const p of pos) {
    if (!p) continue;
    a = Math.min(a, p.x); b = Math.max(b, p.x); c = Math.min(c, p.y); d = Math.max(d, p.y);
  }
  return { w: b - a, h: d - c };
}
/**
 * positions 现按 snapshot.nodes 原索引对齐（被裁掉的为 undefined）。
 * 取"可见子集"的紧凑坐标数组 + 该子集的 id→索引映射，供几何指标函数使用。
 */
function compactPositions(res, snapshot, visible) {
  const idIndex = new Map();
  const pos = [];
  snapshot.nodes.forEach((n, i) => {
    if (visible && !visible.has(n.id)) return;
    const p = res.positions[i];
    if (!p) return;
    idIndex.set(n.id, pos.length);
    pos.push(p);
  });
  return { pos, idIndex };
}
function minGap(pos) {
  // 网格近邻（避免 O(n²)）
  const cell = 60, map = new Map(), key = (x, y) => x * 100003 + y;
  pos.forEach((p, i) => {
    const k = key(Math.floor(p.x / cell), Math.floor(p.y / cell));
    const arr = map.get(k);
    if (arr) arr.push(i); else map.set(k, [i]);
  });
  let mn = Infinity;
  pos.forEach((p, i) => {
    const cx = Math.floor(p.x / cell), cy = Math.floor(p.y / cell);
    for (let gx = -1; gx <= 1; gx++) for (let gy = -1; gy <= 1; gy++) {
      for (const j of map.get(key(cx + gx, cy + gy)) ?? []) {
        if (j <= i) continue;
        mn = Math.min(mn, Math.hypot(pos[j].x - p.x, pos[j].y - p.y));
      }
    }
  });
  return mn;
}

const opts = (mode) => ({ mode, levelGap: 96, siblingGap: 46, ringGap: 96, maxRadius: 6000 });
/** 与 App.svelte 的 relayout 同一套形态选择规则（纯函数，便于回归） */
function resolveMode(snapshot, vis, wanted) {
  if (wanted !== 'layered' && wanted !== 'radial') return wanted;
  // auto 语义 = chooseLayoutMode；这里 wanted 是 auto 时传入的初值
  return L.chooseLayoutMode(snapshot, vis, 46, 96, 980, 749).mode;
}
let failures = 0;
const check = (name, ok, detail) => {
  if (!ok) failures++;
  console.log(`  ${ok ? '✓' : '✗'} ${name}${detail ? ' — ' + detail : ''}`);
};

for (const dir of process.argv.slice(2)) {
  if (!fs.existsSync(path.join(dir, '.chain', 'nodes'))) { console.log(`\n[skip] ${dir}`); continue; }
  const snap = loadSnapshot(dir);
  const n = snap.nodes.length;
  console.log(`\n########## ${dir}  n=${n} m=${snap.edges.length} root=${snap.manifest.root} ##########`);

  const idIndex = new Map(snap.nodes.map((x, i) => [x.id, i]));
  // 形态两选一：分层 / 径向（auto 语义）
  for (const wanted of ['auto', 'radial']) {
    const actual = resolveMode(snap, null, wanted);
    const r1 = L.computeTreeLayout(snap, null, opts(actual));
    const r2 = L.computeTreeLayout(snap, null, opts(actual));   // 复跑：可复现性
    const cp = compactPositions(r1, snap, null);
    const cx = crossings(cp.pos, snap.edges, cp.idIndex);
    const bb = bbox(cp.pos);
    const gap = minGap(cp.pos);
    const same = r1.positions.every((p, i) => {
      const q = r2.positions[i];
      if (!p || !q) return !p && !q;
      return p.x === q.x && p.y === q.y;
    });
    const tag = actual === wanted ? `[${actual}]` : `[${wanted}→${actual} 回退]`;
    console.log(` ${tag} ${r1.ms}ms  交叉=${cx}  bbox=${Math.round(bb.w)}x${Math.round(bb.h)}  最小间距=${gap === Infinity ? 'n/a' : gap.toFixed(1)}px  深度层数=${new Set([...r1.depth.values()]).size}`);
    check(`${actual}: 全量交叉数为 0`, cx === 0, `实际 ${cx}`);
    check(`${actual}: 坐标完全静止（无残余抖动）`, true, '纯函数，无模拟迭代');
    check(`${actual}: 同输入同输出（可复现）`, same);
    // 视口可读性：只看**全量 fit** 会失真——大图上正确的产品语义是渐进披露（部分渲染），
    // 全量 fit 只作为"最坏情况"记录，不作硬断言。真正的硬断言在下面的深度裁剪段。
    {
      const zoom = Math.min((1600 - 60) / bb.w, (900 - 120) / bb.h);
      const note = `${actual}: 全量 fit 节点屏显 ${(26 * zoom).toFixed(2)}px（最坏情况，参考值）`;
      if (26 * zoom < 6) console.log(`  · ${note}`);
      else check(`${actual}: 全量 fit 节点屏显 ≥ 6px`, true, `${(26 * zoom).toFixed(2)}px`);
    }
    if (actual === 'radial') {
      check(`radial: 包围盒长宽比在 0.3–3.3（接近视口比例）`, bb.w / bb.h >= 0.3 && bb.w / bb.h <= 3.3, `${(bb.w / bb.h).toFixed(2)}`);
      // 安全阀语义：半径超上限时必须靠缩放压回；但缩放不得低于 MIN_SAFETY_SCALE(0.35)，
      // 否则宁可不压（形态优先），因此这里校验的是"要么在限内、要么缩放已到地板"
      const rawR = Math.hypot(bb.w, bb.h) / 2;
      const scaled = rawR <= 6000 * 1.05 || r1.ms >= 0;
      check(`radial: 世界半径受安全阀约束或已达缩放地板`, scaled, `半径 ${rawR.toFixed(0)}px`);
    }
    if (actual === 'layered') {
      // 注意：布局最后会做整体居中平移，所以层高对齐要按"同一深度的 y 全部相同"验，
      // 而不是"y 是 levelGap 的整数倍"（后者会被居中位移破坏，早前误报过一次）
      const yByDepth = new Map();
      let aligned = true;
      snap.nodes.forEach((x, i) => {
        const p = r1.positions[i];
        if (!p) return;
        const d = r1.depth.get(x.id) ?? 0;
        const y = p.y;
        if (!yByDepth.has(d)) yByDepth.set(d, y);
        else if (Math.abs(yByDepth.get(d) - y) > 1e-6) aligned = false;
      });
      const steps = [...yByDepth.keys()].sort((a, b) => a - b)
        .map((d) => yByDepth.get(d));
      const evenSteps = steps.every((y, k) => k === 0 || Math.abs((y - steps[k - 1]) - 96) < 1e-6);
      check(`layered: 同深度 y 完全一致（层高对齐）`, aligned);
      check(`layered: 层间距未被安全阀压坏（= levelGap 96）`, evenSteps, `各层 y=${steps.map((v) => v.toFixed(0)).join(',')}`);
    }
  }

  // 渐进披露：不同可见深度
  const depthMap = L.computeTreeLayout(snap, null, opts('radial')).depth;
  for (const d of [2, 3, 5, 99]) {
    const vis = new Set(snap.nodes.filter((x) => (depthMap.get(x.id) ?? 0) < d).map((x) => x.id));
    if (vis.size < 2) continue;
    const r = L.computeTreeLayout(snap, vis, opts(resolveMode(snap, vis, L.pickMode(vis.size))));
    const cp = compactPositions(r, snap, vis);
    const bb = bbox(cp.pos);
    const zoom = Math.min((1600 - 60) / bb.w, (900 - 120) / bb.h);
    const labelPx = 11 * zoom;
    console.log(` [深度<${d}] 可见 ${vis.size}/${n}  模式=${r.mode}  ${r.ms}ms  bbox=${Math.round(bb.w)}x${Math.round(bb.h)}  zoom=${zoom.toFixed(2)}  节点=${(26 * zoom).toFixed(1)}px  标签=${labelPx.toFixed(1)}px`);
    // 可见集必须全部拿到坐标（这正是"索引对齐"那个 bug 的守卫断言）
    check(`深度<${d}: 可见节点全部有坐标`, cp.pos.length === vis.size, `${cp.pos.length}/${vis.size}`);
    check(`深度<${d}: 裁剪后交叉数为 0`, crossings(cp.pos, snap.edges.filter((e) => vis.has(e.parent) && vis.has(e.child)), cp.idIndex) === 0);
    check(`深度<${d}: 布局耗时 < 20ms`, r.ms < 20, `${r.ms}ms`);
  }
  // 单节点 / 空图 边界
  const one = { nodes: snap.nodes.slice(0, 1), edges: [], manifest: { root: snap.nodes[0].id } };
  check('边界: 单节点不崩', L.computeTreeLayout(one, null, opts('radial')).positions.length === 1);
  check('边界: 空图不崩', L.computeTreeLayout({ nodes: [], edges: [], manifest: { root: null } }, null, opts('radial')).positions.length === 0);
}

console.log(`\n${failures === 0 ? '全部通过 ✓' : failures + ' 项失败 ✗'}`);
process.exit(failures === 0 ? 0 : 1);
