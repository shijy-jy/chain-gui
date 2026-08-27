<script lang="ts">
  import type { ChainNode } from '../lib/types';

  let {
    nodes,
    onCreate,
    onCancel,
  }: {
    nodes: ChainNode[];
    onCreate: (input: { id: string; title: string; parent: string | null }) => Promise<void>;
    onCancel: () => void;
  } = $props();

  // v2.0 开发模式知识库：不要求类型/状态（后端默认中性 note/none）
  let id = $state('');           // 留空 = 自动生成
  let title = $state('');
  let parent = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function submit() {
    if (busy) return;
    if (!title.trim()) {
      error = '先写个标题（开发模式不强制，但空节点不好认）';
      return;
    }
    busy = true;
    error = null;
    try {
      await onCreate({
        id: id.trim() || '',
        title: title.trim(),
        parent,
      });
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="dialog-mask" role="button" tabindex="-1" aria-label="关闭对话框"
     onclick={onCancel}
     onkeydown={(e) => { if (e.key === 'Escape') onCancel(); }}>
  <!-- stopPropagation：点面板本身不关闭 -->
  <div class="dialog" role="dialog" aria-modal="true" aria-label="新建节点" tabindex="-1"
       onclick={(e) => e.stopPropagation()}
       onkeydown={(e) => { if (e.key === 'Escape') onCancel(); }}>
    <header>
      <h3>＋ 新建节点</h3>
      <button class="close" onclick={onCancel} aria-label="关闭">✕</button>
    </header>

    <div class="field">
      <label for="new-title">标题</label>
      <input id="new-title" type="text" bind:value={title} placeholder="如：费曼讲义 · 量子力学" disabled={busy} />
    </div>

    <div class="field">
      <label for="new-parent">父节点（建立链接）</label>
      <select id="new-parent" bind:value={parent} disabled={busy}>
        <option value={null}>无（独立节点）</option>
        {#each nodes as n (n.id)}
          <option value={n.id}>{n.title} · {n.id}</option>
        {/each}
      </select>
    </div>

    <div class="field">
      <label for="new-id">id（留空自动生成 node-N）</label>
      <input id="new-id" type="text" bind:value={id} placeholder="如 node-1、算法笔记（字母/数字/-/_）" disabled={busy} />
    </div>

    {#if error}
      <p class="error">⚠ {error}</p>
    {/if}

    <footer>
      <button class="cancel" onclick={onCancel} disabled={busy}>取消</button>
      <button class="save" onclick={submit} disabled={busy}>{busy ? '创建中…' : '创建'}</button>
    </footer>
  </div>
</div>

<style>
  .dialog-mask {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }
  .dialog {
    width: min(420px, 90vw);
    background: #161618;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    padding: 20px 22px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.6);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 14px;
  }
  header h3 { margin: 0; font-size: 14px; font-weight: 500; letter-spacing: 1px; }
  .close {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.35);
    font-size: 14px;
    cursor: pointer;
  }
  .close:hover { color: rgba(255, 255, 255, 0.9); }
  .field { margin-bottom: 12px; }
  label {
    display: block;
    font-size: 10px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
    margin-bottom: 6px;
  }
  input, select {
    width: 100%;
    padding: 8px 12px;
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    font-size: 13px;
    font-family: inherit;
    box-sizing: border-box;
  }
  input:focus, select:focus { outline: none; border-color: rgba(255, 255, 255, 0.35); }
  select option { background: #161618; }
  .error {
    color: #f87171;
    font-size: 12px;
    margin: 4px 0 0;
  }
  footer {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 16px;
  }
  footer button {
    padding: 8px 22px;
    border: none;
    border-radius: 999px;
    cursor: pointer;
    font-size: 13px;
  }
  .cancel {
    background: transparent;
    color: rgba(255, 255, 255, 0.55);
    border: 1px solid rgba(255, 255, 255, 0.15);
  }
  .save { background: rgba(255, 255, 255, 0.92); color: #0a0a0a; font-weight: 500; }
  .save:disabled, .cancel:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
