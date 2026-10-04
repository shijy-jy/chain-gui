<script lang="ts">
  // 三层重构后 · 三维图结构（纯图结构，无水面/涟漪）。
  // 渲染：three.js InstancedMesh 球节点 + LineSegments 边（类型配色 + 状态样式）。
  // 交互（Unity 式）：右键拖拽旋转视角 · 中键拖拽平移 · 滚轮缩放 · 左键点击选中 · 左键双击聚焦。
  // 标签不画在图上（沿用 2D 时代"图上不显示名称"的语义）：悬停浮层由 App 的 hoverTip 承担。
  import { onMount } from 'svelte';
  import * as THREE from 'three';
  import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
  // Bloom 光晕（神经元发光感；大图自动关闭以降级）
  import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
  import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
  import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
  import { NODE_TYPE_COLOR, type LayoutMode } from './node_style';
  import { fnv1a } from './ui/perf';
  // 3D 力导向松弛（开源参考：vasturiano/d3-force-3d，velocity Verlet，纯函数无 DOM 依赖）
  // 只作为**种子后处理**：以确定性神经元布局为初值跑固定 tick 数 → 枝条自然张开且可复现
  import * as d3f from 'd3-force-3d';
  import type { ChainSnapshot, ChainNode, NodeStatus, ScanMode } from './types';

  /** 布局算法（P2-8）：auto = 按工作区模式自动（分析=层级球壳 / 开发=神经元） */
  export type LayoutAlgo = 'auto' | 'neuron' | 'hierarchy';
  /** 覆盖层内边距（px）：图例/缩放控件占掉的画面区域，相机居中时避开（P2-9） */
  export interface Insets {
    left: number;
    right: number;
    top: number;
    bottom: number;
  }

  interface Props {
    snapshot: ChainSnapshot | null;
    selectedId: string | null;
    codeFilter: boolean;
    /** 归档视图：把 snapshot.archived 作为暗色外围球壳渲染（默认关） */
    showArchived: boolean;
    layout: {
      visibleDepth: number;
      siblingGap: number;
      levelGap: number;
      layoutMode: 'auto' | LayoutMode;
      /** 布局算法选择（P2-8） */
      algo?: LayoutAlgo;
      /** 工作区模式：algo='auto' 时决定用哪种布局 */
      mode?: ScanMode;
    };
    /** 覆盖层内边距（P2-9）：居中/适配时把被图例与控件压住的区域让开 */
    insets?: Insets;
    onready: (api: Graph3DApi) => void;
    onselect: (id: string | null) => void;
    onhover: (tip: { x: number; y: number; text: string } | null) => void;
    onfocus: (id: string) => void;
  }

  export interface Graph3DApi {
    focusNode(id: string): void;
    centerAll(): void;
    zoomBy(factor: number): void;
    setSearchHit(id: string): void;
    dispose(): void;
  }

  let {
    snapshot,
    selectedId,
    codeFilter,
    showArchived,
    layout,
    insets = { left: 0, right: 0, top: 0, bottom: 0 },
    onready,
    onselect,
    onhover,
    onfocus,
  }: Props = $props();

  let host: HTMLDivElement;
  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene | null = null;
  let camera: THREE.PerspectiveCamera | null = null;
  let controls: OrbitControls | null = null;
  let nodeMesh: THREE.InstancedMesh | null = null;
  let ringMesh: THREE.InstancedMesh | null = null;
  let archMesh: THREE.InstancedMesh | null = null;
  let shellGroup: THREE.Group | null = null;
  let solidEdges: THREE.LineSegments | null = null;
  let dashedEdges: THREE.LineSegments | null = null;
  let raycaster = new THREE.Raycaster();
  let pointer = new THREE.Vector2();
  let resizeObs: ResizeObserver | null = null;
  let disposed = false;
  let rafId = 0;
  let lastFrame = 0;
  let composer: EffectComposer | null = null;
  let bloomPass: UnrealBloomPass | null = null;
  let pulsePoints: THREE.Points | null = null;
  let pulseRecs: { a: THREE.Vector3; b: THREE.Vector3; t: number; speed: number; col: THREE.Color }[] = [];

  // 当前几何映射：实例索引 → 节点 id（射线命中用）
  let instanceToId: string[] = [];
  // 节点世界坐标（聚焦/脉冲用）
  let idToPos = new Map<string, THREE.Vector3>();
  // 搜索脉冲状态
  let pulseId: string | null = null;
  let pulseUntil = 0;
  // 邻域高亮：hover 优先于选中；邻接表在 rebuild 时重建（1 跳邻居）
  let hoverId: string | null = null;
  let adjacency = new Map<string, Set<string>>();
  // 边记录（颜色/端点/类型）——高亮时只重写颜色属性，不重建几何
  type EdgeRec = { a: THREE.Vector3; b: THREE.Vector3; ca: THREE.Color; cb: THREE.Color; aid: string; bid: string; dashed: boolean };
  let edgeRecords: EdgeRec[] = [];
  // 相机适配标记（首次/换工作区时自动 fit）
  let cameraFitted = false;
  let lastSnap: ChainSnapshot | null = null;

  const Z_PER_DEPTH_L = 120; // （保留常量：旧层板布局参考值）
  const Z_PER_DEPTH_R = 170;
  // 布局 px → 场景单位：固定系数（**不做 span 归一化**——那会让不同规模的图与球径比例失调；
  // 相机由 centerAll 按包围盒自适应，节点间距与球半径天然同比例）
  const UNIT = 0.1;
  /** 球半径（场景单位）：与层距匹配（层距 ~9 单位时球直径约 2 单位，留出枝条间隙） */
  function sphereRadius(px: number): number {
    return Math.max(0.22, px * 0.45 * UNIT);
  }

  function nodeColor(n: ChainNode): THREE.Color {
    const c = new THREE.Color(NODE_TYPE_COLOR[n.type] ?? '#94a3b8');
    const st: NodeStatus = n.status;
    if (st === 'failed') return new THREE.Color('#f87171');
    if (st === 'in_progress') { c.multiplyScalar(1.6); return c; }
    if (st === 'blocked') { c.multiplyScalar(0.55); return c; }
    return c;
  }
  function nodeAlpha(n: ChainNode): number {
    if (n.status === 'pending') return 0.55;
    return 1;
  }

  // 可见集：根起 BFS 限 visibleDepth 层（>=99 = 全部）
  function visibleSet(snap: ChainSnapshot, depth: number): Set<string> {
    const vis = new Set<string>();
    const root = snap.manifest.root;
    const children = new Map<string, string[]>();
    for (const e of snap.edges) {
      const arr = children.get(e.parent) ?? [];
      arr.push(e.child);
      children.set(e.parent, arr);
    }
    const roots: string[] = [];
    if (root && snap.nodes.some((n) => n.id === root)) roots.push(root);
    if (roots.length === 0) {
      for (const n of snap.nodes) if (!n.parent || !snap.nodes.some((x) => x.id === n.parent)) roots.push(n.id);
    }
    if (depth >= 99) {
      snap.nodes.forEach((n) => vis.add(n.id));
      return vis;
    }
    const q: [string, number][] = roots.map((r) => [r, 0] as [string, number]);
    while (q.length) {
      const [id, d] = q.shift()!;
      if (vis.has(id)) continue;
      vis.add(id);
      if (d >= depth) continue;
      for (const c of children.get(id) ?? []) q.push([c, d + 1]);
    }
    return vis;
  }

  /**
   * 三维神经元布局（纯 3D，不沿用 2D 树形态）：
   * - 根 = 胞体；单根时其子树在**整个球面**上均匀铺开（Fibonacci 球面）→ 放射状树突
   * - 每个节点占据一个锥形区域，子节点在父锥角内按黄金角螺旋分布 → 树枝状辐射
   * - 半径 = 深度 × 层距；确定性哈希微扰让枝条不呆板（可复现，无随机种子）
   * - 「形态」滑条 = 展开度：紧凑(layered 0.8) / 标准(auto 1.05) / 舒展(radial 1.35)
   */
  function computeNeuron3D(snap: ChainSnapshot, vis: Set<string>) {
    const children = new Map<string, string[]>();
    for (const e of snap.edges) {
      if (!vis.has(e.parent) || !vis.has(e.child)) continue;
      const a = children.get(e.parent) ?? [];
      a.push(e.child);
      children.set(e.parent, a);
    }
    const roots: string[] = [];
    const pref = snap.manifest.root;
    for (const n of snap.nodes) if (vis.has(n.id) && (!n.parent || !vis.has(n.parent))) roots.push(n.id);
    if (pref && roots.includes(pref)) {
      roots.splice(roots.indexOf(pref), 1);
      roots.unshift(pref);
    }
    if (roots.length === 0) return { posMap: new Map<string, THREE.Vector3>(), arr: [] as { id: string; pos: THREE.Vector3; size: number }[] };

    const LEVEL = Math.max(30, layout.levelGap * 0.95);   // 每层径向间距（px）：拉大 → 结构张开
    const JIT = Math.min(0.45, layout.siblingGap / 120);  // 枝条抖动幅度（复用同层间距滑条）
    const cone0 = layout.layoutMode === 'layered' ? 0.8 : layout.layoutMode === 'radial' ? 1.35 : 1.05;
    const golden = 2.39996323;
    const noise = (id: string, salt: number) => ((fnv1a(`${id}#${salt}`) % 2000) / 1000) - 1;

    const out = new Map<string, THREE.Vector3>();
    const basis = (dir: THREE.Vector3): [THREE.Vector3, THREE.Vector3] => {
      const helper = Math.abs(dir.x) < 0.9 ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0, 1, 0);
      const u = new THREE.Vector3().crossVectors(dir, helper).normalize();
      const v = new THREE.Vector3().crossVectors(dir, u).normalize();
      return [u, v];
    };
    const dirInCone = (axis: THREE.Vector3, half: number, i: number, k: number) => {
      const [u, v] = basis(axis);
      const t = k <= 1 ? 0.5 : (i + 0.5) / k;
      const phi = Math.acos(Math.max(-1, Math.min(1, 1 - t * (1 - Math.cos(half)))));
      const psi = i * golden;
      return new THREE.Vector3()
        .addScaledVector(axis, Math.cos(phi))
        .addScaledVector(u, Math.sin(phi) * Math.cos(psi))
        .addScaledVector(v, Math.sin(phi) * Math.sin(psi))
        .normalize();
    };

    type Frame = { id: string; dir: THREE.Vector3; half: number; depth: number };
    const queue: Frame[] = [];
    if (roots.length === 1) {
      queue.push({ id: roots[0], dir: new THREE.Vector3(0, 1, 0), half: Math.PI, depth: 0 });
    } else {
      roots.forEach((id, i) => {
        const dir = dirInCone(new THREE.Vector3(0, 1, 0), Math.PI, i, roots.length);
        out.set(id, dir.clone().multiplyScalar(LEVEL * 0.5));
        queue.push({ id, dir, half: cone0 * 0.85, depth: 0 });
      });
    }

    let guard = 0;
    while (queue.length && guard++ < 50000) {
      const f = queue.shift()!;
      if (!out.has(f.id)) {
        const r = f.depth * LEVEL * (1 + JIT * 0.35 * noise(f.id, 1));
        out.set(f.id, f.dir.clone().multiplyScalar(r));
      }
      const kids = children.get(f.id) ?? [];
      if (kids.length === 0) continue;
      const half = Math.max(0.3, f.half * 0.72);   // 子锥随深度收窄
      kids.forEach((cid, i) => {
        queue.push({ id: cid, dir: dirInCone(f.dir, half, i, kids.length), half, depth: f.depth + 1 });
      });
    }

    // 确定性微扰 + 力导向松弛 + 居中 + UNIT 缩放
    const raw: { id: string; p: THREE.Vector3 }[] = [];
    // ① 先做 3D 力导向松弛（以神经元布局为种子）：去重叠、枝条张开，tick 数与初值固定 → 可复现
    const ids = [...out.keys()];
    if (ids.length > 1) {
      const simNodes = ids.map((id) => {
        const p = out.get(id)!;
        return { id, x: p.x, y: p.y, z: p.z };
      });
      const links: { source: string; target: string }[] = [];
      for (const [pid, kids] of children) {
        for (const cid of kids) if (out.has(pid) && out.has(cid)) links.push({ source: pid, target: cid });
      }
      const sim: any = (d3f as any)
        .forceSimulation(simNodes, 3)
        .force('link', (d3f as any).forceLink(links, 3).id((d: any) => d.id).distance(LEVEL * 0.85).strength(0.45))
        .force('charge', (d3f as any).forceManyBody(3).strength(-LEVEL * LEVEL * 0.05).theta(0.9))
        .force('center', (d3f as any).forceCenter(0, 0, 0))
        .stop();
      const ticks = ids.length > 600 ? 60 : ids.length > 200 ? 90 : 130;
      for (let t = 0; t < ticks; t++) sim.tick();
      for (const n of simNodes) out.set(n.id, new THREE.Vector3(n.x, n.y, n.z));
    }
    let cx = 0, cy = 0, cz = 0;
    for (const [id, p] of out) {
      const j = new THREE.Vector3(noise(id, 2), noise(id, 3), noise(id, 4)).multiplyScalar(LEVEL * JIT * 0.5);
      const q = p.clone().add(j);
      raw.push({ id, p: q });
      cx += q.x; cy += q.y; cz += q.z;
    }
    const cnt = Math.max(1, raw.length);
    cx /= cnt; cy /= cnt; cz /= cnt;
    const posMap = new Map<string, THREE.Vector3>();
    const arr: { id: string; pos: THREE.Vector3; size: number }[] = [];
    for (const r of raw) {
      const pos = new THREE.Vector3((r.p.x - cx) * UNIT, (r.p.y - cy) * UNIT, (r.p.z - cz) * UNIT);
      posMap.set(r.id, pos);
      arr.push({ id: r.id, pos, size: nodeDisplaySizeFor(r.id) });
    }
    return { posMap, arr };
  }

  /**
   * 分析模式的**层级优先**布局（P2-8）：按 depth 分球壳。
   * - 半径 = depth × 层距（**严格按层**：同一层的节点落在同一个球壳上 → 人一眼看出"第几层"）
   * - 角度：子节点在父节点方向的锥角内均匀铺开，锥角随深度收窄（保持枝条谱系）
   * - 不做力导向松弛：松弛会把层壳揉散，层级语义就没了；只加确定性微扰避免完全重叠
   * - 与神经元布局共用同一套 px → 场景单位映射与球径公式（指标同源）
   */
  function computeHierarchy3D(snap: ChainSnapshot, vis: Set<string>) {
    const children = new Map<string, string[]>();
    for (const e of snap.edges) {
      if (!vis.has(e.parent) || !vis.has(e.child)) continue;
      const a = children.get(e.parent) ?? [];
      a.push(e.child);
      children.set(e.parent, a);
    }
    const roots: string[] = [];
    const pref = snap.manifest.root;
    for (const n of snap.nodes) if (vis.has(n.id) && (!n.parent || !vis.has(n.parent))) roots.push(n.id);
    if (pref && roots.includes(pref)) {
      roots.splice(roots.indexOf(pref), 1);
      roots.unshift(pref);
    }
    if (roots.length === 0) return { posMap: new Map<string, THREE.Vector3>(), arr: [] as { id: string; pos: THREE.Vector3; size: number }[] };

    const LEVEL = Math.max(30, layout.levelGap * 0.95);
    const JIT = Math.min(0.35, layout.siblingGap / 200);
    const cone0 = layout.layoutMode === 'layered' ? 0.8 : layout.layoutMode === 'radial' ? 1.35 : 1.05;
    const golden = 2.39996323;
    const noise = (id: string, salt: number) => ((fnv1a(`${id}#${salt}`) % 2000) / 1000) - 1;
    const basis = (dir: THREE.Vector3): [THREE.Vector3, THREE.Vector3] => {
      const helper = Math.abs(dir.x) < 0.9 ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0, 1, 0);
      const u = new THREE.Vector3().crossVectors(dir, helper).normalize();
      const v = new THREE.Vector3().crossVectors(dir, u).normalize();
      return [u, v];
    };
    const dirInCone = (axis: THREE.Vector3, half: number, i: number, k: number) => {
      const [u, v] = basis(axis);
      const t = k <= 1 ? 0.5 : (i + 0.5) / k;
      const phi = Math.acos(Math.max(-1, Math.min(1, 1 - t * (1 - Math.cos(half)))));
      const psi = i * golden;
      return new THREE.Vector3()
        .addScaledVector(axis, Math.cos(phi))
        .addScaledVector(u, Math.sin(phi) * Math.cos(psi))
        .addScaledVector(v, Math.sin(phi) * Math.sin(psi))
        .normalize();
    };

    // 层壳半径严格按深度：同一 depth 的节点共用一个球壳
    const out = new Map<string, THREE.Vector3>();
    const dirs = new Map<string, THREE.Vector3>();
    const depthOf = new Map<string, number>();
    const spread = Math.max(0.55, cone0);
    if (roots.length === 1) {
      depthOf.set(roots[0], 0);
      dirs.set(roots[0], new THREE.Vector3(0, 1, 0));
      out.set(roots[0], new THREE.Vector3(0, 0, 0));
    } else {
      // 多根：第 0 层仍是一个球壳（半径取半个层距，避免全部重叠在原点）
      roots.forEach((id, i) => {
        const dir = dirInCone(new THREE.Vector3(0, 1, 0), Math.PI, i, roots.length);
        depthOf.set(id, 0);
        dirs.set(id, dir);
        out.set(id, dir.clone().multiplyScalar(LEVEL * 0.5 * spread));
      });
    }

    type Frame = { id: string; dir: THREE.Vector3; half: number; depth: number };
    const queue: Frame[] = roots.map((id) => ({
      id,
      dir: dirs.get(id)!,
      half: roots.length === 1 ? Math.PI : Math.PI * 0.85,
      depth: 0,
    }));
    let guard = 0;
    while (queue.length && guard++ < 50000) {
      const f = queue.shift()!;
      const kids = children.get(f.id) ?? [];
      if (kids.length === 0) continue;
      // 锥角随深度按子节点数收窄：孩子多 → 铺得开；孩子少 → 抱成一枝
      const half = Math.max(0.28, Math.min(Math.PI * 0.92, f.half * 0.75));
      const d = f.depth + 1;
      kids.forEach((cid, i) => {
        // 首次到达即定层（BFS 最短层距，与后端 structure_index 的 depth 同算法）；
        // 环/菱形边不会重复入队，也不会把节点拖到更深的壳上
        if (dirs.has(cid)) return;
        const dir = dirInCone(f.dir, half, i, kids.length);
        depthOf.set(cid, d);
        dirs.set(cid, dir);
        queue.push({ id: cid, dir, half, depth: d });
      });
    }
    // 落位：半径 = 深度 × 层距（严格同层同壳），加确定性微扰（径向/切向都轻微）
    for (const [id, dir] of dirs) {
      const d = depthOf.get(id) ?? 0;
      if (d === 0 && roots.length === 1) { out.set(id, new THREE.Vector3(0, 0, 0)); continue; }
      const r = d === 0 ? LEVEL * 0.5 * spread : d * LEVEL;
      const jr = 1 + JIT * 0.5 * noise(id, 1);
      out.set(id, dir.clone().multiplyScalar(r * jr));
    }

    const raw: { id: string; p: THREE.Vector3 }[] = [];
    let cx = 0, cy = 0, cz = 0;
    for (const [id, p] of out) {
      const j = new THREE.Vector3(noise(id, 2), noise(id, 3), noise(id, 4)).multiplyScalar(LEVEL * JIT * 0.6);
      const q = p.clone().add(j);
      raw.push({ id, p: q });
      cx += q.x; cy += q.y; cz += q.z;
    }
    const cnt = Math.max(1, raw.length);
    cx /= cnt; cy /= cnt; cz /= cnt;
    const posMap = new Map<string, THREE.Vector3>();
    const arr: { id: string; pos: THREE.Vector3; size: number }[] = [];
    for (const r of raw) {
      const pos = new THREE.Vector3((r.p.x - cx) * UNIT, (r.p.y - cy) * UNIT, (r.p.z - cz) * UNIT);
      posMap.set(r.id, pos);
      arr.push({ id: r.id, pos, size: nodeDisplaySizeFor(r.id) });
    }
    // 层壳提示（P2-8 可读性）：每个"有节点的深度"画一层极淡的线框球壳——
    // 同层同壳这件事必须**看得见**，否则球壳布局与神经元布局在视觉上无从区分。
    // 球壳中心 = 根所在处（居中平移后的原点位置 = -centroid × UNIT），半径 = depth × 层距 × UNIT。
    const shellCenter = new THREE.Vector3(-cx * UNIT, -cy * UNIT, -cz * UNIT);
    const shellRadii: number[] = [];
    const perDepth = new Map<number, number>();
    for (const d of depthOf.values()) perDepth.set(d, (perDepth.get(d) ?? 0) + 1);
    for (const d of [...perDepth.keys()].sort((a, b) => a - b)) {
      if (d === 0) continue;
      if (shellRadii.length >= 12) break;   // 深链上限：只提示前 12 层，避免线框噪声
      shellRadii.push(d * LEVEL * UNIT);
    }
    return { posMap, arr, shells: { center: shellCenter, radii: shellRadii } };
  }

  /** 布局算法：显式选择优先；auto 时按工作区模式（分析 = 层级球壳，开发 = 神经元） */
  function useHierarchy(): boolean {
    const algo = layout.algo ?? 'auto';
    if (algo === 'hierarchy') return true;
    if (algo === 'neuron') return false;
    return (layout.mode ?? 'dev') === 'analysis';
  }

  function rebuild() {
    if (!scene || !snapshot || disposed) return;
    const snap = snapshot;
    if (snap !== lastSnap) { lastSnap = snap; cameraFitted = false; rebuildMetricCaches(snap); }
    const vis = visibleSet(snap, layout.visibleDepth);
    const built = useHierarchy()
      ? computeHierarchy3D(snap, vis)
      : { ...computeNeuron3D(snap, vis), shells: null as { center: THREE.Vector3; radii: number[] } | null };
    const { posMap, arr } = built;
    const byId = new Map(snap.nodes.map((n) => [n.id, n]));
    idToPos = posMap;

    // ── 层壳线框（分析模式层级布局）：同层同壳的可视化凭据 ──
    // 每层画 3 条正交大圆（"陀螺环"）而不是整张球面网格：11 层线框球叠在一起会把节点淹掉，
    // 3 环既够看出"这是一个球壳"，又保持在背景里（单个 LineSegments，一次绘制调用）。
    if (shellGroup) {
      scene.remove(shellGroup);
      shellGroup.traverse((o) => {
        const m = o as THREE.Mesh;
        if (m.geometry) m.geometry.dispose();
        const mm = m.material as THREE.Material | THREE.Material[];
        if (Array.isArray(mm)) mm.forEach((x) => x.dispose());
        else if (mm) mm.dispose();
      });
      shellGroup = null;
    }
    if (built.shells && built.shells.radii.length > 0) {
      const SEG = 96;
      const verts: number[] = [];
      for (const r of built.shells.radii) {
        for (let plane = 0; plane < 3; plane++) {
          for (let i = 0; i < SEG; i++) {
            for (const k of [i, i + 1]) {
              const t = (k / SEG) * Math.PI * 2;
              const c = Math.cos(t) * r, s = Math.sin(t) * r;
              if (plane === 0) verts.push(c, s, 0);
              else if (plane === 1) verts.push(c, 0, s);
              else verts.push(0, c, s);
            }
          }
        }
      }
      const sgeo = new THREE.BufferGeometry();
      sgeo.setAttribute('position', new THREE.Float32BufferAttribute(verts, 3));
      const smat = new THREE.LineBasicMaterial({
        color: 0x38bdf8,
        transparent: true,
        opacity: 0.1,
        depthWrite: false,
      });
      const lines = new THREE.LineSegments(sgeo, smat);
      const g = new THREE.Group();
      g.name = 'depth-shells';
      g.position.copy(built.shells.center);
      g.add(lines);
      shellGroup = g;
      scene.add(g);
    }

    // 注意：实例色走 instanceColor（three 自动启用 USE_INSTANCING_COLOR），
    // **不能**设 vertexColors:true——那会让 shader 去取几何体 color 属性（球体没有）→ 全黑。
    const geo = new THREE.SphereGeometry(1, 24, 18);
    const mat = new THREE.MeshBasicMaterial({ transparent: true });
    if (nodeMesh) { nodeMesh.geometry.dispose(); (nodeMesh.material as THREE.Material).dispose(); scene.remove(nodeMesh); }
    nodeMesh = new THREE.InstancedMesh(geo, mat, arr.length);
    nodeMesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    instanceToId = [];
    const m4 = new THREE.Matrix4();
    arr.forEach((a, i) => {
      const n = byId.get(a.id)!;
      const size = sphereRadius(a.size);
      m4.makeScale(size, size, size);
      m4.setPosition(a.pos);
      nodeMesh!.setMatrixAt(i, m4);
      const col = nodeColor(n);
      nodeMesh!.setColorAt(i, col);
      instanceToId.push(a.id);
    });
    nodeMesh.instanceMatrix.needsUpdate = true;
    if (nodeMesh.instanceColor) nodeMesh.instanceColor.needsUpdate = true;
    scene.add(nodeMesh);

    // 归档节点（showArchived）：暗色外围球壳，Fibonacci 球面均匀分布，与活跃图分离
    if (archMesh) { archMesh.geometry.dispose(); (archMesh.material as THREE.Material).dispose(); scene.remove(archMesh); }
    archMesh = null;
    if (showArchived && snap.archived.length > 0) {
      const arc = snap.archived;
      const maxR = arr.reduce((m, a) => Math.max(m, a.pos.length()), 1);
      const shellR = maxR * 1.35;
      const ageo = new THREE.SphereGeometry(1, 10, 8);
      const amat = new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.55 });
      archMesh = new THREE.InstancedMesh(ageo, amat, arc.length);
      const ga = Math.PI * (3 - Math.sqrt(5));
      arc.forEach((n, i) => {
        const yy = 1 - (i + 0.5) / arc.length * 2;
        const rr = Math.sqrt(Math.max(0, 1 - yy * yy));
        const th = ga * i;
        const pos = new THREE.Vector3(Math.cos(th) * rr, yy, Math.sin(th) * rr).multiplyScalar(shellR);
        const sz = sphereRadius(16) * 0.8;
        m4.makeScale(sz, sz, sz);
        m4.setPosition(pos);
        archMesh!.setMatrixAt(i, m4);
        archMesh!.setColorAt(i, nodeColor(n).multiplyScalar(0.45));
      });
      archMesh.instanceMatrix.needsUpdate = true;
      if (archMesh.instanceColor) archMesh.instanceColor.needsUpdate = true;
      scene.add(archMesh);
    }

    // 代码节点描边环（青绿线框，略大）
    const ringIds = arr.filter((a) => byId.get(a.id)?.code_map).map((a) => a.id);
    if (ringMesh) { ringMesh.geometry.dispose(); (ringMesh.material as THREE.Material).dispose(); scene.remove(ringMesh); }
    ringMesh = null;
    if (ringIds.length > 0) {
      const rgeo = new THREE.SphereGeometry(1, 12, 8);
      const rmat = new THREE.MeshBasicMaterial({ color: 0x2dd4bf, wireframe: true, transparent: true, opacity: 0.9 });
      ringMesh = new THREE.InstancedMesh(rgeo, rmat, ringIds.length);
      ringIds.forEach((id, i) => {
        const a = arr.find((x) => x.id === id)!;
        const size = sphereRadius(a.size) * 1.18;
        m4.makeScale(size, size, size);
        m4.setPosition(a.pos);
        ringMesh!.setMatrixAt(i, m4);
      });
      scene.add(ringMesh);
    }

    // ── 边（同时记录端点/颜色，供邻域高亮原位改色）──
    adjacency = new Map<string, Set<string>>();
    const link = (x: string, y: string) => {
      if (!adjacency.has(x)) adjacency.set(x, new Set());
      adjacency.get(x)!.add(y);
    };
    edgeRecords = [];
    for (const e of snap.edges) {
      const a = posMap.get(e.parent), b = posMap.get(e.child);
      if (!a || !b) continue;
      const na = byId.get(e.parent), nb = byId.get(e.child);
      link(e.parent, e.child);
      link(e.child, e.parent);
      edgeRecords.push({
        a, b,
        ca: nodeColor(na!), cb: nodeColor(nb!),
        aid: e.parent, bid: e.child,
        dashed: e.rel === 'solves' || e.rel === 'alternative',
      });
    }
    const solidRecs = edgeRecords.filter((r) => !r.dashed);
    const dashRecs = edgeRecords.filter((r) => r.dashed);
    const fillEdges = (recs: EdgeRec[]) => {
      const pos: number[] = [], col: number[] = [];
      for (const r of recs) {
        pos.push(r.a.x, r.a.y, r.a.z, r.b.x, r.b.y, r.b.z);
        col.push(r.ca.r, r.ca.g, r.ca.b, r.cb.r, r.cb.g, r.cb.b);
      }
      return { pos, col };
    };
    if (solidEdges) { solidEdges.geometry.dispose(); (solidEdges.material as THREE.Material).dispose(); scene.remove(solidEdges); }
    const sFill = fillEdges(solidRecs);
    const sgeo = new THREE.BufferGeometry();
    sgeo.setAttribute('position', new THREE.Float32BufferAttribute(sFill.pos, 3));
    sgeo.setAttribute('color', new THREE.Float32BufferAttribute(sFill.col, 3));
    solidEdges = new THREE.LineSegments(sgeo, new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, opacity: 0.75 }));
    scene.add(solidEdges);
    if (dashedEdges) { dashedEdges.geometry.dispose(); (dashedEdges.material as THREE.Material).dispose(); scene.remove(dashedEdges); }
    const dFill = fillEdges(dashRecs);
    const dgeo = new THREE.BufferGeometry();
    dgeo.setAttribute('position', new THREE.Float32BufferAttribute(dFill.pos, 3));
    dgeo.setAttribute('color', new THREE.Float32BufferAttribute(dFill.col, 3));
    const dmat = new THREE.LineDashedMaterial({ vertexColors: true, transparent: true, opacity: 0.7, dashSize: 2, gapSize: 1.2 });
    dashedEdges = new THREE.LineSegments(dgeo, dmat);
    dashedEdges.computeLineDistances();
    scene.add(dashedEdges);

    writeEdgeColors(hoverId ?? selectedId);
    buildPulses(selectedId);
    applyVisualState();
    // Bloom 仅在小图上开（大图多一遍全屏后处理开销不值）
    if (bloomPass) bloomPass.enabled = arr.length <= 1200;
    // 新工作区/首次构建：相机自动适配包围盒（后续滑条调整保留用户视角）
    if (!cameraFitted) {
      cameraFitted = true;
      centerAll();
    }
  }

  /** 突触脉冲：沿边流动的粒子（借鉴 3d-force-graph linkDirectionalParticles）。
   *  选中节点时只沿它的边流动（聚焦语义），否则沿全部边（上限 260 条防开销）。 */
  function buildPulses(active: string | null) {
    if (!scene) return;
    if (pulsePoints) { pulsePoints.geometry.dispose(); (pulsePoints.material as THREE.Material).dispose(); scene.remove(pulsePoints); pulsePoints = null; }
    pulseRecs = [];
    const recs = (active ? edgeRecords.filter((r) => r.aid === active || r.bid === active) : edgeRecords).slice(0, 260);
    if (recs.length === 0) return;
    recs.forEach((r, i) => {
      pulseRecs.push({
        a: r.a, b: r.b,
        t: (i * 0.37) % 1,
        speed: 0.45 + ((fnv1a(r.aid + '>' + r.bid) % 100) / 100) * 0.55,
        col: r.cb.clone(),
      });
    });
    const pos = new Float32Array(pulseRecs.length * 3);
    const col = new Float32Array(pulseRecs.length * 3);
    pulseRecs.forEach((p, i) => { col[i * 3] = p.col.r; col[i * 3 + 1] = p.col.g; col[i * 3 + 2] = p.col.b; });
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    geo.setAttribute('color', new THREE.BufferAttribute(col, 3));
    const mat = new THREE.PointsMaterial({
      size: 0.85, sizeAttenuation: true, vertexColors: true,
      transparent: true, opacity: 0.95, blending: THREE.AdditiveBlending, depthWrite: false,
    });
    pulsePoints = new THREE.Points(geo, mat);
    scene.add(pulsePoints);
  }

  function stepPulses(dt: number) {
    if (!pulsePoints || pulseRecs.length === 0) return;
    const attr = pulsePoints.geometry.getAttribute('position') as THREE.BufferAttribute;
    const arr = attr.array as Float32Array;
    pulseRecs.forEach((p, i) => {
      p.t += p.speed * dt;
      if (p.t > 1) p.t -= 1;
      const o = i * 3;
      arr[o] = p.a.x + (p.b.x - p.a.x) * p.t;
      arr[o + 1] = p.a.y + (p.b.y - p.a.y) * p.t;
      arr[o + 2] = p.a.z + (p.b.z - p.a.z) * p.t;
    });
    attr.needsUpdate = true;
  }

  /** 邻域高亮：与 active 相连的边保持原色，其余压暗；active=null 全部原色 */
  function writeEdgeColors(active: string | null) {
    const paint = (mesh: THREE.LineSegments | null, recs: EdgeRec[]) => {
      if (!mesh) return;
      const attr = mesh.geometry.getAttribute('color') as THREE.BufferAttribute;
      const arr = attr.array as Float32Array;
      recs.forEach((r, i) => {
        const on = !active || r.aid === active || r.bid === active;
        const k = on ? 1 : 0.14;
        const o = i * 6;
        arr[o] = r.ca.r * k; arr[o + 1] = r.ca.g * k; arr[o + 2] = r.ca.b * k;
        arr[o + 3] = r.cb.r * k; arr[o + 4] = r.cb.g * k; arr[o + 5] = r.cb.b * k;
      });
      attr.needsUpdate = true;
    };
    paint(solidEdges, edgeRecords.filter((r) => !r.dashed));
    paint(dashedEdges, edgeRecords.filter((r) => r.dashed));
  }

  function applyVisualState() {
    if (!nodeMesh || !snapshot || disposed) return;
    const active = hoverId ?? selectedId;
    const near = active ? (adjacency.get(active) ?? new Set<string>()) : null;
    const byId = new Map(snapshot.nodes.map((n) => [n.id, n]));
    const col = new THREE.Color();
    instanceToId.forEach((id, i) => {
      const n = byId.get(id);
      if (!n) return;
      col.copy(nodeColor(n));
      if (codeFilter && !n.code_map) col.setRGB(col.r * 0.15, col.g * 0.15, col.b * 0.15);
      else if (near && id !== active && !near.has(id)) col.multiplyScalar(0.22);
      nodeMesh!.setColorAt(i, col);
      // 选中放大（与重建时同一套半径公式）
      const base = idToPos.get(id)!;
      const sz = sphereRadius(nodeDisplaySizeFor(id));
      const k = id === selectedId ? 1.7 : 1;
      const m4 = new THREE.Matrix4();
      m4.makeScale(sz * k, sz * k, sz * k);
      m4.setPosition(base.x, base.y, base.z);
      nodeMesh!.setMatrixAt(i, m4);
    });
    nodeMesh.instanceMatrix.needsUpdate = true;
    if (nodeMesh.instanceColor) nodeMesh.instanceColor.needsUpdate = true;
    if (solidEdges) (solidEdges.material as THREE.LineBasicMaterial).opacity = codeFilter ? 0.08 : 0.75;
    if (dashedEdges) (dashedEdges.material as THREE.LineDashedMaterial).opacity = codeFilter ? 0.08 : 0.7;
    if (ringMesh) ringMesh.visible = true;
    writeEdgeColors(active);
    // 搜索脉冲
    if (pulseId && performance.now() < pulseUntil) {
      const hit = instanceToId.indexOf(pulseId);
      if (hit >= 0 && nodeMesh) {
        nodeMesh.setColorAt(hit, new THREE.Color('#fbbf24'));
        if (nodeMesh.instanceColor) nodeMesh.instanceColor.needsUpdate = true;
      }
    } else {
      pulseId = null;
    }
  }

  // 节点显示直径（P2-7 人机同源）：球径 = **后端下发的 degree**（与 AI 在工具响应里读到的
  // degree / children_count 同一份 structure_index 算法），显示层不再自己重算结构指标。
  // 平方根缓增 + 封顶（Obsidian 风格）；缺注解（老快照/归档节点）时退回 1。
  let sizeCache = new Map<string, number>();
  let degreeCache = new Map<string, number>();
  function degreeOf(id: string): number {
    const d = degreeCache.get(id);
    return d === undefined ? 0 : d;
  }
  function subtreeOf(id: string): number {
    const s = sizeCache.get(id);
    return s === undefined ? 1 : s;
  }
  /** 后端结构注解 → 尺寸缓存（每次换快照重建一次，O(n)） */
  function rebuildMetricCaches(snap: ChainSnapshot) {
    degreeCache = new Map();
    sizeCache = new Map();
    for (const n of snap.nodes) {
      degreeCache.set(n.id, typeof n.degree === 'number' ? n.degree : 0);
      sizeCache.set(n.id, typeof n.subtree_size === 'number' ? n.subtree_size : 1);
    }
  }
  function nodeDisplaySizeFor(id: string): number {
    const d = degreeOf(id);
    return 14 + Math.min(Math.sqrt(d), 6) * 4;   // px
  }

  function pick(clientX: number, clientY: number): string | null {
    if (!renderer || !camera || !nodeMesh) return null;
    const rect = renderer.domElement.getBoundingClientRect();
    pointer.x = ((clientX - rect.left) / rect.width) * 2 - 1;
    pointer.y = -((clientY - rect.top) / rect.height) * 2 + 1;
    raycaster.setFromCamera(pointer, camera);
    const hits = raycaster.intersectObject(nodeMesh);
    if (hits.length === 0) return null;
    return instanceToId[hits[0].instanceId ?? -1] ?? null;
  }

  // ── 相机动画（聚焦/复位/缩放共用）──
  function animateCamera(toTarget: THREE.Vector3, toPos: THREE.Vector3 | null, ms = 420) {
    if (!camera || !controls) return;
    const t0 = performance.now();
    const fromTarget = controls.target.clone();
    const fromPos = camera.position.clone();
    const step = (now: number) => {
      const t = Math.min(1, (now - t0) / ms);
      const k = 1 - Math.pow(1 - t, 3);
      controls!.target.lerpVectors(fromTarget, toTarget, k);
      if (toPos) camera!.position.lerpVectors(fromPos, toPos, k);
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  function focusNode(id: string) {
    const pos = idToPos.get(id);
    if (!pos || !camera || !controls) return;
    const dir = camera.position.clone().sub(controls.target).normalize();
    const dist = 34;
    animateCamera(pos.clone(), pos.clone().add(dir.multiplyScalar(dist)));
  }
  /**
   * 全局适配（P2-9 覆盖遮挡）：把内容居中到**未被覆盖层压住的区域**，而不是画布正中。
   * - 图例/缩放控件占掉 left/right/top/bottom 后，自由区中心相对画布中心偏移 (L-R)/2, (T-B)/2 px
   * - 把该像素偏移换算成世界单位，沿相机 right/up 轴反推相机 target → 内容落在自由区
   * - 距离按自由区尺寸放大，保证放大后仍整图可见
   */
  function centerAll() {
    if (!camera || !controls || !scene || !renderer) return;
    // 取景只按**节点云**算包围盒：层壳线框（半径 = 最深层）与归档外壳不该把图推远，
    // 否则节点会被自己的参考线挤成中间一小团（贴边浮层的内边距同理在此让开）。
    const box = new THREE.Box3();
    for (const p of idToPos.values()) box.expandByPoint(p);
    if (archMesh && showArchived) box.union(new THREE.Box3().setFromObject(archMesh));
    const dir = camera.position.clone().sub(controls.target).normalize();
    if (box.isEmpty()) { animateCamera(new THREE.Vector3(0, 0, 0), new THREE.Vector3(0, 0, 260)); return; }
    const center = box.getCenter(new THREE.Vector3());
    const sizeVec = box.getSize(new THREE.Vector3());
    const diag = sizeVec.length();
    const w = Math.max(1, renderer.domElement.clientWidth);
    const h = Math.max(1, renderer.domElement.clientHeight);
    const ins = insets ?? { left: 0, right: 0, top: 0, bottom: 0 };
    const freeW = Math.max(160, w - ins.left - ins.right);
    const freeH = Math.max(120, h - ins.top - ins.bottom);
    // 以包围盒对角线估距：0.55 系数让图形占满画面约 70%（早先 0.85 太远、图缩在中间）
    const zoomOut = Math.max(1, w / freeW, h / freeH);
    const dist = Math.max(40, diag * 0.55 * zoomOut);
    // 像素 → 世界单位（透视：屏幕上 1px 在当前距离上对应多少世界单位）
    const worldPerPx = (2 * dist * Math.tan(((camera.fov * Math.PI) / 180) / 2)) / h;
    const dxPx = (ins.left - ins.right) / 2;
    const dyPx = (ins.top - ins.bottom) / 2;
    // 相机基：X=屏幕右（up × dir）、Y=屏幕上（dir × X）；dir = target → camera（相机 Z 轴）
    const right = new THREE.Vector3().crossVectors(camera.up, dir).normalize();
    const up = new THREE.Vector3().crossVectors(dir, right).normalize();
    const target = center
      .clone()
      .addScaledVector(right, -dxPx * worldPerPx)
      .addScaledVector(up, dyPx * worldPerPx);
    animateCamera(target, target.clone().add(dir.multiplyScalar(dist)));
  }
  function zoomBy(factor: number) {
    if (!camera || !controls) return;
    const dir = camera.position.clone().sub(controls.target);
    dir.multiplyScalar(1 / factor);
    animateCamera(controls.target.clone(), controls.target.clone().add(dir), 220);
  }
  function setSearchHit(id: string) {
    pulseId = id;
    pulseUntil = performance.now() + 2200;
    focusNode(id);
  }

  // ── 交互 ──
  let lastHit: string | null = null;
  let lastHitTime = 0;

  function onPointerUp(e: PointerEvent) {
    if (e.button !== 0) return;
    const hit = pick(e.clientX, e.clientY);
    const now = performance.now();
    if (hit && lastHit === hit && now - lastHitTime < 380) {
      // 双击 → 聚焦
      lastHit = null;
      onfocus(hit);
      focusNode(hit);
      return;
    }
    lastHit = hit;
    lastHitTime = now;
    onselect(hit);
    if (hit) focusNode(hit);
  }

  let lastMove = 0;
  function onPointerMove(e: PointerEvent) {
    const now = performance.now();
    if (now - lastMove < 70) return;
    lastMove = now;
    const hit = pick(e.clientX, e.clientY);
    if (hit !== hoverId) { hoverId = hit; applyVisualState(); }
    if (!hit) { onhover(null); return; }
    const node = snapshot?.nodes.find((n) => n.id === hit);
    const typeName: Record<string, string> = { goal: '目标', design: '设计', task: '任务', verification: '验证', note: '笔记' };
    onhover({ x: e.clientX, y: e.clientY, text: `${hit} · ${typeName[node?.type ?? 'note'] ?? '笔记'}` });
  }
  function onPointerLeave() { hoverId = null; applyVisualState(); onhover(null); }

  onMount(() => {
    if (!host) return;
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    host.appendChild(renderer.domElement);
    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(45, 1, 0.1, 3000);
    camera.position.set(0, 0, 260);
    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.minDistance = 6;
    controls.maxDistance = 900;
    controls.mouseButtons = { LEFT: null, MIDDLE: THREE.MOUSE.PAN, RIGHT: THREE.MOUSE.ROTATE };
    controls.target.set(0, 0, 0);

    const el = renderer.domElement;
    el.addEventListener('pointerup', onPointerUp);
    el.addEventListener('pointermove', onPointerMove);
    el.addEventListener('pointerleave', onPointerLeave);

    // Bloom 光晕：EffectComposer + UnrealBloomPass（阈值取高一点，只让亮节点发光）
    composer = new EffectComposer(renderer);
    composer.addPass(new RenderPass(scene, camera));
    bloomPass = new UnrealBloomPass(new THREE.Vector2(1, 1), 0.38, 0.45, 0.4);
    composer.addPass(bloomPass);

    resizeObs = new ResizeObserver(() => {
      if (!renderer || !camera || !host) return;
      const w = host.clientWidth || 1, h = host.clientHeight || 1;
      renderer.setSize(w, h);
      composer?.setSize(w, h);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    });
    resizeObs.observe(host);

    const loop = () => {
      if (disposed) return;
      rafId = requestAnimationFrame(loop);
      const now = performance.now();
      const dt = Math.min(0.05, (now - lastFrame) / 1000);
      lastFrame = now;
      controls?.update();
      stepPulses(dt);
      if (renderer && scene && camera) {
        if (composer && bloomPass && bloomPass.enabled) composer.render();
        else renderer.render(scene, camera);
      }
      if (pulseId && now > pulseUntil) { pulseId = null; applyVisualState(); }
    };
    rafId = requestAnimationFrame(loop);

    onready({
      focusNode,
      centerAll,
      zoomBy,
      setSearchHit,
      dispose: () => {
        disposed = true;
        cancelAnimationFrame(rafId);
        resizeObs?.disconnect();
        controls?.dispose();
        el.removeEventListener('pointerup', onPointerUp);
        el.removeEventListener('pointermove', onPointerMove);
        el.removeEventListener('pointerleave', onPointerLeave);
        scene?.traverse((o) => {
          const mesh = o as THREE.Mesh;
          if (mesh.geometry) mesh.geometry.dispose();
          const m = mesh.material as THREE.Material | THREE.Material[];
          if (Array.isArray(m)) m.forEach((x) => x.dispose());
          else if (m) m.dispose();
        });
        renderer?.dispose();
        composer?.dispose();
        renderer?.domElement.remove();
      },
    });
  });

  // 数据变化：重建几何 / 原位更新视觉态
  $effect(() => {
    void snapshot;
    void layout.visibleDepth;
    void layout.siblingGap;
    void layout.levelGap;
    void layout.layoutMode;
    void layout.algo;
    void layout.mode;
    void showArchived;
    rebuild();
  });
  $effect(() => {
    void selectedId;
    void codeFilter;
    buildPulses(selectedId);   // 选中节点 → 脉冲只沿它的边流动（聚焦语义）
    applyVisualState();
  });
</script>

<div class="g3d-host" bind:this={host}></div>

<style>
  .g3d-host {
    position: absolute;
    inset: 0;
    cursor: default;
  }
  .g3d-host :global(canvas) {
    display: block;
  }
</style>
