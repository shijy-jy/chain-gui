<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
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
  let evidenceText = $state('');
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

  let canFold = $derived(!!onFold && node.parent !== null);

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
    evidenceText = node.evidence.join(', ');
    error = null;
  });

  async function handleSave() {
    if (saving) return;
    saving = true;
    error = null;
    try {
      const tags = tagsText.split(',').map(t => t.trim()).filter(t => t.length > 0);
      const evidence = evidenceText.split(',').map(e => e.trim()).filter(e => e.length > 0);
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
</script>

<aside class="sidebar">
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
    <label for="body">正文</label>
    <textarea id="body" bind:value={body} rows="10" disabled={saving}></textarea>
  </div>

  <div class="field">
    <label for="tags">标签（逗号分隔）</label>
    <input id="tags" type="text" bind:value={tagsText} disabled={saving} />
  </div>

  <div class="field">
    <label for="evidence" title="指向 .chain/artifacts/ 下产物文件的相对路径，逗号分隔">证据（产物相对路径，逗号分隔）</label>
    <input id="evidence" type="text" bind:value={evidenceText} disabled={saving}
           placeholder="如 artifacts/t-001/截图.png" />
  </div>

  {#if error}
    <p class="error">⚠ {error}</p>
  {/if}

  <footer>
    <button class="cancel" onclick={onCancel} disabled={saving}>取消</button>
    <button class="save" onclick={handleSave} disabled={saving}>
      {saving ? '保存中…' : '保存'}
    </button>
  </footer>

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

  <div class="log-block">
    <div class="log-title">过程日志（试错/环境坑，追加到 PROCESS_LOG.md）</div>
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
</aside>

<style>
  .sidebar {
    position: fixed;
    top: 0;
    right: 0;
    width: min(360px, 92vw);   /* v1.4 窄窗口自适应 */
    height: 100vh;
    background: #111111;
    color: rgba(255, 255, 255, 0.85);
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    padding: 24px;
    overflow-y: auto;
    z-index: 1000;
    box-sizing: border-box;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
    padding-bottom: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  .id-row {
    display: flex;
    align-items: center;
    gap: 10px;
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
  }
  .close {
    background: none;
    border: none;
    font-size: 14px;
    cursor: pointer;
    color: rgba(255, 255, 255, 0.35);
    padding: 4px;
    transition: color 0.15s ease;
  }
  .close:hover { color: rgba(255, 255, 255, 0.9); }
  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 18px;
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
  .field { margin-bottom: 18px; }
  label {
    display: block;
    font-size: 10px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
    margin-bottom: 8px;
  }
  input, select, textarea {
    width: 100%;
    padding: 9px 12px;
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
    resize: vertical;
    line-height: 1.6;
  }
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
    margin-top: 24px;
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

  /* 过程日志块（v1.2） */
  .log-block {
    margin-top: 28px;
    padding-top: 18px;
    border-top: 1px dashed rgba(255, 255, 255, 0.12);
  }
  .log-title {
    font-size: 10px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.35);
    margin-bottom: 8px;
  }
  .log-input {
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 11px;
    background: rgba(255, 255, 255, 0.03);
  }
  .log-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 8px;
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
    margin-top: 28px;
    padding-top: 18px;
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
