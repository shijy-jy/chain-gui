<script lang="ts">
  // v2.19 阅读模式（人专用 · AI 不可见）——把「图结构」切到「节点文件树」的阅读视图。
  //
  // 用户痛点：工作区节点一多，用图谱读内容不方便（图上只能看到标题，正文要逐个点侧栏）。
  // 本视图把节点按链协议的父子关系梳理成一棵**文件树**（左），选中即全文阅读（右）：
  // 折叠展开、关键字定位（标题/id/标签/正文）、阅读顺序「上一篇/下一篇」、渲染/原文切换、
  // 字号、面包屑、子节点与证据跳转、代码骨架全屏页入口。
  //
  // ⚠️ 人类专属不变量（刻意设计，改动前先读）：
  //  1. 只读派生：数据来自 `snapshot`（事实源的投影），本组件不写任何文件、不写工作区、不新增后端命令；
  //  2. 不进 MCP：没有对应的 MCP 工具 / CLI 子命令，AI 无法调用；
  //  3. 不进 AI 指南：`resources/AI_GUIDE*.md`（AI 的协议副本）不含本模式的任何描述；
  //  4. 不进 `window.__engramDebug`：CDP/自动化接缝里不暴露本模式，脚本无法识别或驱动；
  //  5. 状态只落 localStorage（WebView 本地），工作区里不留痕——AI 读 .chain/ 看不到任何迹象。
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import { renderBody } from './body_render';
  import { NODE_TYPE_LABEL, NODE_TYPE_COLOR } from './chain_to_cytoscape';
  import {
    buildNodeTree,
    flattenVisible,
    readingOrder,
    defaultExpanded,
    breadcrumbOf,
    searchNodes,
    isTaskOpenLoop,
    parentIdSet,
  } from './node_tree';
  import type { ChainNode, ChainSnapshot, NodeStatus, NodeType, ScanMode } from './types';

  let { snapshot, chainDir, mode, initialNodeId, onExit, onSelect, onOpenCode, onLocate, onSave, onCreate, onDelete, onSetParent }: {
    snapshot: ChainSnapshot;
    chainDir: string | null;
    mode: ScanMode;
    initialNodeId: string | null;
    onExit: () => void;
    onSelect: (node: ChainNode | null) => void;
    onOpenCode: (node: ChainNode) => void;
    onLocate: (nodeId: string) => void;
    /** v2.20 编辑面：保存内容字段（标题/状态/标签/正文） */
    onSave: (nodeId: string, fields: { title: string; status: NodeStatus | null; body: string; tags: string[]; evidence: string[] }) => Promise<void>;
    /** v2.20 新建：返回新节点 id（父节点挂载已在后端完成，图谱/文件树随之出现） */
    onCreate: (input: {
      id: string;
      title: string;
      nodeType: NodeType;
      status: NodeStatus | null;
      parent: string | null;
      rel: string;
      tags: string[];
      body: string;
    }) => Promise<string | null>;
    onDelete: (nodeId: string) => Promise<void>;
    onSetParent: (nodeId: string, parent: string | null, rel: string) => Promise<void>;
  } = $props();

  // ── 本地偏好（只落 localStorage：GUI 私有，工作区文件里无痕）────────────────
  const LS = {
    render: 'engram-read-render',     // preview | raw
    font: 'engram-read-font',         // 0 | 1 | 2
    treew: 'engram-read-treew',       // px
    arch: 'engram-read-archived',     // 0 | 1
    last: 'engram-read-last',         // 上次读到哪个节点
  };
  const FONT_STEPS = [14, 16, 19];

  let includeArchived = $state(localStorage.getItem(LS.arch) === '1');
  let rawMode = $state(localStorage.getItem(LS.render) === 'raw');
  let fontStep = $state(Math.min(2, Math.max(0, Number(localStorage.getItem(LS.font) ?? '1'))));
  let treeW = $state(Math.min(620, Math.max(200, Number(localStorage.getItem(LS.treew) ?? '320'))));
  let treeOpen = $state(true);
  let selId = $state<string | null>(null);

  const fs = $derived(FONT_STEPS[fontStep] ?? 16);

  // ── 数据：图结构 → 文件树 ────────────────────────────────────────────────
  const nodes = $derived(
    includeArchived ? [...snapshot.nodes, ...(snapshot.archived ?? [])] : snapshot.nodes,
  );
  const tree = $derived(buildNodeTree(nodes, { preferredRoot: snapshot.manifest.root }));
  const order = $derived(readingOrder(tree));
  const orderPos = $derived.by(() => new Map(order.map((n, i) => [n.id, i] as const)));
  const openLoopIds = $derived.by(() => {
    const parents = parentIdSet(nodes);
    const set = new Set<string>();
    for (const n of nodes) if (isTaskOpenLoop(n, parents)) set.add(n.id);
    return set;
  });

  let expanded = $state<Set<string>>(new Set());
  const rows = $derived(flattenVisible(tree, expanded));

  // 打开时：定位到「图上当前选中的节点」或上次读到的那篇（都没有 → 第一个根），
  // 并展开它所在的整条路径；其余保持折叠（大图首屏是总览而不是几千行）
  let bootstrapped = false;
  $effect(() => {
    if (bootstrapped) return;
    bootstrapped = true;
    const start = initialNodeId ?? localStorage.getItem(LS.last);
    selId = start;
    expanded = defaultExpanded(tree, start);
  });

  // 选中节点：树里找不到（被删/被归档）→ 回落到第一个根
  const current = $derived.by(() => {
    if (selId) {
      const e = tree.index.get(selId);
      if (e) return e;
    }
    return tree.roots[0] ?? null;
  });
  const currentId = $derived(current?.node.id ?? null);
  const pos = $derived(currentId ? (orderPos.get(currentId) ?? 0) : 0);
  const currentHtml = $derived(current ? renderBody(current.node.body ?? '') : '');
  const crumbs = $derived(current ? breadcrumbOf(tree, current.node.id) : []);

  let readEl = $state<HTMLElement | null>(null);
  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  let evErr = $state<string | null>(null);

  // 换节点 = 回到文首（阅读位置不跨篇残留）
  $effect(() => {
    const _id = currentId;
    if (readEl) readEl.scrollTop = 0;
  });

  function expandPathTo(id: string) {
    const anc = tree.ancestors.get(id) ?? [];
    if (anc.length === 0) return;
    const next = new Set(expanded);
    let changed = false;
    for (const a of anc) {
      if (!next.has(a.node.id)) {
        next.add(a.node.id);
        changed = true;
      }
    }
    if (changed) expanded = next;
  }

  function select(id: string, ensurePath = false) {
    // 编辑/新建中不许切走（草稿会丢）：先保存或取消
    if (surface !== 'read') {
      flash('正在编辑：先保存或取消（Esc 返回阅读）');
      return;
    }
    if (ensurePath) expandPathTo(id);
    selId = id;
    localStorage.setItem(LS.last, id);
    onSelect(tree.index.get(id)?.node ?? null);
    // 搜索命中 / 前后翻页时把该行滚进可视区（已在视口的行不动）
    requestAnimationFrame(() => {
      document.getElementById(`rt-${id}`)?.scrollIntoView({ block: 'nearest' });
    });
  }

  function toggle(id: string) {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expanded = next;
  }

  function expandAll() {
    expanded = new Set(nodes.map((n) => n.id));
  }
  function collapseAll() {
    expanded = new Set(tree.roots.map((r) => r.node.id));
  }

  // ── 阅读顺序导航（忽略折叠的 DFS 前序：整库当一本书读）────────────────────
  function step(n: number) {
    if (order.length === 0) return;
    const j = Math.min(order.length - 1, Math.max(0, pos + n));
    const target = order[j];
    if (target && target.id !== currentId) select(target.id, true);
  }

  // ── 树内上下移动（只走可见行，贴合"文件树"手感）─────────────────────────
  function stepRow(n: number) {
    if (rows.length === 0) return;
    const i = rows.findIndex((r) => r.entry.node.id === currentId);
    const j = Math.min(rows.length - 1, Math.max(0, (i < 0 ? 0 : i) + n));
    const r = rows[j];
    if (r) select(r.entry.node.id);
  }

  function arrowRight() {
    const e = current;
    if (!e || e.children.length === 0) return;
    if (!expanded.has(e.node.id)) {
      toggle(e.node.id);
      return;
    }
    select(e.children[0].node.id);
  }

  function arrowLeft() {
    const e = current;
    if (!e) return;
    if (e.children.length > 0 && expanded.has(e.node.id)) {
      toggle(e.node.id);
      return;
    }
    const anc = tree.ancestors.get(e.node.id) ?? [];
    const p = anc[anc.length - 1];
    if (p) select(p.node.id);
  }

  // ── 树内检索（标题/id/标签/正文；正文较长 → 输入去抖 160ms）─────────────
  let query = $state('');
  let queryDeb = $state('');
  let queryTimer: ReturnType<typeof setTimeout> | undefined;
  function onQueryInput() {
    clearTimeout(queryTimer);
    queryTimer = setTimeout(() => (queryDeb = query), 160);
  }
  const matches = $derived(queryDeb.trim() ? searchNodes(nodes, queryDeb) : []);

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      const first = matches[0];
      if (first) select(first.node.id, true);
    } else if (e.key === 'Escape') {
      e.stopPropagation();
      if (query) {
        query = '';
        queryDeb = '';
      }
    }
  }

  // ── 阅读显示选项 ─────────────────────────────────────────────────────────
  function setRaw(v: boolean) {
    rawMode = v;
    localStorage.setItem(LS.render, v ? 'raw' : 'preview');
  }
  function cycleFont() {
    fontStep = (fontStep + 1) % FONT_STEPS.length;
    localStorage.setItem(LS.font, String(fontStep));
  }
  function toggleArchived() {
    localStorage.setItem(LS.arch, includeArchived ? '1' : '0');
  }
  async function copyBody() {
    if (!current) return;
    try {
      await navigator.clipboard.writeText(current.node.body ?? '');
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1800);
    } catch (e) {
      evErr = `复制失败：${String(e)}`;
    }
  }
  async function openEvidence(rel: string) {
    if (!chainDir) return;
    evErr = null;
    try {
      await invoke('open_evidence', { dir: chainDir, rel });
    } catch (e) {
      evErr = String(e);
    }
  }

  // ── 文件树宽度拖拽（与信息栏同款 pointer 拖拽）───────────────────────────
  function startResize(e: PointerEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = treeW;
    const move = (ev: PointerEvent) => {
      treeW = Math.min(620, Math.max(200, startW + ev.clientX - startX));
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      localStorage.setItem(LS.treew, String(treeW));
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  // ── v2.20 编辑面：文件树模式 = 人的编辑面（阅读 / 编辑 / 新建三态）─────────
  // 写路径全部走 App 注入的回调 → 后端 core 守门（分析模式结构编辑走 *_human 护栏：
  // 新建必挂父节点、禁止删根/删带子节点的节点、改链接禁止成环）。图谱与文件树是同一份
  // snapshot 的两种渲染，任何一次写入两边同时更新。
  type Surface = 'read' | 'edit' | 'new';
  let surface = $state<Surface>('read');
  let busy = $state(false);
  let formError = $state<string | null>(null);
  let formMsg = $state<string | null>(null);
  let msgTimer: ReturnType<typeof setTimeout> | undefined;
  let delArmed = $state(false);

  // 编辑草稿
  let fTitle = $state('');
  let fStatus = $state<NodeStatus>('pending');
  let fTags = $state('');
  let fBody = $state('');
  let fParent = $state<string | null>(null);
  let fRel = $state<string>('contains');
  // 新建草稿
  let nTitle = $state('');
  let nId = $state('');
  let nType = $state<NodeType>('task');
  let nStatus = $state<NodeStatus>('pending');
  let nTags = $state('');
  let nBody = $state('');
  let nParent = $state<string | null>(null);
  let nRel = $state<string>('contains');
  let parentQuery = $state('');
  let parentOpen = $state(false);

  const ANALYSIS_TYPES: NodeType[] = ['goal', 'design', 'task', 'verification'];
  const typeOptions: NodeType[] = $derived(mode === 'dev' ? ['note'] : ANALYSIS_TYPES);
  const statusOptions: NodeStatus[] = $derived(
    mode === 'dev' ? ['none'] : ['pending', 'in_progress', 'success', 'failed', 'blocked'],
  );

  const relOptions = [
    { v: 'contains', t: '包含（从属）' },
    { v: 'solves', t: '解决局限（递进主线）' },
    { v: 'alternative', t: '备选替代' },
  ];

  function flash(msg: string) {
    formMsg = msg;
    clearTimeout(msgTimer);
    msgTimer = setTimeout(() => (formMsg = null), 2200);
  }

  /** 分析模式 id 约定：g-/d-/t-/v- + 三位序号（取现有同前缀最大号 +1） */
  function suggestId(type: NodeType): string {
    const prefix = type === 'goal' ? 'g' : type === 'design' ? 'd' : type === 'task' ? 't' : 'v';
    const re = new RegExp(`^${prefix}-(\\d+)$`);
    let max = 0;
    for (const n of nodes) {
      const m = re.exec(n.id);
      if (m) max = Math.max(max, Number(m[1]));
    }
    return `${prefix}-${String(max + 1).padStart(3, '0')}`;
  }

  function parentTitle(pid: string): string {
    return tree.index.get(pid)?.node.title ?? nodes.find((n) => n.id === pid)?.title ?? pid;
  }

  const parentOptions = $derived(    parentQuery.trim() === ''
      ? []
      : nodes
          .filter(
            (n) =>
              n.id !== currentId &&
              (n.title.toLowerCase().includes(parentQuery.trim().toLowerCase()) ||
                n.id.toLowerCase().includes(parentQuery.trim().toLowerCase())),
          )
          .slice(0, 20),
  );

  function beginEdit() {
    if (surface === 'new') {
      flash('正在新建：先创建或取消（Esc 返回阅读）');
      return;
    }
    const n = current?.node;
    if (!n || busy) return;
    fTitle = n.title;
    fStatus = n.status;
    fTags = n.tags.join(', ');
    fBody = n.body ?? '';
    fParent = n.parent;
    fRel = n.rel ?? 'contains';
    parentQuery = '';
    parentOpen = false;
    delArmed = false;
    formError = null;
    surface = 'edit';
  }

  function beginNew(parentId: string | null) {
    if (surface === 'edit') {
      flash('正在编辑：先保存或取消（Esc 返回阅读）');
      return;
    }
    formError = null;
    delArmed = false;
    nTitle = '';
    nTags = '';
    nBody = '';
    nRel = 'contains';
    nType = mode === 'dev' ? 'note' : 'task';
    nStatus = mode === 'dev' ? 'none' : 'pending';
    nId = mode === 'dev' ? '' : suggestId(nType);
    // 默认挂到当前选中的节点下（"新建的文件挂在它下面"）；没有选中则开发模式可作独立节点
    nParent = mode === 'dev' ? (parentId ?? null) : (parentId ?? currentId);
    parentQuery = '';
    parentOpen = false;
    surface = 'new';
  }

  function onNewTypeChange(t: NodeType) {
    const prev = mode === 'dev' ? '' : suggestId(nType);
    nType = t;
    if (mode !== 'dev' && (nId.trim() === '' || nId === prev)) nId = suggestId(t);
  }

  function cancelForm() {
    surface = 'read';
    formError = null;
    delArmed = false;
  }

  function splitTags(s: string): string[] {
    return s
      .split(/[,，]/)
      .map((x) => x.trim())
      .filter(Boolean);
  }

  async function saveEdit() {
    const n = current?.node;
    if (!n || busy) return;
    if (mode !== 'dev' && !fBody.trim()) {
      formError = '分析模式正文不能为空（链协议：节点必须写清做了什么/结论）';
      return;
    }
    busy = true;
    formError = null;
    try {
      // ① 结构变更先走 set_parent（core 人用护栏：分析模式禁断根、禁成环）
      const parentChanged =
        (fParent ?? null) !== (n.parent ?? null) || (fRel || 'contains') !== (n.rel ?? 'contains');
      if (parentChanged) await onSetParent(n.id, fParent, fRel);
      // ② 内容字段
      await onSave(n.id, {
        title: fTitle.trim() || n.id,
        status: mode === 'dev' ? null : fStatus,
        body: fBody,
        tags: splitTags(fTags),
        evidence: [...n.evidence],
      });
      surface = 'read';
      flash('已保存 ✓');
    } catch (e) {
      formError = String(e);
    } finally {
      busy = false;
    }
  }

  async function submitNew() {
    if (busy) return;
    if (!nTitle.trim()) {
      formError = '先写个标题（文件树/图谱都靠它认节点）';
      return;
    }
    if (mode !== 'dev' && !nParent) {
      formError = '分析模式必须选父节点（链协议要求单根树，不允许新增根）';
      return;
    }
    busy = true;
    formError = null;
    try {
      const newId = await onCreate({
        id: nId.trim(),
        title: nTitle.trim(),
        nodeType: nType,
        status: mode === 'dev' ? null : nStatus,
        parent: nParent,
        rel: nRel,
        tags: splitTags(nTags),
        body: nBody,
      });
      surface = 'read';
      if (newId) {
        await tick();            // 等新 snapshot 变成 props（树里已有新节点）
        expandPathTo(newId);
        select(newId);
        treeOpen = true;
      }
      flash('已创建 ✓（文件树与图谱同一个链）');
    } catch (e) {
      formError = String(e);
    } finally {
      busy = false;
    }
  }

  async function doDelete() {
    const n = current?.node;
    if (!n || busy) return;
    if (!delArmed) {
      delArmed = true;
      flash('再点一次确认删除');
      return;
    }
    busy = true;
    formError = null;
    const parentId = n.parent ?? null;
    try {
      await onDelete(n.id);
      delArmed = false;
      surface = 'read';
      if (parentId) {
        await tick();
        select(parentId);
      }
      flash('已删除 ✓');
    } catch (e) {
      formError = String(e);
      delArmed = false;
    } finally {
      busy = false;
    }
  }

  const statusLabels: Record<NodeStatus, string> = {
    pending: '待开始',
    in_progress: '进行中',
    success: '已完成',
    failed: '失败',
    blocked: '阻塞',
    none: '无状态',
  };
  function statusGlyph(s: NodeStatus): string {
    switch (s) {
      case 'success': return '✓';
      case 'in_progress': return '◐';
      case 'failed': return '✕';
      case 'blocked': return '⛔';
      case 'pending': return '○';
      default: return '';
    }
  }
  const kindLabel: Record<string, string> = { title: '标题', id: 'id', tag: '标签', body: '正文' };

  // 树行回车 = 打开该节点（行是 role="button" 的 div：里面还嵌着折叠按钮，不能嵌 <button>）
  function rowKeydown(e: KeyboardEvent, id: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      select(id, true);
    }
  }

  const shortDir = $derived(
    chainDir && chainDir.length > 42 ? '…' + chainDir.slice(-41) : (chainDir ?? ''),
  );

  // ── 键盘（App 的全局 Esc 在本模式下让位给这里：先清筛选，再退出）──────────
  function onKeydown(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    const typing = !!t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
    // Ctrl/Cmd+S：编辑/新建面上直接保存（人用编辑面的手感）
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's' && !e.altKey) {
      if (surface === 'edit') {
        e.preventDefault();
        saveEdit();
        return;
      }
      if (surface === 'new') {
        e.preventDefault();
        submitNew();
        return;
      }
    }
    if (e.key === 'Escape') {
      if (typing) {
        // 输入框内 Esc：先退出输入（筛选词不清空，便于继续看结果）
        (t as HTMLElement).blur();
        e.stopPropagation();
        return;
      }
      // v2.20 编辑/新建中：Esc = 取消表单回到阅读（不退出文件树模式）
      if (surface !== 'read') {
        cancelForm();
        return;
      }
      onExit();
      return;
    }
    if (typing) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      stepRow(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      stepRow(-1);
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      arrowRight();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      arrowLeft();
    } else if (e.key === 'PageDown') {
      e.preventDefault();
      step(1);
    } else if (e.key === 'PageUp') {
      e.preventDefault();
      step(-1);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="rm-mask">
  <!-- 顶栏：工作区上下文 + 视图返回 + 人类专属标记 -->
  <header class="rm-bar">
    <span class="rm-badge">🗂 文件树模式</span>
    <span class="rm-human" title="人类专属视图：不写工作区任何文件（只经后端 core 守门写节点）、不注册 MCP 工具、不进 AI 指南副本、不进调试接缝——AI 既无法识别也无法使用本视图；本地偏好只落 GUI 本地存储">人专用 · AI 不可见</span>
    {#if chainDir}
      <span class="rm-ws" title={chainDir}>{shortDir}</span>
    {/if}
    <span class="rm-mode" class:dev={mode === 'dev'}>{mode === 'dev' ? '开发' : '分析'}</span>
    <span class="rm-count">{nodes.length} 节点 · {tree.stats.roots} 根{tree.stats.broken > 0 ? ` · ${tree.stats.broken} 悬空` : ''}{tree.stats.cycles > 0 ? ` · ${tree.stats.cycles} 环` : ''}</span>
    <span class="spacer"></span>
    <button class="rm-btn" onclick={() => (treeOpen = !treeOpen)}
            title={treeOpen ? '隐藏文件树（阅读区占满整宽）' : '显示文件树'}>
      {treeOpen ? '◧ 隐藏文件树' : '◨ 显示文件树'}
    </button>
    <button class="rm-btn rm-back" onclick={onExit} title="返回图谱视图（Esc）">← 返回图谱</button>
  </header>

  <div class="rm-body">
    {#if treeOpen}
      <!-- 左：节点文件树（按图结构梳理；一个 .md 文件 = 一行） -->
      <aside class="rm-tree" style:width="{treeW}px">
        <div class="rt-title">
          节点文件树 <span class="rt-sub">按图结构 · 一个节点 = 一个 .md</span>
        </div>
        <div class="rt-head">
          <span class="rt-icon">⌕</span>
          <input class="rt-search" bind:value={query} oninput={onQueryInput} onkeydown={onSearchKey}
                 placeholder="搜索标题 / id / 标签 / 正文…" />
          {#if query}
            <button class="rt-clear" onclick={() => { query = ''; queryDeb = ''; }} title="清空（Esc）">✕</button>
          {/if}
        </div>
        <div class="rt-tools">
          <button class="rt-tool primary" onclick={() => beginNew(currentId)}
                  title={mode === 'dev' ? '新建节点：默认挂在当前节点下（可改父节点或作独立节点）' : '新建节点：必须挂在父节点下（分析模式单根树），创建后同时出现在文件树与图谱里'}>
            ＋ 新建
          </button>
          <button class="rt-tool" onclick={expandAll} title="展开全部节点">展开</button>
          <button class="rt-tool" onclick={collapseAll} title="收起到根层">折叠</button>
          <label class="rt-arch" title="把已归档节点也纳入文件树（默认关，与图谱一致）">
            <input type="checkbox" bind:checked={includeArchived} onchange={toggleArchived} /> 含归档
          </label>
        </div>

        <div class="rt-list">
          {#if queryDeb.trim()}
            {#each matches as m (m.node.id)}
              <div class="trow match" class:sel={m.node.id === currentId} role="button" tabindex="-1"
                   onclick={() => select(m.node.id, true)} onkeydown={(e) => rowKeydown(e, m.node.id)}
                   title={`${m.node.title} · ${m.node.id}`}>
                <div class="trow-line">
                  <span class="tdot" style:background={NODE_TYPE_COLOR[m.node.type]}></span>
                  <span class="ttitle">{m.node.title}</span>
                  <span class="tid">{m.node.id}</span>
                  <span class="tkind">{kindLabel[m.kind]}</span>
                </div>
                {#if m.snippet}<div class="tsnip">{m.snippet}</div>{/if}
              </div>
            {/each}
            {#if matches.length === 0}
              <div class="rt-empty">无匹配节点</div>
            {/if}
          {:else}
            {#each rows as r (r.entry.node.id)}
              {@const n = r.entry.node}
              <div class="trow" class:sel={n.id === currentId} class:arch={n.archived}
                   id="rt-{n.id}" style:padding-left="{4 + r.depth * 13}px" role="button" tabindex="-1"
                   onclick={() => select(n.id)} onkeydown={(e) => rowKeydown(e, n.id)}
                   title={`${n.title} · ${n.id}${r.entry.brokenParent ? `（父节点缺失：${r.entry.brokenParent}）` : ''}${r.entry.cycleBreak ? '（环路：已提升为根）' : ''}`}>
                {#if r.entry.children.length > 0}
                  <button class="tw" onclick={(e) => { e.stopPropagation(); toggle(n.id); }}
                          title={expanded.has(n.id) ? '折叠子节点' : `展开 ${r.entry.children.length} 个子节点`}>
                    {expanded.has(n.id) ? '▾' : '▸'}
                  </button>
                {:else}
                  <span class="tw tw-leaf"></span>
                {/if}
                <span class="tdot" style:background={NODE_TYPE_COLOR[n.type]}></span>
                <span class="ttitle">{n.title}</span>
                {#if r.entry.brokenParent}<span class="tmark broken" title={`父节点缺失：${r.entry.brokenParent}`}>悬</span>{/if}
                {#if r.entry.cycleBreak}<span class="tmark cyc" title="环路：已提升为根（只读展示，不改数据）">环</span>{/if}
                {#if n.archived}<span class="tmark arch" title={n.archived_reason ? `已归档：${n.archived_reason}` : '已归档'}>归</span>{/if}
                {#if n.frozen}<span class="tmark frozen" title={n.freeze_reason ?? '写冲突待裁决（冻结）'}>裁</span>{/if}
                {#if n.derived}<span class="tmark derived" title="蒸馏产物（检索默认降权）">蒸</span>{/if}
                {#if n.code_map}<span class="tmark code" title={`已挂载代码骨架：${n.code_map}`}>{'</>'}</span>{/if}
                {#if openLoopIds.has(n.id)}<span class="tmark loop" title="任务未闭环：无验证子节点且正文无「自验收」注明">开</span>{/if}
                {#if statusGlyph(n.status)}<span class="tstat {n.status}" title={statusLabels[n.status]}>{statusGlyph(n.status)}</span>{/if}
                <span class="tid">{n.id}</span>
              </div>
            {/each}
            {#if rows.length === 0}
              <div class="rt-empty">该工作区没有节点</div>
            {/if}
          {/if}
        </div>
      </aside>
      <div class="rm-vsplit" role="separator" aria-orientation="vertical" onpointerdown={startResize}
           title="拖拽调整文件树宽度"></div>
    {/if}

    <!-- 右：阅读 / 编辑 / 新建（同一个节点文件的三种人用操作面） -->
    <section class="rm-read" bind:this={readEl}>
      <div class="rd-inner">
        {#if !current && surface === 'read'}
          <div class="rd-empty">该工作区没有节点——点左上「＋ 新建」建第一个文件</div>
        {:else}
          {#if surface === 'new'}
            <div class="rd-newhead">
              <span class="chip type">＋ 新建节点</span>
              <span class="rd-hint">
                {mode === 'dev'
                  ? '开发模式：默认挂在当前节点下，也可在下面改成别的父节点或独立节点；类型/状态取中性默认'
                  : '分析模式：必须挂在父节点下（链保持单根树，根 goal 由初始化创建）；类型/状态限协议词表'}
              </span>
            </div>
          {:else}
            <!-- 面包屑：从根到当前节点（大树里"我在哪"一目了然） -->
            <nav class="rd-crumbs">
              {#each crumbs as c, i (c.id)}
                {#if i > 0}<span class="rd-sep">›</span>{/if}
                <button class="rd-crumb" class:cur={i === crumbs.length - 1} disabled={i === crumbs.length - 1}
                        onclick={() => select(c.id, true)} title={c.id}>{c.title}</button>
              {/each}
            </nav>
          {/if}

          <div class="rd-nav">
            {#if order.length > 0}
              <button class="rd-btn" onclick={() => step(-1)} disabled={pos <= 0 || surface !== 'read'}
                      title="上一个节点（阅读顺序）">‹ 上一篇</button>
              <span class="rd-pos" title="按文件树前序的阅读位置">{pos + 1} / {order.length}</span>
              <button class="rd-btn" onclick={() => step(1)} disabled={pos >= order.length - 1 || surface !== 'read'}
                      title="下一个节点（阅读顺序）">下一篇 ›</button>
            {/if}
            <span class="spacer"></span>
            {#if surface === 'read'}
              <button class="rd-btn" class:on={!rawMode} onclick={() => setRaw(false)} title="渲染 Markdown + LaTeX">渲染</button>
              <button class="rd-btn" class:on={rawMode} onclick={() => setRaw(true)} title="按文件正文原样显示（不做 Markdown 渲染）">原文</button>
              <button class="rd-btn" onclick={cycleFont} title="正文字号：小 / 中 / 大（点击循环）">A {fs}px</button>
              <button class="rd-btn" onclick={copyBody} title="复制节点正文到剪贴板">{copied ? '已复制 ✓' : '复制正文'}</button>
              {#if current.node.code_map}
                <button class="rd-btn code" onclick={() => onOpenCode(current.node)}
                        title="打开代码骨架全屏页（大字体 + Mermaid + 完整滚动）">{'⧉'} 代码骨架</button>
              {/if}
              <button class="rd-btn" onclick={() => onLocate(current.node.id)}
                      title="关闭文件树模式，在图谱里居中高亮这个节点（看图结构关系）">在图谱中定位</button>
              {#if current.node.archived}
                <span class="rd-msg" title={`已归档节点在 .chain/archive/ 下，不在活跃链里——编辑/删除请先在归档视图处理`}>已归档：只读</span>
              {:else}
                <button class="rd-btn primary" onclick={() => beginNew(currentId)}
                        title="在当前节点下新建子节点（同时出现在文件树与图谱）">＋ 新建</button>
                <button class="rd-btn" onclick={beginEdit}
                        title="编辑这个节点：标题 / 状态 / 标签 / 正文 / 父节点与关系（Ctrl+S 保存）">✎ 编辑</button>
                <button class="rd-btn danger" class:armed={delArmed} onclick={doDelete} disabled={busy}
                        title={mode === 'dev'
                          ? '删除这个节点（两段式确认）'
                          : '删除这个节点（两段式确认；分析模式：不能删根、不能删还有子节点的节点）'}>
                  {delArmed ? '确认删除？' : '🗑 删除'}
                </button>
              {/if}
            {:else}
              <button class="rd-btn primary" onclick={surface === 'new' ? submitNew : saveEdit} disabled={busy}
                      title="Ctrl+S">
                {busy ? '处理中…' : surface === 'new' ? '创建节点' : '保存'}
              </button>
              <button class="rd-btn" onclick={cancelForm} disabled={busy} title="取消（Esc）">取消</button>
            {/if}
            {#if formMsg}<span class="rd-msg">{formMsg}</span>{/if}
          </div>

          {#if formError}
            <div class="rd-formerr">⚠ {formError}</div>
          {/if}

          {#if surface === 'read'}
            <!-- 元信息：与事实源同字段（frontmatter）逐一对应 -->
            <div class="rd-meta">
              <span class="chip type" style:color={NODE_TYPE_COLOR[current.node.type]}
                    title="节点类型（frontmatter: type）">
                <span class="tdot" style:background={NODE_TYPE_COLOR[current.node.type]}></span>
                {NODE_TYPE_LABEL[current.node.type]}
              </span>
              {#if mode !== 'dev' || current.node.status !== 'none'}
                <span class="chip st-{current.node.status}" title="节点状态（frontmatter: status）">
                  {statusGlyph(current.node.status)} {statusLabels[current.node.status]}
                </span>
              {/if}
              <span class="chip id" title="节点 id（frontmatter: id）">{current.node.id}</span>
              <span class="chip" title="修订号（frontmatter: revision）">rev {current.node.revision}</span>
              <span class="chip" title="创建时间">建于 {current.node.created.slice(0, 16)}</span>
              <span class="chip" title="最后更新">更于 {current.node.updated.slice(0, 16)}</span>
              {#if current.node.archived}<span class="chip warn" title={current.node.archived_reason ?? '已归档'}>已归档</span>{/if}
              {#if current.node.frozen}<span class="chip warn" title={current.node.freeze_reason ?? '并发写冲突，待人工裁决'}>待裁决</span>{/if}
              {#if current.node.derived}<span class="chip warn" title="蒸馏产物（检索默认降权）">蒸馏</span>{/if}
              {#if openLoopIds.has(current.node.id)}
                <span class="chip warn" title="任务未闭环：无验证子节点且正文无「自验收」注明">未闭环</span>
              {/if}
              {#if current.node.folded}
                <span class="chip" title={`子链折叠摘要（原始 ${current.node.folded.original_node_count} 个节点）`}>
                  折叠摘要 · {current.node.folded.original_node_count}
                </span>
              {/if}
            </div>

            {#if current.node.tags.length > 0}
              <div class="rd-tags">
                {#each current.node.tags as t (t)}<span class="rd-tag">{t}</span>{/each}
              </div>
            {/if}

            <!-- 正文：渲染（Markdown + KaTeX）/ 原文（文件正文原样） -->
            <article class="rd-body" style:font-size="{fs}px">
              {#if rawMode}
                <pre class="rd-raw">{current.node.body && current.node.body.trim() ? current.node.body : '（正文为空）'}</pre>
              {:else if current.node.body && current.node.body.trim()}
                <div class="md">{@html currentHtml}</div>
              {:else}
                <div class="rd-none">（正文为空）</div>
              {/if}
            </article>

            <!-- 子节点：读完一篇顺着往下读（点击跳转并展开路径） -->
            {#if current.children.length > 0}
              <section class="rd-kids">
                <div class="rd-sec-title">子节点（{current.children.length}）</div>
                <div class="rd-kid-list">
                  {#each current.children as k (k.node.id)}
                    <button class="rd-kid" onclick={() => select(k.node.id, true)} title={k.node.id}>
                      <span class="tdot" style:background={NODE_TYPE_COLOR[k.node.type]}></span>
                      <span class="rd-kid-t">{k.node.title}</span>
                      <span class="tid">{k.node.id}</span>
                      {#if k.node.rel && k.node.rel !== 'contains'}
                        <span class="rd-rel" title="递进关系类型（frontmatter: rel）">
                          {k.node.rel === 'solves' ? '解决局限' : k.node.rel === 'alternative' ? '备选替代' : k.node.rel}
                        </span>
                      {/if}
                    </button>
                  {/each}
                </div>
              </section>
            {/if}

            <!-- 证据文件：文件名点击即用系统程序打开（与信息栏同一条后端命令） -->
            {#if current.node.evidence.length > 0}
              <section class="rd-kids">
                <div class="rd-sec-title">证据文件（{current.node.evidence.length}）</div>
                <div class="rd-kid-list">
                  {#each current.node.evidence as rel (rel)}
                    <button class="rd-kid" onclick={() => openEvidence(rel)} title={`打开：${rel}`}>
                      <span class="rd-ev-icon">📄</span>
                      <span class="rd-kid-t">{rel.split(/[\\/]/).pop()}</span>
                      <span class="tid">{rel}</span>
                    </button>
                  {/each}
                </div>
                {#if evErr}<div class="rd-err">⚠ {evErr}</div>{/if}
              </section>
            {/if}

            <!-- 篇末导航：长文读完不用回到顶部 -->
            <div class="rd-foot">
              <button class="rd-btn" onclick={() => step(-1)} disabled={pos <= 0}>‹ 上一篇</button>
              <button class="rd-btn" onclick={() => step(1)} disabled={pos >= order.length - 1}>下一篇 ›</button>
            </div>
          {:else if surface === 'edit'}
            <!-- 编辑面：人对同一个节点文件改内容（写回仍走 core 守门：原子写 + 乐观锁） -->
            <div class="rd-form">
              <label class="fld" for="f-title">
                <span>标题</span>
                <input id="f-title" type="text" bind:value={fTitle} disabled={busy} />
              </label>

              <div class="fld-row">
                {#if mode !== 'dev'}
                  <label class="fld" for="f-status">
                    <span>状态</span>
                    <select id="f-status" bind:value={fStatus} disabled={busy}>
                      {#each statusOptions as s (s)}<option value={s}>{statusLabels[s]}</option>{/each}
                    </select>
                  </label>
                {/if}
                <label class="fld grow" for="f-tags">
                  <span>标签（逗号分隔）</span>
                  <input id="f-tags" type="text" bind:value={fTags} disabled={busy} placeholder="如：渲染, 时序复用" />
                </label>
              </div>

              <div class="fld">
                <span>
                  父节点（挂载位置）· 当前：{current.node.parent ?? '无（根）'} · 类型 {NODE_TYPE_LABEL[current.node.type]}
                  {mode === 'dev' ? '' : '（分析模式必须挂在父节点下，不能断成根）'}
                </span>
                <div class="parent-row">
                  <div class="parent-search-wrap">
                    <input class="parent-search" type="text"
                           placeholder={fParent ? `${parentTitle(fParent)} · ${fParent}` : '搜索要挂到的父节点（标题/id）…'}
                           bind:value={parentQuery} onfocus={() => (parentOpen = true)}
                           onblur={() => setTimeout(() => (parentOpen = false), 150)} disabled={busy} />
                    {#if parentOpen && parentOptions.length > 0}
                      <div class="parent-results">
                        {#each parentOptions as n (n.id)}
                          <button class="parent-opt" onclick={() => { fParent = n.id; parentQuery = ''; parentOpen = false; }}>
                            {n.title} · {n.id}
                          </button>
                        {/each}
                      </div>
                    {/if}
                  </div>
                  <select class="rel-sel" bind:value={fRel} disabled={busy || !fParent}
                          title="与父节点的关系类型（frontmatter: rel）">
                    {#each relOptions as r (r.v)}<option value={r.v}>{r.t}</option>{/each}
                  </select>
                  {#if mode === 'dev'}
                    <button class="parent-clear" onclick={() => (fParent = null)} disabled={busy}
                            title="断开链接，变成独立节点（开发模式允许）">断开 ✕</button>
                  {/if}
                </div>
              </div>

              <label class="fld" for="f-body">
                <span>正文（Markdown + LaTeX；$…$ 行内公式，$$…$$ 独立公式）</span>
                <textarea id="f-body" class="fld-body" bind:value={fBody} disabled={busy} spellcheck="false"></textarea>
              </label>

              <div class="fld-foot">
                <span class="rd-hint">保存走 core 守门：原子写 + rev {current.node.revision}；并发冲突会被拒绝（绝不静默覆盖）</span>
              </div>
            </div>
          {:else}
            <!-- 新建面：写一个节点文件，挂到选定父节点下——图谱/文件树同一份链，两边同时长出这个节点 -->
            <div class="rd-form">
              <label class="fld" for="n-title">
                <span>标题（必填）</span>
                <input id="n-title" type="text" bind:value={nTitle} disabled={busy}
                       placeholder={mode === 'dev' ? '如：体积云散射笔记' : '如：实现 ReSTIR DI 的时空复用'} />
              </label>

              <div class="fld-row">
                {#if mode !== 'dev'}
                  <label class="fld" for="n-type">
                    <span>类型（协议词表）</span>
                    <select id="n-type" value={nType} onchange={(e) => onNewTypeChange((e.target as HTMLSelectElement).value as NodeType)} disabled={busy}>
                      {#each typeOptions as t (t)}<option value={t}>{NODE_TYPE_LABEL[t]}</option>{/each}
                    </select>
                  </label>
                  <label class="fld" for="n-status">
                    <span>状态</span>
                    <select id="n-status" bind:value={nStatus} disabled={busy}>
                      {#each statusOptions as s (s)}<option value={s}>{statusLabels[s]}</option>{/each}
                    </select>
                  </label>
                {/if}
                <label class="fld" for="n-id">
                  <span>id（文件名称；留空自动生成）</span>
                  <input id="n-id" type="text" bind:value={nId} disabled={busy}
                         placeholder={mode === 'dev' ? '留空 = node-N' : '如 t-016'} />
                </label>
              </div>

              <div class="fld">
                <span>
                  挂在哪个节点下{mode === 'dev' ? '' : '（分析模式必填）'}
                  {#if nParent}· 当前选择：{parentTitle(nParent)} · {nParent}{/if}
                </span>
                <div class="parent-row">
                  <div class="parent-search-wrap">
                    <input class="parent-search" type="text"
                           placeholder={nParent ? `${parentTitle(nParent)} · ${nParent}` : '搜索父节点（标题/id）…'}
                           bind:value={parentQuery} onfocus={() => (parentOpen = true)}
                           onblur={() => setTimeout(() => (parentOpen = false), 150)} disabled={busy} />
                    {#if parentOpen && parentOptions.length > 0}
                      <div class="parent-results">
                        {#each parentOptions as n (n.id)}
                          <button class="parent-opt" onclick={() => { nParent = n.id; parentQuery = ''; parentOpen = false; }}>
                            {n.title} · {n.id}
                          </button>
                        {/each}
                      </div>
                    {/if}
                  </div>
                  <select class="rel-sel" bind:value={nRel} disabled={busy || !nParent}
                          title="与父节点的关系类型（frontmatter: rel）">
                    {#each relOptions as r (r.v)}<option value={r.v}>{r.t}</option>{/each}
                  </select>
                  {#if mode === 'dev'}
                    <button class="parent-clear" onclick={() => (nParent = null)} disabled={busy}
                            title="不挂父节点 = 独立节点（开发模式允许）">独立节点 ✕</button>
                  {/if}
                </div>
              </div>

              <label class="fld" for="n-tags">
                <span>标签（逗号分隔，可留空）</span>
                <input id="n-tags" type="text" bind:value={nTags} disabled={busy} placeholder="如：渲染, 路径追踪" />
              </label>

              <label class="fld" for="n-body">
                <span>正文（可直接开写；Markdown + LaTeX）</span>
                <textarea id="n-body" class="fld-body" bind:value={nBody} disabled={busy} spellcheck="false"
                          placeholder={mode === 'dev' ? '# 标题\n\n内容…' : '做了什么 / 怎么做的 / 结论与剩余问题（分析模式正文不能为空）'}></textarea>
              </label>

              <div class="fld-foot">
                <span class="rd-hint">
                  创建后：文件树里出现在所选父节点下面，图谱里同一个父节点下同时长出这个节点（同一份 .chain/nodes 事实源）
                </span>
              </div>
            </div>
          {/if}
        {/if}
      </div>
    </section>
  </div>
</div>

<style>
  .rm-mask {
    position: fixed;
    inset: 0;
    z-index: 2600; /* 低于全屏代码页（3000）：阅读模式下仍可展开代码骨架 */
    display: flex;
    flex-direction: column;
    background: #0b0d12;
    color: var(--text-1);
    animation: rm-in 0.18s var(--ease-out);
  }
  @keyframes rm-in {
    from { opacity: 0; transform: scale(0.99); }
    to { opacity: 1; transform: scale(1); }
  }

  /* ── 顶栏 ─────────────────────────────────────────────────────────── */
  .rm-bar {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--line);
  }
  .rm-badge {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.5px;
    color: #a5d2ff;
  }
  .rm-human {
    font-size: 10px;
    color: rgba(167, 139, 250, 0.9);
    border: 1px solid rgba(167, 139, 250, 0.4);
    border-radius: 999px;
    padding: 1px 8px;
    cursor: help;
    white-space: nowrap;
  }
  .rm-ws {
    font-size: 11px;
    color: var(--text-2);
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: 'Consolas', monospace;
  }
  .rm-mode {
    font-size: 10px;
    color: #a78bfa;
    border: 1px solid rgba(167, 139, 250, 0.45);
    border-radius: 999px;
    padding: 1px 8px;
  }
  .rm-mode.dev {
    color: #34d399;
    border-color: rgba(52, 211, 153, 0.45);
  }
  .rm-count {
    font-size: 11px;
    color: var(--text-3);
  }
  .spacer { flex: 1 1 auto; }
  .rm-btn {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.82);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 8px;
    padding: 5px 11px;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s var(--ease-soft), border-color 0.15s var(--ease-soft);
  }
  .rm-btn:hover { background: rgba(255, 255, 255, 0.12); }
  .rm-back {
    color: #ffd9a0;
    border-color: rgba(251, 191, 36, 0.4);
  }
  .rm-back:hover { background: rgba(251, 191, 36, 0.18); }

  /* ── 主体：左树右读 ───────────────────────────────────────────────── */
  .rm-body {
    flex: 1 1 auto;
    display: flex;
    min-height: 0;
  }
  .rm-tree {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: rgba(255, 255, 255, 0.022);
    border-right: 1px solid var(--line);
  }
  .rm-vsplit {
    flex: 0 0 6px;
    cursor: col-resize;
    background: transparent;
    transition: background 0.15s var(--ease-soft);
  }
  .rm-vsplit:hover { background: rgba(167, 139, 250, 0.28); }

  /* 树搜索 */
  .rt-title {
    flex: 0 0 auto;
    padding: 9px 12px 2px;
    font-size: 11px;
    letter-spacing: 1px;
    color: var(--text-2);
  }
  .rt-sub {
    font-size: 9.5px;
    letter-spacing: 0;
    color: var(--text-3);
    margin-left: 4px;
  }
  .rt-head {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px 6px;
    border-bottom: 1px solid var(--line);
  }
  .rt-icon { color: var(--text-3); font-size: 12px; }
  .rt-search {
    flex: 1 1 auto;
    min-width: 0;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 7px;
    color: var(--text-1);
    font-size: 12px;
    padding: 5px 8px;
  }
  .rt-clear {
    background: transparent;
    border: none;
    color: var(--text-3);
    cursor: pointer;
    font-size: 11px;
  }
  .rt-clear:hover { color: #f87171; }
  .rt-tools {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
  }
  .rt-tool {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 11px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .rt-tool:hover { background: rgba(255, 255, 255, 0.12); }
  /* v2.20 人用编辑面：主按钮（新建/保存） */
  .rt-tool.primary {
    color: #a5d2ff;
    border-color: rgba(165, 210, 255, 0.45);
    background: rgba(165, 210, 255, 0.12);
  }
  .rt-tool.primary:hover { background: rgba(165, 210, 255, 0.22); }
  .rt-arch {
    margin-left: auto;
    font-size: 11px;
    color: var(--text-2);
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    user-select: none;
  }
  .rt-list {
    flex: 1 1 auto;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 4px 6px 16px;
  }
  .rt-empty {
    padding: 18px 10px;
    text-align: center;
    color: var(--text-3);
    font-size: 12px;
  }

  /* 树行 */
  .trow {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 6px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12.5px;
    line-height: 1.5;
  }
  .trow:hover { background: rgba(255, 255, 255, 0.06); }
  .trow.sel {
    background: rgba(167, 139, 250, 0.2);
    box-shadow: inset 2px 0 0 #a78bfa;
  }
  .trow.arch { opacity: 0.62; }
  .trow.match { flex-direction: column; align-items: stretch; }
  .trow-line {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .tw {
    flex: 0 0 14px;
    width: 14px;
    height: 15px;
    line-height: 13px;
    text-align: center;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.72);
    font-size: 11px;
    cursor: pointer;
    padding: 0;
  }
  .tw:hover { color: #fff; }
  .tw-leaf { cursor: default; }
  .tdot {
    flex: 0 0 7px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .ttitle {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tid {
    flex: 0 0 auto;
    font-family: 'Consolas', monospace;
    font-size: 10px;
    color: var(--text-3);
  }
  .tkind {
    flex: 0 0 auto;
    font-size: 9px;
    color: rgba(165, 210, 255, 0.85);
    border: 1px solid rgba(165, 210, 255, 0.3);
    border-radius: 4px;
    padding: 0 4px;
  }
  .tsnip {
    font-size: 10.5px;
    color: var(--text-3);
    padding-left: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tmark {
    flex: 0 0 auto;
    font-size: 9px;
    line-height: 13px;
    border-radius: 4px;
    padding: 0 3px;
    border: 1px solid transparent;
  }
  .tmark.arch { color: #cbd5e1; border-color: rgba(203, 213, 225, 0.35); }
  .tmark.frozen { color: #fca5a5; border-color: rgba(248, 113, 113, 0.4); }
  .tmark.derived { color: #c4b5fd; border-color: rgba(167, 139, 250, 0.4); }
  .tmark.code { color: #34d399; border-color: rgba(52, 211, 153, 0.45); font-family: 'Consolas', monospace; }
  .tmark.loop { color: #fbbf24; border-color: rgba(251, 191, 36, 0.45); }
  .tmark.broken { color: #fb923c; border-color: rgba(251, 146, 60, 0.45); }
  .tmark.cyc { color: #f472b6; border-color: rgba(244, 114, 182, 0.45); }
  .tstat { flex: 0 0 auto; font-size: 10px; }
  .tstat.success { color: #34d399; }
  .tstat.in_progress { color: #22d3ee; }
  .tstat.failed { color: #f87171; }
  .tstat.blocked { color: #fbbf24; }
  .tstat.pending { color: rgba(255, 255, 255, 0.3); }

  /* ── 阅读区 ───────────────────────────────────────────────────────── */
  .rm-read {
    flex: 1 1 auto;
    min-width: 0;
    overflow-y: auto;
    padding: 16px 30px 64px;
  }
  .rd-inner {
    max-width: 900px;
    margin: 0 auto;
  }
  .rd-empty, .rd-none {
    padding: 40px 0;
    text-align: center;
    color: var(--text-3);
    font-size: 13px;
  }
  .rd-crumbs {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 10px;
    font-size: 11.5px;
  }
  .rd-crumb {
    background: transparent;
    border: none;
    color: var(--text-2);
    cursor: pointer;
    padding: 1px 3px;
    font-size: 11.5px;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rd-crumb:hover:not(:disabled) { color: #a5d2ff; text-decoration: underline; }
  .rd-crumb.cur { color: var(--text-1); cursor: default; }
  .rd-sep { color: var(--text-3); }

  .rd-nav, .rd-foot {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    padding: 6px 0;
  }
  .rd-foot {
    margin-top: 26px;
    border-top: 1px solid var(--line);
    padding-top: 12px;
  }
  .rd-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.13);
    border-radius: 7px;
    color: rgba(255, 255, 255, 0.8);
    font-size: 11.5px;
    padding: 3px 9px;
    cursor: pointer;
    transition: background 0.14s var(--ease-soft);
  }
  .rd-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.12); }
  .rd-btn:disabled { opacity: 0.35; cursor: default; }
  .rd-btn.on {
    color: #a5d2ff;
    border-color: rgba(165, 210, 255, 0.5);
    background: rgba(165, 210, 255, 0.14);
  }
  .rd-btn.code { color: #34d399; border-color: rgba(52, 211, 153, 0.45); }
  .rd-btn.code:hover { background: rgba(52, 211, 153, 0.16); }
  .rd-pos {
    font-size: 11px;
    color: var(--text-3);
    font-family: 'Consolas', monospace;
    min-width: 54px;
    text-align: center;
  }

  .rd-meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin: 8px 0 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    color: var(--text-2);
    background: rgba(255, 255, 255, 0.045);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 2px 9px;
  }
  .chip.type { color: var(--text-1); }
  .chip.id, .chip.st-success, .chip.st-in_progress, .chip.st-failed, .chip.st-blocked {
    font-family: 'Consolas', monospace;
  }
  .chip.st-success { color: #34d399; border-color: rgba(52, 211, 153, 0.35); }
  .chip.st-in_progress { color: #22d3ee; border-color: rgba(34, 211, 238, 0.35); }
  .chip.st-failed { color: #f87171; border-color: rgba(248, 113, 113, 0.35); }
  .chip.st-blocked { color: #fbbf24; border-color: rgba(251, 191, 36, 0.35); }
  .chip.warn { color: #fbbf24; border-color: rgba(251, 191, 36, 0.4); }
  .rd-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 10px;
  }
  .rd-tag {
    font-size: 10.5px;
    color: #a5d2ff;
    background: rgba(165, 210, 255, 0.1);
    border-radius: 5px;
    padding: 1px 7px;
  }

  .rd-body { margin-top: 14px; }
  .rd-raw {
    white-space: pre-wrap;
    word-break: break-word;
    font-family: 'Consolas', 'Cascadia Code', monospace;
    font-size: 0.9em;
    line-height: 1.7;
    color: rgba(255, 255, 255, 0.84);
    background: rgba(0, 0, 0, 0.28);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 16px 18px;
    margin: 0;
  }

  /* 渲染正文：大字号、清晰层级（阅读为第一目标） */
  .md :global(h1) {
    font-size: 1.45em;
    margin: 0.2em 0 0.6em;
    padding-bottom: 0.3em;
    border-bottom: 1px solid var(--line);
  }
  .md :global(h2) {
    font-size: 1.2em;
    margin: 1.5em 0 0.5em;
    padding-left: 10px;
    border-left: 3px solid #60a5fa;
  }
  .md :global(h3) {
    font-size: 1.08em;
    margin: 1.2em 0 0.4em;
    color: rgba(255, 255, 255, 0.86);
  }
  .md :global(h4), .md :global(h5), .md :global(h6) {
    font-size: 1em;
    margin: 1em 0 0.35em;
    color: var(--text-2);
  }
  .md :global(p) { margin: 0.7em 0; line-height: 1.75; }
  .md :global(ul), .md :global(ol) { padding-left: 1.5em; margin: 0.6em 0; }
  .md :global(li) { margin: 0.3em 0; line-height: 1.75; }
  .md :global(blockquote) {
    margin: 0.9em 0;
    padding: 2px 0 2px 14px;
    border-left: 3px solid rgba(255, 255, 255, 0.22);
    color: var(--text-2);
    background: rgba(255, 255, 255, 0.02);
  }
  .md :global(code) {
    font-family: 'Consolas', 'Cascadia Code', monospace;
    font-size: 0.88em;
    color: #9eceff;
  }
  .md :global(p code), .md :global(li code), .md :global(td code) {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    padding: 1px 5px;
  }
  .md :global(pre) {
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid var(--line);
    border-radius: 9px;
    padding: 13px 15px;
    overflow-x: auto;
    line-height: 1.6;
  }
  .md :global(pre code) { color: #cbd5e1; background: transparent; padding: 0; }
  .md :global(a) { color: #7dd3fc; }
  .md :global(hr) { border: none; border-top: 1px solid var(--line); margin: 1.6em 0; }
  .md :global(table) {
    border-collapse: collapse;
    margin: 0.9em 0;
    font-size: 0.94em;
    display: block;
    overflow-x: auto;
    max-width: 100%;
  }
  .md :global(th), .md :global(td) {
    border: 1px solid var(--line);
    padding: 5px 10px;
    text-align: left;
  }
  .md :global(th) { background: rgba(255, 255, 255, 0.05); }
  .md :global(img) { max-width: 100%; border-radius: 8px; }

  .rd-kids { margin-top: 28px; }
  .rd-sec-title {
    font-size: 11px;
    letter-spacing: 1px;
    color: var(--text-3);
    margin-bottom: 8px;
  }
  .rd-kid-list { display: flex; flex-direction: column; gap: 3px; }
  .rd-kid {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    text-align: left;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid var(--line);
    border-radius: 7px;
    color: var(--text-1);
    font-size: 12.5px;
    padding: 5px 10px;
    cursor: pointer;
  }
  .rd-kid:hover { background: rgba(255, 255, 255, 0.09); border-color: rgba(167, 139, 250, 0.4); }
  .rd-kid-t { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rd-rel {
    flex: 0 0 auto;
    font-size: 9.5px;
    color: #a5d2ff;
    border: 1px solid rgba(165, 210, 255, 0.32);
    border-radius: 4px;
    padding: 0 4px;
  }
  .rd-ev-icon { flex: 0 0 auto; font-size: 11px; }
  .rd-err { margin-top: 6px; font-size: 11px; color: #f87171; }

  /* ── v2.20 人用编辑面（新建 / 编辑表单）───────────────────────────── */
  .rd-btn.primary {
    color: #a5d2ff;
    border-color: rgba(165, 210, 255, 0.5);
    background: rgba(165, 210, 255, 0.14);
  }
  .rd-btn.primary:hover:not(:disabled) { background: rgba(165, 210, 255, 0.24); }
  .rd-btn.danger { color: #fca5a5; border-color: rgba(248, 113, 113, 0.35); }
  .rd-btn.danger:hover:not(:disabled) { background: rgba(248, 113, 113, 0.18); }
  .rd-btn.danger.armed {
    color: #fff;
    background: rgba(248, 113, 113, 0.55);
    border-color: #f87171;
  }
  .rd-msg {
    font-size: 11px;
    color: #86efac;
    margin-left: 4px;
  }
  .rd-formerr {
    margin: 6px 0 2px;
    padding: 7px 10px;
    font-size: 11.5px;
    color: #fca5a5;
    background: rgba(248, 113, 113, 0.1);
    border: 1px solid rgba(248, 113, 113, 0.35);
    border-radius: 8px;
  }
  .rd-newhead {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .rd-hint {
    font-size: 11px;
    color: var(--text-3);
    line-height: 1.6;
  }
  .rd-form {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .fld {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .fld > span {
    font-size: 11px;
    color: var(--text-2);
    letter-spacing: 0.3px;
  }
  .fld input,
  .fld select,
  .fld textarea {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    color: var(--text-1);
    font-size: 12.5px;
    padding: 7px 10px;
    font-family: inherit;
  }
  .fld input:focus,
  .fld select:focus,
  .fld textarea:focus {
    outline: none;
    border-color: rgba(125, 211, 252, 0.6);
  }
  .fld-row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .fld-row .fld { flex: 0 0 auto; min-width: 140px; }
  .fld.grow { flex: 1 1 200px; }
  .fld-body {
    min-height: 320px;
    line-height: 1.7;
    font-family: 'Consolas', 'Cascadia Code', 'Microsoft YaHei', monospace;
    font-size: 13px !important;
    resize: vertical;
    white-space: pre-wrap;
  }
  .fld-foot { padding-bottom: 8px; }
  .parent-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .parent-search-wrap { position: relative; flex: 1 1 auto; min-width: 0; }
  .parent-search {
    width: 100%;
    box-sizing: border-box;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    color: rgba(255, 255, 255, 0.85);
    font-size: 12px;
    padding: 7px 10px;
  }
  .parent-search:focus { outline: none; border-color: rgba(125, 211, 252, 0.55); }
  .parent-results {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 30;
    background: rgba(24, 26, 32, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 8px;
    max-height: 220px;
    overflow-y: auto;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }
  .parent-opt {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.8);
    font-size: 11.5px;
    padding: 7px 10px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .parent-opt:hover { background: rgba(125, 211, 252, 0.16); }
  .rel-sel {
    flex: 0 0 auto;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    color: var(--text-1);
    font-size: 11.5px;
    padding: 7px 8px;
  }
  .parent-clear {
    flex: 0 0 auto;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    color: var(--text-2);
    font-size: 11px;
    padding: 6px 9px;
    cursor: pointer;
  }
  .parent-clear:hover { color: #fca5a5; border-color: rgba(248, 113, 113, 0.4); }
</style>
