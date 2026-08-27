<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { ChainNode, NodeStatus, NodeType } from './types';

  let { node, chainDir, onSave, onCancel, onFold }: {
    node: ChainNode;
    chainDir: string | null;
    onSave: (fields: { title: string; status: NodeStatus; body: string; tags: string[]; evidence: string[] }) => Promise<void>;
    onCancel: () => void;
    onFold?: () => Promise<void>;
  } = $props();

  // 初始值用字面量（不用 node.xxx），避免 Svelte 5 state_referenced_locally 警告；
  // 实际值由下面的 $effect 同步（组件挂载和 node 切换时都会跑）
  let title = $state('');
  let status = $state<NodeStatus>('pending');
  let body = $state('');
  let tagsText = $state('');
  let evidence = $state<string[]>([]);   // 协议不变：存相对路径；界面只显示文件名
  let saving = $state(false);
  let error = $state<string | null>(null);

  // 过程日志（v1.2）
  let logText = $state('');
  let logSaving = $state(false);
  let logMessage = $state<string | null>(null);

  // 折叠（v1.3）：两段式确认，防止误触
  let foldArmed = $state(false);
  let foldBusy = $state(false);
  let foldMessage = $state<string | null>(null);

  // 证据（v1.8）：文件名列表 + 点击打开 + 文件选择器添加
  let evBusy = $state(false);
  let evMessage = $state<string | null>(null);

  // v1.8 VSCode 式分栏：面板宽度 + 各内容区高度/折叠状态。
  // 模块级 $state——切换节点重新挂载组件时保留用户的布局调整。
  const panel = $state({
    width: 380,
    bodyH: 300,
    evidenceH: 136,
    logH: 112,
    bodyOpen: true,
    evidenceOpen: true,
    logOpen: true,
  });

  // —— 布局拖拽：横向边界条调整上方内容区高度（VSCode 分栏手感）——
  function resizeSection(which: 'bodyH' | 'evidenceH' | 'logH') {
    return (e: PointerEvent) => {
      e.preventDefault();
      const startY = e.clientY;
      const startH = panel[which];
      const el = e.currentTarget as HTMLElement;
      el.setPointerCapture(e.pointerId);
      const move = (ev: PointerEvent) => {
        panel[which] = Math.min(Math.max(startH + (ev.clientY - startY), 60), 900);
      };
      const up = (ev: PointerEvent) => {
        el.removeEventListener('pointermove', move);
        el.removeEventListener('pointerup', up);
        el.releasePointerCapture(ev.pointerId);
      };
      el.addEventListener('pointermove', move);
      el.addEventListener('pointerup', up);
    };
  }

  // 面板左缘竖向条：拖拽调整面板整体宽度
  function resizeWidth(e: PointerEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = panel.width;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const w = Math.min(Math.max(startW - (ev.clientX - startX), 320), Math.round(window.innerWidth * 0.72));
      panel.width = w;
    };
    const up = (ev: PointerEvent) => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.releasePointerCapture(ev.pointerId);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
  }

  // —— 证据：只显示文件名；点击用系统默认程序打开 ——
  const evName = (rel: string) => rel.split(/[\\/]/).pop() ?? rel;

  async function openEvidence(rel: string) {
    if (!chainDir || evBusy) return;
    evMessage = null;
    try {
      await invoke('open_evidence', { dir: chainDir, rel });
    } catch (e) {
      evMessage = String(e);
    }
  }

  async function pickEvidence() {
    if (!chainDir || saving || evBusy) return;
    evMessage = null;
    const selected = await open({ multiple: true });
    if (!selected) return;
    const files = Array.isArray(selected) ? selected : [selected];
    evBusy = true;
    try {
      const rels: string[] = [];
      for (const abs of files) {
        try {
          const rel = await invoke<string>('evidence_rel_path', { dir: chainDir, abs });
          rels.push(rel);
        } catch (e) {
          evMessage = String(e);
        }
      }
      if (rels.length > 0) {
        evidence = Array.from(new Set([...evidence, ...rels]));
      }
    } finally {
      evBusy = false;
    }
  }

  function removeEvidence(rel: string) {
    evidence = evidence.filter((r) => r !== rel);
  }

  async function handleFold() {
    if (!onFold || foldBusy) return;
    if (!foldArmed) {
      foldArmed = true;
      foldMessage = '再次点击确认：子链所有节点将归档，本节点变为摘要';
      return;
    }
    foldBusy = true;
    foldMessage = null;
    try {
      await onFold();
    } catch (e) {
      foldMessage = String(e);
      foldArmed = false;
    } finally {
      foldBusy = false;
    }
  }

  const statusOptions: NodeStatus[] = ['pending', 'in_progress', 'success', 'failed', 'blocked'];
  const statusLabels: Record<NodeStatus, string> = {
    pending: '待开始',
    in_progress: '进行中',
    success: '已完成',
    failed: '失败',
    blocked: '阻塞',
  };

  // 类型色点：与画布节点配色一致，详情面板和图谱互相呼应
  const typeColors: Record<NodeType, string> = {
    goal: '#a78bfa',
    design: '#60a5fa',
    task: '#22d3ee',
    verification: '#34d399',
  };
  let typeColor = $derived(typeColors[node.type]);

  // node 变化时重置表单（effect 只追踪读取的 node.xxx，写入的 state 不触发重跑）
  $effect(() => {
    title = node.title;
    status = node.status;
    body = node.body;
    tagsText = node.tags.join(', ');
    evidence = [...node.evidence];
    error = null;
  });

  async function handleSave() {
    if (saving) return;
    saving = true;
    error = null;
    try {
      const tags = tagsText.split(',').map(t => t.trim()).filter(t => t.length > 0);
      await onSave({ title: title.trim(), status, body, tags, evidence });
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  async function handleAppendLog() {
    if (!chainDir || logSaving) return;
    const text = logText.trim();
    if (!text) {
      logMessage = '先写点内容再追加';
      return;
    }
    logSaving = true;
    logMessage = null;
    try {
      const ts = await invoke<string>('append_log', { dir: chainDir, text });
      logMessage = `已追加（${ts.slice(11, 19)}）`;
      logText = '';
    } catch (e) {
      logMessage = String(e);
    } finally {
      logSaving = false;
    }
  }

  let canFold = $derived(!!onFold && node.parent !== null);
</script>

<aside class="sidebar" style:width="{panel.width}px">
  <!-- v1.8 面板左缘拖拽条：调整面板宽度 -->
  <div class="width-handle" role="separator" aria-orientation="vertical" onpointerdown={resizeWidth} title="拖拽调整面板宽度"></div>

  <header>
    <div class="id-row">
      <span class="type-dot" style="background: {typeColor}; box-shadow: 0 0 8px {typeColor};"></span>
      <h2>{node.id}</h2>
    </div>
    <button class="close" onclick={onCancel} aria-label="关闭">✕</button>
  </header>

  <div class="meta-row">
    <span class="meta-item">rev {node.revision}</span>
    <span class="meta-item" title="创建时间">建于 {node.created.slice(0, 16)}</span>
    <span class="meta-item" title="最后更新">更于 {node.updated.slice(0, 16)}</span>
    <span class="meta-item" title="父节点">父 {node.parent ?? '无（根）'}</span>
  </div>

  <!-- 固定小字段区（不参与分栏拖拽） -->
  <div class="fixed-fields">
    <div class="field">
      <label for="title">标题</label>
      <input id="title" type="text" bind:value={title} disabled={saving} />
    </div>
    <div class="field">
      <label for="status">状态</label>
      <select id="status" bind:value={status} disabled={saving}>
        {#each statusOptions as opt}
          <option value={opt}>{statusLabels[opt]}</option>
        {/each}
      </select>
    </div>
    <div class="field">
      <label for="tags">标签（逗号分隔）</label>
      <input id="tags" type="text" bind:value={tagsText} disabled={saving} />
    </div>
  </div>

  <!-- v1.8 正文区：可折叠 + 可拖边界调高度 -->
  <button type="button" class="pane-head" onclick={() => (panel.bodyOpen = !panel.bodyOpen)}>
    <span class="chev">{panel.bodyOpen ? '▾' : '▸'}</span>正文
    <span class="pane-hint">点击折叠 · 拖下方边界调高度</span>
  </button>
  {#if panel.bodyOpen}
    <div class="pane" style:height="{panel.bodyH}px">
      <textarea id="body" class="body-input" bind:value={body} disabled={saving}></textarea>
    </div>
    <div class="h-handle" role="separator" aria-orientation="horizontal" onpointerdown={resizeSection('bodyH')} title="拖拽调整正文高度"><span class="grip"></span></div>
  {/if}

  <!-- v1.8 证据区：文件名列表（点击打开）+ 文件选择器添加 -->
  <button type="button" class="pane-head" onclick={() => (panel.evidenceOpen = !panel.evidenceOpen)}>
    <span class="chev">{panel.evidenceOpen ? '▾' : '▸'}</span>证据（{evidence.length}）
    <span class="pane-hint">点击文件名打开</span>
  </button>
  {#if panel.evidenceOpen}
    <div class="pane ev-pane" style:height="{panel.evidenceH}px">
      {#if evidence.length === 0}
        <div class="ev-empty">暂无证据产物，点下方按钮添加</div>
      {:else}
        <div class="evidence-list">
          {#each evidence as rel (rel)}
            <div class="ev-row">
              <button class="ev-name" title="打开 {rel}" onclick={() => openEvidence(rel)}>{evName(rel)}</button>
              <button class="ev-del" title="移除该证据" onclick={() => removeEvidence(rel)} disabled={saving}>✕</button>
            </div>
          {/each}
        </div>
      {/if}
      <button class="ev-add" onclick={pickEvidence} disabled={saving || evBusy || !chainDir}>
        {evBusy ? '添加中…' : '＋ 添加证据文件'}
      </button>
      {#if evMessage}<p class="ev-msg">⚠ {evMessage}</p>{/if}
    </div>
    <div class="h-handle" role="separator" aria-orientation="horizontal" onpointerdown={resizeSection('evidenceH')} title="拖拽调整证据区高度"><span class="grip"></span></div>
  {/if}

  <!-- v1.8 日志区：可折叠 + 可拖边界调高度 -->
  <button type="button" class="pane-head" onclick={() => (panel.logOpen = !panel.logOpen)}>
    <span class="chev">{panel.logOpen ? '▾' : '▸'}</span>过程日志
    <span class="pane-hint">一行一条 · 自动加时间戳</span>
  </button>
  {#if panel.logOpen}
    <div class="pane log-pane" style:height="{panel.logH}px">
      <textarea class="log-input" bind:value={logText} rows="2" disabled={logSaving}
                placeholder="如：环境坑：…；失败尝试：…（一行一条，自动加时间戳）"></textarea>
      <div class="log-actions">
        <button class="log-append" onclick={handleAppendLog} disabled={logSaving || !chainDir}>
          {logSaving ? '追加中…' : '追加日志'}
        </button>
        {#if logMessage}
          <span class="log-msg">{logMessage}</span>
        {/if}
      </div>
    </div>
    <div class="h-handle" role="separator" aria-orientation="horizontal" onpointerdown={resizeSection('logH')} title="拖拽调整日志区高度"><span class="grip"></span></div>
  {/if}

  <!-- 底部固定区：折叠 / 错误 / 保存（始终可见，不随分栏滚动） -->
  <div class="bottom-fixed">
    {#if canFold}
      <div class="fold-block">
        <div class="fold-title">子链折叠（v1.3）</div>
        <button class="fold-btn" class:armed={foldArmed} onclick={handleFold} disabled={foldBusy}>
          {foldBusy ? '折叠中…' : foldArmed ? '⚠ 确认折叠？' : '折叠此子链'}
        </button>
        {#if foldMessage}
          <p class="fold-msg">{foldMessage}</p>
        {/if}
      </div>
    {/if}

    {#if error}
      <p class="error">⚠ {error}</p>
    {/if}

    <footer>
      <button class="cancel" onclick={onCancel} disabled={saving}>取消</button>
      <button class="save" onclick={handleSave} disabled={saving}>
        {saving ? '保存中…' : '保存'}
      </button>
    </footer>
  </div>
</aside>

<style>
  .sidebar {
    position: fixed;
    top: 0;
    right: 0;
    height: 100vh;
    background: #111111;
    color: rgba(255, 255, 255, 0.85);
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    padding: 20px 24px 16px;
    overflow: hidden;
    z-index: 1000;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    min-width: 320px;
  }

  /* v1.8 面板左缘拖拽条（调整面板宽度） */
  .width-handle {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 2;
  }
  .width-handle:hover { background: rgba(255, 255, 255, 0.12); }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    padding-bottom: 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }
  .id-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .type-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  header h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 500;
    letter-spacing: 1px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .close {
    background: none;
    border: none;
    font-size: 14px;
    cursor: pointer;
    color: rgba(255, 255, 255, 0.35);
    padding: 4px;
    transition: color 0.15s ease;
    flex-shrink: 0;
  }
  .close:hover { color: rgba(255, 255, 255, 0.9); }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
    flex-shrink: 0;
  }
  .meta-item {
    font-size: 10px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.35);
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.07);
    padding: 2px 8px;
    border-radius: 999px;
  }

  /* 固定小字段区：标题/状态/标签 */
  .fixed-fields { flex-shrink: 0; }
  .field { margin-bottom: 12px; }
  label {
    display: block;
    font-size: 10px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
    margin-bottom: 6px;
  }
  input, select, textarea {
    width: 100%;
    padding: 8px 12px;
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    font-size: 13px;
    font-family: inherit;
    box-sizing: border-box;
    transition: border-color 0.15s ease, background 0.15s ease;
  }
  input:focus, select:focus, textarea:focus {
    outline: none;
    border-color: rgba(255, 255, 255, 0.35);
    background: rgba(255, 255, 255, 0.06);
  }
  textarea {
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 12px;
    resize: none;
    line-height: 1.6;
  }

  /* v1.8 分栏头部：点击折叠/展开（button 语义，键盘 Enter/Space 可用） */
  .pane-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 2px;
    margin-top: 4px;
    font-size: 10px;
    font-family: inherit;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
    background: none;
    border: none;
    width: 100%;
    text-align: left;
    cursor: pointer;
    user-select: none;
    flex-shrink: 0;
  }
  .pane-head:hover { color: rgba(255, 255, 255, 0.75); }
  .chev { font-size: 9px; width: 10px; }
  .pane-hint {
    margin-left: auto;
    font-size: 9px;
    letter-spacing: 0;
    text-transform: none;
    color: rgba(255, 255, 255, 0.22);
  }

  /* v1.8 内容区：固定高度（由拖拽调整），内部滚动 */
  .pane {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
  .body-input { flex: 1; min-height: 0; }

  /* v1.8 横向边界拖拽条（调整上方内容区高度） */
  .h-handle {
    height: 9px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: row-resize;
    flex-shrink: 0;
    touch-action: none;
  }
  .h-handle:hover .grip, .h-handle:active .grip { background: rgba(255, 255, 255, 0.3); }
  .grip {
    width: 44px;
    height: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.12);
    transition: background 0.15s ease;
  }

  /* v1.8 证据区 */
  .ev-pane { gap: 8px; }
  .ev-empty {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.28);
    padding: 8px 2px;
  }
  .evidence-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-right: 2px;
  }
  .ev-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .ev-name {
    flex: 1;
    min-width: 0;
    text-align: left;
    font-size: 12px;
    font-family: 'Consolas', monospace;
    color: #7dd3fc;
    background: rgba(125, 211, 252, 0.06);
    border: 1px solid rgba(125, 211, 252, 0.18);
    border-radius: 6px;
    padding: 5px 10px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background 0.15s ease;
  }
  .ev-name:hover { background: rgba(125, 211, 252, 0.16); }
  .ev-del {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.4);
    background: none;
    border: 1px solid transparent;
    border-radius: 5px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .ev-del:hover:not(:disabled) { color: #f87171; border-color: rgba(248, 113, 113, 0.4); }
  .ev-del:disabled { opacity: 0.4; cursor: not-allowed; }
  .ev-add {
    flex-shrink: 0;
    font-size: 11px;
    padding: 5px 12px;
    background: rgba(125, 211, 252, 0.1);
    color: #7dd3fc;
    border: 1px dashed rgba(125, 211, 252, 0.35);
    border-radius: 999px;
    cursor: pointer;
    transition: background 0.15s ease;
  }
  .ev-add:hover:not(:disabled) { background: rgba(125, 211, 252, 0.2); }
  .ev-add:disabled { opacity: 0.4; cursor: not-allowed; }
  .ev-msg {
    margin: 0;
    font-size: 10px;
    font-family: 'Consolas', monospace;
    color: #fbbf24;
    word-break: break-all;
  }

  /* 底部固定区：折叠 + 错误 + 保存 */
  .bottom-fixed { flex-shrink: 0; }
  .error {
    color: #f87171;
    background: rgba(248, 113, 113, 0.1);
    border: 1px solid rgba(248, 113, 113, 0.25);
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 12px;
  }
  footer {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 14px;
  }
  footer button {
    padding: 8px 22px;
    border: none;
    border-radius: 999px;
    cursor: pointer;
    font-size: 13px;
    transition: background 0.15s ease;
  }
  .cancel {
    background: transparent;
    color: rgba(255, 255, 255, 0.55);
    border: 1px solid rgba(255, 255, 255, 0.15);
  }
  .cancel:hover:not(:disabled) { background: rgba(255, 255, 255, 0.08); }
  .save {
    background: rgba(255, 255, 255, 0.92);
    color: #0a0a0a;
    font-weight: 500;
  }
  .save:hover:not(:disabled) { background: #ffffff; }
  .save:disabled, .cancel:disabled { opacity: 0.4; cursor: not-allowed; }

  /* 日志区（v1.2 起，v1.8 改分栏） */
  .log-pane { gap: 8px; }
  .log-input {
    flex: 1;
    min-height: 0;
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 11px;
    background: rgba(255, 255, 255, 0.03);
  }
  .log-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .log-append {
    font-size: 11px;
    padding: 5px 14px;
    background: rgba(52, 211, 153, 0.15);
    color: #34d399;
    border: 1px solid rgba(52, 211, 153, 0.3);
    border-radius: 999px;
    cursor: pointer;
    transition: background 0.15s ease;
  }
  .log-append:hover:not(:disabled) { background: rgba(52, 211, 153, 0.28); }
  .log-append:disabled { opacity: 0.4; cursor: not-allowed; }
  .log-msg {
    font-size: 11px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.45);
  }

  /* 折叠块（v1.3） */
  .fold-block {
    padding-top: 10px;
    margin-top: 6px;
    border-top: 1px dashed rgba(255, 255, 255, 0.12);
  }
  .fold-title {
    font-size: 10px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.35);
    margin-bottom: 8px;
  }
  .fold-btn {
    font-size: 11px;
    padding: 5px 14px;
    background: rgba(251, 191, 36, 0.12);
    color: #fbbf24;
    border: 1px solid rgba(251, 191, 36, 0.3);
    border-radius: 999px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .fold-btn:hover:not(:disabled) { background: rgba(251, 191, 36, 0.22); }
  .fold-btn.armed {
    background: rgba(248, 113, 113, 0.25);
    color: #f87171;
    border-color: rgba(248, 113, 113, 0.5);
  }
  .fold-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .fold-msg {
    margin: 8px 0 0;
    font-size: 11px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.5);
  }
</style>
