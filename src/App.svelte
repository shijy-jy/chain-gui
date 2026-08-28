<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import cytoscape from 'cytoscape';
  import type { StylesheetJson, Core } from 'cytoscape';
  import { chainToElements, NODE_TYPE_LABEL, NODE_TYPE_COLOR } from './lib/chain_to_cytoscape';
  import { computeRippleLayers, ripplePulseAmp, RIPPLE_MAX_DEPTH, type RippleLayers } from './lib/ripple';
  import Sidebar from './lib/Sidebar.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import CreateNodeDialog from './components/CreateNodeDialog.svelte';
  import WorkspaceSidebar from './components/WorkspaceSidebar.svelte';
  import type { ChainSnapshot, ChainNode, NodeStatus, NodeType, ScanMode, WorkspaceInfo } from './lib/types';

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
  // v2.0 性能修复（大图卡死根因）：
  //   - id→index Map 替代 indexOf（O(n·m)→O(m)）
  //   - cy.batch() 批量写位置（cytoscape 官方性能建议：避免每帧逐元素触发样式重算）
  //   - 早停：最大位移 <0.3px 连续 12 帧 → 收敛
  //   - 自适应迭代上限：节点越多帧数越少；n>400 直接跳过模拟（O(n²)/帧 不可行）
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

    // v2.0 超大图谱：跳过力模拟，直接适配视野（散点位置已在 chainToElements 预写入）
    if (n > 400) {
      cyRef.fit(undefined, 60);
      return;
    }

    // id → 索引映射
    const idx = new Map<string, number>();
    nodeArr.forEach((nd, i) => idx.set(nd.id(), i));

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
    // v2.0 自适应迭代上限：节点越多每帧 O(n²) 越贵，代数递减
    const MAX_ITER = n > 250 ? 100 : n > 100 ? 200 : 400;

    let iter = 0;
    let still = 0;   // 连续低位移帧计数（早停）
    const tick = () => {
      forceRun = null;
      if (iter++ >= MAX_ITER || alpha < 0.01 || still > 12) {
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
        const si = idx.get(e.source().id());
        const ti = idx.get(e.target().id());
        if (si === undefined || ti === undefined) return;
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
      let maxStep = 0;
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
        if (sp > maxStep) maxStep = sp;
      }
      alpha *= ALPHA_DECAY;
      if (maxStep < 0.3) still++; else still = 0;
      // 5) 写回画布（v2.0 批量写入：一次样式重算而非每节点一次）
      cyRef.batch(() => {
        for (let i = 0; i < n; i++) nodeArr[i].position({ x: pos[i].x, y: pos[i].y });
      });
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
  let lastDir: string | null = null;   // v2.0：跟踪已加载目录，切换时清图（非响应式）
  let snapshot = $state<ChainSnapshot | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let selectedNode = $state<ChainNode | null>(null);
  let showCreate = $state(false);

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
    hoverTip = null;
    lastDir = null;
    lastIdsSig = '';
    lastDataSig = '';
    lastSliderSig = '';
    stopForce();
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
  let waterBaseGrad: CanvasGradient | null = null;

  // ── v2.3 涟漪参数（测试面板，用户可调）──
  const waveParams = $state({
    energy: 0.55,      // 能量：环透明度与振动幅度
    period: 1.6,       // 周期（秒/圈，环从中心扩到边界的时间）
    lineWidth: 1.0,    // 粗细：环线宽（px，0.5–2.5）
    fade: 1.2,         // 衰减：环扩张过程中的透明度衰减速度（0.3–2.5，越大淡得越快）
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

  function applyRippleClasses(cyRef: Core, activeDepth: number) {
    const rip = ripple;
    if (!rip) return;
    cyRef.batch(() => {
      cyRef.nodes().forEach((n) => {
        const d = rip.layers.depth.get(n.id());
        n.removeClass('rip-dim rip-d0 rip-d1 rip-d2 rip-d3 rip-d4 rip-d5 rip-d6');
        // v2.4 分析模式 maxDepth=1：波前虽扩满全场，超过直接相连层的节点永不点亮
        if (d !== undefined && d <= activeDepth && d <= rip.maxDepth) n.addClass(`rip-d${d}`);
        else n.addClass('rip-dim');   // 波外、波前未达或超出响应层：压暗等待
      });
      // v2.2 涟漪期间连线整体淡出（transition 0.2s 平滑），联系改由亮度层级+波纹表达
      cyRef.edges().addClass('rip-hide');
    });
  }

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
  // 周围泛起局部小涟漪（相位随层级滞后）；节点只上下轻颤（不斜向）。
  function drawWater(cyRef: Core | null, t: number, frame: number) {
    if (!waterCanvas) return;
    if (!waterCtx) waterCtx = waterCanvas.getContext('2d');
    const ctx = waterCtx;
    if (!ctx) return;
    const w = waterCanvas.clientWidth;
    const h = waterCanvas.clientHeight;
    if (w === 0 || h === 0) return;
    if (waterCanvas.width !== w || waterCanvas.height !== h) {
      waterCanvas.width = w;
      waterCanvas.height = h;
      waterBaseGrad = null;
    }
    if (!waterBaseGrad) {
      const g = ctx.createLinearGradient(0, 0, 0, h);
      g.addColorStop(0, '#070d18');
      g.addColorStop(0.55, '#0a1524');
      g.addColorStop(1, '#060b13');
      waterBaseGrad = g;
    }
    ctx.globalAlpha = 1;
    ctx.fillStyle = waterBaseGrad;
    ctx.fillRect(0, 0, w, h);

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
    if (cyRef) {
      for (const s of active) {
        const ele = cyRef.getElementById(s.id);
        if (!ele.empty()) {
          const p = ele.renderedPosition();
          s.gx = p.x;
          s.gy = p.y;
        }
      }
    }

    const period = Math.max(0.2, waveParams.period);
    const fadePow = Math.max(0.3, waveParams.fade);
    const energyK = waveParams.energy / 0.55;
    const lw = Math.max(0.5, Math.min(2.5, waveParams.lineWidth));

    if (active.length === 0) return;

    // ── 细环：每个波源 3 圈同心细环持续扩散（圆内，边界 80% 起淡）──
    ctx.lineWidth = lw;
    for (const s of active) {
      for (let k = 0; k < 3; k++) {
        const ph = ((t / period) + k / 3) % 1;
        const r = ph * s.radPx;
        const edgeFade = r > s.radPx * 0.8 ? Math.max(0, 1 - (r / s.radPx - 0.8) / 0.2) : 1;
        const alpha = Math.pow(1 - ph, fadePow) * 0.5 * s.level * energyK * edgeFade;
        if (alpha <= 0.01) continue;
        ctx.beginPath();
        ctx.arc(s.gx, s.gy, r, 0, Math.PI * 2);
        ctx.strokeStyle = `rgba(165, 210, 255, ${alpha.toFixed(3)})`;
        ctx.stroke();
      }
    }

    // ── 节点局部涟漪：像波浪拍打礁石——小、快、碎（时钟为主波周期的 0.4 倍）──
    const rip = ripple;
    if (cyRef && rip) {
      const mainSrc = active.find((s) => s.main);
      const mainLevel = mainSrc ? mainSrc.level : 0;
      const nodePeriod = Math.max(0.25, period * 0.4);
      cyRef.nodes().forEach((n: any) => {
        const d = rip.layers.depth.get(n.id());
        if (d === undefined || d === 0 || d > rip.maxDepth) return;
        const strength = ripplePulseAmp(d) / 0.1;
        const p = n.renderedPosition();
        for (let k = 0; k < 2; k++) {
          const ph = ((t / nodePeriod) + d * 0.18 + k * 0.5) % 1;
          const r = ph * (18 + strength * 20);
          const alpha = Math.pow(1 - ph, fadePow) * 0.4 * strength * mainLevel * energyK;
          if (alpha <= 0.01) continue;
          ctx.beginPath();
          ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
          ctx.strokeStyle = `rgba(165, 210, 255, ${alpha.toFixed(3)})`;
          ctx.stroke();
        }
      });
    }

    // ── 节点运动（俯视语义）：一切"浮沉"都是缩放——源节点慢速深呼吸（向水里沉进浮出），
    //    受影响节点快而小的缩放脉冲（礁石式小涟漪）；点击瞬间沉水 = 额外缩小 ──
    if (!cyRef) return;
    const scaledNow = new Set<string>();
    cyRef.batch(() => {
      cyRef.nodes().forEach((n: any) => {
        const id = n.id();
        let strength = 0;
        let slowSrc = false;
        const src = active.find((s) => s.id === id);
        if (src) {
          strength = src.main ? 1.0 : 0.55;
          slowSrc = true;
        } else if (rip) {
          const d = rip.layers.depth.get(id);
          if (d === undefined || d > rip.maxDepth) return;
          strength = ripplePulseAmp(d) / 0.1;
        } else {
          return;
        }
        let scale: number;
        if (slowSrc) {
          scale = 1 + Math.sin((t / (period * 1.6)) * Math.PI * 2) * 0.13 * strength * energyK;
        } else {
          scale = 1 + Math.sin((t / period) * Math.PI * 2 - (rip?.layers.depth.get(id) ?? 0) * 0.5) * 0.1 * strength * energyK;
        }
        const dip = dips.find((dd) => dd.id === id);
        if (dip) {
          const dt = (nowMs - dip.t0) / 620;
          if (dt < 1) scale *= 1 - Math.sin(Math.PI * dt) * 0.35;   // 沉水：再缩小至 0.65
        }
        const base = nodeSize(n);
        n.style('width', `${base * scale}px`);
        n.style('height', `${base * scale}px`);
        scaledNow.add(id);
      });
    });
    // 清理上一帧仍在缩放、本帧已不再参与涟漪的节点
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

  // 水面动画循环（约 30fps，开发模式常驻）
  function startWaterLoop() {
    if (waterRaf !== null) return;
    const t0 = performance.now();
    const tick = () => {
      waterRaf = null;
      waterFrame += 1;
      if (waterFrame % 2 === 0) {
        drawWater(cy, (performance.now() - t0) / 1000, waterFrame);
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
    let radPx = 140;
    cyRef.nodes().forEach((n: any) => {
      const p = n.renderedPosition();
      const d = Math.hypot(p.x - srcPos.x, p.y - srcPos.y);
      if (d > radPx) radPx = d;
    });
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
      //     开发模式保留全层扩散（上限 RIPPLE_MAX_DEPTH）
      const maxDepth = scanMode === 'analysis' ? 1 : RIPPLE_MAX_DEPTH;
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
  const nodeSize = (ele: any): number => 14 + Math.min(Math.sqrt(ele.degree()), 6) * 4;

  // v2.0 边宽度函数：与两端节点大小挂钩（小节点 0.8px → 大节点 2.0px）
  const edgeBaseWidth = (ele: any): number => {
    const s = Math.min(nodeSize(ele.source()), nodeSize(ele.target()));
    return 0.8 + (s - 14) * 0.05;
  };

  // v1.4 图例数据（与节点配色一致，UI 上直接提示颜色含义；v2.0 增知识库中性「笔记」）
  const typeLegend: { t: string; label: string; color: string }[] = [
    { t: 'goal', label: '目标 goal', color: '#a78bfa' },
    { t: 'design', label: '设计 design', color: '#60a5fa' },
    { t: 'task', label: '任务 task', color: '#22d3ee' },
    { t: 'verification', label: '验证 verification', color: '#34d399' },
    { t: 'note', label: '笔记 note（知识库）', color: '#94a3b8' },
  ];
  let showLegend = $state(true);

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
    { selector: 'node:selected', style: { 'border-width': 2, 'border-color': '#ffffff', 'border-opacity': 0.95, 'border-style': 'solid' } },
    // v2.0 边：粗细与节点大小挂钩（用户反馈：边应随节点大小，且要细）——
    // 小节点(14px) 0.8px → 大节点(38px) 2.0px；曲率收敛（52→30px 控制距离，短边不再鼓大包）；
    // 渐变按 cytoscape 官方性能建议在大图（>300 边）降级为实线
    {
      selector: 'edge',
      style: {
        'width': edgeBaseWidth,
        'curve-style': 'bezier',
        'control-point-distances': '30px',
        'control-point-weights': 0.5,
        'line-cap': 'round',
        'line-color': 'rgba(148,163,184,0.45)',   // 实线兜底色
        'line-fill': 'linear-gradient',           // 停靠点颜色/位置由逐边内联样式提供（见 chainToElements）
        'target-arrow-shape': 'triangle',
        'target-arrow-color': 'rgba(255,255,255,0.35)',   // 兜底（每条边都有逐边样式覆盖为目标色）
        'arrow-scale': 0.55,
        'opacity': 0.5,
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
  ];

  async function loadChain() {
    if (!chainDir) return;
    // v2.0 修复：切换目录时清空旧图 + 重置三签名——
    // 新旧目录节点 id 相同（如都是初始化的 g-001）时签名判定"无变化"，
    // $effect 不重建，旧目录节点会残留重叠；扫描失败时旧图也必须清掉。
    // 同目录"重新扫描"不触发清理（保留 v1.6 的位置续排手感）。
    if (chainDir !== lastDir) {
      lastDir = chainDir;
      lastIdsSig = '';
      lastDataSig = '';
      lastSliderSig = '';
      selectedNode = null;
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
    // 失败时 invoke reject，错误由 Sidebar 的 catch 显示；成功才更新 snapshot 并关侧栏
    // v2.0 开发模式：status 为 null = 不写状态（知识库节点可有可无）
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

  // v2.0 三签名防抖（大图卡死的另一根因：watcher 噪音推送反复触发全图重建+重排）：
  // - idsSig（节点集合）：变化才重建元素 + 散点重排
  // - dataSig（字段内容）：变化只原位更新元素数据（不重排、不重建——保存/外部编辑不扰动布局）
  // - sliderSig（滑条）：变化只从当前位置续排
  let lastIdsSig = '';
  let lastDataSig = '';
  let lastSliderSig = '';

  // ⚠️ Svelte 5 坑：if (cy && snapshot) 短路求值会让 effect 漏追踪 snapshot
  // （第一次跑时 cy=null，JS 短路求值不会读 snapshot，Svelte 5 不会追踪）
  // 修法：先单独读 snapshot 强制让 Svelte 5 追踪到
  $effect(() => {
    if (!snapshot) return;  // 强制追踪 snapshot
    if (!cy) return;
    const snap = snapshot;   // TS 收窄：嵌套闭包里保持非空类型
    const cyRef = cy;

    // 追踪滑块值，拖动时触发重新模拟
    const _r = repulsion;
    const _g = gravity;
    const _e = edgeLen;
    const sliderSig = `${_r}-${_g}-${_e}`;

    const idsSig = snap.nodes.map(x => x.id).sort().join(',');
    const dataSig = snap.nodes
      .map(x => `${x.id}|${x.type}|${x.updated}|${x.revision}|${x.status}|${x.title}|${x.tags.join('~')}|${x.evidence.join('~')}`)
      .sort()
      .join(';');

    if (idsSig !== lastIdsSig) {
      // 节点集合变化：全量重建 + 预散点 + 首帧视图 + 力模拟
      lastIdsSig = idsSig;
      lastDataSig = dataSig;
      lastSliderSig = sliderSig;
      stopForce();
      cyRef.elements().remove();
      // v2.4 两模式统一：连线渲染为"若有若无"的淡线（.ghost），点击后整组淡出改由涟漪表达
      cyRef.add(chainToElements(snap, { withEdges: true }));
      cyRef.edges().addClass('ghost');
      startWaterLoop();
      // v1.7 首帧视图：同步 fit 全图 + 根节点对准屏幕中央（消除"左上角堆叠→跳中央"的闪烁）
      initialView(cyRef, snap.manifest.root);
      runForceLayout(cyRef);   // v1.5 全局力导向：预散点起步，收敛后平滑适配视野
      return;
    }

    if (dataSig !== lastDataSig) {
      // 内容变化（保存/外部编辑/watcher 推送）：原位更新节点与边数据，位置与布局不动
      lastDataSig = dataSig;
      cyRef.batch(() => {
        for (const def of chainToElements(snap)) {
          const ele = cyRef.getElementById(def.data.id as string);
          if (ele.nonempty()) ele.data(def.data);
        }
      });
      return;
    }

    if (sliderSig !== lastSliderSig) {
      // 仅滑条变化：从当前位置续排（保留 v1.6 手感）
      lastSliderSig = sliderSig;
      runForceLayout(cyRef);
      return;
    }
    // watcher 重推但内容无变化：直接忽略，避免大图反复重建卡顿
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
        hoverTip = null;
        // v2.4 两模式统一：点击节点 = 波源开关（再点停止），波纹表达关系；编辑改双击
        toggleWaveSource(n.id());
      });
      // v2.4 两模式统一：双击节点 = 打开编辑侧栏（单击已被涟漪交互占用）
      cy.on('dbltap', 'node', (evt) => {
        const n = evt.target;
        stopForce();
        const nodeData = snapshot?.nodes.find(x => x.id === n.id());
        if (nodeData) selectedNode = nodeData;
      });
      cy.on('tap', (evt) => {
        if (evt.target === cy) {
          selectedNode = null;  // 点空白处关侧栏
          clearFocus();
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
          text: `${n.id()} · ${NODE_TYPE_LABEL[n.data('nodeType') as NodeType] ?? ''}`,
        };
      });
      cy.on('mouseout', 'node', () => (hoverTip = null));
      // v2.0 边悬停高亮（cytoscape 无 :hover 选择器，用事件类实现）
      cy.on('mouseover', 'edge', (evt) => evt.target.addClass('edge-hover'));
      cy.on('mouseout', 'edge', (evt) => evt.target.removeClass('edge-hover'));
      // v1.5 拖动节点：按住时暂停模拟，松开后从当前位置续排（Obsidian 手感）
      cy.on('grab', () => {
        hoverTip = null;
        dragging = true;   // v2.3 拖拽时暂停水面振动偏移
        stopForce();
      });
      cy.on('free', () => {
        dragging = false;
        if (ripple) return;   // v2.3 涟漪中不重排（位置由水面驱动，布局已稳定）
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
        // v2.3 波源只由"再点同一节点"关闭（Esc 不停止波场）
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
    <span class="logo">⛓ chain-gui</span>
    {#if shortDir}
      <span class="dir" title={chainDir ?? ''}>{shortDir}</span>
    {/if}
    <span class="mode-chip" class:dev={scanMode === 'dev'} title={scanMode === 'dev' ? '开发模式：自由知识图谱' : '分析模式：严格链协议'}>
      {scanMode === 'dev' ? '开发' : '分析'}
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

    <!-- v2.3 波纹参数测试面板：能量/周期/粗细/衰减由用户调节（测试期两模式通用） -->
    <div class="wave-params">
        <button type="button" class="wp-head" onclick={() => (wavePanelOpen = !wavePanelOpen)}>
          <span class="chev">{wavePanelOpen ? '▾' : '▸'}</span> 波纹参数（测试）
        </button>
        {#if wavePanelOpen}
          <div class="wp-body">
            <label class="wp-row" title="波源能量：环透明度与振动幅度（次级波源自动为其 55%）">能量
              <span class="wp-val">{waveParams.energy.toFixed(2)}</span>
              <input type="range" min="0.1" max="1.4" step="0.05" value={waveParams.energy}
                     onchange={(e) => (waveParams.energy = +(e.target as HTMLInputElement).value)} />
            </label>
            <label class="wp-row" title="波动周期：一圈环从中心扩到边界的时间（秒）">周期
              <span class="wp-val">{waveParams.period.toFixed(1)}s</span>
              <input type="range" min="0.3" max="4" step="0.1" value={waveParams.period}
                     onchange={(e) => (waveParams.period = +(e.target as HTMLInputElement).value)} />
            </label>
            <label class="wp-row" title="波纹粗细：涟漪环线宽（px）">粗细
              <span class="wp-val">{waveParams.lineWidth.toFixed(1)}px</span>
              <input type="range" min="0.5" max="2.5" step="0.1" value={waveParams.lineWidth}
                     onchange={(e) => (waveParams.lineWidth = +(e.target as HTMLInputElement).value)} />
            </label>
            <label class="wp-row" title="环扩张过程中的透明度衰减速度：越大淡得越快">衰减
              <span class="wp-val">{waveParams.fade.toFixed(1)}</span>
              <input type="range" min="0.3" max="2.5" step="0.1" value={waveParams.fade}
                     onchange={(e) => (waveParams.fade = +(e.target as HTMLInputElement).value)} />
            </label>
          </div>
        {/if}
      </div>

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
        <div class="legend-row"><span class="legend-label small">单击节点 = 波纹传播 · 双击 = 编辑 · 再点波源 = 停止</span></div>
        <div class="legend-row"><span class="legend-label small">点击最亮 → 直接相关次之 → 逐级递减（只有相关节点受波震动）</span></div>
        <div class="legend-row"><span class="legend-label small">悬停 = 显示 id · 滚轮 = 缩放</span></div>
        {#if scanMode === 'analysis'}
          <div class="legend-row"><span class="legend-label small">连线渐变 = 源类型色 → 目标类型色</span></div>
        {/if}
        <div class="legend-row"><span class="legend-label small">拖动节点松手 = 自动重新布局</span></div>
        {#if scanMode === 'dev'}
          <div class="legend-sep"></div>
          <div class="legend-row"><span class="rel-sample rel-solid"></span><span class="legend-label small">实线 = 包含（从属）</span></div>
          <div class="legend-row"><span class="rel-sample rel-dashed"></span><span class="legend-label small">虚线 = 解决局限（递进主线）</span></div>
          <div class="legend-row"><span class="rel-sample rel-dotted"></span><span class="legend-label small">点线 = 备选替代</span></div>
          <div class="legend-sep"></div>
          <div class="legend-row"><span class="legend-label small">水面波场：单击节点 = 生成波源（持续向四周传播）</span></div>
          <div class="legend-row"><span class="legend-label small">再点同一节点 = 逐渐停止 · 点其它节点 = 次级波源（能量较弱）</span></div>
          <div class="legend-row"><span class="legend-label small">点击最亮 → 直接相关次之 → 逐级递减 · 双击 = 编辑</span></div>
        {/if}
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

  <StatusBar snapshot={snapshot} chainDir={chainDir} mode={scanMode} onrescan={loadChain} />
  </div>
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
  }
  .mode-chip.dev {
    color: #34d399;
    background: rgba(52, 211, 153, 0.1);
    border-color: rgba(52, 211, 153, 0.3);
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
    z-index: 1;
    background: transparent;   /* v2.4 两模式统一：透出水面画布 */
  }
  /* v2.2 水面画布（开发模式）：节点层之下，透明背景透出 */
  .water-canvas {
    position: absolute;
    inset: 0;
    z-index: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
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

  /* v2.3 波纹参数测试面板（开发模式，右上角） */
  .wave-params {
    position: absolute;
    top: 12px;
    right: 16px;
    z-index: 10;
    width: 218px;
    background: rgba(15, 15, 17, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 8px 12px;
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
    color: rgba(255, 255, 255, 0.5);
    text-align: right;
  }
  .wp-row input[type="range"] {
    width: 100%;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }
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
  .legend-sep {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 8px 0;
  }
</style>
