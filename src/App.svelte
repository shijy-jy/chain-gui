<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import cytoscape from 'cytoscape';
  import type { StylesheetJson, Core } from 'cytoscape';
  import { chainToElements, NODE_TYPE_LABEL, NODE_TYPE_COLOR } from './lib/chain_to_cytoscape';
  import { computeRippleLayers, rippleFreq, ripplePulseAmp, type RippleLayers } from './lib/ripple';
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
      if (m === 'dev') startWaterLoop(); else stopWaterLoop();   // v2.2 水面随模式
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

  // ── v2.2 涟漪视图（开发模式专属）──────────────────────────────────────
  // 设计：点击主节点 → 波前沿链接逐层扩散（350ms/层，上限 6 层）；
  //       同层同亮度、逐级指数衰减；层越近呼吸脉动越强、边脉冲越快；
  //       主节点持续发射 2-3 圈扩散涟漪环；再点/空白/Esc 快速收回。
  // 技术：BFS 分层（src/lib/ripple.ts 纯逻辑已无头测试）+ 类样式 +
  //       RAF（呼吸缩放 + line-dash-offset 流动 + overlay canvas 涟漪环）。
  let ripple = $state<{ source: string; activeDepth: number; layers: RippleLayers } | null>(null);
  let rippleTimer: ReturnType<typeof setInterval> | ReturnType<typeof setTimeout> | undefined;
  // v2.2 水面淡入/淡出时间戳（涟漪环与光晕平滑过渡，不突现突灭）
  let rippleStartAt = 0;
  let rippleFadeUntil = 0;
  // v2.3 浅水方程（SWE）水面场：粗网格高度场 + 垂直速度，物理传播/衰减/反射
  let sweH: Float32Array | null = null;
  let sweU: Float32Array | null = null;
  let sweW = 0;
  let sweHgt = 0;
  let sweCanvas: HTMLCanvasElement | null = null;
  let sweCtx: CanvasRenderingContext2D | null = null;
  let sweImage: ImageData | null = null;
  // 节点振动偏移的原始位置（波纹停止时恢复，避免布局漂移）
  let nodeOrigins = new Map<string, { x: number; y: number }>();
  let dragging = false;
  // v2.2 水面画布（开发模式）：背景水体 + 环境波纹 + 涟漪/震动在水面上的表达
  let waterCanvas: HTMLCanvasElement;
  let waterCtx: CanvasRenderingContext2D | null = null;
  let waterRaf: number | null = null;
  let waterFrame = 0;

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
        if (d !== undefined && d <= activeDepth) n.addClass(`rip-d${d}`);
        else n.addClass('rip-dim');   // 波外或波前未达：压暗等待
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
        // v2.3 恢复振动前的原始位置（防布局漂移）
        const origs = nodeOrigins;
        cyRef.nodes().forEach((n: any) => {
          const o = origs.get(n.id());
          if (o) n.position({ x: o.x, y: o.y });
        });
      });
    }
    nodeOrigins = new Map();
    ripple = null;
  }

  // ── v2.3 浅水方程水面（开发模式）───────────────────────────────────────
  // 真实物理波场：主节点 = 持续扰动源；波内节点 = Huygens 次波源（把接收到的波再辐射，
  // 强度随层深衰减 → 次级波纹自然涌现）；节点振动 = 所在位置水面高度（波到了自然颤，
  // 联系强弱由波场物理衰减决定，不再人工呼吸）；光晕由水面高度驱动。
  // 粗网格（约 12px/格）+ ImageData 渲染上采样（柔和水面感）。
  const SWE_G = 0.55;       // 重力
  const SWE_DT = 0.16;      // 子步时间步长
  const SWE_DAMP = 0.975;   // 每子步速度阻尼（波自然衰减）
  const SWE_VISC = 0.03;    // 数值粘性（拉普拉斯扩散，防爆炸）

  function ensureSwe(w: number, h: number) {
    const gw = Math.min(220, Math.max(80, Math.round(w / 12)));
    const gh = Math.min(130, Math.max(45, Math.round(h / 12)));
    if (sweW === gw && sweHgt === gh && sweH) return;
    sweW = gw;
    sweHgt = gh;
    sweH = new Float32Array(gw * gh);
    sweU = new Float32Array(gw * gh);
    for (let i = 0; i < sweH.length; i++) sweH[i] = (Math.random() - 0.5) * 0.02;
    if (!sweCanvas) {
      sweCanvas = document.createElement('canvas');
      sweCtx = sweCanvas.getContext('2d');
    }
    sweCanvas.width = gw;
    sweCanvas.height = gh;
    sweImage = sweCtx!.createImageData(gw, gh);
  }

  function sweStep() {
    const hh = sweH!;
    const u = sweU!;
    const ww = sweW;
    const ht = sweHgt;
    for (let y = 0; y < ht; y++) {
      for (let x = 0; x < ww; x++) {
        const i = y * ww + x;
        const xl = x > 0 ? x - 1 : x;
        const xr = x < ww - 1 ? x + 1 : x;
        const yu = y > 0 ? y - 1 : y;
        const yd = y < ht - 1 ? y + 1 : y;
        const lap = hh[yu * ww + x] + hh[yd * ww + x] + hh[y * ww + xl] + hh[y * ww + xr] - 4 * hh[i];
        u[i] = (u[i] + SWE_G * lap * SWE_DT) * SWE_DAMP;
        hh[i] += u[i] * SWE_DT + SWE_VISC * lap;
        hh[i] *= 0.9995;
        if (!Number.isFinite(hh[i])) hh[i] = 0;
        if (!Number.isFinite(u[i])) u[i] = 0;
      }
    }
  }

  function sweepos(x: number, y: number) {
    return {
      gx: Math.min(sweW - 2, Math.max(0, Math.floor(x / (waterCanvas.clientWidth / sweW)))),
      gy: Math.min(sweHgt - 2, Math.max(0, Math.floor(y / (waterCanvas.clientHeight / sweHgt)))),
    };
  }

  function sweAt(gx: number, gy: number) {
    return sweH![gy * sweW + gx];
  }

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
    }
    ensureSwe(w, h);

    // 淡入/淡出包络（平滑过渡）
    const nowMs = performance.now();
    const fadeIn = ripple ? Math.min(1, (nowMs - rippleStartAt) / 450) : 0;
    const fadeOut = nowMs < rippleFadeUntil ? Math.max(0, (rippleFadeUntil - nowMs) / 600) : 0;
    const envelope = Math.max(fadeIn, fadeOut);
    const rip = ripple;

    // ── 扰动源 ──
    const hh = sweH!;
    // 环境：缓慢移动的微风源（无涟漪时水面也轻微活着）
    {
      const gx = Math.floor((Math.sin(t * 0.13) * 0.5 + 0.5) * (sweW - 1));
      const gy = Math.floor((Math.cos(t * 0.1) * 0.5 + 0.5) * (sweHgt - 1));
      hh[gy * sweW + gx] += 0.012;
    }
    if (rip && cyRef) {
      // v2.3 全域波动场：点击主节点即释放——两条全域行波叠加，整个水面开始涌动，
      // 所有波内节点都浸在这个场里（联系强弱决定振动幅度，见下方节点振动段）
      const A = 0.055 * envelope;
      const kx = (Math.PI * 2) / sweW;
      const ky = (Math.PI * 2) / sweHgt;
      const w1 = Math.sin(t * Math.PI * 2 * 0.7);
      const w2 = Math.sin(t * Math.PI * 2 * 0.45 + 1.7);
      for (let y = 0; y < sweHgt; y++) {
        for (let x = 0; x < sweW; x++) {
          const i = y * sweW + x;
          hh[i] += (Math.sin(x * kx + y * ky * 0.6 - t * 1.2) * w1 * 0.6 + Math.sin(x * kx * 0.5 - y * ky * 0.8 + t * 0.9) * w2 * 0.4) * A * 0.5;
        }
      }
      // 主节点：持续扰动源（波列从源头向外物理扩散，叠加在全域场之上）
      const src = cyRef.getElementById(rip.source);
      if (!src.empty()) {
        const p = src.renderedPosition();
        const g = sweepos(p.x, p.y);
        hh[g.gy * sweW + g.gx] += Math.sin(t * Math.PI * 2 * 1.05) * 0.5 * envelope;
      }
      // 波内节点：Huygens 次波源——把接收到的波再辐射（次级波纹，强度随层深衰减）
      const tickMaxDepth = rip.layers.depth.size > 200 ? 3 : 6;
      cyRef.nodes().forEach((n: any) => {
        const d = rip.layers.depth.get(n.id());
        if (d === undefined || d === 0 || d > tickMaxDepth) return;
        const p = n.renderedPosition();
        const g = sweepos(p.x, p.y);
        const idx = g.gy * sweW + g.gx;
        const reEmit = ripplePulseAmp(d) * 0.9;   // 次级波纹强度 = 联系强度（层深）
        hh[idx] += hh[idx] * reEmit;
      });
    }

    // ── 物理步进（每帧 2 子步）──
    sweStep();
    sweStep();

    // ── 渲染高度场：亮度 = 高度 + 斜率；低清网格上采样出柔和水面 ──
    {
      const img = sweImage!;
      const data = img.data;
      for (let y = 0; y < sweHgt; y++) {
        for (let x = 0; x < sweW; x++) {
          const i = y * sweW + x;
          const v = hh[i];
          const xr = Math.min(sweW - 1, x + 1);
          const yd = Math.min(sweHgt - 1, y + 1);
          const slope = Math.abs(hh[i] - hh[y * sweW + xr]) + Math.abs(hh[i] - hh[yd * sweW + x]);
          let lum = v * 2.4 + slope * 2.0;
          if (lum > 1.2) lum = 1.2;
          if (lum < -0.5) lum = -0.5;
          const j = i * 4;
          const baseR = 9 + y * 0.006, baseG = 20 + y * 0.008, baseB = 33 + y * 0.01;
          const hiR = 96, hiG = 165, hiB = 250;
          if (lum >= 0) {
            data[j] = baseR + (hiR - baseR) * Math.min(1, lum);
            data[j + 1] = baseG + (hiG - baseG) * Math.min(1, lum);
            data[j + 2] = baseB + (hiB - baseB) * Math.min(1, lum);
          } else {
            const k = Math.min(1, -lum);
            data[j] = baseR * (1 - k * 0.7);
            data[j + 1] = baseG * (1 - k * 0.7);
            data[j + 2] = baseB * (1 - k * 0.7);
          }
          data[j + 3] = 255;
        }
      }
      sweCtx!.putImageData(img, 0, 0);
      ctx.imageSmoothingEnabled = true;
      ctx.drawImage(sweCanvas!, 0, 0, w, h);
    }

    if (!cyRef || !rip || envelope <= 0.01) return;

    // ── 节点振动 = 全域场强度 × 联系强度（层深）：场影响所有节点，强弱表达联系 ──
    const vibMaxDepth = rip.layers.depth.size > 200 ? 3 : 6;
    cyRef.batch(() => {
      cyRef.nodes().forEach((n: any) => {
        const d = rip.layers.depth.get(n.id());
        if (d === undefined || d > vibMaxDepth) return;
        const p = n.renderedPosition();
        const g = sweepos(p.x, p.y);
        const hLocal = sweAt(g.gx, g.gy);
        const strength = ripplePulseAmp(d) / 0.1;   // 1.0（直接相关）→ 0.15（最远层）
        const orig = nodeOrigins.get(n.id());
        if (orig && !dragging) {
          const px = Math.max(-2.4, Math.min(2.4, hLocal * 30));
          n.position({ x: orig.x + px * strength * 0.6, y: orig.y + px * strength * 0.8 });
        }
        const haloA = Math.min(0.55, Math.abs(hLocal) * 2.4) * envelope * Math.max(0.25, strength * 0.5);
        if (haloA <= 0.012) return;
        const haloR = nodeSize(n) * 1.2 + Math.abs(hLocal) * 60 + 8;
        const color = NODE_TYPE_COLOR[n.data('nodeType') as NodeType] ?? '#94a3b8';
        const r = parseInt(color.slice(1, 3), 16);
        const gc = parseInt(color.slice(3, 5), 16);
        const b = parseInt(color.slice(5, 7), 16);
        const grad = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, haloR);
        grad.addColorStop(0, `rgba(${r}, ${gc}, ${b}, ${haloA.toFixed(3)})`);
        grad.addColorStop(1, `rgba(${r}, ${gc}, ${b}, 0)`);
        ctx.beginPath();
        ctx.arc(p.x, p.y, haloR, 0, Math.PI * 2);
        ctx.fillStyle = grad;
        ctx.fill();
      });
    });
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

  function startRipple(nodeId: string) {
    if (!cy || !snapshot) return;
    const cyRef = cy;
    // 再点同一主节点 → 快速收回（0.6s，逐层熄灭）
    if (ripple && ripple.source === nodeId) {
      dismissRipple(true);
      return;
    }
    clearRipple();
    const layers = computeRippleLayers(buildAdjacency(snapshot), nodeId);
    ripple = { source: nodeId, activeDepth: 0, layers };
    rippleStartAt = performance.now();
    // v2.3 记录原始位置（节点振动由水面高度驱动，停止时恢复防布局漂移）
    nodeOrigins = new Map(cyRef.nodes().map((n: any) => [n.id(), { x: n.position('x'), y: n.position('y') }] as const));
    applyRippleClasses(cyRef, 0);
    // 波前逐层扩散
    rippleTimer = setInterval(() => {
      if (!ripple) return;
      ripple.activeDepth += 1;
      if (ripple.activeDepth >= ripple.layers.byDepth.length) {
        clearInterval(rippleTimer as any);
        rippleTimer = undefined;
        return;
      }
      applyRippleClasses(cyRef, ripple.activeDepth);
    }, 350);
  }

  function dismissRipple(_animated: boolean) {
    // v2.2 平滑过渡停止：立即移除层级类——节点/连线自带的 opacity transition（0.2-0.25s）
    // 会把亮度平滑带回初始状态；水面波场按 rippleFadeUntil 在 0.6s 内淡出（扰动源衰减）。
    if (!cy) return;
    rippleFadeUntil = performance.now() + 600;
    clearRipple();
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

  // v2.0 开发模式：新建节点（id 留空 = 后端自动生成；类型/状态一律中性 note/none）
  async function handleCreateNode(input: { id: string; title: string; parent: string | null }) {
    if (!chainDir) return;
    const newSnapshot = await invoke<ChainSnapshot>('create_node', {
      dir: chainDir,
      input: {
        id: input.id || null,
        title: input.title,
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
      // v2.2 开发模式：连线渲染为"若有若无"的淡线（.ghost），点击后整组淡出改由涟漪表达；
      // 水面循环随模式启停
      cyRef.add(chainToElements(snap, { withEdges: true }));
      if (scanMode === 'dev') {
        cyRef.edges().addClass('ghost');
        startWaterLoop();
      } else {
        stopWaterLoop();
      }
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
        if (scanMode === 'dev') {
          // v2.2 涟漪视图（开发模式）：点击主节点 = 水波纹传播；编辑改双击
          startRipple(n.id());
          return;
        }
        stopForce();   // v1.5 点击聚焦时暂停力模拟，避免与镜头动画抢位置
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
      // v2.2 开发模式：双击节点 = 打开编辑侧栏（单击已被涟漪交互占用）
      cy.on('dbltap', 'node', (evt) => {
        if (scanMode !== 'dev') return;
        const n = evt.target;
        stopForce();
        const nodeData = snapshot?.nodes.find(x => x.id === n.id());
        if (nodeData) selectedNode = nodeData;
      });
      cy.on('tap', (evt) => {
        if (evt.target === cy) {
          selectedNode = null;  // 点空白处关侧栏
          clearFocus();
          if (ripple) dismissRipple(true);   // v2.2 空白处涟漪快速收回
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
        if (ripple) dismissRipple(true);   // v2.2 Esc 涟漪快速收回
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

  <div class="canvas-wrap" class:dev-water={scanMode === 'dev'}>
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
        {#if scanMode === 'analysis'}
          <div class="legend-row"><span class="legend-label small">连线渐变 = 源类型色 → 目标类型色</span></div>
        {/if}
        <div class="legend-row"><span class="legend-label small">拖动节点松手 = 自动重新布局</span></div>
        {#if scanMode === 'dev'}
          <div class="legend-sep"></div>
          <div class="legend-row"><span class="legend-label small">水面涟漪：单击节点 = 波纹传播</span></div>
          <div class="legend-row"><span class="legend-label small">点击最亮 → 直接相关次之 → 逐级递减</span></div>
          <div class="legend-row"><span class="legend-label small">双击节点 = 编辑 · 空白/Esc = 波纹收回</span></div>
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
    background: #0a0a0a;
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
  .dev-water .cy-container {
    background: transparent;
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
