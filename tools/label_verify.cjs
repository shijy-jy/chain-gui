// 标签布局回归：用真实工作区数据验证"标签不压字、不盖节点"的约束
//
// 用法：node tools/label_verify.cjs "G:\perf1500" "D:\TA" "G:\ta"
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const OUT = path.join(ROOT, '_scratch', 'label_layout.build.cjs');
fs.mkdirSync(path.dirname(OUT), { recursive: true });
execFileSync(process.execPath, [
  path.join(ROOT, 'node_modules', 'esbuild', 'bin', 'esbuild'),
  path.join(ROOT, 'src', 'lib', 'label_layout.ts'),
  '--format=cjs', '--platform=node', `--outfile=${OUT}`, '--log-level=warning',
], { stdio: 'inherit' });
const L = require(OUT);
const TREE = path.join(ROOT, '_scratch', 'tree_layout.build.cjs');
if (!fs.existsSync(TREE)) {
  execFileSync(process.execPath, [
    path.join(ROOT, 'node_modules', 'esbuild', 'bin', 'esbuild'),
    path.join(ROOT, 'src', 'lib', 'tree_layout.ts'),
    '--format=cjs', '--platform=node', `--outfile=${TREE}`, '--log-level=warning',
  ], { stdio: 'inherit' });
}
const T = require(TREE);

function pf(t) {
  const m = t.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return null;
  const o = {};
  for (const l of m[1].split(/\r?\n/)) {
    const kv = l.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (kv) { const v = kv[2].trim(); o[kv[1]] = (v === 'null' || v === '') ? null : v.replace(/^["']|["']$/g, ''); }
  }
  return o;
}
function loadSnapshot(dir) {
  const nd = path.join(dir, '.chain', 'nodes');
  const nodes = [], edges = [], byId = new Map();
  for (const f of fs.readdirSync(nd).filter((x) => x.endsWith('.md'))) {
    const fm = pf(fs.readFileSync(path.join(nd, f), 'utf8'));
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

const TYPE_LABEL = { goal: '目标', design: '设计', task: '任务', verification: '验证', note: '' };
function labelOf(n) {
  const pre = TYPE_LABEL[n.type] ?? '';
  return pre ? `${pre} · ${n.title}` : n.title;
}

let failures = 0;
const check = (name, ok, detail) => {
  if (!ok) failures++;
  console.log(`  ${ok ? '✓' : '✗'} ${name}${detail ? ' — ' + detail : ''}`);
};

const VW = 1600, VH = 900;

for (const dir of process.argv.slice(2)) {
  if (!fs.existsSync(path.join(dir, '.chain', 'nodes'))) { console.log(`\n[skip] ${dir}`); continue; }
  const snap = loadSnapshot(dir);
  const n = snap.nodes.length;
  console.log(`\n########## ${dir}  n=${n} ##########`);

  const depthMap = T.computeTreeLayout(snap, null, { mode: 'radial', levelGap: 96, siblingGap: 46, ringGap: 96, maxRadius: 0 }).depth;
  // 度（可视度）：优先级用
  const deg = new Map(snap.nodes.map((x) => [x.id, 0]));
  for (const e of snap.edges) { deg.set(e.parent, (deg.get(e.parent) ?? 0) + 1); deg.set(e.child, (deg.get(e.child) ?? 0) + 1); }

  for (const visDepth of [3, 99]) {
    const vis = visDepth >= 99
      ? null
      : new Set(snap.nodes.filter((x) => (depthMap.get(x.id) ?? 0) < visDepth).map((x) => x.id));
    const visibleNodes = snap.nodes.filter((x) => !vis || vis.has(x.id));
    if (visibleNodes.length < 2) continue;

    const layout = T.computeTreeLayout(snap, vis, {
      mode: T.pickMode(visibleNodes.length), levelGap: 96, siblingGap: 46, ringGap: 96, maxRadius: 6000,
    });
    // 世界包围盒 → 与 App 一样 fit 到视口
    let mnx = Infinity, mxx = -Infinity, mny = Infinity, mxy = -Infinity;
    snap.nodes.forEach((x, i) => {
      const p = layout.positions[i];
      if (!p) return;
      mnx = Math.min(mnx, p.x); mxx = Math.max(mxx, p.x); mny = Math.min(mny, p.y); mxy = Math.max(mxy, p.y);
    });
    // 取景：必须与 App.svelte 的 fitVisible 同口径 ——
    // 用"与标签无关"的包围盒（节点位置+尺寸）并加 34px 余量，再按视图留 60px padding 算 zoom。
    // 早前只按"中心点包围盒"算，zoom 偏大、边缘节点落到视口外，误报成"标签全被筛掉"。
    const INFLATE = 34, PAD = 60;
    const nodesBB = { w: mxx - mnx, h: mxy - mny };
    const zoom = Math.min(
      (VW - PAD * 2) / (nodesBB.w + INFLATE * 2),
      (VH - PAD * 2) / (nodesBB.h + INFLATE * 2),
    );
    if (!Number.isFinite(zoom) || zoom <= 0) {
      console.log(`  [诊断] 取景异常：bbox=${nodesBB.w}x${nodesBB.h} zoom=${zoom} 可见=${visibleNodes.length}`);
      continue;
    }
    // 世界中心（布局已居中于原点，这里以实际包围盒中心为准，与 App 的 pan 一致）
    const wcx = (mnx + mxx) / 2, wcy = (mny + mxy) / 2;

    const candidates = [];
    snap.nodes.forEach((x, i) => {
      const p = layout.positions[i];
      const sz = layout.sizes[i];
      if (!p || sz === undefined) return;
      // 屏幕坐标 = (世界 − 图心) × zoom + 视口中心（与 App 的 pan 公式等价）
      const cx = (p.x - wcx) * zoom + VW / 2;
      const cy = (p.y - wcy) * zoom + VH / 2;
      candidates.push({
        id: x.id, x: cx / zoom, y: cy / zoom,
        size: sz, text: L.fitLabelText(labelOf(x), L.DEFAULT_LABEL_METRICS, L.DEFAULT_LABEL_METRICS.fontSize),
        degree: deg.get(x.id) ?? 0, depth: depthMap.get(x.id) ?? 0,
      });
    });
    if (process.env.LABEL_DEBUG) {
      const xs = candidates.map((c) => c.x * zoom);
      const ys = candidates.map((c) => c.y * zoom);
      const c0 = candidates[0];
      console.log(`  [诊断] 世界 bbox=${(mxx - mnx).toFixed(0)}x${(mxy - mny).toFixed(0)} 世界中心=(${((mnx + mxx) / 2).toFixed(1)}, ${((mny + mxy) / 2).toFixed(1)}) zoom=${zoom.toFixed(4)}`);
      console.log(`  [诊断] 候选屏幕范围 x∈[${Math.min(...xs).toFixed(0)},${Math.max(...xs).toFixed(0)}] y∈[${Math.min(...ys).toFixed(0)},${Math.max(...ys).toFixed(0)}]（应覆盖 [0,${VW}]x[0,${VH}]）`);
      console.log(`  [诊断] 候选0=${c0.id.slice(0, 10)} 屏幕=(${(c0.x * zoom).toFixed(1)}, ${(c0.y * zoom).toFixed(1)}) 世界=(${c0.x.toFixed(1)}, ${c0.y.toFixed(1)})`);
    }

    for (const budget of [40, 120, 0]) {
      const res = L.selectVisibleLabels(candidates, {
        zoom, viewportW: VW, viewportH: VH, maxLabels: budget, maxScreenAreaRatio: 0.14,
      });
      const s = res.stats;
      const tag = `[可见深度 ${visDepth >= 99 ? '全部' : visDepth} · zoom=${zoom.toFixed(2)} · 预算=${budget || '不限'}]`;
      console.log(`${tag} 候选=${s.candidates} 进视口=${s.inViewport} 显示=${s.accepted} 因盖节点拒=${s.rejectedNodeCover} 因压字拒=${s.rejectedLabelOverlap} 超预算拒=${s.rejectedBudget} 视口外=${s.rejectedOffscreen} 覆盖率=${(s.coverage * 100).toFixed(1)}%`);
      check(`${tag} 覆盖率 ≤ 15%`, s.coverage <= 0.15, `${(s.coverage * 100).toFixed(1)}%`);
      check(`${tag} 预算生效（若设置了上限）`, budget === 0 || s.accepted <= budget, `${s.accepted}/${budget || '∞'}`);
      check(`${tag} 至少显示 1 个标签`, s.accepted >= 1, `候选=${s.candidates} 进视口=${s.inViewport}`);
      check(`${tag} 显示数 + 各类拒绝 = 候选数`, s.accepted + s.rejectedNodeCover + s.rejectedLabelOverlap + s.rejectedBudget + s.rejectedOffscreen === s.candidates,
        `${s.accepted}+${s.rejectedNodeCover}+${s.rejectedLabelOverlap}+${s.rejectedBudget}+${s.rejectedOffscreen} vs ${s.candidates}`);
    }
  }
}

// 单元级：几何估算与截断
console.log('\n### 几何估算');
const M = L.DEFAULT_LABEL_METRICS;
const typeless = L.estimateLabelBox('短标题', M, M.fontSize);   // 纯标题（note 类型不带前缀）
const short = L.estimateLabelBox('任务 · 短标题', M, M.fontSize);
const long = L.estimateLabelBox('任务 · 这是一个非常长的标题会被截断处理掉多余部分', M, M.fontSize);
check('纯标题单行（高度 ≤ 20px 屏幕）', typeless.h <= 20, `${typeless.h.toFixed(1)}px，${typeless.lines} 行`);
check('带类型前缀的两行以内', short.lines <= M.maxLines, `${short.lines} 行 / ${short.h.toFixed(1)}px`);
check('长文本最多 2 行（高度有上限）', long.lines <= M.maxLines, `${long.lines} 行 / ${long.h.toFixed(1)}px`);
check('标签宽度不超过 maxWidth+边距', long.w <= M.maxWidth + (M.pad + M.outline) * 2, `${long.w.toFixed(1)}px`);
const fitted = L.fitLabelText('任务 · 这是一个非常长的标题会被截断处理掉多余部分', M, M.fontSize);
check('截断后带省略号', fitted.endsWith('…'), fitted);

console.log(`\n${failures === 0 ? '全部通过 ✓' : failures + ' 项失败 ✗'}`);
process.exit(failures === 0 ? 0 : 1);
