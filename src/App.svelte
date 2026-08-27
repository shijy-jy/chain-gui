<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import cytoscape from 'cytoscape';
  import type { StylesheetJson, Core } from 'cytoscape';
  import { chainToElements, NODE_TYPE_LABEL } from './lib/chain_to_cytoscape';
  import Sidebar from './lib/Sidebar.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import CreateNodeDialog from './components/CreateNodeDialog.svelte';
  import type { ChainSnapshot, ChainNode, NodeStatus, NodeType, ScanMode } from './lib/types';

  // v1.5：cose 在链式图上会缩成团块 → 换自研全局力导向模拟（d3-force 风格）：
  //   所有节点两两斥力（库仑式）+ 边弹簧吸引 + 弱中心引力 = 神经元式全局铺开
  // v1.6 物理标定 + 钳制（修复"堆在一起/点击爆开"）：
  //   - 斥力默认 30000：与弹簧刚度 0.15、理想长 120 的平衡距离 ≈ 55px（节点 14-38px），铺开不塌缩
  //   - 单次力上限 MAX_F 60 + 每帧位移钳制 MAX_STEP 10：近距离 K/d² 不再爆炸
  //   - 散点位置首帧立即写入：加载即圆环，不再有 (0,0) 堆叠瞬间
  let repulsion = $state(30000);   // 全局节点间斥力强度
  let gravity = $state(0.15);      // 链接弹簧刚度（相连节点吸引）
  let edgeLen = $state(120);       // 弹簧理想长度

  // v1.5 力导向模拟运行时（requestAnimationFrame 句柄；null = 未在运行）
  let forceRun: number | null = null;

  function stopForce() {
    if (forceRun !== null) {
      cancelAnimationFrame(forceRun);
      forceRun = null;
    }
  }

  // v1.7：初始散点位置改由 chainToElements 预写入（根节点锚定原点，其余绕根圆环），
  // 这里统一"从当前位置续排"——首次加载从预散点起排，拖动/滑条调整从当前位置起排。
  function runForceLayout(cyRef: Core) {
    stopForce();
    const nodeArr = cyRef.nodes().toArray();   // 固定顺序的节点数组（模拟期间成员不变）
    const edges = cyRef.edges();
    const n = nodeArr.length;
    if (n === 0) return;
    if (n === 1) {
      cyRef.fit(undefined, 80);
      return;
    }

    const pos = nodeArr.map((nd) => ({ x: nd.position('x'), y: nd.position('y') }));

    const vx = new Float64Array(n);
    const vy = new Float64Array(n);

    const K = repulsion;   // 库仑斥力常数（所有节点两两互斥）
    const SPRING = gravity; // 边弹簧刚度
    const REST = edgeLen;   // 弹簧理想长度

    // v1.6 钳制：单次力上限 + 每帧位移上限（近距离不爆炸）
    const MAX_F = 60;
    const MAX_STEP = 10;

    let alpha = 1.0;
    const ALPHA_DECAY = 0.97;   // 模拟冷却（~150 tick ≈ 2.5s 收敛）
    const MAX_ITER = 400;

    let iter = 0;
    const tick = () => {
      forceRun = null;
      if (iter++ >= MAX_ITER || alpha < 0.01) {
        // v1.7 收敛后平滑适配视野（原先瞬跳，观感像"图自己跳到中央"）
        cyRef.animate({
          fit: { eles: cyRef.elements(), padding: 60 },
          duration: 300,
          easing: 'ease-out',
        });
        return;
      }
      // 1) 全节点两两斥力 O(n²)：F = min(K/d², MAX_F)（这就是"神经元链接"式全局铺开的关键）
      for (let i = 0; i < n; i++) {
        for (let j = i + 1; j < n; j++) {
          let dx = pos[j].x - pos[i].x;
          let dy = pos[j].y - pos[i].y;
          let d2 = dx * dx + dy * dy;
          if (d2 < 4) {
            d2 = 4;
            dx = (Math.random() - 0.5) * 4;
            dy = (Math.random() - 0.5) * 4;
          }
          const d = Math.sqrt(d2);
          const f = Math.min((K / d2) * alpha, MAX_F);
          const fx = (dx / d) * f;
          const fy = (dy / d) * f;
          vx[i] -= fx;
          vy[i] -= fy;
          vx[j] += fx;
          vy[j] += fy;
        }
      }
      // 2) 边弹簧吸引：F = 刚度 × (当前长 - 理想长)，沿边方向
      edges.forEach((e) => {
        const si = nodeArr.indexOf(e.source());
        const ti = nodeArr.indexOf(e.target());
        if (si < 0 || ti < 0) return;
        let dx = pos[ti].x - pos[si].x;
        let dy = pos[ti].y - pos[si].y;
        const d = Math.max(Math.sqrt(dx * dx + dy * dy), 1);
        const f = SPRING * (d - REST) * alpha;
        const fx = (dx / d) * f;
        const fy = (dy / d) * f;
        vx[si] += fx;
        vy[si] += fy;
        vx[ti] -= fx;
        vy[ti] -= fy;
      });
      // 3) 弱中心引力（防整体漂移）
      const gc = 0.05 * alpha;
      for (let i = 0; i < n; i++) {
        vx[i] -= pos[i].x * gc;
        vy[i] -= pos[i].y * gc;
      }
      // 4) 阻尼 + 积分 + 位移钳制（v1.6: 每帧最多 MAX_STEP，杜绝飞出/爆开）
      for (let i = 0; i < n; i++) {
        vx[i] *= 0.86;
        vy[i] *= 0.86;
        let sx = vx[i];
        let sy = vy[i];
        const sp = Math.hypot(sx, sy);
        if (sp > MAX_STEP) {
          sx = (sx / sp) * MAX_STEP;
          sy = (sy / sp) * MAX_STEP;
        }
        pos[i].x += sx;
        pos[i].y += sy;
      }
      alpha *= ALPHA_DECAY;
      // 5) 写回画布
      for (let i = 0; i < n; i++) nodeArr[i].position({ x: pos[i].x, y: pos[i].y });
      forceRun = requestAnimationFrame(tick);
    };
    forceRun = requestAnimationFrame(tick);
  }

  // v1.7 首帧视图：全图同步 fit 后再把根节点对准屏幕中央。
  // 以前首帧渲染在模型原点（= 视口左上角），力模拟收敛后才 fit，产生"左上角堆叠→跳中央"的闪烁。
  function initialView(cyRef: Core, rootId: string | null) {
    if (cyRef.nodes().length === 0) return;
    cyRef.fit(undefined, 70);   // 同步适配全图（无动画，首帧即最终视口）
    if (rootId) {
      const rootEl = cyRef.getElementById(rootId);
      if (rootEl.nonempty()) cyRef.center(rootEl);   // 根节点对准屏幕中央
    }
  }

  let chainDir = $state<string | null>(null);
  let snapshot = $state<ChainSnapshot | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let selectedNode = $state<ChainNode | null>(null);
  let needsInit = $state(false);
  let initializing = $state(false);

  // v2.0 软件模式：analysis（严格链协议）/ dev（自由知识图谱），持久化到 localStorage
  let scanMode = $state<ScanMode>(
    (localStorage.getItem('chain-gui-mode') as ScanMode) ?? 'analysis'
  );
  let showCreate = $state(false);

  function switchMode(m: ScanMode) {
    if (m === scanMode) return;
    scanMode = m;
    localStorage.setItem('chain-gui-mode', m);
    // 通知后端（watcher 后续文件变化按新模式重扫），有目录时拿新快照刷新视图
    invoke<ChainSnapshot | null>('set_mode', { mode: m })
      .then((snap) => {
        if (snap) snapshot = snap;
      })
      .catch((e) => (error = `切换模式失败：${String(e)}`));
  }

  let container: HTMLDivElement;
  let cy: Core | null = null;
  let unlisten: (() => void) | undefined;

  // 重要性 = 大小：连接数（度）越多，圆点越大。
  // v1.4：改为平方根缓增 + 封顶，大小对比温和（Obsidian 风格）：
  //   度0→14px, 度4→22px, 度9→26px, 度16→30px, 度36+→38px（封顶）
  const nodeSize = (ele: any): number => 14 + Math.min(Math.sqrt(ele.degree()), 6) * 4;

  // v1.4 图例数据（与节点配色一致，UI 上直接提示颜色含义）
  const typeLegend: { t: string; label: string; color: string }[] = [
    { t: 'goal', label: '目标 goal', color: '#a78bfa' },
    { t: 'design', label: '设计 design', color: '#60a5fa' },
    { t: 'task', label: '任务 task', color: '#22d3ee' },
    { t: 'verification', label: '验证 verification', color: '#34d399' },
  ];
  let showLegend = $state(true);

  // v1.7 悬停浮层：显示 id · 类型（id 已从画布标签移除以突出标题命名，悬停/点击可追溯）
  let hoverTip = $state<{ x: number; y: number; text: string } | null>(null);

  const style: StylesheetJson = [
    {
      selector: 'node',
      style: {
        'shape': 'ellipse',
        'label': 'data(label)',   // v1.7 显示名 = 「类型 · 标题」，生成见 chain_to_cytoscape
        'font-size': '11px',
        'color': 'rgba(255,255,255,0.92)',
        'text-opacity': 1,
        'text-valign': 'bottom',
        'text-margin-y': 8,
        'text-wrap': 'wrap',
        'text-max-width': 150,
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
        'transition-property': 'opacity, background-opacity, line-color, width',
        'transition-duration': '0.25s',
      } as any,
    },
    // 类型 = 颜色
    { selector: 'node[nodeType = "goal"]',         style: { 'background-color': '#a78bfa' } },
    { selector: 'node[nodeType = "design"]',       style: { 'background-color': '#60a5fa' } },
    { selector: 'node[nodeType = "task"]',         style: { 'background-color': '#22d3ee' } },
    { selector: 'node[nodeType = "verification"]', style: { 'background-color': '#34d399' } },
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
    { selector: 'node:selected', style: { 'border-width': 2, 'border-color': '#ffffff', 'border-opacity': 0.95, 'border-style': 'solid' } },
    // v2.0 边：现代图谱观感（参照 Obsidian 类型色边 / SqlMesh 渐变连线）
    // - 渐变线：源类型色 → 目标类型色（方向感一眼可读）
    // - 圆头微曲线 + 低饱和底透明度 + 悬停点亮
    {
      selector: 'edge',
      style: {
        'width': 1.2,
        'curve-style': 'bezier',
        'control-point-distances': '52px',
        'control-point-weights': 0.5,
        'line-cap': 'round',
        'line-fill': 'linear-gradient',
        'line-gradient-stop-colors': 'data(gradColors)',
        'line-gradient-stop-positions': '0%, 100%',
        'target-arrow-shape': 'triangle',
        'target-arrow-color': 'data(tgtColor)',
        'arrow-scale': 0.6,
        'opacity': 0.5,
        // v1.4 聚焦过渡
        'transition-property': 'opacity, width',
        'transition-duration': '0.2s',
      },
    },
    { selector: 'edge:hover', style: { 'opacity': 1, 'width': 2.4 } },
    // v1.6.1 Obsidian 风格点击聚焦：淡出无关元素、点亮选中节点与邻居
    // 注意：dim 不透明度不宜过低（0.10 在黑底上近似隐形，用户误以为"数据全没了"），
    // 且关闭侧栏/按 Esc/点空白处都必须解除聚焦
    { selector: 'node.focus-dim', style: { 'opacity': 0.16, 'text-opacity': 0.15 } },
    { selector: 'edge.focus-dim', style: { 'opacity': 0.06 } },
    { selector: 'node.focus-lit', style: { 'opacity': 1, 'text-opacity': 1 } },
    { selector: 'edge.focus-lit', style: { 'opacity': 1, 'width': 2.2 } },
  ];

  async function pickChainDir() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') {
      chainDir = selected;
      await loadChain();
    }
  }

  async function loadChain() {
    if (!chainDir) return;
    loading = true;
    error = null;
    needsInit = false;
    try {
      snapshot = await invoke<ChainSnapshot>('scan_chain', { dir: chainDir, mode: scanMode });
    } catch (e) {
      const msg = String(e);
      if (msg.includes('不存在 .chain')) {
        needsInit = true;
      } else {
        error = msg;
      }
    } finally {
      loading = false;
    }
  }

  async function handleInit() {
    if (!chainDir || initializing) return;
    initializing = true;
    error = null;
    try {
      await invoke<ChainSnapshot>('init_chain', { dir: chainDir, mode: scanMode });
      // init_chain 不启动文件监听；这里补一次 scan_chain 让 watcher 生效（改文件自动刷新）
      snapshot = await invoke<ChainSnapshot>('scan_chain', { dir: chainDir, mode: scanMode });
      needsInit = false;
    } catch (e) {
      error = String(e);
    } finally {
      initializing = false;
    }
  }

  async function handleSave(fields: { title: string; status: NodeStatus; body: string; tags: string[]; evidence: string[] }) {
    if (!chainDir || !selectedNode) return;
    // 失败时 invoke reject，错误由 Sidebar 的 catch 显示；成功才更新 snapshot 并关侧栏
    const newSnapshot = await invoke<ChainSnapshot>('update_node', {
      dir: chainDir,
      nodeId: selectedNode.id,
      fields: fields,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    selectedNode = null;
    cy?.elements().removeClass('focus-dim focus-lit');   // v1.4 关闭侧栏同时解除聚焦
  }

  // v2.0 开发模式：新建节点（id 留空 = 后端自动生成）
  async function handleCreateNode(input: { id: string; title: string; node_type: NodeType; status: NodeStatus; parent: string | null }) {
    if (!chainDir) return;
    const newSnapshot = await invoke<ChainSnapshot>('create_node', {
      dir: chainDir,
      input: {
        id: input.id || null,
        title: input.title,
        node_type: input.node_type,
        status: input.status,
        parent: input.parent,
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
    cy?.elements().removeClass('focus-dim focus-lit');
  }

  // v2.0 开发模式：建立/断开链接
  async function handleSetParent(nodeId: string, parent: string | null) {
    if (!chainDir) return;
    const newSnapshot = await invoke<ChainSnapshot>('set_parent', {
      dir: chainDir,
      nodeId,
      parent,
      mode: scanMode,
    });
    snapshot = newSnapshot;
    const nodeData = newSnapshot.nodes.find((x) => x.id === nodeId);
    if (nodeData) selectedNode = nodeData;
  }

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
    cy?.elements().removeClass('focus-dim focus-lit');
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

  // v1.6 节点集合签名：watcher 推送同一批节点（如无关快照）时不再随机重散，
  // 而是从当前位置续排；只有节点集合真正变化才散点重排
  let lastSig = '';

  // ⚠️ Svelte 5 坑：if (cy && snapshot) 短路求值会让 effect 漏追踪 snapshot
  // （第一次跑时 cy=null，JS 短路求值不会读 snapshot，Svelte 5 不会追踪）
  // 修法：先单独读 snapshot 强制让 Svelte 5 追踪到
  $effect(() => {
    if (!snapshot) return;  // 强制追踪 snapshot
    if (!cy) return;
    const cyRef = cy;

    // 追踪滑块值，拖动时触发重新模拟
    const _r = repulsion;
    const _g = gravity;
    const _e = edgeLen;

    const sig = snapshot.nodes.map(x => x.id).sort().join(',');
    if (sig === lastSig && cyRef.elements().length > 0) {
      // v1.6 同一批节点（滑条调整/watcher 重推）→ 不重新散点；
      // v1.7 但必须重建元素数据（外部编辑标题/状态后标签要刷新），重建后恢复各节点原位置
      const keep = new Map(cyRef.nodes().map((nd) => [nd.id(), nd.position()] as const));
      cyRef.elements().remove();
      cyRef.add(chainToElements(snapshot));
      cyRef.nodes().forEach((nd) => {
        const p = keep.get(nd.id());
        if (p) nd.position(p);
      });
      runForceLayout(cyRef);
      return;
    }
    lastSig = sig;
    stopForce();
    cyRef.elements().remove();
    cyRef.add(chainToElements(snapshot));
    // v1.7 首帧视图：同步 fit 全图 + 根节点对准屏幕中央（消除"左上角堆叠→跳中央"的闪烁）
    initialView(cyRef, snapshot.manifest.root);
    runForceLayout(cyRef);   // v1.5 全局力导向：预散点起步，收敛后平滑适配视野
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
      const clearFocus = () => cy?.elements().removeClass('focus-dim focus-lit');
      cy.on('tap', 'node', (evt) => {
        const n = evt.target;
        stopForce();   // v1.5 点击聚焦时暂停力模拟，避免与镜头动画抢位置
        hoverTip = null;
        // v1.4 Obsidian 式点击动态: 全图淡出 → 点亮节点+邻居 → 平滑缩放聚焦
        const nbh = n.closedNeighborhood();
        cy?.elements().addClass('focus-dim');
        nbh.removeClass('focus-dim').addClass('focus-lit');
        cy?.animate({
          fit: { eles: nbh, padding: 110 },
          duration: 450,
          easing: 'ease-in-out',
        });
        const nodeId = n.id();
        const nodeData = snapshot?.nodes.find(x => x.id === nodeId);
        if (nodeData) selectedNode = nodeData;
      });
      cy.on('tap', (evt) => {
        if (evt.target === cy) {
          selectedNode = null;  // 点空白处关侧栏
          clearFocus();
        }
      });
      // v1.7 悬停浮层：id · 类型（id 已从画布标签移除，悬停即可追溯）
      cy.on('mouseover', 'node', (evt) => {
        const n = evt.target;
        const rp = n.renderedPosition();
        hoverTip = {
          x: rp.x,
          y: rp.y - 24,
          text: `${n.id()} · ${NODE_TYPE_LABEL[n.data('nodeType') as NodeType] ?? ''}`,
        };
      });
      cy.on('mouseout', 'node', () => (hoverTip = null));
      // v1.5 拖动节点：按住时暂停模拟，松开后从当前位置续排（Obsidian 手感）
      cy.on('grab', () => {
        hoverTip = null;
        stopForce();
      });
      cy.on('free', () => {
        if (cy) runForceLayout(cy);
      });
    } catch (e) {
      error = `[cytoscape init failed] ${(e as Error).message}`;
    }

    const onResize = () => { cy?.resize(); cy?.fit(undefined, 60); };
    window.addEventListener('resize', onResize);
    // v1.6.1 Esc 一键退出聚焦模式并关侧栏（防"节点不见了"的错觉）
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        selectedNode = null;
        cy?.elements().removeClass('focus-dim focus-lit');
      }
    };
    window.addEventListener('keydown', onKeydown);

    // M5: 监听后端 chain-changed 事件，自动刷新图谱（侧栏编辑中不覆盖）
    // 前端去抖：watcher 后端已有 300ms 去抖，但 AI 批量操作时前端再兜一层防连环打断
    let chainDebounce: ReturnType<typeof setTimeout> | undefined;
    listen<ChainSnapshot>('chain-changed', (e) => {
      if (selectedNode) return;
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

  onDestroy(() => {
    unlisten?.();
    stopForce();
    cy?.destroy();
  });

  // M10: 复制 AI 使用指南到剪贴板（贴给任何 AI 即完成协议交底）
  let guideCopied = $state(false);
  let guideCopyTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyAiGuide() {
    try {
      const guide = await invoke<string>('get_ai_guide');
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
</script>

<main>
  <header class="toolbar">
    <span class="logo">⛓ chain-gui</span>
    <button class="pick" onclick={pickChainDir} disabled={loading}>
      {loading ? '加载中…' : chainDir ? '换目录' : '选目录'}
    </button>
    {#if shortDir}
      <span class="dir" title={chainDir ?? ''}>{shortDir}</span>
    {/if}
    <!-- v2.0 模式切换：分析=严格链协议校验；开发=自由知识图谱（字段宽松、任意增删节点与链接） -->
    <span class="mode-switch" title="分析模式：严格 chain 协议校验（AI 协作图谱）&#10;开发模式：自由搭建知识库——字段无必填、可自由增删节点与链接">
      <button class="pick mode-btn" class:active={scanMode === 'analysis'} onclick={() => switchMode('analysis')}>分析</button>
      <button class="pick mode-btn" class:active={scanMode === 'dev'} onclick={() => switchMode('dev')}>开发</button>
    </span>
    <span class="spacer"></span>
    {#if snapshot}
      <span class="slider-group">
        <label class="slider-label" title="节点间全局斥力：所有节点两两互斥，越大越散（神经元式铺开）">排斥<span class="slider-val">{repulsion}</span>
          <input type="range" min="2000" max="80000" step="1000" value={repulsion}
            onchange={(e) => repulsion = +(e.target as HTMLInputElement).value} />
        </label>
        <label class="slider-label" title="链接弹簧刚度：相连节点相互吸引，越大越紧">引力<span class="slider-val">{gravity.toFixed(2)}</span>
          <input type="range" min="0.05" max="0.6" step="0.01" value={gravity}
            onchange={(e) => gravity = +(e.target as HTMLInputElement).value} />
        </label>
        <label class="slider-label" title="弹簧理想长度：越大图越舒展">边距<span class="slider-val">{edgeLen}</span>
          <input type="range" min="80" max="260" step="10" value={edgeLen}
            onchange={(e) => edgeLen = +(e.target as HTMLInputElement).value} />
        </label>
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

  <div class="canvas-wrap">
    {#if needsInit}
      <div class="empty-hint">
        <div class="empty-icon">⛓</div>
        <p>该目录还不是 chain 工程</p>
        <p class="sub-hint">初始化将创建 .chain/nodes/ 并生成一个示例节点</p>
        <button class="init-btn" onclick={handleInit} disabled={initializing}>
          {initializing ? '初始化中…' : '初始化 chain'}
        </button>
      </div>
    {:else if !snapshot && !loading}
      <div class="empty-hint">
        <div class="empty-icon">⛓</div>
        <p>点左上角「选目录」选择 .chain 父目录</p>
      </div>
    {/if}
    <div bind:this={container} class="cy-container"></div>

    <!-- v1.7 悬停浮层：节点 id · 类型（定位跟随节点渲染坐标） -->
    {#if hoverTip}
      <div class="hover-tip" style:left="{hoverTip.x}px" style:top="{hoverTip.y}px">{hoverTip.text}</div>
    {/if}

    <!-- v1.4 缩放控件（右下角）：滚轮之外的按钮式缩放 + 全局适配 + 图例开关 -->
    <div class="zoom-controls">
      <button class="zc-btn" onclick={() => cy?.zoom(cy.zoom() * 1.4)} title="放大（滚轮亦可）">+</button>
      <button class="zc-btn" onclick={() => cy?.fit(undefined, 60)} title="适配全部节点">⤢</button>
      <button class="zc-btn" onclick={() => cy?.zoom(cy.zoom() / 1.4)} title="缩小（滚轮亦可）">−</button>
      <button class="zc-btn" onclick={() => (showLegend = !showLegend)} title="图例开关">{showLegend ? '◉' : '○'}</button>
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
        <div class="legend-sep"></div>
        <div class="legend-row"><span class="legend-label small">圆点大小 = 连接数（平缓）</span></div>
        <div class="legend-row"><span class="legend-label small">点击 = 聚焦局部 · 悬停 = 显示 id · 滚轮 = 缩放</span></div>
        <div class="legend-row"><span class="legend-label small">连线渐变 = 源类型色 → 目标类型色</span></div>
        <div class="legend-row"><span class="legend-label small">拖动节点松手 = 自动重新布局</span></div>
      </div>
    {/if}
  </div>

  {#if selectedNode}
    <Sidebar
      node={selectedNode}
      chainDir={chainDir}
      mode={scanMode}
      allNodes={snapshot?.nodes ?? []}
      onSave={handleSave}
      onCancel={() => {
        selectedNode = null;
        cy?.elements().removeClass('focus-dim focus-lit');   // v1.6.1 关闭侧栏必须解除聚焦
      }}
      onFold={handleFold}
      onDelete={handleDeleteNode}
      onSetParent={handleSetParent}
    />
  {/if}

  {#if showCreate && snapshot}
    <CreateNodeDialog
      nodes={snapshot.nodes}
      onCreate={handleCreateNode}
      onCancel={() => (showCreate = false)}
    />
  {/if}

  <StatusBar snapshot={snapshot} chainDir={chainDir} onrescan={loadChain} />
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #0a0a0a;
    color: rgba(255, 255, 255, 0.85);
  }
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;              /* v1.4 窄窗口自适应换行 */
    gap: 8px 12px;
    min-height: 52px;
    height: auto;
    padding: 8px 20px;
    background: #0f0f0f;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    flex-shrink: 0;
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
    transition: background 0.15s ease;
  }
  .pick:hover:not(:disabled) { background: rgba(255, 255, 255, 0.16); }
  .pick:disabled { opacity: 0.4; cursor: not-allowed; }
  .dir {
    font-family: 'Consolas', monospace;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.35);
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
    color: rgba(255, 255, 255, 0.5);
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
  .spacer { flex: 1; }
  /* v2.0 模式切换 */
  .mode-switch {
    display: inline-flex;
    gap: 0;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    overflow: hidden;
  }
  .mode-btn {
    border: none;
    border-radius: 0;
    padding: 6px 14px;
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
  }
  .mode-btn + .mode-btn { border-left: 1px solid rgba(255, 255, 255, 0.12); }
  .mode-btn:hover { color: rgba(255, 255, 255, 0.9); }
  .mode-btn.active { background: rgba(255, 255, 255, 0.14); color: #fff; }
  .create-btn {
    background: rgba(52, 211, 153, 0.12);
    border: 1px dashed rgba(52, 211, 153, 0.4);
    color: #34d399;
  }
  .create-btn:hover:not(:disabled) { background: rgba(52, 211, 153, 0.2); }
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
    color: rgba(255, 255, 255, 0.4);
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
  }
  /* v1.7 悬停浮层（id · 类型）：跟随节点渲染坐标，不拦截鼠标 */
  .hover-tip {
    position: absolute;
    transform: translate(-50%, -100%);
    padding: 4px 8px;
    font-size: 10px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.9);
    background: rgba(25, 25, 28, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    pointer-events: none;
    z-index: 20;
    white-space: nowrap;
  }
  .empty-hint {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.25);
    pointer-events: none;
    z-index: 1;
  }
  .empty-icon { font-size: 40px; margin-bottom: 12px; opacity: 0.5; }
  .empty-hint p { font-size: 13px; margin: 0; letter-spacing: 0.5px; }
  .sub-hint { font-size: 11px !important; color: rgba(255, 255, 255, 0.2); margin-top: 6px !important; }
  .init-btn {
    margin-top: 16px;
    font-size: 12px;
    padding: 8px 24px;
    background: rgba(255, 255, 255, 0.95);
    color: #0a0a0a;
    border: none;
    border-radius: 999px;
    cursor: pointer;
    font-weight: 500;
    letter-spacing: 0.5px;
    transition: all 0.15s ease;
    /* v2.0 修复：父容器 .empty-hint 有 pointer-events:none（防挡画布），
       pointer-events 会被子元素继承导致按钮点不到——必须显式恢复 */
    pointer-events: auto;
  }
  .init-btn:hover:not(:disabled) { background: #fff; box-shadow: 0 0 12px rgba(255, 255, 255, 0.15); }
  .init-btn:disabled { opacity: 0.4; cursor: not-allowed; }

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
    transition: all 0.15s ease;
  }
  .zc-btn:hover { background: rgba(45, 45, 50, 0.9); color: #fff; }

  /* v1.4 颜色图例（左下角） */
  .legend {
    position: absolute;
    left: 16px;
    bottom: 34px;
    padding: 12px 14px;
    background: rgba(15, 15, 17, 0.88);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    z-index: 10;
    max-width: 220px;
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
  .legend-label.small { font-size: 10px; color: rgba(255, 255, 255, 0.4); }
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
  .legend-sep {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 8px 0;
  }
</style>
