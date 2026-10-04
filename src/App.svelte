<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { NODE_TYPE_LABEL, NODE_TYPE_COLOR, type LayoutMode } from './lib/node_style';
  import Sidebar from './lib/Sidebar.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import ReaderMode from './lib/ReaderMode.svelte';
  import DialogueReader from './lib/DialogueReader.svelte';
  import Graph3D, { type Graph3DApi } from './lib/Graph3D.svelte';
  import WorkspaceSidebar from './components/WorkspaceSidebar.svelte';
  import PerfOverlay from './components/PerfOverlay.svelte';
  import CodeViewer from './lib/CodeViewer.svelte';
  import { panel, SIDEBAR_COLLAPSED_WIDTH } from './lib/panel_state.svelte.ts';
  import { perfTierName } from './lib/ui/perf';
  import type { ChainSnapshot, ChainNode, NodeType, ScanMode, WorkspaceInfo } from './lib/types';

  // ── 布局参数（驱动 Graph3D 的三维神经元布局）──────────────────────────────
  // 形态由 Graph3D.computeNeuron3D 解释：可见深度 = 枝条铺开到第几层（渐进披露），
  // 层间距 / 同层间距 / 形态 = 枝条的径向间距与展开度（确定性布局，无物理迭代）。

  // 布局模式：auto 按可见规模自动选（≤300 分层 / 否则径向），也可手动锁定
  let layoutMode = $state<'auto' | LayoutMode>(
    (localStorage.getItem('engram-layout-mode') as 'auto' | LayoutMode) || 'auto',
  );
  // P2-8 布局算法：auto = 按工作区模式自动（分析模式 = 层级球壳 / 开发模式 = 神经元）；
  // 也可手动锁定（分析项目里想看神经元形态、或知识库里想看层级时用）。
  let layoutAlgo = $state<'auto' | 'neuron' | 'hierarchy'>(
    (localStorage.getItem('engram-layout-algo') as 'auto' | 'neuron' | 'hierarchy') || 'auto',
  );
  function applyLayoutAlgo(a: 'auto' | 'neuron' | 'hierarchy') {
    layoutAlgo = a;
    localStorage.setItem('engram-layout-algo', a);
  }
  let levelGap = $state(96);      // 层间距（px）
  let siblingGap = $state(46);    // 同层最小中心距（px）

  // 渐进披露：可见深度（1 = 只看根与第一层）。屏幕像素有物理下限——
  // 实测可读预算约 150–300 节点，所以大图默认只展开浅层，而不是把 1500 个点压成雾。
  let visibleDepth = $state(Number(localStorage.getItem('engram-visible-depth') ?? 3));
  let autoDepth = $state(true);   // 自动：按节点规模选一个"能读"的深度（用户手动调过则关闭）

  function applyLayoutMode(m: 'auto' | LayoutMode) {
    layoutMode = m;
    localStorage.setItem('engram-layout-mode', m);
  }

  /** 自动深度：在"看得见结构"和"一屏放得下"之间取平衡 */
  function autoDepthFor(total: number): number {
    if (total <= 300) return 99;    // 不裁剪
    if (total <= 800) return 4;
    if (total <= 2000) return 3;
    return 2;
  }

  let chainDir = $state<string | null>(null);
  let lastDir: string | null = null;   // v2.0：跟踪已加载目录，切换时清图（非响应式）
  let snapshot = $state<ChainSnapshot | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let selectedNode = $state<ChainNode | null>(null);
  // v2.6 侧栏常驻：点空白 = 收起为右缘细条（保留所选节点），顶部按钮拉出；单击节点切换显示内容
  let sidebarCollapsed = $state(false);
  // v2.6 双击聚焦视图：focusNodeId = 最近一次聚焦的节点（由 Graph3D 的相机聚焦回调写入）
  let focusNodeId: string | null = null;
  // v2.12 归档视图开关（加性）：淡色虚线纳入归档节点（默认关，零破坏）
  let showArchived = $state(false);
  // v2.19 显示方式切换：图谱视图 ↔ 阅读模式（节点文件树 + 全文阅读）。
  // ⚠️ 阅读模式是人类专属视图（刻意设计）：不写工作区任何文件、不注册 MCP 工具、不进 AI 指南副本、
  //    不进 __engramDebug 调试接缝；视图偏好只落 GUI 本地 localStorage——AI 读 .chain/ 看不到痕迹，
  //    既无法识别也无法使用（详见 src/lib/ReaderMode.svelte 头部不变量说明）。
  let readMode = $state(localStorage.getItem('engram-view-mode') === 'read');
  // 三层重构 P3：对话阅读面（只读；覆盖层，Esc 关闭，不进 __engramDebug）
  let dialogueMode = $state(false);
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
  }

  function exitReadMode() {
    readMode = false;
    localStorage.setItem('engram-view-mode', 'graph');
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

  // ── 三层重构 P2：人治写通道已移除（设计稿 v1 §9）─────────────────────────
  // 文件树模式从"人的编辑面"降为"只读阅读面"：handleReadSave/Create/Delete/SetParent
  // 与图谱侧 handleSave/CreateNode/DeleteNode/SetParent/fold 全部删除。
  // 记忆写入唯一入口 = MCP remember（AI 通道）；GUI 只保留维护通道
  // （reindex / code_map 挂载 / 快照 / 过程日志）。

  // v1.1 多工作区：左侧栏管理；每个文件夹绑定自己的模式（.chain/.mode 标签）
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
    hoverTip = null;
    lastDir = null;
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

  let unlisten: (() => void) | undefined;

  // v1.4 图例数据（v2.15 单一来源：从 NODE_TYPE_COLOR 派生，不再手工重复颜色表；标签保持原样）
  const LEGEND_LABEL_EXTRA: Record<NodeType, string> = { goal: '', design: '', task: '', verification: '', note: '（知识库）' };
  const typeLegend = (Object.keys(NODE_TYPE_COLOR) as NodeType[]).map((t) => ({
    t,
    label: `${NODE_TYPE_LABEL[t]} ${t}${LEGEND_LABEL_EXTRA[t]}`,
    color: NODE_TYPE_COLOR[t],
  }));
  let showLegend = $state(true);
  // 图例开关的用户偏好：用户显式点过图例开关后记进 localStorage（尊重用户选择，不再被自动逻辑改写）
  let legendUserSet = typeof localStorage !== 'undefined' && localStorage.getItem('engram-legend-user') === '1';
  // P2-9 覆盖遮挡：图例默认**折叠**成一行小标题（点标题展开），把画面留给图；
  // 展开与折叠都会被 measureInsets 计入，相机取景自动避开（用户偏好持久化）。
  let legendFolded = $state(
    typeof localStorage !== 'undefined' ? localStorage.getItem('engram-legend-folded') !== '0' : true,
  );
  function toggleLegendFold() {
    legendFolded = !legendFolded;
    localStorage.setItem('engram-legend-folded', legendFolded ? '1' : '0');
  }

  // v2.14 「代码」筛选开关：一键高亮有代码骨架的节点（其余压暗）
  let codeFilter = $state(false);
  // v2.15 性能浮层开关
  let perfOpen = $state(false);

  // v1.7 悬停浮层：显示 id · 类型（id 已从画布标签移除以突出标题命名，悬停/点击可追溯）
  let hoverTip = $state<{ x: number; y: number; text: string } | null>(null);

  // 三层重构后 · 三维图谱（纯图结构；Unity 式交互）
  let graphApi: Graph3DApi | null = $state(null);
  let wrapEl: HTMLDivElement;
  function on3DHover(tip: { x: number; y: number; text: string } | null) {
    if (!tip) { hoverTip = null; return; }
    const r = wrapEl?.getBoundingClientRect();
    hoverTip = r ? { x: tip.x - r.left, y: tip.y - r.top, text: tip.text } : tip;
  }

  // ── P2-9 覆盖遮挡：把贴边的浮层（图例 / 搜索框 / 缩放控件）量成相机内边距 ──
  // 相机 centerAll 时把内容居中到"未被压住"的自由区，而不是画布正中——浮层不再盖住节点。
  let legendEl = $state<HTMLDivElement | undefined>(undefined);
  let searchEl = $state<HTMLDivElement | undefined>(undefined);
  let zoomEl = $state<HTMLDivElement | undefined>(undefined);
  let insets = $state({ left: 0, right: 0, top: 0, bottom: 0 });
  function measureInsets() {
    const base = wrapEl?.getBoundingClientRect();
    if (!base || base.width === 0) return;
    let left = 0, right = 0, top = 0, bottom = 0;
    const EDGE = 28;   // 距画布边缘多少 px 以内算"贴边遮挡"
    const consider = (el?: HTMLElement) => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      if (r.width < 8 || r.height < 8) return;
      const l = r.left - base.left, rr = base.right - r.right;
      const t = r.top - base.top, b = base.bottom - r.bottom;
      if (l <= EDGE) left = Math.max(left, r.width + l);
      else if (rr <= EDGE) right = Math.max(right, r.width + rr);
      if (t <= EDGE) top = Math.max(top, r.height + t);
      if (b <= EDGE) bottom = Math.max(bottom, r.height + b);
    };
    consider(legendEl);
    consider(searchEl);
    consider(zoomEl);
    // 单个方向最多让出 32%：浮层再大也不至于把图挤成一条缝
    const capW = base.width * 0.32, capH = base.height * 0.32;
    insets = {
      left: Math.round(Math.min(left, capW)),
      right: Math.round(Math.min(right, capW)),
      top: Math.round(Math.min(top, capH)),
      bottom: Math.round(Math.min(bottom, capH)),
    };
  }
  // 浮层尺寸/位置变化后重新量（快照、图例开合、信息栏宽度、窗口缩放）
  $effect(() => {
    void snapshot;
    void showLegend;
    void legendFolded;
    void sidebarCollapsed;
    void panel.width;
    void searchText;
    const id = setTimeout(measureInsets, 0);
    const onResize = () => measureInsets();
    window.addEventListener('resize', onResize);
    return () => { clearTimeout(id); window.removeEventListener('resize', onResize); };
  });

  async function loadChain() {
    if (!chainDir) return;
    // v2.0 修复：切换目录时清空旧图状态（新旧目录节点 id 相同（如都是初始化的 g-001）时，
    // 若沿用旧选中/聚焦状态会出现"新图停在旧节点上"的错觉）。
    // 同目录"重新扫描"不触发清理（保留 v1.6 的位置续排手感）。
    if (chainDir !== lastDir) {
      lastDir = chainDir;
      selectedNode = null;
      sidebarCollapsed = false;
      focusNodeId = null;      // v2.6 切目录重置双击聚焦
      hoverTip = null;
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

  // ── 三层重构 P2：handleSave / handleCreateNode / handleDeleteNode / handleSetParent
  // （信息栏编辑面写通道）已移除——GUI 为只读观察面，记忆写入唯一入口 = MCP remember。

  // v2.13 代码栏：挂载/移除后重扫（后端返回新快照，刷新信息栏的 code_map 状态）——维护通道保留
  function handleCodeMapChange(newSnapshot: ChainSnapshot, nodeId: string) {
    snapshot = newSnapshot;
    const nodeData = newSnapshot.nodes.find((x) => x.id === nodeId) ?? null;
    if (nodeData) selectedNode = nodeData;
  }

  // v2.14 「代码」筛选开关：一键高亮有代码骨架的节点（其余压暗）。
  // 渲染由 Graph3D 按 codeFilter 属性完成（3D 球体变暗），App 只持有开关状态。
  function toggleCodeFilter() {
    codeFilter = !codeFilter;
  }

  // v2.15 工具栏按钮注册表（未来扩展：追加一条描述符即可，渲染层自动接线）
  const toolButtons = $derived.by(() => [
    {
      id: 'zoom-in',
      label: '+',
      title: '相机拉近（滚轮亦可）',
      active: false,
      onClick: () => graphApi?.zoomBy(1.4),
    },
    {
      id: 'fit',
      label: '⤢',
      title: '回全图（3D 相机复位到覆盖全部可见节点）',
      active: false,
      onClick: () => graphApi?.centerAll(),
    },
    {
      id: 'zoom-out',
      label: '−',
      title: '相机拉远（滚轮亦可）',
      active: false,
      onClick: () => graphApi?.zoomBy(1 / 1.4),
    },
    { id: 'code', label: '</>', title: '高亮有代码骨架的节点（青绿描边 + 辉光，其余压暗）', active: codeFilter, onClick: toggleCodeFilter },
    { id: 'perf', label: '⚡', title: '性能浮层：FPS/帧耗时/规模分档', active: perfOpen, onClick: () => (perfOpen = !perfOpen) },
    { id: 'dialogue', label: '💬', title: '对话阅读面（工作过程，只读）——三层重构：对话是讲解、节点是脉络', active: false, onClick: () => (dialogueMode = true) },
    {
      id: 'legend',
      label: showLegend ? '◉' : '○',
      title: '图例开关（左下角类型配色图例的显示/隐藏）',
      active: false,
      onClick: () => {
        showLegend = !showLegend;
        // 用户显式操作 → 记下来，不再自动收起
        legendUserSet = true;
        localStorage.setItem('engram-legend-user', '1');
      },
    },
  ]);

  // ── 三层重构 P2：折叠（fold_chain 结构写通道）已随人治下线移除 ──

  // v1.3：快照（工具栏按钮 → 输入标签 → 创建）——只读复制通道，保留
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

  // 自动深度：按新图规模取一个"能读"的展开层数（用户手动调过滑条后就尊重用户）。
  // 旧实现在 2D 画布的全量重建 effect 里顺带做，画布退役后独立成一个最小 effect——
  // 语义不变：只在新快照到来时按规模重设，用户一旦手调 autoDepth=false 就不再干预。
  $effect(() => {
    const snap = snapshot;
    if (!snap) return;
    if (autoDepth) visibleDepth = autoDepthFor(snap.nodes.length);
  });

  onMount(() => {
    // v2.15 调试/自动化钩子（CDP 驱动验证与未来插件扩展的稳定接缝，只读访问）。
    // ⚠️ 三维视图（Graph3D）刻意**不进**调试接缝（设计约束，勿加）：这里只暴露数据与布局参数，
    // 画面状态一律通过真实服务/用户操作观察。
    (window as any).__engramDebug = {
      get snapshot() {
        return snapshot;
      },
      get mode() {
        return scanMode;
      },
      get layout() {
        return {
          mode: layoutMode,
          algo: layoutAlgo,
          effectiveAlgo: layoutAlgo === 'auto' ? (scanMode === 'analysis' ? 'hierarchy' : 'neuron') : layoutAlgo,
          levelGap,
          siblingGap,
          visibleDepth,
          autoDepth,
          insets,
          legendFolded,
        };
      },
    };

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
        // 三维视图的「退出聚焦」= 相机复位回全图（与 2D 时代 Esc 退出聚焦同语义）
        if (focusNodeId !== null) {
          focusNodeId = null;
          graphApi?.centerAll();
          return;
        }
        sidebarCollapsed = true;
      }
    };
    window.addEventListener('keydown', onKeydown);

    // M5: 监听后端 chain-changed 事件，自动刷新图谱（三层重构 P2：写通道已移除，
    // 不再有"自写窗口"——所有写入都来自外部 AI，实时进图）
    // 前端去抖：watcher 后端已有 300ms 去抖，但 AI 批量操作时前端再兜一层防连环打断
    let chainDebounce: ReturnType<typeof setTimeout> | undefined;
    listen<ChainSnapshot>('chain-changed', (e) => {
      // v2.19 文件树模式是纯阅读面（无侧栏在途编辑）：不吃"编辑中不覆盖"的保护，外部写入实时进树
      if (selectedNode && !readMode) return;
      clearTimeout(chainDebounce);
      chainDebounce = setTimeout(() => { snapshot = e.payload; }, 150);
    }).then(u => unlisten = u);
    listen<string>('chain-error', (e) => {
      console.warn('[chain-gui] watcher error:', e.payload);
    });

    return () => {
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
    if (!graphApi || !snapshot) return;
    if (!snapshot.nodes.some((n) => n.id === nodeId)) return;
    searchOpen = false;
    graphApi.focusNode(nodeId);
    graphApi.setSearchHit(nodeId);
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
        <label class="slider-label" title="展开度（三维神经元布局）：标准=分支锥角 1.05；紧凑=0.8（枝条抱拢、结构更密）；舒展=1.35（枝条张开、树突更明显）">形态<span class="slider-val">{layoutMode === 'auto' ? '标准' : layoutMode === 'radial' ? '舒展' : '紧凑'}</span>
          <span class="slider-track">
            <select class="layout-select" value={layoutMode}
              onchange={(e) => applyLayoutMode((e.target as HTMLSelectElement).value as 'auto' | LayoutMode)}>
              <option value="auto">标准</option>
              <option value="layered">紧凑</option>
              <option value="radial">舒展</option>
            </select>
          </span>
        </label>
        <label class="slider-label" title="布局算法（P2-8）：自动 = 按工作区模式（分析模式用层级球壳：半径 = 深度 × 层距，同层同壳，一眼看出第几层；开发模式用三维神经元展开）；也可手动锁定">布局<span class="slider-val">{layoutAlgo === 'auto' ? (scanMode === 'analysis' ? '层级（自动）' : '神经元（自动）') : layoutAlgo === 'hierarchy' ? '层级球壳' : '神经元'}</span>
          <span class="slider-track">
            <select class="layout-select" value={layoutAlgo}
              onchange={(e) => applyLayoutAlgo((e.target as HTMLSelectElement).value as 'auto' | 'neuron' | 'hierarchy')}>
              <option value="auto">自动（按模式）</option>
              <option value="hierarchy">层级球壳</option>
              <option value="neuron">神经元</option>
            </select>
          </span>
        </label>
        <button class="pick" title="相机复位：按包围盒重新取景（布局是确定性的，无需重排）"
                onclick={() => graphApi?.centerAll()}>重排</button>
      </span>
    {/if}
    <button class="pick" onclick={copyAiGuide} title="复制 AI 使用指南全文，贴给 AI 即完成协议交底">
      {guideCopied ? '已复制 ✓' : '复制 AI 指南'}
    </button>
    {#if chainDir}
      <button class="pick" onclick={loadChain} disabled={loading || !snapshot}>
        重新扫描
      </button>
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
       三维画布、右下角缩放按钮、左下角图例只出现在信息栏左侧，不再被压住；无图谱时不预留 -->
  <div class="canvas-wrap" bind:this={wrapEl} style:margin-right={(snapshot ? (sidebarCollapsed ? SIDEBAR_COLLAPSED_WIDTH : panel.width) : 0) + 'px'}>
    {#if !snapshot && !loading}
      <div class="empty-hint">
        <div class="empty-icon">⛓</div>
        <p>在左侧工作区栏添加或选择一个文件夹</p>
        <p class="sub-hint">添加时按当前页签确定模式：分析（AI 链协议）/ 开发（自由知识库）</p>
      </div>
    {/if}

    <!-- 三层重构后 · 三维图结构：球节点 + 类型配色边；右键旋转 / 中键平移 / 滚轮缩放 / 左键选中 / 双击聚焦 -->
    {#if snapshot}
      <Graph3D
        snapshot={snapshot}
        selectedId={selectedNode?.id ?? null}
        codeFilter={codeFilter}
        showArchived={showArchived}
        layout={{ visibleDepth, siblingGap, levelGap, layoutMode, algo: layoutAlgo, mode: scanMode }}
        {insets}
        onready={(api) => (graphApi = api)}
        onselect={(id) => {
          if (!snapshot || !id) return;
          selectedNode = snapshot.nodes.find((x) => x.id === id) ?? null;
          if (selectedNode) sidebarCollapsed = false;
        }}
        onhover={on3DHover}
        onfocus={(id) => (focusNodeId = id)}
      />
    {/if}

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
      <div class="node-search" bind:this={searchEl}>
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

    <!-- v1.4 缩放控件（右下角）：滚轮之外的按钮式缩放 + 全局适配 + 图例开关
         v2.15 注册表渲染：按钮由 toolButtons 描述符数组驱动，加按钮 = 追加一条 -->
    <div class="zoom-controls" bind:this={zoomEl}>
      {#each toolButtons as b (b.id)}
        <button class="zc-btn" class:active={b.active} onclick={b.onClick} title={b.title}>{b.label}</button>
      {/each}
    </div>

    <!-- v1.4 颜色图例（左下角）：类型配色 + 状态样式提示
         P2-9：默认折叠成一行小标题（点标题展开/收起），避免长期压住节点；展开时相机取景会让开 -->
    {#if snapshot && showLegend}
      <div class="legend" class:folded={legendFolded} bind:this={legendEl}>
        <button class="legend-title" onclick={toggleLegendFold}
                title={legendFolded ? '展开图例（默认折叠，避免压住节点）' : '折叠图例（把画面留给图）'}>
          <span class="legend-caret">{legendFolded ? '▸' : '▾'}</span> 图例
        </button>
        {#if !legendFolded}
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
        <div class="legend-row"><span class="legend-label small"><b>球体大小 = 度数 degree</b>（与 AI 读到的 degree / children_count 同一份指标，见右侧信息栏「结构」）</span></div>
        <div class="legend-row"><span class="legend-label small">左键单击 = 选中（右侧信息栏显示名称/正文）· <b>左键双击 = 相机聚焦该节点</b></span></div>
        <div class="legend-row"><span class="legend-label small">图上不显示名称：靠类型配色 + 球体大小（度）+ 悬停看 id，名称在右侧信息栏</span></div>
        <div class="legend-row"><span class="legend-label small"><b>右键拖拽 = 旋转视角 · 中键拖拽 = 平移 · 滚轮 = 缩放</b> · ⤢ 按钮 = 回全图</span></div>
        <div class="legend-row"><span class="legend-label small">搜索框 = 关键字定位节点（相机飞过去 + 脉冲高亮）· 悬停 = 显示 id</span></div>
        {#if scanMode === 'analysis'}
          <div class="legend-row"><span class="legend-label small">连线渐变 = 源类型色 → 目标类型色</span></div>
        {/if}
        <div class="legend-row"><span class="legend-label small">布局 = {layoutAlgo === 'hierarchy' || (layoutAlgo === 'auto' && scanMode === 'analysis')
          ? '层级球壳（半径 = 深度 × 层距，同层同壳；青色陀螺环 = 各深度层；与 AI 读到的 depth 同源）'
          : '三维神经元展开（根 = 胞体，子树自球面/锥面辐射成树突）'}· 形态滑条 = 展开度</span></div>
        <div class="legend-row"><span class="legend-label small">可见深度 = 只铺开到第几层，避免上千个球体挤成一团</span></div>
        {#if scanMode === 'dev'}
          <div class="legend-sep"></div>
          <div class="legend-row"><span class="rel-sample rel-solid"></span><span class="legend-label small">实线 = 包含（从属）</span></div>
          <div class="legend-row"><span class="rel-sample rel-dashed"></span><span class="legend-label small">虚线 = 解决局限（递进主线）</span></div>
          <div class="legend-row"><span class="rel-sample rel-dotted"></span><span class="legend-label small">点线 = 备选替代</span></div>
        {/if}
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
      onCancel={() => {
        // v2.6 常驻信息栏：✕/取消 = 收起为右缘细条（不再"关闭"）
        sidebarCollapsed = true;
        focusNodeId = null;
      }}
      onCodeMapChange={handleCodeMapChange}
      collapsed={sidebarCollapsed}
      onExpand={() => (sidebarCollapsed = false)}
    />
  {/if}

  <StatusBar snapshot={snapshot} chainDir={chainDir} mode={scanMode} onrescan={loadChain} />

  <!-- 三层重构 P3：对话阅读面（只读覆盖层；渲染期投影，不落盘） -->
  {#if dialogueMode && chainDir}
    <DialogueReader chainDir={chainDir} onExit={() => (dialogueMode = false)} />
  {/if}
  </div>

  <!-- v2.19/v2.20 文件树模式：覆盖整个窗口的独立视图（左=节点文件树，右=只读阅读）。
       图谱与画布保持原样挂在下面（零破坏、退出即原状）；本视图不进 __engramDebug，脚本/AI 无从识别。
       三层重构 P2：写操作面（新建/编辑/删除/改挂载）已移除——本视图为只读阅读面。 -->
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
  /* P2-9 图例标题即折叠开关（默认折叠 → 只占一行，不再压住节点） */
  .legend-title {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 1px;
    color: rgba(255, 255, 255, 0.55);
    margin-bottom: 8px;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    font-family: inherit;
    transition: color 0.18s var(--ease-soft);
  }
  .legend-title:hover { color: rgba(255, 255, 255, 0.9); }
  .legend-caret { font-size: 9px; opacity: 0.75; }
  .legend.folded {
    padding: 6px 10px;
    border-radius: 8px;
  }
  .legend.folded .legend-title { margin-bottom: 0; }
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
