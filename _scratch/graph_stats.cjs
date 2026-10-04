// 图谱结构表征：读 .chain/nodes/*.md 的 frontmatter，输出布局相关统计。
// 用法：node _scratch/graph_stats.cjs "G:\\perf1500" ["D:\\TA"]
const fs = require('fs');
const path = require('path');

function parseFrontmatter(txt) {
  const m = txt.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return null;
  const out = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (!kv) continue;
    let v = kv[2].trim();
    if (v === 'null' || v === '') v = null;
    else if (v.startsWith('[') && v.endsWith(']')) {
      v = v.slice(1, -1).split(',').map((s) => s.trim().replace(/^["']|["']$/g, '')).filter(Boolean);
    } else v = v.replace(/^["']|["']$/g, '');
    out[kv[1]] = v;
  }
  return out;
}

function analyze(dir) {
  const nodesDir = path.join(dir, '.chain', 'nodes');
  if (!fs.existsSync(nodesDir)) return { dir, error: 'no .chain/nodes' };
  const files = fs.readdirSync(nodesDir).filter((f) => f.endsWith('.md'));
  const nodes = new Map();
  for (const f of files) {
    const fm = parseFrontmatter(fs.readFileSync(path.join(nodesDir, f), 'utf8'));
    if (!fm || !fm.id) continue;
    nodes.set(fm.id, fm);
  }
  const edges = [];
  const indeg = new Map(), outdeg = new Map();
  for (const [id, fm] of nodes) { indeg.set(id, 0); outdeg.set(id, 0); }
  for (const [id, fm] of nodes) {
    const p = fm.parent;
    if (p && p !== 'null' && nodes.has(p)) {
      edges.push({ parent: p, child: id, rel: fm.rel || 'contains' });
      outdeg.set(p, (outdeg.get(p) || 0) + 1);
      indeg.set(id, (indeg.get(id) || 0) + 1);
    }
  }
  const n = nodes.size, m = edges.length;
  const deg = new Map();
  for (const [id] of nodes) deg.set(id, (indeg.get(id) || 0) + (outdeg.get(id) || 0));

  // 连通分量（无向）+ 深度
  const adj = new Map();
  for (const [id] of nodes) adj.set(id, []);
  for (const e of edges) { adj.get(e.parent).push(e.child); adj.get(e.child).push(e.parent); }
  const seen = new Set(); const comps = [];
  for (const [id] of nodes) {
    if (seen.has(id)) continue;
    const members = []; let h = 0; const q = [id]; seen.add(id);
    const depth = new Map([[id, 0]]);
    while (h < q.length) {
      const cur = q[h++]; members.push(cur);
      for (const nb of adj.get(cur)) if (!seen.has(nb)) { seen.add(nb); depth.set(nb, depth.get(cur) + 1); q.push(nb); }
    }
    let maxd = 0; for (const d of depth.values()) if (d > maxd) maxd = d;
    comps.push({ members, maxDepth: maxd });
  }
  comps.sort((a, b) => b.members.length - a.members.length);

  // 度数分布
  const degs = [...deg.values()].sort((a, b) => b - a);
  const maxDeg = degs[0] || 0;
  const hist = {};
  for (const d of degs) hist[d] = (hist[d] || 0) + 1;
  const relCount = {};
  for (const e of edges) relCount[e.rel] = (relCount[e.rel] || 0) + 1;
  const multiParent = [...indeg.values()].filter((d) => d > 1).length;

  // 环检测（有向：沿 child->parent 走，是否回到自身）
  let cycles = 0;
  const state = new Map();
  for (const [id] of nodes) {
    if (state.get(id) === 2) continue;
    const stack = [id]; const local = new Set();
    let cur = id;
    while (cur !== null && cur !== undefined) {
      if (local.has(cur)) { cycles++; break; }
      if (state.get(cur) === 2) break;
      local.add(cur);
      const fm = nodes.get(cur);
      const p = fm && fm.parent && fm.parent !== 'null' && nodes.has(fm.parent) ? fm.parent : null;
      cur = p;
    }
    for (const x of local) state.set(x, 2);
  }

  return {
    dir, n, m, components: comps.length,
    largestComp: comps[0] ? comps[0].members.length : 0,
    secondComp: comps[1] ? comps[1].members.length : 0,
    singletonComponents: comps.filter((c) => c.members.length === 1).length,
    maxDepth: comps[0] ? comps[0].maxDepth : 0,
    maxDegree: maxDeg, avgDegree: +(2 * m / Math.max(n, 1)).toFixed(2),
    topDegrees: degs.slice(0, 12),
    degreeHistTop: Object.entries(hist).sort((a, b) => b[1] - a[1]).slice(0, 12),
    relCount, multiParentNodes: multiParent,
    multiParentEdges: edges.length - (n - comps.length),
    cycles,
    avgTitleLen: +( [...nodes.values()].reduce((s, f) => s + String(f.title || '').length, 0) / Math.max(n, 1)).toFixed(1),
  };
}

const dirs = process.argv.slice(2);
const out = dirs.map(analyze);
console.log(JSON.stringify(out, null, 2));
