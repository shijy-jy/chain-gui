<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import cytoscape from 'cytoscape';
  import type { StylesheetJson, Core } from 'cytoscape';
  import { chainToElements, NODE_TYPE_LABEL, NODE_TYPE_COLOR } from './lib/chain_to_cytoscape';
  import {
    computeTreeLayout,
    pickMode,
    chooseLayoutMode,
    type LayoutMode,
  } from './lib/tree_layout';
  import { computeRippleLayers, ripplePulseAmp, type RippleLayers } from './lib/ripple';
  import Sidebar from './lib/Sidebar.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import CreateNodeDialog from './components/CreateNodeDialog.svelte';
  import WorkspaceSidebar from './components/WorkspaceSidebar.svelte';
  import PerfOverlay from './components/PerfOverlay.svelte';
  import CodeViewer from './lib/CodeViewer.svelte';
  import ReaderMode from './lib/ReaderMode.svelte';
  import { panel, SIDEBAR_COLLAPSED_WIDTH } from './lib/panel_state.svelte.ts';
  import { perfPolicy, perfTierName, fnv1a, createFrameMonitor, type FrameMonitor } from './lib/ui/perf';
  import type { ChainSnapshot, ChainNode, NodeStatus, NodeType, ScanMode, WorkspaceInfo } from './lib/types';

  // ═══════════════════════════════════════════════════════════════════════════
  // v3.0 布局层：树感知确定性布局（替代 v1.5–v2.17 的自研力导向模拟）
  //
  // 为什么换：Engram 的图在数学上是森林（每节点最多一个 parent、无环，实测 5 个工作区
  // 全是 multiParentNodes=0 / cycles=0）。树有 O(n) 解析解，用力导向去逼近它是用迭代法
  // 解一元一次方程。实测对照（1500 节点真实基准 G:\perf1500）：
  //     力导向：361997 次边交叉、世界 46023×49541、80 帧后仍以 26521px/帧漂移、174ms
  //     树布局：0 次交叉、世界 1056×1132、完全静止、1.4ms
  // 小图同样没收敛（80 节点真实图末帧仍 211px/帧、48 对间距违例）——"不流畅不舒服"
  // 的主体其实是这个永不静止的漂移，而不是大图性能。
  //
  // 新结构：布局是**纯函数**（同输入同输出，见 lib/tree_layout.ts），
  // 动画只是"旧坐标 → 新坐标"的补间，不再有物理迭代 ⇒ 不可能抖、不可能超调。
  // ═══════════════════════════════════════════════════════════════════════════

  // 布局模式：auto 按可见规模自动选（≤300 分层 / 否则径向），也可手动锁定
  let layoutMode = $state<'auto' | LayoutMode>(
    (localStorage.getItem('engram-layout-mode') as 'auto' | LayoutMode) || 'auto',
  );
  let levelGap = $state(96);      // 层间距（px）
  let siblingGap = $state(46);    // 同层最小中心距（px）
  // 大图安全阀：世界半径上限（px）。径向布局 1500 节点实测半径约 570，远在阈值内；
  // 仅在极端规模下压缩，避免世界撑到六位数像素
  const MAX_WORLD_RADIUS = 6000;

  // 渐进披露：可见深度（1 = 只看根与第一层）。屏幕像素有物理下限——
  // 实测可读预算约 150–300 节点，所以大图默认只展开浅层，而不是把 1500 个点压成雾。
  let visibleDepth = $state(Number(localStorage.getItem('engram-visible-depth') ?? 3));
  let autoDepth = $state(true);   // 自动：按节点规模选一个"能读"的深度（用户手动调过则关闭）

  function applyLayoutMode(m: 'auto' | LayoutMode) {
    layoutMode = m;
    localStorage.setItem('engram-layout-mode', m);
  }

  // v3.0 补间动画运行时（requestAnimationFrame 句柄；null = 未在运行）
  let layoutRaf: number | null = null;

  // v3.0 可见度派生半径覆盖表：渐进披露裁剪后节点的"可见度"变小，
  // 圆点应随之收敛（不再被看不见的连接撑着）。由 relayout 填充，style 映射读取。
  let nodeSizeOverride: Map<string, number> = new Map();
  // v3.0 被深度裁剪隐藏的节点 id（relayout 自己维护，避免用 el.style() 反查样式）
  let hiddenNodes: Set<string> = new Set();

  /** 上一次布局参数签名（避免重复重排） */
  let lastCamSig = '';

  /** 量取图例面板相对画布的矩形（用于遮挡判定；DOM 侧数据，只在需要时读一次） */
  function measureLegendRect(cyRef: Core | null) {
    if (typeof document === 'undefined') return;
    const el = document.querySelector('.legend') as HTMLElement | null;
    const host = cyRef?.container() as HTMLElement | undefined;
    if (!el || !host) { legendRect = null; return; }
    const a = el.getBoundingClientRect();
    const b = host.getBoundingClientRect();
    legendRect = { x: a.left - b.left, y: a.top - b.top, w: a.width, h: a.height };
  }

  // 兼容旧调用点（clearGraph / onDestroy / 重建路径）：停掉正在跑的补间
  function stopForce() {
    if (layoutRaf !== null) {
      cancelAnimationFrame(layoutRaf);
      layoutRaf = null;
    }
  }

  // 平滑缓动：一次算完的布局不需要物理，只需要"看着它落位"
  const easeOutCubic = (t: number): number => 1 - Math.pow(1 - t, 3);

  // 最近一次布局的元信息（诊断浮层用）
  let layoutInfo = $state<{ mode: LayoutMode; ms: number; visible: number; total: number } | null>(null);
  /** v3.0 最近一次形态选择的两个候选可读性（px），用于解释"为什么选了径向/分层" */
  let lastModePick: { layeredPx: number; radialPx: number } | null = null;
  // v3.0 自检埋点：最近一次 relayout 的入参/结果（回归脚本据此断言核心不变量）
  let lastRelayoutTrace: {
    visSize: number; visNull: boolean; visibleDepth: number;
    targetSize: number; showCount: number; hideCount: number;
  } | null = null;

  /** 当前可见节点集合（渐进披露裁剪）；null = 全部可见 */
  function visibleSet(): Set<string> | null {
    const snap = snapshot;
    if (!snap) return null;
    // 深度裁剪只由 visibleDepth 决定（不按规模设门槛）：
    // 早前小图直接 return null，导致「可见深度」滑条在 80 节点图上完全没反应（实测发现）。
    // 小图默认就是"全部"（autoDepthFor 返回 99），所以不需要额外的规模门槛。
    if (visibleDepth >= 99) return null;
    const depth = shallowDepth(snap);
    const keep = new Set<string>();
    for (const n of snap.nodes) {
      if ((depth.get(n.id) ?? 0) < visibleDepth) keep.add(n.id);
    }
    // 至少留 2 个节点，否则整体隐藏反而像"图没了"
    return keep.size >= 2 ? keep : null;
  }

  /**
   * 真正的根节点 id。
   *
   * 踩坑记录：一直以为 `snap.manifest.root` 是根节点 id，实测它是**工作区路径**
   * （如 `D:\TA`），拿它当 id 去查节点必然查不到——于是真正的根（无父节点者，
   * 如 `知识库索引`）在深度表里拿不到 depth，深度裁剪时被判成"深度未知"而排除，
   * 结果根节点被隐藏、图上只剩几个散点（实测踩过）。
   * 正确口径：出现在任何边 child 端的节点都不是根；优先取 manifest.root 若它确实是节点 id。
   */
  function rootNodeId(snap: ChainSnapshot): string | null {
    const idSet = new Set(snap.nodes.map((n) => n.id));
    const mr = snap.manifest?.root;
    if (mr && idSet.has(mr)) {
      const isChild = snap.edges.some((e) => e.child === mr);
      if (!isChild) return mr;
    }
    const hasParent = new Set(snap.edges.map((e) => e.child));
    for (const n of snap.nodes) if (!hasParent.has(n.id)) return n.id;
    return snap.nodes.length > 0 ? snap.nodes[0].id : null;
  }

  /** 轻量深度表（只走 parent 边，O(n)；布局内部还会自己算一遍，但那是在裁剪之后） */
  function shallowDepth(snap: ChainSnapshot): Map<string, number> {
    const children = new Map<string, string[]>();
    for (const n of snap.nodes) children.set(n.id, []);
    const hasParent = new Set<string>();
    for (const e of snap.edges) {
      children.get(e.parent)?.push(e.child);
      hasParent.add(e.child);
    }
    const depth = new Map<string, number>();
    const rid = rootNodeId(snap);
    const roots: string[] = [];
    if (rid) roots.push(rid);
    for (const n of snap.nodes) if (!hasParent.has(n.id) && n.id !== rid) roots.push(n.id);
    for (const r of roots) {
      if (depth.has(r)) continue;
      depth.set(r, 0);
      const q = [r];
      let h = 0;
      while (h < q.length) {
        const cur = q[h++];
        for (const c of children.get(cur) ?? []) {
          if (!depth.has(c)) {
            depth.set(c, (depth.get(cur) ?? 0) + 1);
            q.push(c);
          }
        }
      }
    }
    // 未被覆盖的（悬空/不可达）按最深处理：宁可让它默认可见，也不要凭空隐藏
    for (const n of snap.nodes) if (!depth.has(n.id)) depth.set(n.id, 0);
    return depth;
  }

  /** 自动深度：在"看得见结构"和"一屏放得下"之间取平衡 */
  function autoDepthFor(total: number): number {
    if (total <= 300) return 99;    // 不裁剪
    if (total <= 800) return 4;
    if (total <= 2000) return 3;
    return 2;
  }

  /**
   * 重排全图（v3.0 核心）。
   * - 位置：computeTreeLayout 纯函数一次算完（O(n)，毫秒级）
   * - 动画：旧坐标 → 新坐标的 300ms 缓动补间；期间不跑任何物理
   * - 收敛：补间结束即完全静止（不再有任何 rAF）
   * @param fit 结束后是否平滑适配视口
   */
  function relayout(cyRef: Core, fit = true) {
    stopForce();
    const snap = snapshot;
    if (!snap || snap.nodes.length === 0) return;

    const total = snap.nodes.length;
    const vis = visibleSet();
    const visibleCount = vis ? vis.size : total;
    let reqMode: LayoutMode = layoutMode === 'auto' ? pickMode(visibleCount) : layoutMode;
    // 形态选择：auto 时把分层/径向**都估一遍**比可读性（见 chooseLayoutMode 的判据演进记录）；
    // 手动锁定形态则尊重用户，不再自动切换。
    if (layoutMode === 'auto') {
      const pick = chooseLayoutMode(
        snap, vis, siblingGap, levelGap,
        Math.max(cyRef.width(), 320), Math.max(cyRef.height(), 240),
      );
      reqMode = pick.mode;
      lastModePick = { layeredPx: Math.round(pick.layeredPx * 10) / 10, radialPx: Math.round(pick.radialPx * 10) / 10 };
    } else {
      lastModePick = null;
    }

    const result = computeTreeLayout(snap, vis, {
      mode: reqMode,
      levelGap,
      siblingGap,
      // radial 的环间距直接用「层间距」滑条——早前这里固定成 DEFAULT_LAYOUT_OPTIONS.ringGap，
      // 导致径向模式下"层间距"滑条完全没反应（实测发现），用户看不到任何反馈
      ringGap: levelGap,
      maxRadius: MAX_WORLD_RADIUS,
    });
    layoutInfo = { mode: result.mode, ms: result.ms, visible: visibleCount, total };
    // v3.0 自检埋点：记录本次重排的入参/结果。核心不变量 = targetSize 应等于可见集规模
    // （除被用户钉住的节点外）——"索引错位导致部分可见节点拿不到坐标"那个 bug 就是在这里暴露的。
    lastRelayoutTrace = {
      visSize: vis ? vis.size : -1,
      visNull: vis === null,
      visibleDepth,
      targetSize: 0,
      showCount: 0,
      hideCount: 0,
    };

    // id → 目标坐标（只对可见节点写入）
    const target = new Map<string, { x: number; y: number }>();
    const sizeMap = new Map<string, number>();
    snap.nodes.forEach((n, i) => {
      if (vis && !vis.has(n.id)) return;
      const p = result.positions[i];
      const sz = result.sizes[i];
      if (p) {
        target.set(n.id, p);
        if (sz !== undefined) sizeMap.set(n.id, sz);
      }
    });
    // v3.0 钉住的节点（用户手动拖过）：只跳过"位置归位"，仍参与可见性/半径判断
    const locked = new Set<string>();
    cyRef.nodes().forEach((nd: any) => { if (nd.locked()) locked.add(nd.id()); });

    // 可见性同步：被裁掉的节点不渲染（cytoscape 用 display:none，保留元素身份与状态）。
    // v3.0 性能：不用 el.style('display') 反查（那会逐元素重算样式，1500 节点下很贵），
    // 直接与布局层自己维护的 hiddenNodes 集合求差。
    //
    // 关键修复：hiddenNodes 必须是**完整期望集**（所有不在 target 的节点），
    // 不能用本帧新增的 hideNodes 做增量赋值——从"全部展开"跳到更浅的深度时
    // （如 depth 99→2），首帧时 wasHidden 全为 false，hideNodes 只装到"深层节点"，
    // 而 target 里被排除掉的可见层节点（如根）既不在 hideNodes、又不在 hiddenNodes，
    // 于是"该藏的没藏、该露的被漏"，根节点被误隐藏，图上只剩 1 个点（实测踩过）。
    const showNodes: any[] = [];
    const hideNodes: any[] = [];
    const nextHidden = new Set<string>();
    cyRef.nodes().forEach((nd: any) => {
      const wantHidden = !target.has(nd.id());
      if (wantHidden) nextHidden.add(nd.id());
      const wasHidden = hiddenNodes.has(nd.id());
      if (wasHidden && !wantHidden) showNodes.push(nd);
      else if (!wasHidden && wantHidden) hideNodes.push(nd);
    });
    const hideEdges = cyRef.edges().filter((ed: any) =>
      !target.has(ed.source().id()) || !target.has(ed.target().id()),
    );
    if (lastRelayoutTrace) {
      lastRelayoutTrace.targetSize = target.size;
      lastRelayoutTrace.showCount = showNodes.length;
      lastRelayoutTrace.hideCount = hideNodes.length;
    }
    cyRef.batch(() => {
      for (const nd of hideNodes) nd.style('display', 'none');
      for (const ed of hideEdges) ed.style('display', 'none');
      for (const nd of showNodes) nd.style('display', 'element');
      cyRef.edges().forEach((ed: any) => {
        const want = target.has(ed.source().id()) && target.has(ed.target().id());
        if (want) ed.removeStyle('display');
      });
      // 半径随可见度重算（裁剪后度变小 → 圆点收敛，不再被"看不见的连接"撑着）
      nodeSizeOverride = sizeMap;
      hiddenNodes = nextHidden;
      cyRef.style().update();   // 一次性重算样式（含 width/height 的 nodeSize 映射）
    });

    const visibleEles = cyRef.elements().filter((el: any) => !hiddenNodes.has(el.id())
      && !(el.isEdge() && (hiddenNodes.has(el.source().id()) || hiddenNodes.has(el.target().id()))));
    if (visibleEles.length === 0) return;

    // 起点快照（新节点没有旧位置 → 直接落到目标，避免从 (0,0) 飞入）
    const from = new Map<string, { x: number; y: number }>();
    visibleEles.nodes().forEach((nd: any) => {
      const p = nd.position();
      from.set(nd.id(), { x: p.x, y: p.y });
    });

    const DUR = 300;
    const t0 = performance.now();
    const step = () => {
      layoutRaf = null;
      const t = Math.min(1, (performance.now() - t0) / DUR);
      const k = easeOutCubic(t);
      cyRef.batch(() => {
        visibleEles.nodes().forEach((nd: any) => {
          const id = nd.id();
          if (locked.has(id)) return;   // 钉住的节点不归位
          const to = target.get(id);
          if (!to) return;
          const f = from.get(id) ?? to;
          nd.position({
            x: f.x + (to.x - f.x) * k,
            y: f.y + (to.y - f.y) * k,
          });
        });
      });
      if (t < 1) {
        layoutRaf = requestAnimationFrame(step);
      } else {
        // 落位后处理：
        //  - 先按"与标签无关"的包围盒适配视口（fitVisible；fit 不能把标签算进去，否则与
        //    "标签预算随 zoom 变化"构成振荡回路）
        //  - 再按新的 zoom 重算标签预算（此时 zoom 已定，标签是 zoom 的纯函数）
        if (fit) {
          const focusNow = focusSet;
          if (focusNow) {
            const focusEles = cyRef.nodes().filter(
              (nd: any) => focusNow.has(nd.id()) && !hiddenNodes.has(nd.id()),
            );
            if (focusEles.length > 0) {
              cyRef.animate({ fit: { eles: focusEles, padding: 90 }, duration: 260, easing: 'ease-out' });
            }
          } else {
            fitVisible(cyRef, { animate: true, padding: 60 });
          }
        }
        // v3.3 图上不显示名称，无需在布局落位后重算任何文字
        // 图例遮挡检查（图例是 DOM 覆盖层，看不到图，只能用几何判定）
        setTimeout(() => { const c = cy; if (c) { measureLegendRect(c); autoCollapseLegend(c); } }, 380);
      }
    };
    layoutRaf = requestAnimationFrame(step);
  }

  /**
   * 适配视口（v3.0 统一出口）。
   *
   * 两个坑都在这里踩过：
   *  1. fit 的元素集必须排除被深度裁剪隐藏的节点，否则看不见的节点也算进包围盒。
   *     注意不要用 el.style('display') 反查（逐元素样式重算，1500 节点很贵），
   *     直接用布局层维护的 hiddenNodes 集合。
   *  2. **必须用"与标签无关"的包围盒**：cytoscape 的 fit 走的是元素渲染包围盒，
   *     而标签属于渲染包围盒 —— 于是"标签显示 → 包围盒变大 → zoom 变小 → 标签隐藏 → 包围盒变小"
   *     构成振荡回路。这里显式用 `nodes.boundingBox()` 加上节点半径自己算，
   *     标签再多也不影响取景，标签预算因此是 zoom 的纯函数（无反馈）。
   */
  function fitVisible(cyRef: Core, o: { animate?: boolean; padding?: number } = {}) {
    const nodes = cyRef.nodes().filter((nd: any) => !hiddenNodes.has(nd.id()));
    if (nodes.length === 0) return;
    const bb = nodes.boundingBox();
    const pad = o.padding ?? 60;
    // 把节点半径与标签高度余量算进去（否则边缘节点的标签会贴边被裁）
    const inflate = 34;
    const zoomTarget = Math.min(
      Math.max(cyRef.width() - pad * 2, 1) / Math.max(bb.w + inflate * 2, 1),
      Math.max(cyRef.height() - pad * 2, 1) / Math.max(bb.h + inflate * 2, 1),
    );
    const z = Math.max(cyRef.minZoom(), Math.min(cyRef.maxZoom(), zoomTarget));
    // cytoscape 的 center 只接受元素集，不接受坐标点 → 直接算 pan：
    //   屏幕位置 = 世界坐标 × zoom + pan，要让图心落在视口中心，故 pan = 视口中心 − 图心 × zoom
    const pan = {
      x: cyRef.width() / 2 - (bb.x1 + bb.w / 2) * z,
      y: cyRef.height() / 2 - (bb.y1 + bb.h / 2) * z,
    };
    if (o.animate) {
      cyRef.animate({ zoom: z, pan }, { duration: 260, easing: 'ease-out' });
    } else {
      cyRef.zoom(z);
      cyRef.pan(pan);
    }
  }

  let chainDir = $state<string | null>(null);
  let lastDir: string | null = null;   // v2.0：跟踪已加载目录，切换时清图（非响应式）
  let snapshot = $state<ChainSnapshot | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let selectedNode = $state<ChainNode | null>(null);
  // v2.6 侧栏常驻：点空白 = 收起为右缘细条（保留所选节点），顶部按钮拉出；单击节点切换显示内容
  let sidebarCollapsed = $state(false);
  // v2.6 双击聚焦视图：focusSet = 聚焦范围内（BFS ≤ 6 层）的节点集合；null = 全局视图
  let focusNodeId: string | null = null;
  let focusSet: Set<string> | null = null;
  let showCreate = $state(false);
  // v2.12 归档视图开关（加性）：淡色虚线纳入归档节点（默认关，零破坏）
  let showArchived = $state(false);
  // v2.19 显示方式切换：图谱视图 ↔ 阅读模式（节点文件树 + 全文阅读）。
  // ⚠️ 阅读模式是人类专属视图（刻意设计）：不写工作区任何文件、不注册 MCP 工具、不进 AI 指南副本、
  //    不进 __engramDebug 调试接缝；视图偏好只落 GUI 本地 localStorage——AI 读 .chain/ 看不到痕迹，
  //    既无法识别也无法使用（详见 src/lib/ReaderMode.svelte 头部不变量说明）。
  let readMode = $state(localStorage.getItem('engram-view-mode') === 'read');
  // v2.12 重嵌按钮（记忆层 L2）：状态消息
  let reindexMsg = $state<string | null>(null);
  let reindexBusy = $state(false);

  async function handleReindex() {
    if (!chainDir || reindexBusy) return;
    reindexBusy = true;
    reindexMsg = null;
    try {
      const msg = await invoke<string>('reindex_embeddings', { dir: chainDir });
      reindexMsg = `✓ ${msg}`;
    } catch (e) {
      reindexMsg = `重嵌失败：${String(e)}（模型缺失时可先用关键词检索）`;
    } finally {
      reindexBusy = false;
    }
  }

  // ── v2.19 阅读模式（人专用）：图结构 ↔ 节点文件树的显示方式切换 ────────────
  // 只切"怎么显示"：不动数据、不动布局参数、不动侧栏编辑语义（ARCHITECTURE §5 加性改动）。
  function enterReadMode() {
    readMode = true;
    localStorage.setItem('engram-view-mode', 'read');
    // 覆盖层下水面/涟漪不可见：停掉渲染循环（退出时恢复），阅读时不空烧 CPU
    stopWaterLoop();
  }

  function exitReadMode() {
    readMode = false;
    localStorage.setItem('engram-view-mode', 'graph');
    startWaterLoop();
  }

  function toggleReadMode() {
    if (readMode) exitReadMode();
    else enterReadMode();
  }

  // 阅读模式下选中的节点 = 图上选中的节点（退出阅读模式即落在刚读的那篇上）
  function handleReadSelect(n: ChainNode | null) {
    selectedNode = n;
  }

  // 阅读模式里的「⧉ 代码骨架」：复用全屏代码页（z 更高，Esc 先关它）
  function handleReadOpenCode(n: ChainNode) {
    selectedNode = n;
    panel.codeFullscreen = true;
  }

  // 阅读模式里的「在图谱中定位」：退出阅读模式并把该节点居中高亮（看图结构关系）
  function handleReadLocate(id: string) {
    exitReadMode();
    jumpToNode(id);
  }

  // ── v2.20 文件树模式 = 人的编辑面（写路径全部走 core 守门）────────────────
  // 结构编辑走后端 *_human 通道：开发模式规则不变；分析模式允许人编辑结构，
  // 但 core 内守协议护栏（新建必挂父节点、禁删根/删带子节点的节点、改链接禁成环）。
  // MCP 工具仍走非 human 版本——AI 侧工具契约与行为零变化。

  // v2.20 自写窗口：我们自己的写入会让 watcher 在 300-450ms 后推来"这次写入之前"的扫描结果，
  // 晚到的旧 payload 会覆盖刚写回的 snapshot（表现为新建节点正文闪回旧值）。
  // 写入期间忽略 watcher 推送（写入返回的 snapshot 才是最新）；窗口外的外部写入照常实时刷新。
  let selfWriteUntil = 0;
  function markSelfWrite() {
    selfWriteUntil = performance.now() + 1200;
  }

  /** 保存内容字段（标题/状态/标签/正文）：与信息栏保存同一条 update_node */
  async function handleReadSave(nodeId: string, fields: { title: string; status: NodeStatus | null; body: string; tags: string[]; evidence: string[] }) {
    if (!chainDir) return;
    markSelfWrite();
    const newSnapshot = await invoke<ChainSnapshot>('update_node', {
      dir: chainDir,
      nodeId,
      fields,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    selectedNode = newSnapshot.nodes.find((x) => x.id === nodeId) ?? null;
  }

  /** 新建节点（文件树里的「＋ 新建」）：挂到选定父节点下，图谱与文件树同一份链同时长出它 */
  async function handleReadCreate(input: {
    id: string;
    title: string;
    nodeType: NodeType;
    status: NodeStatus | null;
    parent: string | null;
    rel: string;
    tags: string[];
    body: string;
  }): Promise<string | null> {
    if (!chainDir) return null;
    markSelfWrite();
    const before = new Set((snapshot?.nodes ?? []).map((n) => n.id));
    const created0 = await invoke<ChainSnapshot>('create_node_human', {
      dir: chainDir,
      input: {
        id: input.id || null,
        title: input.title,
        node_type: input.nodeType,
        status: input.status,
        parent: input.parent,
        rel: input.rel,
      },
      mode: scanMode,
    });
    const created = created0.nodes.find((n) => !before.has(n.id)) ?? null;
    // 先落地结构：即便接下来的内容写入失败，新节点/挂载也已经进树进图（错误照常抛给界面）
    snapshot = created0;
    selectedNode = created;
    // 正文/标签：CreateNodeInput 不含这两个字段（与 MCP 契约同构）→ 建完立刻补一次内容写入
    if (created && ((input.body ?? '').trim() !== '' || input.tags.length > 0)) {
      const finalSnapshot = await invoke<ChainSnapshot>('update_node', {
        dir: chainDir,
        nodeId: created.id,
        fields: {
          title: input.title,
          status: scanMode === 'dev' ? null : input.status,
          body: input.body,
          tags: input.tags,
          evidence: [],
        },
        mode: scanMode,
      });
      snapshot = finalSnapshot;
      selectedNode = finalSnapshot.nodes.find((n) => n.id === created.id) ?? created;
    }
    return created?.id ?? null;
  }

  /** 删除节点（两段式确认在文件树界面里完成）：分析模式不能删根、不能删还有子节点的节点 */
  async function handleReadDelete(nodeId: string) {
    if (!chainDir) return;
    markSelfWrite();
    const newSnapshot = await invoke<ChainSnapshot>('delete_node_human', {
      dir: chainDir,
      nodeId,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    selectedNode = null;
  }

  /** 改挂载位置（父节点 + 关系）：分析模式不允许断成根、不允许成环 */
  async function handleReadSetParent(nodeId: string, parent: string | null, rel: string) {
    if (!chainDir) return;
    markSelfWrite();
    const newSnapshot = await invoke<ChainSnapshot>('set_parent_human', {
      dir: chainDir,
      nodeId,
      parent,
      rel,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    selectedNode = newSnapshot.nodes.find((x) => x.id === nodeId) ?? null;
  }

  // v2.1 多工作区：左侧栏管理；每个文件夹绑定自己的模式（.chain/.mode 标签）
  let workspaces = $state<WorkspaceInfo[]>([]);
  let wsBusy = $state(false);
  let wsError = $state<string | null>(null);

  // v2.1 当前模式：由左侧栏页签决定，持久化（重启后回到上次模式层）
  let scanMode = $state<ScanMode>(
    (localStorage.getItem('chain-gui-mode') as ScanMode) ?? 'analysis'
  );

  function refreshWorkspaces() {
    invoke<WorkspaceInfo[]>('list_workspaces')
      .then((ws) => (workspaces = ws))
      .catch((e) => (wsError = String(e)));
  }

  function clearGraph() {
    chainDir = null;
    snapshot = null;
    selectedNode = null;
    sidebarCollapsed = false;
    // v2.19 工作区没了就没有可读的东西：阅读模式一并退出
    if (readMode) exitReadMode();
    focusNodeId = null;      // v2.6 切模式层重置双击聚焦
    focusSet = null;
    hoverTip = null;
    lastDir = null;
    lastIdsSig = 0;
    lastDataSig = 0;
    lastSliderSig = '';
    stopForce();
    // v3.0 布局态清理：跨工作区残留的半径覆盖/隐藏集合会让新图错位
    nodeSizeOverride = new Map();
    hiddenNodes = new Set();
    layoutInfo = null;
    clearRipple();   // v2.2 切工作区时涟漪一并清理
    cy?.elements().remove();
    cy?.elements().removeClass('focus-dim focus-lit edge-hover');
  }

  // v2.1 打开一个工作区（模式由标签决定，后端强校验）
  async function openWorkspace(ws: WorkspaceInfo) {
    chainDir = ws.path;
    scanMode = ws.mode === 'dev' ? 'dev' : 'analysis';
    localStorage.setItem('chain-gui-mode', scanMode);
    localStorage.setItem('chain-gui-last-dir', ws.path);
    await loadChain();
  }

  // v2.1 切换模式页签：默认打开该层第一个工作区；该层为空则清空画布
  async function handleSwitchMode(m: ScanMode) {
    if (m === scanMode) return;
    scanMode = m;
    localStorage.setItem('chain-gui-mode', m);
    const first = workspaces.find((w) => w.mode === m);
    if (first) {
      await openWorkspace(first);
    } else {
      clearGraph();
      startWaterLoop();   // v2.4 两模式统一水面
    }
  }

  // v2.1 添加工作区：选目录 → 后端按当前页签模式初始化/补签标签 → 归层 → 自动打开
  async function handleAddWorkspace() {
    if (wsBusy) return;
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== 'string') return;
    wsBusy = true;
    wsError = null;
    try {
      const list = await invoke<WorkspaceInfo[]>('add_workspace', { dir: selected, mode: scanMode });
      workspaces = list;
      const added = list.find((w) => w.path.toLowerCase() === selected.toLowerCase());
      if (added) await openWorkspace(added);
    } catch (e) {
      wsError = String(e);
    } finally {
      wsBusy = false;
    }
  }

  // v2.1 移除工作区：仅移出列表；若移除的是当前打开的，清空画布
  async function handleRemoveWorkspace(dir: string) {
    try {
      workspaces = await invoke<WorkspaceInfo[]>('remove_workspace', { dir });
      if (chainDir === dir) clearGraph();
    } catch (e) {
      wsError = String(e);
    }
  }

  // ── v2.2 涟漪视图 ────────────────────────────────────────────────────
  // 设计：点击主节点 → 波前沿链接逐层扩散（350ms/层，上限 6 层）；
  //       同层同亮度、逐级指数衰减；层越近呼吸脉动越强、边脉冲越快；
  //       主节点持续发射 2-3 圈扩散涟漪环；再点停止（点空白/Esc 不停）。
  // v2.4 两模式差异：分析模式 maxDepth=1 —— 波环照常扩满全场，但只有
  //       点击节点(d0)与直接相连(d1)点亮并震动，更远节点保持压暗
  //       （严格"只有有关系的节点受影响"，避免图被整片点亮）；
  //       开发模式 maxDepth=6 —— 亮度与震动随波前逐层铺开（自由图谱语义）。
  // 技术：BFS 分层（src/lib/ripple.ts 纯逻辑已无头测试）+ 类样式 +
  //       RAF（呼吸缩放 + overlay canvas 涟漪环）。
  let ripple = $state<{ source: string; activeDepth: number; layers: RippleLayers; maxDepth: number } | null>(null);
  let rippleTimer: ReturnType<typeof setInterval> | ReturnType<typeof setTimeout> | undefined;
  // v2.3 波源列表（细环涟漪）：main=主波源（亮度分层按它计算）；
  // level 逐帧渐入渐出（点击=生成源、再点=逐渐停止，均有过渡）；
  // radPx = 场半径（以该波源为圆心、到最远节点中心的距离，圆外无涟漪）
  let waveSources = $state<{ id: string; main: boolean; level: number; target: number; gx: number; gy: number; radPx: number }[]>([]);
  // 点击瞬间的"沉水"动画（主节点先轻轻沉一下再起波）
  let dips = $state<{ id: string; t0: number }[]>([]);
  // 当前被涟漪缩放动画覆盖的节点（停止时清理样式旁路，防残留）
  let rippleScaled = new Set<string>();
  let dragging = false;
  // v2.3 水面画布（开发模式）：深海军蓝基底 + 细线同心涟漪环（无波峰波谷着色）
  let waterCanvas: HTMLCanvasElement;
  let waterCtx: CanvasRenderingContext2D | null = null;
  let waterRaf: number | null = null;
  let waterFrame = 0;

  // ── v2.3 涟漪参数（测试面板，用户可调）──
  const waveParams = $state({
    energy: 0.55,      // 能量：环透明度与振动幅度
    period: 1.6,       // 周期（秒/圈，环从中心扩到边界的时间）
    lineWidth: 1.0,    // 粗细：环线宽（px，0.5–2.5）
    fade: 1.2,         // 衰减：环扩张过程中的透明度衰减速度（0.3–2.5，越大淡得越快）
    contrast: 0.28,    // v2.4 亮度对比：每层的亮度衰减比例（越大对比越强，越远越暗）
  });
  let wavePanelOpen = $state(true);

  function buildAdjacency(snap: ChainSnapshot): Map<string, string[]> {
    // 涟漪邻接来自数据（snapshot.edges）——开发模式不渲染连线，但联系数据仍在
    const adj = new Map<string, string[]>();
    for (const n of snap.nodes) adj.set(n.id, []);
    for (const e of snap.edges) {
      adj.get(e.parent)?.push(e.child);
      adj.get(e.child)?.push(e.parent);   // 知识链接不分方向：无向传播
    }
    return adj;
  }

  // v2.6 双击聚焦：拉近到以节点为中心的 BFS ≤ 6 层关系范围（明显拉近效果）；
  // 再双击同一节点回到全局视图。聚焦只动视口，不影响布局与波纹。
  /**
   * 双击聚焦 = **真的拉近看**（v3.2 修复"双击切不到近处"）。
   *
   * 旧实现的问题：`animate({ fit: {eles, padding}, zoom: targetZoom })` —— cytoscape 的
   * `fit` 与 `zoom` 同时给时 **fit 优先、zoom 被忽略**（fit 内部自己算缩放）。
   * 于是那个"至少放大 1.25 倍"的 targetZoom 从来没生效，双击只是重新适配了一下视野，
   * 观感上就是"点了没反应、切不到近处"（用户反馈）。
   *
   * 现在改为完全显式：自己算目标 zoom 与 pan，不再依赖 fit。
   * 语义：双击节点 = 以它为中心拉到**舒适阅读尺度**（标签能看清），
   *      再双击同一节点 / Esc / 点空白 = 退回全图。
   */
  const FOCUS_MIN_MAGNIFY = 1.8;   // 相对当前 zoom 的最小放大倍数（保证"确实拉近了"）
  const FOCUS_MAX_ZOOM = 4;        // 与 maxZoom 一致，避免糊

  function toggleFocus(cyRef: Core, nodeId: string) {
    if (!snapshot) return;
    // 退出聚焦：回到全图（用与 relayout 同一套"与标签无关"的取景，避免被标签撑大）
    if (focusNodeId === nodeId) {
      focusNodeId = null;
      focusSet = null;
      applyFocusClasses(cyRef, null);
      fitVisible(cyRef, { animate: true, padding: 60 });
      return;
    }
    const layers = computeRippleLayers(buildAdjacency(snapshot), nodeId);
    // 聚焦范围：按图规模自适应跳数（v3.2）。
    // 原来固定 6 层，在小图上几乎等于全图——实测 104 节点的图 focusSet=103，
    // 于是"只有 1 个节点被压暗"，聚焦等于没有视觉层次（用户"不觉得看近了"的真因）。
    // 现在小图用 1–2 跳（真正的"局部放大"），大图放宽到 3–8 跳。
    const total = snapshot.nodes.length;
    const hops = total <= 60 ? 1 : total <= 200 ? 2 : total <= 600 ? 3 : total <= 1500 ? 6 : 8;
    const set = new Set<string>();
    layers.byDepth.forEach((arr, d) => {
      if (d <= hops) arr.forEach((id) => set.add(id));
    });
    focusNodeId = nodeId;
    focusSet = set;

    // 可见集合（排除深度裁剪隐藏的节点，否则包围盒会被看不见的点撑大）
    const foc = cyRef.nodes().filter((nd: any) => set.has(nd.id()) && !hiddenNodes.has(nd.id()));
    if (foc.length === 0) return;
    const bb = foc.boundingBox();
    const cur = cyRef.zoom();
    // 目标缩放 = max(刚好装下这簇, 至少放大 FOCUS_MIN_MAGNIFY 倍)，封顶 maxZoom。
    // 两个分支的语义：
    //   簇很大（装下也没放大多少）→ 取放大 1.8 倍，宁可裁掉边缘也让人"看到近处"
    //   簇较小（fit 本身就放大好几倍）→ 取 fit，一屏刚好装下整簇
    const fitZoom = Math.min(
      (cyRef.width() - 160) / Math.max(bb.w + 40, 1),
      (cyRef.height() - 160) / Math.max(bb.h + 40, 1),
    );
    const z = Math.max(
      cyRef.minZoom(),
      Math.min(cyRef.maxZoom(), Math.min(FOCUS_MAX_ZOOM, Math.max(fitZoom, cur * FOCUS_MIN_MAGNIFY))),
    );
    // 以被双击节点为中心（而不是包围盒中心）：用户点哪个就看哪个
    const node = cyRef.getElementById(nodeId);
    const c = node.nonempty() ? node.position() : { x: bb.x1 + bb.w / 2, y: bb.y1 + bb.h / 2 };
    const pan = { x: cyRef.width() / 2 - c.x * z, y: cyRef.height() / 2 - c.y * z };
    // 聚焦态视觉层次（v3.2）：把范围外的节点压暗。
    // 不加这一步时，背景几十个同亮度圆点会让人"不觉得看近了"（实测：双击后视口内 46–48 个
    // 节点、zoom 放大 1.8 倍，但因为没有层次，观感仍是"一整片"）。
    applyFocusClasses(cyRef, set);
    cyRef.animate({ zoom: z, pan }, { duration: 380, easing: 'ease-in-out' });
    // ⚠️ 必须等**动画落定后**再算标签：在 zoom 动画进行中算，标签是按中间态的 zoom 装箱的，
    // 动画结束 zoom 变了、装箱结果随之错位（实测：聚焦态出现 17.6% 的标签重叠、最严重一对 63%）。
    // 这里等 420ms（> 380ms 动画）+ 一帧余量。
  }

  /** 聚焦态的类切换：范围内点亮、范围外压暗（样式见 node.focus-dim / focus-lit） */
  function applyFocusClasses(cyRef: Core, set: Set<string> | null) {
    cyRef.batch(() => {
      if (!set) {
        cyRef.elements().removeClass('focus-dim focus-lit');
        return;
      }
      cyRef.nodes().forEach((nd: any) => {
        if (set.has(nd.id())) { nd.removeClass('focus-dim'); nd.addClass('focus-lit'); }
        else { nd.removeClass('focus-lit'); nd.addClass('focus-dim'); }
      });
      cyRef.edges().forEach((ed: any) => {
        const inSet = set.has(ed.source().id()) && set.has(ed.target().id());
        if (inSet) { ed.removeClass('focus-dim'); ed.addClass('focus-lit'); }
        else { ed.removeClass('focus-lit'); ed.addClass('focus-dim'); }
      });
    });
  }

  /**
   * 统一退出聚焦：清类 + 重置状态 + 重算标签。
   * 三者必须一起做——早前多处只调 removeClass 而不重置 focusSet，
   * 于是"视觉上已退出聚焦、但 focusSet 还在"，后续 fit/labels 的行为与画面脱节。
   */
  function clearFocus(cyRef: Core | null) {
    focusNodeId = null;
    focusSet = null;
    if (cyRef) cyRef.elements().removeClass('focus-dim focus-lit');
  }

  /**
   * 以**视口中心**为锚点缩放（v3.2）。
   *
   * 踩坑：按钮原来直接 `cy.zoom(cy.zoom()*1.4)`。cytoscape 的 `zoom(数字)` 只改倍率、不动 pan，
   * 而屏幕位置 = 世界坐标×zoom + pan —— 于是缩放变成"绕世界原点"，放大时画面会整体漂走
   * （观感＝"放大后图跑了"）。
   * 正确做法：`cy.zoom({ level, renderedPosition })` —— 传坐标点形式时 cytoscape 会
   * 保持该点不动地缩放，等价于"以视口中心为锚"。
   */
  function zoomBy(cyRef: Core | null, factor: number) {
    if (!cyRef) return;
    const old = cyRef.zoom();
    const next = Math.max(cyRef.minZoom(), Math.min(cyRef.maxZoom(), old * factor));
    if (Math.abs(next - old) < 1e-6) return;
    cyRef.zoom({ level: next, renderedPosition: { x: cyRef.width() / 2, y: cyRef.height() / 2 } });
  }

  function applyRippleClasses(cyRef: Core, activeDepth: number) {
    const rip = ripple;
    if (!rip) return;
    cyRef.batch(() => {
      if (activeDepth === 0) {
        // 首次：全图归一——波源点亮，其余压暗（v2.13 起只在波前推进时增量点亮新层，
        // 不再每 350ms 全图重写 class——大图扩散期减少无谓样式抖动）
        cyRef.nodes().forEach((n) => {
          n.removeClass('rip-dim rip-d0 rip-d1 rip-d2 rip-d3 rip-d4 rip-d5 rip-d6');
          const d = rip.layers.depth.get(n.id());
          if (d === 0) n.addClass('rip-d0');
          else n.addClass('rip-dim');   // 波外或波前未达：压暗等待
        });
      } else {
        // 增量：只点亮本层刚到达的节点（此前各层状态已在上一 tick 落定）
        // v2.15 大图：遍历层数组，不再全图扫描
        const layer = rip.layers.byDepth[activeDepth] ?? [];
        for (const id of layer) {
          const n = cyRef.getElementById(id);
          if (n.nonempty()) {
            n.removeClass('rip-dim');
            n.addClass(`rip-d${activeDepth}`);
          }
        }
      }
      // v2.2 涟漪期间连线整体淡出（transition 0.2s 平滑），联系改由亮度层级+波纹表达
      cyRef.edges().addClass('rip-hide');
    });
  }

  // v2.5 亮度对比参数驱动：默认曲线精心设计为"点击节点与直接相关明显亮、
  // 更深层明显渐暗"——d0=1、d1=0.8、d2=0.4、d3=0.2、d4=0.1、d5=0.05、d6=0.03；
  // 对比滑条 = 相对这条基准曲线的陡峭倍数（默认 0.28 = 1.0 倍），越大深层越暗。
  // 运行时改写 cytoscape 样式表（cy.style().selector()），滑条拖动即时生效。
  function updateRippleStyle(cyRef: Core) {
    const BASE = [1, 0.8, 0.4, 0.2, 0.1, 0.05, 0.03];
    const k = waveParams.contrast / 0.28;   // 默认 1.0 = 设计曲线原样
    // v2.15：链式写全部 7 个 selector 后单次 update（原实现 7 次全图样式重算+重绘）
    let ss = cyRef.style();
    for (let d = 0; d <= 6; d++) {
      const v = Math.pow(BASE[d], k);
      ss = ss.selector(`node.rip-d${d}`).style({ 'opacity': v, 'text-opacity': Math.min(1, v + 0.05) });
    }
    ss.update();
  }

  // 对比滑条变化 → 立即改写涟漪亮度样式（cy 未就绪时跳过，onMount 里会再补一次）
  $effect(() => {
    const c = waveParams.contrast;   // 追踪对比值
    const cyRef = cy;
    if (cyRef) updateRippleStyle(cyRef);
  });

  // v2.6 常驻信息栏为画布预留空间：宽度/收起/图谱有无变化时同步 cytoscape 视口，
  // 收起↔展开切换后重新适配当前视野（聚焦中则适配聚焦范围）
  let lastSbState: boolean | null = null;
  $effect(() => {
    const _c = sidebarCollapsed;
    const _w = panel.width;
    const _has = !!snapshot;
    const cyRef = cy;
    if (!cyRef) return;
    requestAnimationFrame(() => cyRef.resize());
    if (lastSbState !== null && lastSbState !== _c) {
      const f = focusSet;
      const eles = f ? cyRef.nodes().filter((nd) => f.has(nd.id())) : cyRef.elements();
      cyRef.animate({ fit: { eles, padding: 60 }, duration: 250, easing: 'ease-out' });
    }
    lastSbState = _c;
  });

  function clearRipple() {
    if (rippleTimer !== undefined) {
      clearInterval(rippleTimer as any);
      clearTimeout(rippleTimer as any);
      rippleTimer = undefined;
    }
    const cyRef = cy;
    if (cyRef) {
      cyRef.batch(() => {
        cyRef.nodes().removeClass('rip-dim rip-d0 rip-d1 rip-d2 rip-d3 rip-d4 rip-d5 rip-d6');
        cyRef.edges().removeClass('rip-hide');   // v2.2 淡线恢复（transition 平滑过渡回初始）
        // v2.3 清理缩放动画样式旁路（所有节点恢复原始尺寸）
        for (const id of rippleScaled) {
          const ele = cyRef.getElementById(id);
          if (!ele.empty()) {
            ele.removeStyle('width');
            ele.removeStyle('height');
          }
        }
        rippleScaled = new Set();
      });
    }
    ripple = null;
    waveSources = [];   // v2.3 切工作区/模式时全部波源立即停（点按停止走 level 渐变）
  }

  // ── v2.3 细环涟漪水面（开发模式）：深海军蓝基底，无波峰波谷着色 ──
  // 点击节点 = 波源（先"沉一下水"再起波），细线同心圆环持续向四周扩散（周期可调）；
  // 环只在以波源为圆心、到最远节点为半径的圆形域内；波传到哪个节点，哪个节点
  // 周围泛起局部小涟漪（相位随层级滞后）。
  // v2.13 流畅度优化：
  //  - 节点渲染位置缓存（renderedPosition 只在平移/缩放/力模拟时重算——静止大图零矩阵开销）
  //  - 每帧 cytoscape 样式旁路收窄到「波源呼吸 + 沉水」少数元素（原全图脉冲每帧重绘是卡顿主因；
  //    受影响节点的"礁石颤"改由 canvas 局部小环承担，涟漪语义不变）
  //  - 固定描边色 + globalAlpha（省去每环 rgba 字符串分配）；按时长节流 ~30fps（120Hz 屏不翻倍）
  // v2.15 扁平位置缓存：pan/zoom 期间每帧只写 Float64Array（不新建 1500 个 Map 条目/对象）；
  // id→索引只在节点集变化时重建（重建分支重置 posIdsSig）
  let posIdx = new Map<string, number>();
  let posX = new Float64Array(0);
  let posY = new Float64Array(0);
  let posIdsSig = '';
  let posKey = '';
  const posMiss = { x: 0, y: 0 };
  const posOut = { x: 0, y: 0 };
  function ensurePositions(cyRef: Core | null) {
    if (!cyRef) {
      posIdx = new Map();
      posX = new Float64Array(0);
      posY = new Float64Array(0);
      posIdsSig = '';
      posKey = '';
      return;
    }
    const pan = cyRef.pan();
    // 平移/缩放/布局补间运行中 → 位置变化，需重算；否则复用缓存（静止大图每帧零开销）
    const key = `${pan.x.toFixed(1)}|${pan.y.toFixed(1)}|${cyRef.zoom().toFixed(3)}|${layoutRaf !== null ? 1 : 0}`;
    if (key === posKey) return;
    posKey = key;
    if (posIdsSig === '') {
      const ids: string[] = [];
      cyRef.nodes().forEach((n: any) => {
        ids.push(n.id());
      });
      const m = new Map<string, number>();
      ids.forEach((id, i) => m.set(id, i));
      posIdx = m;
      posX = new Float64Array(ids.length);
      posY = new Float64Array(ids.length);
      posIdsSig = 'ok';
    }
    cyRef.nodes().forEach((n: any) => {
      const i = posIdx.get(n.id());
      if (i === undefined) return;
      const p = n.renderedPosition();
      posX[i] = p.x;
      posY[i] = p.y;
    });
  }
  function nodePos(id: string): { x: number; y: number } {
    const i = posIdx.get(id);
    if (i === undefined) return posMiss;
    posOut.x = posX[i];
    posOut.y = posY[i];
    return posOut;   // 共享输出对象：调用方立即消费 p.x/p.y（drawWater 语义）
  }

  function drawWater(cyRef: Core | null, t: number, frame: number) {
    if (!waterCanvas) return;
    // v2.19 阅读模式覆盖层下水面不可见：不绘制（循环若被其它路径重启也保持零开销）
    if (readMode) return;
    if (!waterCtx) waterCtx = waterCanvas.getContext('2d');
    const ctx = waterCtx;
    if (!ctx) return;
    const w = waterCanvas.clientWidth;
    const h = waterCanvas.clientHeight;
    if (w === 0 || h === 0) return;
    if (waterCanvas.width !== w || waterCanvas.height !== h) {
      waterCanvas.width = w;
      waterCanvas.height = h;
    }

    // ── 波源生命周期 ──
    const active: typeof waveSources = [];
    for (const s of waveSources) {
      s.level += (s.target - s.level) * 0.08;
      if (s.level < 0.01 && s.target === 0) continue;
      active.push(s);
    }
    if (active.length !== waveSources.length) {
      waveSources = active.filter((s) => s.level >= 0.01 || s.target > 0);
    }
    const nowMs = performance.now();
    if (dips.length > 0) {
      dips = dips.filter((d) => nowMs - d.t0 < 820);
    }
    // v2.13：波源全部停息 → 清理残留缩放旁路后零绘制（上一帧已擦净，CSS 底色透出）
    if (active.length === 0) {
      if (cyRef && rippleScaled.size > 0) {
        for (const id of rippleScaled) {
          const ele = cyRef.getElementById(id);
          if (!ele.empty()) {
            ele.removeStyle('width');
            ele.removeStyle('height');
          }
        }
        rippleScaled = new Set();
      }
      return;
    }
    ensurePositions(cyRef);
    for (const s of active) {
      const p = nodePos(s.id);
      s.gx = p.x;
      s.gy = p.y;
    }

    // v2.13 60fps：底色由 CSS 承担，每帧只 clearRect 擦掉上一帧的环（比渐变填充快 2-3 倍）
    ctx.clearRect(0, 0, w, h);

    const period = Math.max(0.2, waveParams.period);
    const fadePow = Math.max(0.3, waveParams.fade);
    const energyK = waveParams.energy / 0.55;
    const lw = Math.max(0.5, Math.min(2.5, waveParams.lineWidth));

    // ── 细环：每个波源 3 圈同心细环持续扩散（圆内，边界 80% 起淡）──
    ctx.lineWidth = lw;
    ctx.strokeStyle = '#a5d2ff';   // v2.13：固定描边色，透明度走 globalAlpha（省字符串分配）
    for (const s of active) {
      for (let k = 0; k < 3; k++) {
        const ph = ((t / period) + k / 3) % 1;
        const r = ph * s.radPx;
        const edgeFade = r > s.radPx * 0.8 ? Math.max(0, 1 - (r / s.radPx - 0.8) / 0.2) : 1;
        const alpha = Math.pow(1 - ph, fadePow) * 0.5 * s.level * energyK * edgeFade;
        if (alpha <= 0.01) continue;
        ctx.globalAlpha = alpha;
        ctx.beginPath();
        ctx.arc(s.gx, s.gy, r, 0, Math.PI * 2);
        ctx.stroke();
      }
    }

    // ── 节点局部涟漪：像波浪拍打礁石——小、快、碎（时钟为主波周期的 0.4 倍）──
    const rip = ripple;
    if (cyRef && rip) {
      const mainSrc = active.find((s) => s.main);
      const mainLevel = mainSrc ? mainSrc.level : 0;
      const nodePeriod = Math.max(0.25, period * 0.4);
      // v2.15 大图：只遍历波内节点（byDepth 层数组），不再每帧扫全图 1500 节点
      const byDepth = rip.layers.byDepth;
      const maxD = Math.min(rip.maxDepth, byDepth.length - 1);
      for (let d = 1; d <= maxD; d++) {
        const strength = ripplePulseAmp(d) / 0.1;
        const layer = byDepth[d];
        for (let li = 0; li < layer.length; li++) {
          const p = nodePos(layer[li]);   // v2.13：缓存位置（静止时零矩阵开销）
          for (let k = 0; k < 2; k++) {
            const ph = ((t / nodePeriod) + d * 0.18 + k * 0.5) % 1;
            const r = ph * (18 + strength * 20);
            const alpha = Math.pow(1 - ph, fadePow) * 0.4 * strength * mainLevel * energyK;
            if (alpha <= 0.01) continue;
            ctx.globalAlpha = alpha;
            ctx.beginPath();
            ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
            ctx.stroke();
          }
        }
      }
    }
    ctx.globalAlpha = 1;

    // ── 节点运动（俯视语义）：v2.13 收窄为「波源深呼吸 + 点击沉水」，且**隔帧更新（30Hz）**——
    //    每帧写 cytoscape 样式会以 60Hz 触发整图重绘（大图代价高），而呼吸是慢正弦，
    //    30Hz 视觉无差；高速环动画由 canvas 以 60fps 承担 ──
    if (!cyRef) return;
    if (frame % 2 !== 0) return;
    const scaledNow = new Set<string>();
    cyRef.batch(() => {
      for (const s of active) {
        const ele = cyRef.getElementById(s.id);
        if (ele.empty()) continue;
        const base = nodeSize(ele);
        let scale = 1 + Math.sin((t / (period * 1.6)) * Math.PI * 2) * 0.13 * (s.main ? 1.0 : 0.55) * energyK;
        const dip = dips.find((dd) => dd.id === s.id);
        if (dip) {
          const dt = (nowMs - dip.t0) / 620;
          if (dt < 1) scale *= 1 - Math.sin(Math.PI * dt) * 0.35;   // 沉水：再缩小至 0.65
        }
        ele.style('width', `${base * scale}px`);
        ele.style('height', `${base * scale}px`);
        scaledNow.add(s.id);
      }
    });
    // 清理上一帧仍在缩放、本帧已退出涟漪的波源
    for (const id of rippleScaled) {
      if (!scaledNow.has(id)) {
        const ele = cyRef.getElementById(id);
        if (!ele.empty()) {
          ele.removeStyle('width');
          ele.removeStyle('height');
        }
      }
    }
    rippleScaled = scaledNow;
  }

  // 水面动画循环（按时长节流 ~60fps——环动画位移大（约 600px/s），30fps 每帧跳 20px 可见顿挫；
  //   每帧重活已在上轮优化消除：静态位置缓存、呼吸样式 30Hz、底色走 CSS，60fps 预算充足；
  //   120Hz+ 屏上以 16ms 为上限不超速）
  function startWaterLoop() {
    if (waterRaf !== null) return;
    const t0 = performance.now();
    let lastDraw = 0;
    const tick = () => {
      waterRaf = null;
      const now = performance.now();
      if (now - lastDraw >= 16) {
        lastDraw = now;
        waterFrame += 1;
        drawWater(cy, (now - t0) / 1000, waterFrame);
      }
      waterRaf = requestAnimationFrame(tick);
    };
    waterRaf = requestAnimationFrame(tick);
  }

  function stopWaterLoop() {
    if (waterRaf !== null) {
      cancelAnimationFrame(waterRaf);
      waterRaf = null;
    }
    if (waterCtx) {
      waterCtx.clearRect(0, 0, waterCanvas?.width ?? 0, waterCanvas?.height ?? 0);
    }
  }

  // v2.3 点击节点 = 波源开关：新节点 → 生成波源（首个=主波源，带亮度分层；后续=次级波源，能量弱）；
  // 再点同一节点 → level 渐变归零（逐渐停止，有过渡）。亮度分层仅由主波源驱动。
  // v2.4 分析模式：亮度/震动只到 d1（直接相连），更远节点保持压暗——"只有有关系的节点受影响"。
  function toggleWaveSource(nodeId: string) {
    if (!cy || !snapshot) return;
    const cyRef = cy;
    const existing = waveSources.find((s) => s.id === nodeId);
    if (existing) {
      // 再点同一节点：逐渐停止（level 缓动归零，渲染循环中淡出后移除）
      existing.target = 0;
      if (existing.main) {
        // 主波源停止：亮度层级类立即移除（opacity transition 平滑带回初始）
        if (rippleTimer !== undefined) {
          clearInterval(rippleTimer as any);
          clearTimeout(rippleTimer as any);
          rippleTimer = undefined;
        }
        ripple = null;
        cyRef.nodes().removeClass('rip-dim rip-d0 rip-d1 rip-d2 rip-d3 rip-d4 rip-d5 rip-d6');
        cyRef.edges().removeClass('rip-hide');
      }
      return;
    }

    // 新波源
    const isMain = !waveSources.some((s) => s.main);
    const srcEl = cyRef.getElementById(nodeId);
    const srcPos = srcEl.renderedPosition();
    // v2.3 场半径：以本波源为圆心、到最远节点中心的距离（圆外平静；单节点取最小值）
    // v2.15 大图：包围盒四角近似最远距离（O(1)，不再每点一次扫全图 1500 节点）
    let radPx = 140;
    const bb = cyRef.elements().boundingBox();
    const corners: [number, number][] = [[bb.x1, bb.y1], [bb.x2, bb.y2], [bb.x1, bb.y2], [bb.x2, bb.y1]];
    for (const [bx, by] of corners) {
      const d = Math.hypot(bx - srcPos.x, by - srcPos.y);
      if (d > radPx) radPx = d;
    }
    waveSources.push({
      id: nodeId,
      main: isMain,
      level: 0,
      target: isMain ? 1 : 0.55,
      gx: 0,
      gy: 0,
      radPx,
    });
    // v2.3 点击瞬间"沉水"动画（先轻轻沉一下，波场随之启动）
    dips = [...dips, { id: nodeId, t0: performance.now() }];
    if (isMain) {
      const layers = computeRippleLayers(buildAdjacency(snapshot), nodeId);
      // v2.4 分析模式只让"有关系的节点"（d0 点击 + d1 直接相连）点亮与震动；
      //     开发模式保留全层扩散，深度按节点数分档（v2.15：密集图 6 层≈全图，降深省类与环）
      const maxDepth =
        scanMode === 'analysis' ? 1 : perfPolicy(snapshot.nodes.length).rippleMaxDepth;
      ripple = { source: nodeId, activeDepth: 0, layers, maxDepth };
      applyRippleClasses(cyRef, 0);
      // 波前逐层扩散（亮度/震动只到 maxDepth；波环本身由水面循环持续扩散）
      rippleTimer = setInterval(() => {
        if (!ripple) return;
        ripple.activeDepth += 1;
        applyRippleClasses(cyRef, ripple.activeDepth);
        if (ripple.activeDepth >= ripple.maxDepth || ripple.activeDepth >= ripple.layers.byDepth.length - 1) {
          clearInterval(rippleTimer as any);
          rippleTimer = undefined;
          return;
        }
      }, 350);
    }
  }

  let container: HTMLDivElement;
  let cy: Core | null = null;
  let unlisten: (() => void) | undefined;

  // 重要性 = 大小：连接数（度）越多，圆点越大。
  // v1.4：改为平方根缓增 + 封顶，大小对比温和（Obsidian 风格）：
  //   度0→14px, 度4→22px, 度9→26px, 度16→30px, 度36+→38px（封顶）
  const nodeSize = (ele: any): number => {
    // v3.0 优先用布局层算出的"可见度半径"（渐进披露裁剪后度会变小）；
    // 未参与布局的节点（归档节点、模拟期新增）回退到 cytoscape 实时度
    const o = nodeSizeOverride.get(ele.id());
    if (o !== undefined) return o;
    return 14 + Math.min(Math.sqrt(ele.degree()), 6) * 4;
  };

  // v2.0 边宽度函数：与两端节点大小挂钩（小节点 0.8px → 大节点 2.0px）
  const edgeBaseWidth = (ele: any): number => {
    const s = Math.min(nodeSize(ele.source()), nodeSize(ele.target()));
    return 0.8 + (s - 14) * 0.05;
  };

  // v1.4 图例数据（v2.15 单一来源：从 NODE_TYPE_COLOR 派生，不再手工重复颜色表；标签保持原样）
  const LEGEND_LABEL_EXTRA: Record<NodeType, string> = { goal: '', design: '', task: '', verification: '', note: '（知识库）' };
  const typeLegend = (Object.keys(NODE_TYPE_COLOR) as NodeType[]).map((t) => ({
    t,
    label: `${NODE_TYPE_LABEL[t]} ${t}${LEGEND_LABEL_EXTRA[t]}`,
    color: NODE_TYPE_COLOR[t],
  }));
  let showLegend = $state(true);
  // v3.0 图例遮挡自动收起：图例会盖住图（实测 80 节点图约占 1/3 画布）。
  // 用户显式点过图例开关后就不再自动干预（存 localStorage）。
  let legendUserSet = typeof localStorage !== 'undefined' && localStorage.getItem('engram-legend-user') === '1';
  let legendRect: { x: number; y: number; w: number; h: number } | null = null;

  /**
   * 图例是否压住了图。
   *
   * 判据演进：一开始只看"有多少节点落在图例矩形内"，结果漏判——图例常压在
   * **连线与空白**上（节点是离散的，图例恰好从节点缝隙穿过），此时画面依然被切掉一块。
   * 改为与**图的包围盒**比面积：图例占掉图面积的 12% 以上就认为挡到了。
   */
  function autoCollapseLegend(cyRef: Core) {
    if (legendUserSet || !showLegend || !legendRect) return;
    const zoom = cyRef.zoom();
    const pan = cyRef.pan();
    const r = legendRect;
    // 可见节点在屏幕上的包围盒
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    let count = 0;
    cyRef.nodes().forEach((nd: any) => {
      if (hiddenNodes.has(nd.id())) return;
      const p = nd.position();
      const sx = p.x * zoom + pan.x;
      const sy = p.y * zoom + pan.y;
      if (sx < minX) minX = sx;
      if (sx > maxX) maxX = sx;
      if (sy < minY) minY = sy;
      if (sy > maxY) maxY = sy;
      count++;
    });
    if (count === 0) return;
    const gx0 = Math.max(minX, 0), gy0 = Math.max(minY, 0);
    const gx1 = Math.min(maxX, cyRef.width()), gy1 = Math.min(maxY, cyRef.height());
    const graphArea = Math.max(gx1 - gx0, 1) * Math.max(gy1 - gy0, 1);
    const ox = Math.min(r.x + r.w, gx1) - Math.max(r.x, gx0);
    const oy = Math.min(r.y + r.h, gy1) - Math.max(r.y, gy0);
    const overlap = Math.max(ox, 0) * Math.max(oy, 0);
    if (overlap / graphArea > 0.12) showLegend = false;
  }

  // v2.14 「代码」筛选开关：一键高亮有代码骨架的节点（其余压暗）
  let codeFilter = $state(false);
  // v2.15 性能浮层开关
  let perfOpen = $state(false);

  // v1.7 悬停浮层：显示 id · 类型（id 已从画布标签移除以突出标题命名，悬停/点击可追溯）
  let hoverTip = $state<{ x: number; y: number; text: string } | null>(null);

  const style: StylesheetJson = [
    // v2.2 画布背景透明：分析模式由 CSS 底色覆盖，开发模式透出水面画布
    {
      selector: 'core',
      style: {
        'background-color': 'transparent',
      } as any,
    },
    {
      selector: 'node',
      style: {
        'shape': 'ellipse',
        // v3.3 图谱上**不再显示节点名称**（用户要求：以后不在图上显示）。
        // 节点的身份靠：类型配色 + 大小（度）+ 悬停浮层（显示 id · 类型 + 角标）+ 点击右侧信息栏。
        // 下面所有 text-* 属性保持"显式关闭"，避免 cytoscape 默认值把 label 又画出来。
        'label': '',
        'text-opacity': 0,
        'min-zoomed-font-size': 0,
        'text-outline-width': 0,
        'text-wrap': 'none',
        'width': nodeSize,
        'height': nodeSize,
        'background-color': '#888888',
        'border-width': 0,
        'shadow-blur': 0,
        'shadow-color': '#ffffff',
        'shadow-opacity': 0,
        'shadow-offset-x': 0,
        'shadow-offset-y': 0,
        // v1.4 点击聚焦的淡入淡出过渡
        // v2.2 去掉 width：呼吸脉动逐帧写 width 旁路，宽度过渡会让脉动"拖泥带水"
        'transition-property': 'opacity, background-opacity, line-color',
        'transition-duration': '0.25s',
      } as any,
    },
    // 类型 = 颜色
    { selector: 'node[nodeType = "goal"]',         style: { 'background-color': '#a78bfa' } },
    { selector: 'node[nodeType = "design"]',       style: { 'background-color': '#60a5fa' } },
    { selector: 'node[nodeType = "task"]',         style: { 'background-color': '#22d3ee' } },
    { selector: 'node[nodeType = "verification"]', style: { 'background-color': '#34d399' } },
    { selector: 'node[nodeType = "note"]',         style: { 'background-color': '#94a3b8' } },
    // 状态 = 光晕 / 透明度
    { selector: 'node[nodeStatus = "pending"]',     style: { 'background-opacity': 0.45 } },
    { selector: 'node[nodeStatus = "in_progress"]', style: { 'shadow-blur': 18, 'shadow-opacity': 0.55 } },
    { selector: 'node[nodeStatus = "success"]',     style: { 'shadow-blur': 8,  'shadow-opacity': 0.22 } },
    {
      selector: 'node[nodeStatus = "failed"]',
      style: {
        'background-color': '#f87171',
        'background-opacity': 1,
        'shadow-color': '#f87171',
        'shadow-blur': 18,
        'shadow-opacity': 0.6,
      },
    },
    {
      selector: 'node[nodeStatus = "blocked"]',
      style: {
        'background-opacity': 0.3,
        'border-width': 1,
        'border-color': 'rgba(255,255,255,0.35)',
        'border-style': 'dashed',
      },
    },
    // v2.14 代码骨架挂载：青绿描边（图上一眼可见哪些节点有代码栏；置于 :selected 之前，选中态白描边仍优先生效）
    { selector: 'node[codeMap]', style: { 'border-width': 2, 'border-color': '#34d399', 'border-opacity': 0.95, 'border-style': 'solid' } },
    // v2.16 支链闭环：任务无验证子节点 → 琥珀虚线框（图上开环一目了然；置于 codeMap 后，开环警示优先于代码描边）
    { selector: 'node[openLoop]', style: { 'border-width': 2, 'border-color': '#fbbf24', 'border-opacity': 0.9, 'border-style': 'dashed' } },
    // v2.14 「代码」筛选：非代码节点压暗、代码节点青绿辉光
    { selector: 'node.code-dim', style: { 'opacity': 0.1, 'text-opacity': 0.08 } },
    { selector: 'node.code-lit', style: { 'shadow-blur': 22, 'shadow-opacity': 0.85, 'shadow-color': '#34d399' } },
    { selector: 'node:selected', style: { 'border-width': 2, 'border-color': '#ffffff', 'border-opacity': 0.95, 'border-style': 'solid', 'shadow-blur': 14, 'shadow-opacity': 0.35, 'shadow-color': '#ffffff' } },
    // v2.0 边：粗细与节点大小挂钩（用户反馈：边应随节点大小，且要细）——
    // 小节点(14px) 0.8px → 大节点(38px) 2.0px；曲率收敛（52→30px 控制距离，短边不再鼓大包）；
    // v2.15 渐变真降级：>300 边全部实线（chainToElements 逐边内联决定，base 不再设 line-fill）
    {
      selector: 'edge',
      style: {
        'width': edgeBaseWidth,
        'curve-style': 'bezier',
        'control-point-distances': '30px',
        'control-point-weights': 0.5,
        'line-cap': 'round',
        'line-color': 'rgba(148,163,184,0.5)',    // 实线兜底色
        'target-arrow-shape': 'triangle',
        'target-arrow-color': 'rgba(255,255,255,0.4)',   // 兜底（每条边都有逐边样式覆盖为目标色）
        'arrow-scale': 0.55,
        'opacity': 0.55,
        // v1.4 聚焦过渡
        'transition-property': 'opacity, width',
        'transition-duration': '0.2s',
      },
    },
    // 注意：cytoscape 不支持 `edge:hover` 选择器（会报 invalid selector）；
    // 悬停高亮改用事件加 .edge-hover 类实现（见 onMount 的 mouseover/mouseout 监听）
    { selector: 'edge.edge-hover', style: { 'opacity': 1, 'width': (ele: any) => Math.min(edgeBaseWidth(ele) * 1.8, 3) } },
    // v1.6.1 Obsidian 风格点击聚焦：淡出无关元素、点亮选中节点与邻居
    // 注意：dim 不透明度不宜过低（0.10 在黑底上近似隐形，用户误以为"数据全没了"），
    // 且关闭侧栏/按 Esc/点空白处都必须解除聚焦
    { selector: 'node.focus-dim', style: { 'opacity': 0.16, 'text-opacity': 0.15 } },
    { selector: 'edge.focus-dim', style: { 'opacity': 0.06 } },
    { selector: 'node.focus-lit', style: { 'opacity': 1, 'text-opacity': 1 } },
    { selector: 'edge.focus-lit', style: { 'opacity': 1, 'width': 2.2 } },
    // v2.2 涟漪视图（开发模式）：波外压暗
    { selector: 'node.rip-dim', style: { 'opacity': 0.06, 'text-opacity': 0.05 } },
    // 同层同亮度、逐级大幅递减（d0=点击节点最亮、d1=直接相关次之）
    { selector: 'node.rip-d0', style: { 'opacity': 1, 'text-opacity': 1 } },
    { selector: 'node.rip-d1', style: { 'opacity': 0.8, 'text-opacity': 0.85 } },
    { selector: 'node.rip-d2', style: { 'opacity': 0.58, 'text-opacity': 0.62 } },
    { selector: 'node.rip-d3', style: { 'opacity': 0.4, 'text-opacity': 0.44 } },
    { selector: 'node.rip-d4', style: { 'opacity': 0.27, 'text-opacity': 0.3 } },
    { selector: 'node.rip-d5', style: { 'opacity': 0.18, 'text-opacity': 0.2 } },
    { selector: 'node.rip-d6', style: { 'opacity': 0.12, 'text-opacity': 0.14 } },
    // v2.2 空闲时的"若有若无"淡线（开发模式）；涟漪中整组淡出隐藏
    { selector: 'edge.ghost', style: { 'opacity': 0.13 } },
    { selector: 'edge.rip-hide', style: { 'opacity': 0 } },
    // v2.4 递进关系线型：solves=虚线（解决局限的递进主线）、alternative=点线（备选方案）
    { selector: 'edge[rel = "solves"]', style: { 'line-style': 'dashed', 'line-dash-pattern': [7, 5] } },
    { selector: 'edge[rel = "alternative"]', style: { 'line-style': 'dotted', 'line-dash-pattern': [2, 5] } },
    // v2.4 搜索命中高亮脉冲：白描边 + 青白辉光（1.6s 后由 App 移除）
    { selector: 'node.search-hit', style: { 'border-width': 3, 'border-color': '#ffffff', 'border-opacity': 0.95, 'shadow-blur': 30, 'shadow-opacity': 0.9, 'shadow-color': '#7dd3fc' } },
  ];

  async function loadChain() {
    if (!chainDir) return;
    // v2.0 修复：切换目录时清空旧图 + 重置三签名——
    // 新旧目录节点 id 相同（如都是初始化的 g-001）时签名判定"无变化"，
    // $effect 不重建，旧目录节点会残留重叠；扫描失败时旧图也必须清掉。
    // 同目录"重新扫描"不触发清理（保留 v1.6 的位置续排手感）。
    if (chainDir !== lastDir) {
      lastDir = chainDir;
      lastIdsSig = 0;
      lastDataSig = 0;
      lastSliderSig = '';
      selectedNode = null;
      sidebarCollapsed = false;
      focusNodeId = null;      // v2.6 切目录重置双击聚焦
      focusSet = null;
      hoverTip = null;
      stopForce();
      clearRipple();   // v2.2
      cy?.elements().remove();
    }
    loading = true;
    error = null;
    try {
      snapshot = await invoke<ChainSnapshot>('scan_chain', { dir: chainDir, mode: scanMode });
    } catch (e) {
      const msg = String(e);
      if (msg.includes('不存在 .chain')) {
        // v2.1 添加工作区时后端已自动初始化；这里只可能是文件被外部删除
        snapshot = null;
        error = '该目录的 .chain 已被移除，请在工作区栏移除后重新添加';
      } else {
        error = msg;
      }
    } finally {
      loading = false;
    }
  }

  async function handleSave(fields: { title: string; status: NodeStatus | null; body: string; tags: string[]; evidence: string[] }) {
    if (!chainDir || !selectedNode) return;
    // 失败时 invoke reject，错误由 Sidebar 的 catch 显示；成功才更新 snapshot 并刷新信息栏
    // v2.0 开发模式：status 为 null = 不写状态（知识库节点可有可无）
    const savedId = selectedNode.id;
    const newSnapshot = await invoke<ChainSnapshot>('update_node', {
      dir: chainDir,
      nodeId: savedId,
      fields: fields,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    // v2.6 常驻信息栏：保存后刷新为最新数据继续显示（旧逻辑关闭侧栏已退役）
    selectedNode = newSnapshot.nodes.find((x) => x.id === savedId) ?? null;
    clearFocus(cy ?? null);   // 内容变了：退出聚焦，避免与旧范围的高亮错位
  }

  // v2.0 开发模式：新建节点（id 留空 = 后端自动生成；类型/状态一律中性 note/none；v2.4 rel 递进关系）
  async function handleCreateNode(input: { id: string; title: string; parent: string | null; rel: string }) {
    if (!chainDir) return;
    const newSnapshot = await invoke<ChainSnapshot>('create_node', {
      dir: chainDir,
      input: {
        id: input.id || null,
        title: input.title,
        parent: input.parent,
        rel: input.rel,
      },
      mode: scanMode,
    });
    snapshot = newSnapshot;
    showCreate = false;
  }

  // v2.0 开发模式：删除节点（两段式确认在 Sidebar 内完成）
  async function handleDeleteNode(nodeId: string) {
    if (!chainDir) return;
    const newSnapshot = await invoke<ChainSnapshot>('delete_node', {
      dir: chainDir,
      nodeId,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    selectedNode = null;
    sidebarCollapsed = false;
    cy?.elements().removeClass('focus-dim focus-lit');
  }

  // v2.0 开发模式：建立/断开链接（v2.4 rel 递进关系类型）
  async function handleSetParent(nodeId: string, parent: string | null, rel: string) {
    if (!chainDir) return;
    const newSnapshot = await invoke<ChainSnapshot>('set_parent', {
      dir: chainDir,
      nodeId,
      parent,
      rel,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    const nodeData = newSnapshot.nodes.find((x) => x.id === nodeId);
    if (nodeData) selectedNode = nodeData;
  }

  // v2.13 代码栏：挂载/移除后重扫（后端返回新快照，刷新信息栏的 code_map 状态）
  function handleCodeMapChange(newSnapshot: ChainSnapshot, nodeId: string) {
    snapshot = newSnapshot;
    const nodeData = newSnapshot.nodes.find((x) => x.id === nodeId) ?? null;
    if (nodeData) selectedNode = nodeData;
  }

  // v2.14 「代码」筛选：有代码骨架的节点加辉光、其余压暗；图谱重建后需重放（类随元素重建消失）
  function applyCodeFilter() {
    if (!cy) return;
    cy.batch(() => {
      if (codeFilter) {
        cy!.nodes().forEach((n: any) =>
          n.data('codeMap') ? n.addClass('code-lit') : n.addClass('code-dim'),
        );
      } else {
        cy!.nodes().removeClass('code-lit code-dim');
      }
    });
  }
  function toggleCodeFilter() {
    codeFilter = !codeFilter;
    applyCodeFilter();
  }

  // v2.15 工具栏按钮注册表（未来扩展：追加一条描述符即可，渲染层自动接线）
  const toolButtons = $derived.by(() => [
    {
      id: 'zoom-in',
      label: '+',
      title: '放大（滚轮亦可）',
      active: false,
      onClick: () => zoomBy(cy, 1.4),
    },
    {
      id: 'fit',
      label: '⤢',
      title: '回到全图（适配全部可见节点；不受标签影响）',
      active: false,
      // 不用裸 cy.fit()：它走元素渲染包围盒、会把标签算进去，
      // 与"标签预算随 zoom 变化"构成振荡回路（同 fitVisible 的注释）。退出聚焦态。
      onClick: () => {
        if (!cy) return;
        focusNodeId = null;
        focusSet = null;
        cy.elements().removeClass('focus-dim focus-lit');
        fitVisible(cy, { animate: true, padding: 60 });
      },
    },
    {
      id: 'zoom-out',
      label: '−',
      title: '缩小（滚轮亦可）',
      active: false,
      onClick: () => zoomBy(cy, 1 / 1.4),
    },
    { id: 'code', label: '</>', title: '高亮有代码骨架的节点（青绿描边 + 辉光，其余压暗）', active: codeFilter, onClick: toggleCodeFilter },
    { id: 'perf', label: '⚡', title: '性能浮层：FPS/帧耗时/规模分档', active: perfOpen, onClick: () => (perfOpen = !perfOpen) },
    {
      id: 'legend',
      label: showLegend ? '◉' : '○',
      title: '图例开关（图例挡到图时 v3.0 会自动收起；手动点过之后就听你的）',
      active: false,
      onClick: () => {
        showLegend = !showLegend;
        // 用户显式操作 → 记下来，不再自动收起
        legendUserSet = true;
        localStorage.setItem('engram-legend-user', '1');
      },
    },
  ]);

  // v1.3：折叠子链（两段式确认在 Sidebar 内完成，这里只执行；v2.0 仅分析模式）
  async function handleFold() {
    if (!chainDir || !selectedNode) return;
    const newSnapshot = await invoke<ChainSnapshot>('fold_chain', {
      dir: chainDir,
      nodeId: selectedNode.id,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    selectedNode = null;
    sidebarCollapsed = false;
    clearFocus(cy ?? null);   // 节点结构变了：聚焦范围可能已失效，一并退出
  }

  // v1.3：快照（工具栏按钮 → 输入标签 → 创建）
  let snapTag = $state('');
  let snapBusy = $state(false);
  let snapMessage = $state<string | null>(null);

  async function handleSnapshot() {
    if (!chainDir || snapBusy) return;
    const tag = snapTag.trim();
    if (!tag) {
      snapMessage = '先填快照标签（如"重构前"）';
      return;
    }
    snapBusy = true;
    snapMessage = null;
    try {
      const id = await invoke<string>('snapshot_chain', { dir: chainDir, tag });
      snapMessage = `快照已创建：${id}`;
      snapTag = '';
    } catch (e) {
      snapMessage = String(e);
    } finally {
      snapBusy = false;
    }
  }

  // v2.0 三签名防抖（大图卡死的另一根因：watcher 噪音推送反复触发全图重建+重排）：
  // - idsSig（节点集合）：变化才重建元素 + 散点重排
  // - dataSig（字段内容）：变化只原位更新元素数据（不重排、不重建——保存/外部编辑不扰动布局）
  // - sliderSig（滑条）：变化只从当前位置续排
  // v2.15 签名改为数字哈希（fnv1a）：idsKey/dataKey 是 number，sliderSig 保持字符串
  let lastIdsSig = 0;
  let lastDataSig = 0;
  let lastSliderSig = '';

  // ⚠️ Svelte 5 坑：if (cy && snapshot) 短路求值会让 effect 漏追踪 snapshot
  // （第一次跑时 cy=null，JS 短路求值不会读 snapshot，Svelte 5 不会追踪）
  // 修法：先单独读 snapshot 强制让 Svelte 5 追踪到
  // v2.15 快照签名 effect（只追踪 snapshot/归档开关）：fnv1a 数字哈希替代
  // 1500 节点 sort+join 巨型字符串；滑条独立成 effect，拖动不再重算签名
  $effect(() => {
    if (!snapshot) return;  // 强制追踪 snapshot
    if (!cy) return;
    const snap = snapshot;   // TS 收窄：嵌套闭包里保持非空类型
    const cyRef = cy;

    // 节点集合签名（含归档开关）
    const idsKey =
      fnv1a(snap.nodes.map((x) => x.id).join('|')) ^
      (showArchived ? fnv1a((snap.archived ?? []).map((x) => x.id).join('|')) : 0);
    // 内容签名：只取影响画布的字段（title 驱动标签；tags/evidence 不进签名——图上不渲染）
    const dataKey = fnv1a(
      snap.nodes.map((x) => `${x.id}|${x.type}|${x.updated}|${x.revision}|${x.status}|${x.title}`).join(';'),
    );

    if (idsKey !== lastIdsSig) {
      // 节点集合变化：全量重建 + 预散点 + 首帧视图 + 力模拟
      lastIdsSig = idsKey;
      lastDataSig = dataKey;
      stopForce();
      cyRef.elements().remove();
      focusNodeId = null;   // v2.6 节点集合重建时退出双击聚焦
      focusSet = null;
      posIdsSig = '';       // v2.15 位置缓存索引失效（节点集变了）
      // v3.0 布局态重置：新图不继承旧图的半径覆盖/隐藏集合
      nodeSizeOverride = new Map();
      hiddenNodes = new Set();
      lastCamSig = '';
      // 自动深度：按新图规模取一个"能读"的展开层数（用户手动调过则尊重用户）
      if (autoDepth) visibleDepth = autoDepthFor(snap.nodes.length);
      // v2.4 两模式统一：连线渲染为"若有若无"的淡线（.ghost），点击后整组淡出改由涟漪表达
      cyRef.add(chainToElements(snap, { withEdges: true, includeArchived: showArchived }));
      cyRef.edges().addClass('ghost');
      // v3.3 图上不显示名称：min-zoomed-font-size 保持 0（无文字可裁剪）。
      
      cyRef.style().selector('node').style({ 'min-zoomed-font-size': 0 }).update();
      applyCodeFilter();   // v2.14 全量重建会清空类，筛选开着重放
      startWaterLoop();
      // v1.7 首帧视图：以前在这里同步 fit 全图 + 根节点居中（消除"左上角堆叠→跳中央"的闪烁）。
      // v3.0：布局末尾本来就有一次"落位后 fit"，这里再 fit 一次是重复劳动
      // （主线程同步布局计算 + 两段相机动画）。首帧视口交给 relayout 的补间收尾统一负责。
      relayout(cyRef);   // v3.0 树感知确定性布局：算完即静止，无物理迭代
      return;
    }

    if (dataKey !== lastDataSig) {
      // 内容变化（保存/外部编辑/watcher 推送）：原位更新节点与边数据，位置与布局不动
      lastDataSig = dataKey;
      cyRef.batch(() => {
        for (const def of chainToElements(snap)) {
          const ele = cyRef.getElementById(def.data.id as string);
          if (ele.nonempty()) ele.data(def.data);
        }
      });
      applyCodeFilter();   // v2.14 节点数据原位更新（挂载/移除代码栏后）同步筛选类
      return;
    }
    // watcher 重推但内容无变化：直接忽略，避免大图反复重建卡顿
  });

  // v3.0 布局参数独立 effect：参数或布局模式变化 → 重排（不碰快照签名，大图不重算 O(n) 哈希）
  $effect(() => {
    const sig = `${layoutMode}|${levelGap}|${siblingGap}|${visibleDepth}`;
    const cyRef = cy;
    if (!cyRef || !snapshot) return;
    if (sig !== lastSliderSig) {
      lastSliderSig = sig;
      relayout(cyRef);   // 纯函数重算 + 300ms 补间（旧坐标起排，起点即当前画面）
    }
  });

  onMount(() => {
    try {
      cy = cytoscape({
        container,
        style,
        elements: [],
        // v1.4 滚轮缩放: 灵敏度与缩放范围 (节点多时可缩至全局总览, 近看细节)
        wheelSensitivity: 0.3,
        minZoom: 0.08,
        maxZoom: 4,
      });
      updateRippleStyle(cy);   // v2.4 亮度对比公式接管 rip-dN 样式（滑条默认值）
      // v2.15 调试/自动化钩子（CDP 驱动验证与未来插件扩展的稳定接缝，只读访问）
      (window as any).__engramDebug = {
        get cy() {
          return cy;
        },
        get snapshot() {
          return snapshot;
        },
        get mode() {
          return scanMode;
        },
        // v3.0 布局自检接缝（回归脚本用：验证静止性/可复现性/可见性不变量，避免读私有变量）
        get hiddenCount() {
          return hiddenNodes.size;
        },
        // v3.2 聚焦态自检接缝：聚焦范围/命中节点/各类计数
        get focus() {
          if (!cy) return null;
          return {
            nodeId: focusNodeId,
            setSize: focusSet ? focusSet.size : 0,
            dim: cy.nodes().filter((n: any) => n.hasClass('focus-dim')).length,
            lit: cy.nodes().filter((n: any) => n.hasClass('focus-lit')).length,
            zoom: +cy.zoom().toFixed(4),
          };
        },
        get relayoutTrace() {
          return lastRelayoutTrace;
        },
        // v3.3 图上不显示名称的自检：应恒为 0（名称只在右侧信息栏）
        get names() {
          if (!cy) return null;
          // 注意 cytoscape 的 style() 返回字符串（'0' 而非数字 0），比较要按字符串
          return {
            nodesWithVisibleLabel: cy.nodes().filter(
              (n: any) => String(n.style('label') ?? '') !== '' || String(n.style('text-opacity')) !== '0',
            ).length,
          };
        },
        // 可见节点数（渲染器口径）—— 用于断言"预算说显示多少，画面就真的显示多少"
        get visibleCount() {
          if (!cy) return -1;
          return cy.nodes().filter((n: any) => !hiddenNodes.has(n.id())).length;
        },
        get layout() {
          return {
            info: layoutInfo,
            mode: layoutMode,
            levelGap,
            siblingGap,
            visibleDepth,
            autoDepth,
            modePick: lastModePick,
            visible: cy ? cy.nodes().filter((n: any) => !hiddenNodes.has(n.id())).length : 0,
            hidden: hiddenNodes.size,
            positions: cy
              ? cy.nodes().map((n: any) => ({
                  id: n.id(),
                  x: Math.round(n.position('x') * 100) / 100,
                  y: Math.round(n.position('y') * 100) / 100,
                }))
              : [],
          };
        },
        relayout: () => { if (cy) relayout(cy); },
        // v3.0 回归接缝：等待布局补间结束（最长 2s），供脚本判定"完全静止"
        settle: () =>
          new Promise<boolean>((resolve) => {
            const t0 = performance.now();
            const poll = () => {
              if (layoutRaf === null || performance.now() - t0 > 2000) resolve(layoutRaf === null);
              else setTimeout(poll, 50);
            };
            poll();
          }),
      };
      cy.on('tap', 'node', (evt) => {
        const n = evt.target;
        hoverTip = null;
        // v2.6 两模式统一：点击节点 = 波源开关（再点停止），波纹表达关系；
        // 同时右侧常驻信息栏切换到该节点内容
        toggleWaveSource(n.id());
        const nodeData =
          snapshot?.nodes.find(x => x.id === n.id()) ??
          snapshot?.archived?.find(x => x.id === n.id());
        if (nodeData) selectedNode = nodeData;
      });
      // v2.6 两模式统一：单击 = 波源开关 + 切换右侧信息栏内容；双击 = 视图聚焦/退出聚焦
      cy.on('dbltap', 'node', (evt) => {
        const n = evt.target;
        hoverTip = null;
        if (cy) toggleFocus(cy, n.id());
      });
      cy.on('tap', (evt) => {
        if (evt.target === cy) {
          // v2.6 点空白处 = 侧栏收起为右缘细条（保留所选节点，可一键展开），不再关闭
          sidebarCollapsed = true;
          searchOpen = false;   // 同时收起搜索下拉
          // v2.3 波源只由"再点同一节点"关闭（点空白不停止波场）
        }
      });
      // v1.7 悬停浮层：id · 类型（id 已从画布标签移除，悬停即可追溯）
      cy.on('mouseover', 'node', (evt) => {
        const n = evt.target;
        const rp = n.renderedPosition();
        hoverTip = {
          x: rp.x,
          y: rp.y - 24,
          text: `${n.id()} · ${NODE_TYPE_LABEL[n.data('nodeType') as NodeType] ?? ''}${n.data('codeMap') ? ' · 代码骨架' : ''}${n.data('openLoop') ? ' · 未闭环（缺验证节点）' : ''}`,
        };
      });
      cy.on('mouseout', 'node', () => (hoverTip = null));
      // v2.0 边悬停高亮（cytoscape 无 :hover 选择器，用事件类实现）
      cy.on('mouseover', 'edge', (evt) => evt.target.addClass('edge-hover'));
      cy.on('mouseout', 'edge', (evt) => evt.target.removeClass('edge-hover'));
      // v1.5 拖动节点：按住时模拟照常运行（被抓节点位置每帧同步进模拟），松开后视位移决定是否重排
      let grabStartPos: { x: number; y: number } | null = null;
      cy.on('grab', (evt) => {
        hoverTip = null;
        dragging = true;
        grabStartPos = { ...evt.target.position() };   // v2.5 记录按下位置，用于区分点击与拖拽
      });
      cy.on('free', (evt) => {
        dragging = false;
        // v2.5 只有真实拖拽（位移 ≥ 8px）且模拟已收敛时才重排——
        // cytoscape 对每次单击也触发 grab/free，旧逻辑导致"点一下节点就整图重排"，
        // 边界节点被挤出视图、双击第二下落空打不开信息栏。
        // 重布局只允许发生在：打开/切换图谱、滑条改动、外部增删节点、真实拖拽松手。
        const start = grabStartPos;
        grabStartPos = null;
        const el = evt.target;
        let moved = Infinity;
        if (start && el) {
          moved = Math.hypot(el.position('x') - start.x, el.position('y') - start.y);
        }
        // v3.0：确定布局下"重排"意味着把节点弹回原位，会抹掉用户的摆放意图。
        // 因此拖拽松手不重排，而是把这个节点**钉住**（locked）：后续重排会跳过它，
        // 其余节点仍按树布局归位。想归位用工具栏「重排」按钮。
        if (moved >= 8 && el && !el.locked()) el.lock();
      });
    } catch (e) {
      error = `[cytoscape init failed] ${(e as Error).message}`;
    }

    // v3.3 图上不显示名称（初始图谱只有圆点）；不再需要相机变化时重算标签
    const onResize = () => { cy?.resize(); if (cy) fitVisible(cy, { padding: 60 }); };
    window.addEventListener('resize', onResize);
    // v2.6 Esc：优先退出双击聚焦视图；否则收起右侧信息栏（常驻侧栏无"关闭"语义）
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        // v2.18 全屏代码页优先关闭
        if (panel.codeFullscreen) {
          panel.codeFullscreen = false;
          return;
        }
        // v2.19 阅读模式：Esc 归 ReaderMode 自己管（先退出筛选输入，再退出阅读模式）
        if (readMode) return;
        if (focusNodeId !== null && cy) {
          // 退出聚焦回全图：用与 relayout 同一套"与标签无关"的取景（裸 fit 会被标签撑大）
          clearFocus(cy);
          fitVisible(cy, { animate: true, padding: 60 });
          return;
        }
        sidebarCollapsed = true;
        clearFocus(cy);
        // v2.3 波源只由"再点同一节点"关闭（Esc 不停止波场）
      }
    };
    window.addEventListener('keydown', onKeydown);

    // M5: 监听后端 chain-changed 事件，自动刷新图谱（侧栏编辑中不覆盖）
    // 前端去抖：watcher 后端已有 300ms 去抖，但 AI 批量操作时前端再兜一层防连环打断
    let chainDebounce: ReturnType<typeof setTimeout> | undefined;
    listen<ChainSnapshot>('chain-changed', (e) => {
      // v2.20 自写窗口内：刚写回的 snapshot 才是最新，晚到的旧扫描结果直接丢弃（防闪回）
      if (performance.now() < selfWriteUntil) return;
      // v2.19 文件树模式是纯阅读/编辑面（无侧栏在途编辑）：不吃"编辑中不覆盖"的保护，外部写入实时进树
      if (selectedNode && !readMode) return;
      clearTimeout(chainDebounce);
      chainDebounce = setTimeout(() => { snapshot = e.payload; }, 150);
    }).then(u => unlisten = u);
    listen<string>('chain-error', (e) => {
      console.warn('[chain-gui] watcher error:', e.payload);
    });

    return () => {
      window.removeEventListener('resize', onResize);
      window.removeEventListener('keydown', onKeydown);
    };
  });

  // v2.1 启动：加载工作区列表 → 恢复到上次打开的工作区（无则打开当前模式层第一个）
  onMount(() => {
    invoke<WorkspaceInfo[]>('list_workspaces')
      .then(async (ws) => {
        workspaces = ws;
        const last = localStorage.getItem('chain-gui-last-dir');
        const target =
          ws.find((w) => w.path === last) ??
          ws.find((w) => w.mode === scanMode) ??
          ws[0];
        if (target) await openWorkspace(target);
      })
      .catch((e) => (wsError = String(e)));
  });

  onDestroy(() => {
    unlisten?.();
    stopForce();
    clearRipple();   // v2.2
    stopWaterLoop(); // v2.2
    cy?.destroy();
  });

  // M10: 复制 AI 使用指南到剪贴板（贴给任何 AI 即完成协议交底）
  // v2.1 双指南：按当前工作区模式复制对应指南（分析=链协议 / 开发=知识库搭建）
  let guideCopied = $state(false);
  let guideCopyTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyAiGuide() {
    try {
      const guide = await invoke<string>('get_ai_guide', { mode: scanMode });
      await navigator.clipboard.writeText(guide);
      guideCopied = true;
      clearTimeout(guideCopyTimer);
      guideCopyTimer = setTimeout(() => (guideCopied = false), 2000);
    } catch (e) {
      error = `复制 AI 指南失败：${String(e)}`;
    }
  }

  let shortDir = $derived(
    chainDir && chainDir.length > 48 ? '…' + chainDir.slice(-47) : chainDir
  );

  // ── v2.4 节点关键字搜索：标题/id/tags 模糊匹配，点击结果居中定位 + 高亮脉冲 ──
  let searchText = $state('');
  let searchOpen = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;   // 高亮消退定时器

  const searchResults = $derived.by(() => {
    const q = searchText.trim().toLowerCase();
    const nodes = snapshot?.nodes ?? [];
    if (!q) return [];
    return nodes
      .filter((nd) =>
        nd.title.toLowerCase().includes(q) ||
        nd.id.toLowerCase().includes(q) ||
        nd.tags.some((t) => t.toLowerCase().includes(q))
      )
      .slice(0, 20)
      .map((nd) => ({ id: nd.id, title: nd.title, type: nd.type }));
  });

  function jumpToNode(nodeId: string) {
    if (!cy) return;
    const el = cy.getElementById(nodeId);
    if (el.empty()) return;
    // v2.5 不再 stopForce——搜索定位只动视口（center/zoom），与位置模拟互不冲突；
    // 旧代码会把正在铺开的布局杀在半途（与点击冻结同类问题）
    searchOpen = false;
    // 居中定位（动画）+ 高亮脉冲（1.6s 后消退）
    cy.animate({
      center: { eles: el },
      zoom: Math.max(cy.zoom(), 1.0),
      duration: 350,
      easing: 'ease-out',
    });
    cy.elements().removeClass('search-hit');
    el.addClass('search-hit');
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => cy?.elements().removeClass('search-hit'), 1600);
  }

  function handleSearchKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      const first = searchResults[0];
      if (first) jumpToNode(first.id);
    } else if (e.key === 'Escape') {
      e.stopPropagation();   // 不让全局 Esc（关侧栏）误伤搜索交互
      searchOpen = false;
    }
  }
</script>

<main>
  <!-- v2.1 左侧工作区栏：两层（分析/开发）+ 列表 + 添加/移除 + 一键切换图谱 -->
  <WorkspaceSidebar
    workspaces={workspaces}
    mode={scanMode}
    currentDir={chainDir}
    busy={wsBusy}
    error={wsError}
    onSwitchMode={handleSwitchMode}
    onOpen={openWorkspace}
    onAdd={handleAddWorkspace}
    onRemove={handleRemoveWorkspace}
  />

  <div class="app-col">
  <header class="toolbar">
    <span class="logo">🌊 Engram</span>
    {#if shortDir}
      <span class="dir" title={chainDir ?? ''}>{shortDir}</span>
    {/if}
    <span class="mode-chip" class:dev={scanMode === 'dev'} title={scanMode === 'dev' ? '开发模式：自由知识图谱' : '分析模式：严格链协议'}>
      {scanMode === 'dev' ? '开发' : '分析'}
    </span>
    <!-- v2.19 显示方式切换：图谱 ↔ 文件树（两种显示 = 两种编辑面；人专用入口，AI 无此入口） -->
    {#if snapshot}
      <button class="pick read-toggle" class:active={readMode} onclick={toggleReadMode}
              title={readMode
                ? '返回图谱模式（Esc）：结束文件树模式'
                : '文件树模式（原阅读模式，v2.20 起可新建/编辑/删除节点）：按节点图结构梳理成文件树，在软件内直接阅读与编辑——人专用视图（不写工作区额外文件、MCP 无此工具、AI 不可识别不可用）'}>
        {readMode ? '◧ 图谱模式' : '🗂 文件树模式'}
      </button>
    {/if}
    <span class="spacer"></span>
    {#if snapshot}
      {#if (snapshot.archived?.length ?? 0) > 0}
        <button class="pick" class:active={showArchived}
                onclick={() => (showArchived = !showArchived)}
                title="归档视图（v2.12）：淡色虚线显示已归档节点（默认不进图）；点击节点可在信息栏查看全文">
          {showArchived ? '归档视图 ✓' : '归档视图'}
        </button>
      {/if}
      <button class="pick" onclick={handleReindex} disabled={reindexBusy || !chainDir}
              title="重嵌（v2.12）：用本地模型全库重建嵌入索引，语义召回与向量检索随之启用">
        {reindexBusy ? '重嵌中…' : '重嵌索引'}
      </button>
      {#if reindexMsg}
        <span class="snap-msg" title={reindexMsg}>{reindexMsg}</span>
      {/if}
      <span class="slider-group">
        <label class="slider-label" title="可见深度（v3.0 渐进披露）：图上只铺开到第几层，其余折叠。屏幕像素有物理下限——实测约 150–300 个圆点是「还能分辨结构」的上限，所以大图默认只展开浅层，而不是把上千个点压成一团雾">可见深度<span class="slider-val">{visibleDepth >= 99 ? '全部' : visibleDepth + ' 层'}</span>
          <span class="slider-track">
            <input type="range" min="1" max="8" step="1" value={Math.min(visibleDepth, 8)}
              onchange={(e) => {
                let v = +(e.target as HTMLInputElement).value;
                if (v >= 8) v = 99;   // 滑到最右 = 全部展开
                visibleDepth = v;
                autoDepth = false;
                localStorage.setItem('engram-visible-depth', String(v));
              }} />
            <button class="slider-def-dot" style:left="calc(6px + (100% - 12px) * 0.2857)" title="自动（按节点规模）"
                    onclick={(e) => { e.preventDefault(); autoDepth = true; visibleDepth = autoDepthFor(snapshot?.nodes.length ?? 0); localStorage.removeItem('engram-visible-depth'); }}></button>
          </span>
        </label>
        <label class="slider-label" title="层间距（v3.0）：相邻层级之间的中心距，越大结构越舒展">层间距<span class="slider-val">{levelGap}px</span>
          <span class="slider-track">
            <input type="range" min="60" max="200" step="4" value={levelGap}
              onchange={(e) => levelGap = +(e.target as HTMLInputElement).value} />
            <button class="slider-def-dot" style:left="calc(6px + (100% - 12px) * 0.2571)" title="默认值 96px，点击恢复"
                    onclick={(e) => { e.preventDefault(); levelGap = 96; }}></button>
          </span>
        </label>
        <label class="slider-label" title="同层间距（v3.0）：同一层内相邻节点的最小中心距，越大越不挤；实际生效值不小于节点直径">同层间距<span class="slider-val">{siblingGap}px</span>
          <span class="slider-track">
            <input type="range" min="20" max="100" step="2" value={siblingGap}
              onchange={(e) => siblingGap = +(e.target as HTMLInputElement).value} />
            <button class="slider-def-dot" style:left="calc(6px + (100% - 12px) * 0.325)" title="默认值 46px，点击恢复"
                    onclick={(e) => { e.preventDefault(); siblingGap = 46; }}></button>
          </span>
        </label>
        <label class="slider-label" title="布局形态（v3.0）：分层=父在上子在下（贴合链式阅读，适合中小图）；径向=根居中向外辐射（大图唯一能压住规模的形态）；自动=按可见节点数选（≤300 分层，否则径向）">形态<span class="slider-val">{layoutMode === 'auto' ? `自动·${layoutInfo?.mode === 'radial' ? '径向' : '分层'}` : layoutMode === 'radial' ? '径向' : '分层'}</span>
          <span class="slider-track">
            <select class="layout-select" value={layoutMode}
              onchange={(e) => applyLayoutMode((e.target as HTMLSelectElement).value as 'auto' | LayoutMode)}>
              <option value="auto">自动</option>
              <option value="layered">分层</option>
              <option value="radial">径向</option>
            </select>
          </span>
        </label>
        <button class="pick" title="全部归位：清除手动钉住的节点，按树布局重排到规范位置"
                onclick={() => { cy?.nodes().unlock(); relayout(cy!); }}>重排</button>
      </span>
    {/if}
    <button class="pick" onclick={copyAiGuide} title="复制 AI 使用指南全文，贴给 AI 即完成协议交底">
      {guideCopied ? '已复制 ✓' : '复制 AI 指南'}
    </button>
    {#if chainDir}
      <button class="pick" onclick={loadChain} disabled={loading || !snapshot}>
        重新扫描
      </button>
      {#if scanMode === 'dev'}
        <button class="pick create-btn" onclick={() => (showCreate = true)} title="新建节点（开发模式）">
          ＋ 节点
        </button>
      {/if}
      <div class="snap-group" title="创建链状态快照，支持受控回溯（.chain/logs/）">
        <input class="snap-input" bind:value={snapTag} placeholder="快照标签…" disabled={snapBusy} />
        <button class="pick snap-btn" onclick={handleSnapshot} disabled={snapBusy || !snapshot}>
          {snapBusy ? '创建中…' : '快照'}
        </button>
        {#if snapMessage}
          <span class="snap-msg">{snapMessage}</span>
        {/if}
      </div>
    {/if}
  </header>

  {#if error}
    <div class="error-bar">
      <span>⚠ {error}</span>
      <button class="dismiss" onclick={() => (error = null)}>✕</button>
    </div>
  {/if}

  <!-- v2.6 为常驻信息栏预留空间：margin-right 真正收窄画布容器（padding 对绝对定位子元素无效），
       画布、右下角缩放按钮、右上角波纹面板只出现在信息栏左侧，不再被压住；无图谱时不预留 -->
  <div class="canvas-wrap" style:margin-right={(snapshot ? (sidebarCollapsed ? SIDEBAR_COLLAPSED_WIDTH : panel.width) : 0) + 'px'}>
    {#if !snapshot && !loading}
      <div class="empty-hint">
        <div class="empty-icon">⛓</div>
        <p>在左侧工作区栏添加或选择一个文件夹</p>
        <p class="sub-hint">添加时按当前页签确定模式：分析（AI 链协议）/ 开发（自由知识库）</p>
      </div>
    {/if}

    <!-- v2.2 水面画布（开发模式）：水体 + 环境波纹 + 涟漪/震动在水面的表达，位于节点层之下 -->
    <canvas bind:this={waterCanvas} class="water-canvas"></canvas>

    <div bind:this={container} class="cy-container"></div>

    <!-- v1.7 悬停浮层：节点 id · 类型（定位跟随节点渲染坐标） -->
    {#if hoverTip}
      <div class="hover-tip" style:left="{hoverTip.x}px" style:top="{hoverTip.y}px">{hoverTip.text}</div>
    {/if}

    {#if perfOpen}
      <PerfOverlay
        nodes={snapshot?.nodes.length ?? 0}
        edges={snapshot?.edges.length ?? 0}
        tier={perfTierName(snapshot?.nodes.length ?? 0)}
      />
    {/if}

    <!-- v2.18 全屏代码页：⧉ 展开——覆盖整个窗口的独立页面（大字体 + Mermaid + 完整滚动），Esc/✕ 关闭 -->
    {#if panel.codeFullscreen && selectedNode && chainDir}
      <div class="code-fs-mask">
        <button class="code-fs-close" onclick={() => (panel.codeFullscreen = false)} title="关闭（Esc）">✕</button>
        <CodeViewer ws={chainDir} nodeId={selectedNode.id} />
      </div>
    {/if}

    <!-- v2.4 节点关键字搜索：标题/id/标签模糊匹配，点击结果居中定位 + 高亮 -->
    {#if snapshot}
      <div class="node-search">
        <div class="ns-input-row">
          <span class="ns-icon">⌕</span>
          <input class="ns-input" bind:value={searchText} placeholder="搜索节点（标题 / id / 标签）…"
                 oninput={() => (searchOpen = true)}
                 onkeydown={handleSearchKey} />
          {#if searchText}
            <button class="ns-clear" onclick={() => { searchText = ''; searchOpen = false; }} title="清空">✕</button>
          {/if}
        </div>
        {#if searchOpen && searchText.trim().length > 0}
          <div class="ns-results">
            {#if searchResults.length === 0}
              <div class="ns-empty">无匹配节点</div>
            {:else}
              {#each searchResults as r (r.id)}
                <button class="ns-item" onclick={() => jumpToNode(r.id)}>
                  <span class="ns-dot" style:background={NODE_TYPE_COLOR[r.type]}></span>
                  <span class="ns-title">{r.title}</span>
                  <span class="ns-id">{r.id}</span>
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <!-- v2.3 波纹参数测试面板：能量/周期/粗细/衰减由用户调节（测试期两模式通用） -->
    <div class="wave-params">
        <button type="button" class="wp-head" onclick={() => (wavePanelOpen = !wavePanelOpen)}>
          <span class="chev">{wavePanelOpen ? '▾' : '▸'}</span> 波纹参数（测试）
        </button>
        {#if wavePanelOpen}
          <div class="wp-body">
            <label class="wp-row" title="波源能量：环透明度与振动幅度（次级波源自动为其 55%）">能量
              <span class="wp-val">{waveParams.energy.toFixed(2)}</span>
              <span class="slider-track">
                <input type="range" min="0.1" max="1.4" step="0.05" value={waveParams.energy}
                       onchange={(e) => (waveParams.energy = +(e.target as HTMLInputElement).value)} />
                <button class="slider-def-dot" style:left="calc(5.5px + (100% - 11px) * 0.3462)" title="默认值 0.55，点击恢复"
                        onclick={(e) => { e.preventDefault(); waveParams.energy = 0.55; }}></button>
              </span>
            </label>
            <label class="wp-row" title="波动周期：一圈环从中心扩到边界的时间（秒）">周期
              <span class="wp-val">{waveParams.period.toFixed(1)}s</span>
              <span class="slider-track">
                <input type="range" min="0.3" max="4" step="0.1" value={waveParams.period}
                       onchange={(e) => (waveParams.period = +(e.target as HTMLInputElement).value)} />
                <button class="slider-def-dot" style:left="calc(5.5px + (100% - 11px) * 0.3514)" title="默认值 1.6s，点击恢复"
                        onclick={(e) => { e.preventDefault(); waveParams.period = 1.6; }}></button>
              </span>
            </label>
            <label class="wp-row" title="波纹粗细：涟漪环线宽（px）">粗细
              <span class="wp-val">{waveParams.lineWidth.toFixed(1)}px</span>
              <span class="slider-track">
                <input type="range" min="0.5" max="2.5" step="0.1" value={waveParams.lineWidth}
                       onchange={(e) => (waveParams.lineWidth = +(e.target as HTMLInputElement).value)} />
                <button class="slider-def-dot" style:left="calc(5.5px + (100% - 11px) * 0.25)" title="默认值 1.0px，点击恢复"
                        onclick={(e) => { e.preventDefault(); waveParams.lineWidth = 1.0; }}></button>
              </span>
            </label>
            <label class="wp-row" title="环扩张过程中的透明度衰减速度：越大淡得越快">衰减
              <span class="wp-val">{waveParams.fade.toFixed(1)}</span>
              <span class="slider-track">
                <input type="range" min="0.3" max="2.5" step="0.1" value={waveParams.fade}
                       onchange={(e) => (waveParams.fade = +(e.target as HTMLInputElement).value)} />
                <button class="slider-def-dot" style:left="calc(5.5px + (100% - 11px) * 0.4091)" title="默认值 1.2，点击恢复"
                        onclick={(e) => { e.preventDefault(); waveParams.fade = 1.2; }}></button>
              </span>
            </label>
            <label class="wp-row" title="亮度对比：相对默认亮度曲线的陡峭倍数（默认 0.28 = 1 倍：点击节点与直接相关明显亮、更深层明显渐暗），越大深层越暗">亮度对比
              <span class="wp-val">{waveParams.contrast.toFixed(2)}</span>
              <span class="slider-track">
                <input type="range" min="0.05" max="0.85" step="0.05" value={waveParams.contrast}
                       onchange={(e) => (waveParams.contrast = +(e.target as HTMLInputElement).value)} />
                <button class="slider-def-dot" style:left="calc(5.5px + (100% - 11px) * 0.2875)" title="默认值 0.28，点击恢复"
                        onclick={(e) => { e.preventDefault(); waveParams.contrast = 0.28; }}></button>
              </span>
            </label>
          </div>
        {/if}
      </div>

    <!-- v1.4 缩放控件（右下角）：滚轮之外的按钮式缩放 + 全局适配 + 图例开关
         v2.15 注册表渲染：按钮由 toolButtons 描述符数组驱动，加按钮 = 追加一条 -->
    <div class="zoom-controls">
      {#each toolButtons as b (b.id)}
        <button class="zc-btn" class:active={b.active} onclick={b.onClick} title={b.title}>{b.label}</button>
      {/each}
    </div>

    <!-- v1.4 颜色图例（左下角）：类型配色 + 状态样式提示 -->
    {#if snapshot && showLegend}
      <div class="legend">
        <div class="legend-title">图例</div>
        {#each typeLegend as l (l.t)}
          <div class="legend-row">
            <span class="dot" style:background={l.color}></span>
            <span class="legend-label">{l.label}</span>
          </div>
        {/each}
        <div class="legend-sep"></div>
        <div class="legend-row"><span class="dot ring-glow"></span><span class="legend-label">进行中（发光）</span></div>
        <div class="legend-row"><span class="dot dot-red"></span><span class="legend-label">失败（红）</span></div>
        <div class="legend-row"><span class="dot dot-dim"></span><span class="legend-label">待开始（半透明）</span></div>
        <div class="legend-row"><span class="dot dot-dash"></span><span class="legend-label">阻塞（虚线框）</span></div>
        <div class="legend-row"><span class="dot dot-code"></span><span class="legend-label">已挂载代码骨架（青绿描边 + {'</>'} 角标）</span></div>
        <div class="legend-row"><span class="dot dot-openloop"></span><span class="legend-label">任务未闭环（缺验证节点，琥珀虚线框）</span></div>
        <div class="legend-sep"></div>
        <div class="legend-row"><span class="legend-label small">圆点大小 = 连接数（平缓）</span></div>
        <div class="legend-row"><span class="legend-label small">单击节点 = 波源 + 右侧信息栏显示名称/正文 · <b>双击节点 = 拉近看该节点及周边（再双击/ Esc 退回）</b></span></div>
        <div class="legend-row"><span class="legend-label small">图上不显示名称：靠类型配色 + 圆点大小（度）+ 悬停看 id，名称在右侧信息栏</span></div>
        <div class="legend-row"><span class="legend-label small">滚轮 = 缩放 · ＋/－按钮 = 逐步缩放 · ⌖ 按钮 = 回到全图</span></div>
        <div class="legend-row"><span class="legend-label small">点击最亮 → 直接相关次之 → 逐级递减（只有相关节点受波震动）</span></div>
        <div class="legend-row"><span class="legend-label small">搜索框 = 关键字定位节点 · 悬停 = 显示 id</span></div>
        {#if scanMode === 'analysis'}
          <div class="legend-row"><span class="legend-label small">连线渐变 = 源类型色 → 目标类型色</span></div>
        {/if}
        <div class="legend-row"><span class="legend-label small">拖动节点松手 = 钉住该节点（工具栏「重排」可归位）</span></div>
        <div class="legend-row"><span class="legend-label small">布局 = 树结构确定性排版（层内父居中 / 径向扇区），连线交叉恒为 0</span></div>
        <div class="legend-row"><span class="legend-label small">可见深度 = 只铺开到第几层，避免上千个圆点挤成一团</span></div>
        {#if scanMode === 'dev'}
          <div class="legend-sep"></div>
          <div class="legend-row"><span class="rel-sample rel-solid"></span><span class="legend-label small">实线 = 包含（从属）</span></div>
          <div class="legend-row"><span class="rel-sample rel-dashed"></span><span class="legend-label small">虚线 = 解决局限（递进主线）</span></div>
          <div class="legend-row"><span class="rel-sample rel-dotted"></span><span class="legend-label small">点线 = 备选替代</span></div>
          <div class="legend-sep"></div>
          <div class="legend-row"><span class="legend-label small">水面波场：单击节点 = 生成波源（持续向四周传播）</span></div>
          <div class="legend-row"><span class="legend-label small">再点同一节点 = 逐渐停止 · 点其它节点 = 次级波源（能量较弱）</span></div>
          <div class="legend-row"><span class="legend-label small">点击最亮 → 直接相关次之 → 逐级递减 · 双击 = 聚焦视图</span></div>
        {/if}
      </div>
    {/if}
  </div>

  {#if snapshot}
    <Sidebar
      node={selectedNode}
      chainDir={chainDir}
      mode={scanMode}
      allNodes={snapshot?.nodes ?? []}
      onSave={handleSave}
      onCancel={() => {
        // v2.6 常驻信息栏：✕/取消 = 收起为右缘细条（不再"关闭"）
        sidebarCollapsed = true;
        clearFocus(cy ?? null);
      }}
      onFold={handleFold}
      onDelete={handleDeleteNode}
      onSetParent={handleSetParent}
      onCodeMapChange={handleCodeMapChange}
      collapsed={sidebarCollapsed}
      onExpand={() => (sidebarCollapsed = false)}
    />
  {/if}

  {#if showCreate && snapshot}
    <CreateNodeDialog
      nodes={snapshot.nodes}
      onCreate={handleCreateNode}
      onCancel={() => (showCreate = false)}
    />
  {/if}

  <StatusBar snapshot={snapshot} chainDir={chainDir} mode={scanMode} onrescan={loadChain} />
  </div>

  <!-- v2.19/v2.20 文件树模式：覆盖整个窗口的独立视图（左=节点文件树，右=阅读/编辑/新建）。
       图谱与画布保持原样挂在下面（零破坏、退出即原状）；本视图不进 __engramDebug，脚本/AI 无从识别。
       写操作（新建/编辑/删除/改挂载）全部经 App 回调 → 后端 core 守门。 -->
  {#if readMode && snapshot}
    <ReaderMode
      snapshot={snapshot}
      chainDir={chainDir}
      mode={scanMode}
      initialNodeId={selectedNode?.id ?? null}
      onExit={exitReadMode}
      onSelect={handleReadSelect}
      onOpenCode={handleReadOpenCode}
      onLocate={handleReadLocate}
      onSave={handleReadSave}
      onCreate={handleReadCreate}
      onDelete={handleReadDelete}
      onSetParent={handleReadSetParent}
    />
  {/if}
</main>

<style>
  main {
    display: flex;
    height: 100vh;
    background: #0a0a0a;
    color: rgba(255, 255, 255, 0.85);
  }
  .app-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  /* v2.1 工具栏模式徽标（只读展示，切换在左侧栏） */
  .mode-chip {
    font-size: 10px;
    padding: 3px 10px;
    border-radius: 999px;
    color: #a78bfa;
    background: rgba(167, 139, 250, 0.1);
    border: 1px solid rgba(167, 139, 250, 0.3);
    letter-spacing: 1px;
    box-shadow: 0 0 14px rgba(167, 139, 250, 0.12);
  }
  .mode-chip.dev {
    color: #34d399;
    background: rgba(52, 211, 153, 0.1);
    border-color: rgba(52, 211, 153, 0.3);
    box-shadow: 0 0 14px rgba(52, 211, 153, 0.12);
  }
  /* v2.19 阅读模式切换按钮（图谱 ↔ 文件树；激活态呼应阅读覆盖层的青蓝色） */
  .read-toggle {
    font-size: 11.5px;
    padding: 5px 13px;
    letter-spacing: 0.5px;
  }
  .read-toggle.active {
    color: #a5d2ff;
    background: rgba(165, 210, 255, 0.16);
    border-color: rgba(165, 210, 255, 0.55);
    box-shadow: 0 0 14px rgba(165, 210, 255, 0.16);
  }
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;              /* v1.4 窄窗口自适应换行 */
    gap: 8px 12px;
    min-height: 52px;
    height: auto;
    padding: 8px 20px;
    background: rgba(15, 15, 15, 0.92);
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    backdrop-filter: blur(14px);
    flex-shrink: 0;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  }
  .logo {
    font-weight: 500;
    font-size: 13px;
    letter-spacing: 2px;
    color: rgba(255, 255, 255, 0.9);
  }
  .pick {
    font-size: 12px;
    padding: 6px 16px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 999px;
    cursor: pointer;
    transition:
      background 0.2s var(--ease-soft),
      border-color 0.2s var(--ease-soft),
      transform 0.15s var(--ease-out);
  }
  .pick:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.16);
    border-color: rgba(255, 255, 255, 0.22);
    transform: translateY(-1px);
  }
  .pick:active:not(:disabled) { transform: translateY(0) scale(0.97); }
  .pick:disabled { opacity: 0.4; cursor: not-allowed; }
  .dir {
    font-family: 'Consolas', monospace;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.45);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 40vw;
  }
  .slider-group {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .slider-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.6);
    white-space: nowrap;
  }
  .slider-val {
    display: inline-block;
    min-width: 36px;
    font-family: 'Consolas', monospace;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.65);
    text-align: right;
  }
  .slider-label input[type="range"] {
    width: 80px;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }
  .slider-label input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: background 0.15s;
  }
  .slider-label input[type="range"]::-webkit-slider-thumb:hover {
    background: rgba(255, 255, 255, 0.95);
  }
  /* v2.5 默认值圆点：轨道上标记软件的默认设置，点击恢复默认。
     对齐要点：圆点 left 用 calc(半thumb + (100% - thumb) × 默认比例) 按真实 thumb 行程定位；
     .slider-track 消除 inline 基线空隙（line-height/font-size 归零），保证 top:50% 与轨道中心重合 */
  .slider-track {
    position: relative;
    display: inline-block;
    line-height: 0;
    font-size: 0;
    vertical-align: middle;
  }
  .slider-track input[type="range"] {
    width: 80px;
    margin: 0;   /* UA 默认 margin 会让输入框在轨道内偏移 2px，圆点对不齐 */
  }
  .slider-def-dot {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #7dd3fc;
    border: 1px solid rgba(255, 255, 255, 0.55);
    box-shadow: 0 0 6px rgba(125, 211, 252, 0.9);
    cursor: pointer;
    padding: 0;
    z-index: 2;
    transition: transform 0.12s ease;
  }
  .slider-def-dot:hover { transform: translate(-50%, -50%) scale(1.45); }
  /* v3.0 布局形态选择器：与滑条同高同宽，视觉上仍是一排参数控件 */
  .layout-select {
    height: 20px;
    width: 74px;
    background: rgba(255, 255, 255, 0.06);
    color: #e6edf3;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 4px;
    font-size: 11px;
    padding: 0 4px;
    cursor: pointer;
    vertical-align: middle;
  }
  .layout-select:hover { border-color: rgba(255, 255, 255, 0.32); }
  .layout-select:focus { outline: none; border-color: rgba(125, 211, 252, 0.6); }
  .layout-select option { background: #16181d; color: #e6edf3; }
  .spacer { flex: 1; }
  .create-btn {
    background: rgba(52, 211, 153, 0.12);
    border: 1px dashed rgba(52, 211, 153, 0.4);
    color: #34d399;
    transition:
      background 0.2s var(--ease-soft),
      border-color 0.2s var(--ease-soft),
      transform 0.15s var(--ease-out);
  }
  .create-btn:hover:not(:disabled) {
    background: rgba(52, 211, 153, 0.2);
    border-color: rgba(52, 211, 153, 0.65);
    transform: translateY(-1px);
  }
  .create-btn:active:not(:disabled) { transform: translateY(0) scale(0.97); }
  .snap-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .snap-input {
    font-size: 11px;
    padding: 6px 10px;
    width: 110px;
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 999px;
  }
  .snap-input:focus { outline: none; border-color: rgba(255, 255, 255, 0.3); }
  .snap-input:disabled { opacity: 0.4; }
  .snap-btn { flex-shrink: 0; }
  .snap-msg {
    font-size: 10px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.5);
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .error-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 20px;
    background: rgba(248, 113, 113, 0.12);
    color: #f87171;
    font-size: 12px;
    font-family: 'Consolas', monospace;
    flex-shrink: 0;
  }
  .dismiss {
    background: none;
    border: none;
    color: #f87171;
    cursor: pointer;
    font-size: 14px;
    padding: 0 4px;
  }
  .canvas-wrap {
    flex: 1;
    position: relative;
    min-height: 0;
  }
  .cy-container {
    position: absolute;
    inset: 0;
    z-index: 1;
    background: transparent;   /* v2.4 两模式统一：透出水面画布 */
  }
  /* v2.2 水面画布（开发模式）：节点层之下；v2.13 渐变底色走 CSS（每帧只 clearRect 擦旧环，
     免去每帧全屏渐变填充——60fps 流畅度优化） */
  .water-canvas {
    position: absolute;
    inset: 0;
    z-index: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    background: linear-gradient(180deg, #070d18 0%, #0a1524 55%, #060b13 100%);
  }
  /* v1.7 悬停浮层（id · 类型）：跟随节点渲染坐标，不拦截鼠标 */
  .hover-tip {
    position: absolute;
    transform: translate(-50%, -100%);
    padding: 5px 9px;
    font-size: 10px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.9);
    background: rgba(25, 25, 28, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    pointer-events: none;
    z-index: 20;
    white-space: nowrap;
    backdrop-filter: blur(10px);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
    animation: fade-slide-in 0.15s var(--ease-out);
  }
  .empty-hint {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.35);
    pointer-events: none;
    z-index: 1;
  }
  .empty-icon {
    font-size: 40px;
    margin-bottom: 12px;
    opacity: 0.5;
    animation: hint-float 3.2s ease-in-out infinite;
  }
  .empty-hint p { font-size: 13px; margin: 0; letter-spacing: 0.5px; }
  .sub-hint { font-size: 11px !important; color: rgba(255, 255, 255, 0.2); margin-top: 6px !important; }

  /* v2.4 节点关键字搜索（画布左上角） */
  .node-search {
    position: absolute;
    top: 12px;
    left: 14px;
    z-index: 30;
    width: 264px;
  }
  .ns-input-row {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-float);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 6px 10px;
    backdrop-filter: blur(14px);
    box-shadow: var(--shadow-float);
    transition: border-color 0.2s var(--ease-soft);
  }
  .ns-input-row:focus-within {
    border-color: rgba(167, 139, 250, 0.45);
  }
  .ns-icon { color: rgba(255, 255, 255, 0.45); font-size: 13px; }
  .ns-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: rgba(255, 255, 255, 0.92);
    font-size: 12px;
    min-width: 0;
  }
  .ns-input::placeholder { color: rgba(255, 255, 255, 0.45); }
  .ns-clear {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.45);
    cursor: pointer;
    font-size: 11px;
    padding: 0 2px;
  }
  .ns-clear:hover { color: rgba(255, 255, 255, 0.85); }
  .ns-results {
    margin-top: 8px;
    background: var(--bg-float);
    border: 1px solid var(--line);
    border-radius: 10px;
    max-height: 300px;
    overflow-y: auto;
    backdrop-filter: blur(14px);
    box-shadow: var(--shadow-float);
    animation: fade-slide-in 0.22s var(--ease-out);
  }
  .ns-empty {
    padding: 10px 12px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
  }
  .ns-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: transparent;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding: 7px 10px;
    cursor: pointer;
    text-align: left;
  }
  .ns-item:last-child { border-bottom: none; }
  .ns-item:hover { background: rgba(255, 255, 255, 0.07); }
  .ns-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    box-shadow: 0 0 6px rgba(255, 255, 255, 0.4);
  }
  .ns-title {
    flex: 1;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.88);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ns-id {
    font-size: 10px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.5);
    flex: none;
  }

  /* v2.3 波纹参数测试面板（开发模式，右上角） */
  .wave-params {
    position: absolute;
    top: 12px;
    right: 16px;
    z-index: 10;
    width: 218px;
    background: var(--bg-float);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 10px 14px;
    backdrop-filter: blur(14px);
    box-shadow: var(--shadow-float);
    animation: fade-slide-in 0.25s var(--ease-out);
  }
  .wp-head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.6);
    font-size: 11px;
    font-family: inherit;
    letter-spacing: 1px;
    cursor: pointer;
    padding: 2px 0;
  }
  .wp-head:hover { color: rgba(255, 255, 255, 0.9); }
  .wp-body { margin-top: 6px; }
  .wp-row {
    display: grid;
    grid-template-columns: 34px 46px 1fr;
    align-items: center;
    gap: 8px;
    margin: 6px 0;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.65);
  }
  .wp-val {
    font-family: 'Consolas', monospace;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.6);
    text-align: right;
  }
  .wp-row input[type="range"] {
    width: 100%;
    height: 4px;
    margin: 0;   /* 同上：归零默认 margin，保证圆点与轨道/thumb 行程对齐 */
    -webkit-appearance: none;
    appearance: none;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }
  .wp-row .slider-track { width: 100%; }
  .wp-row input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: rgba(125, 211, 252, 0.85);
    cursor: pointer;
  }
  /* v1.4 缩放控件（右下角） */
  .zoom-controls {
    position: absolute;
    right: 16px;
    bottom: 34px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    z-index: 10;
  }
  .zc-btn {
    width: 30px;
    height: 30px;
    font-size: 14px;
    line-height: 1;
    background: rgba(20, 20, 22, 0.82);
    color: rgba(255, 255, 255, 0.75);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 8px;
    cursor: pointer;
    backdrop-filter: blur(8px);
    transition:
      background 0.2s var(--ease-soft),
      color 0.2s var(--ease-soft),
      transform 0.15s var(--ease-out);
  }
  .zc-btn:hover {
    background: rgba(45, 45, 50, 0.9);
    color: #fff;
    transform: translateY(-1px);
  }
  .zc-btn:active { transform: translateY(0) scale(0.94); }
  /* v2.14 「代码」筛选按钮激活态 */
  .zc-btn.active {
    color: #34d399;
    border-color: rgba(52, 211, 153, 0.65);
    background: rgba(52, 211, 153, 0.14);
  }

  /* v2.18 全屏代码页覆盖层 */
  .code-fs-mask {
    position: fixed;
    inset: 0;
    z-index: 3000;
    background: #0d1117;
    overflow-y: auto;
    animation: codefs-in 0.18s var(--ease-out);
  }
  @keyframes codefs-in {
    from { opacity: 0; transform: scale(0.985); }
    to { opacity: 1; transform: scale(1); }
  }
  .code-fs-close {
    position: fixed;
    top: 14px;
    right: 18px;
    z-index: 3001;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.18);
    border-radius: 999px;
    width: 36px;
    height: 36px;
    font-size: 15px;
    cursor: pointer;
    transition: background 0.15s var(--ease-soft);
  }
  .code-fs-close:hover {
    background: rgba(248, 113, 113, 0.3);
  }

  /* v1.4 颜色图例（左下角） */
  .legend {
    position: absolute;
    left: 16px;
    bottom: 34px;
    padding: 12px 14px;
    background: var(--bg-float);
    border: 1px solid var(--line);
    border-radius: 12px;
    z-index: 10;
    max-width: 220px;
    backdrop-filter: blur(14px);
    box-shadow: var(--shadow-float);
    animation: fade-slide-in 0.3s var(--ease-out);
  }
  .legend-title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 1px;
    color: rgba(255, 255, 255, 0.55);
    margin-bottom: 8px;
  }
  .legend-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 3px 0;
  }
  .legend-label { font-size: 11px; color: rgba(255, 255, 255, 0.72); white-space: nowrap; }
  .legend-label.small { font-size: 10px; color: rgba(255, 255, 255, 0.5); }
  /* v2.4 递进关系线型样例 */
  .rel-sample {
    display: inline-block;
    width: 18px;
    height: 0;
    border-top: 1px solid rgba(165, 210, 255, 0.7);
    flex-shrink: 0;
  }
  .rel-dashed { border-top-style: dashed; }
  .rel-dotted { border-top-style: dotted; }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #888;
  }
  .dot.ring-glow {
    background: #22d3ee;
    box-shadow: 0 0 8px 1px rgba(34, 211, 238, 0.8);
  }
  .dot-red { background: #f87171; }
  .dot-dim { opacity: 0.4; }
  .dot-dash {
    background: transparent;
    border: 1px dashed rgba(255, 255, 255, 0.55);
  }
  /* v2.14 图例：代码骨架描边样点 */
  .dot-code {
    background: transparent;
    border: 2px solid #34d399;
  }
  /* v2.16 图例：任务未闭环样点（琥珀虚线） */
  .dot-openloop {
    background: transparent;
    border: 2px dashed #fbbf24;
  }
  .legend-sep {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 8px 0;
  }
</style>
