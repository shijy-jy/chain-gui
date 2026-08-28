<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { ChainSnapshot, ScanMode, SnapshotMeta } from '../lib/types';

  let {
    snapshot,
    chainDir,
    mode,
    onrescan,
  }: {
    snapshot: ChainSnapshot | null;
    chainDir: string | null;
    mode: ScanMode;
    onrescan?: () => void;
  } = $props();

  let drawerOpen = $state(false);
  let logOpen = $state(false);
  let activeOpen = $state(false);
  let snapOpen = $state(false);

  // 指南版本（v1.2 起；v2.1 双指南：按当前工作区模式取对应指南版本）
  let guideVersion = $state<number | null>(null);
  let logContent = $state<string>('');
  let logLoading = $state(false);
  let snapshots = $state<SnapshotMeta[]>([]);
  let snapLoading = $state(false);

  $effect(() => {
    // 模式变化或首次进入时取对应指南版本
    const _m = mode;
    invoke<number>('get_guide_version', { mode: _m })
      .then(v => (guideVersion = v))
      .catch(() => {});
  });

  function closeOthers(keep: 'log' | 'active' | 'snap') {
    if (keep !== 'log') logOpen = false;
    if (keep !== 'active') activeOpen = false;
    if (keep !== 'snap') snapOpen = false;
  }

  async function toggleLog() {
    if (logOpen) {
      logOpen = false;
      return;
    }
    if (!chainDir) return;
    closeOthers('log');
    logLoading = true;
    try {
      logContent = await invoke<string>('get_process_log', { dir: chainDir });
    } catch (e) {
      logContent = `读取失败：${String(e)}`;
    } finally {
      logLoading = false;
    }
    logOpen = true;
  }

  function toggleActive() {
    if (!snapshot) return;
    closeOthers('active');
    activeOpen = !activeOpen;
  }

  async function toggleSnaps() {
    if (snapOpen) {
      snapOpen = false;
      return;
    }
    if (!chainDir) return;
    closeOthers('snap');
    snapLoading = true;
    try {
      snapshots = await invoke<SnapshotMeta[]>('list_snapshots', { dir: chainDir });
    } catch (e) {
      snapshots = [];
    } finally {
      snapLoading = false;
    }
    snapOpen = true;
  }

  let errorCount = $derived(snapshot?.validation.errors.length ?? 0);
  let warningCount = $derived(snapshot?.validation.warnings.length ?? 0);
  let hasIssues = $derived(errorCount > 0 || warningCount > 0);

  function toggleDrawer() {
    if (hasIssues) drawerOpen = !drawerOpen;
  }
</script>

{#if drawerOpen && snapshot}
  <div class="drawer">
    <div class="drawer-header">
      <span class="drawer-title">校验详情</span>
      <button class="drawer-close" onclick={() => (drawerOpen = false)}>✕</button>
    </div>
    <div class="drawer-list">
      {#each snapshot.validation.errors as err}
        <div class="issue-row error">
          <span class="dot dot-error"></span>
          <span class="issue-text">{err}</span>
        </div>
      {/each}
      {#each snapshot.validation.warnings as warn}
        <div class="issue-row warning">
          <span class="dot dot-warn"></span>
          <span class="issue-text">{warn}</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

{#if logOpen}
  <div class="drawer log-drawer">
    <div class="drawer-header">
      <span class="drawer-title">过程日志（.chain/PROCESS_LOG.md）</span>
      <button class="drawer-close" onclick={() => (logOpen = false)}>✕</button>
    </div>
    <pre class="log-content">{logLoading ? '读取中…' : (logContent || '（空）——在节点编辑栏的「过程日志」里追加第一条')}</pre>
  </div>
{/if}

{#if activeOpen && snapshot}
  <div class="drawer log-drawer">
    <div class="drawer-header">
      <span class="drawer-title">活跃链摘要（v1.3，只含未完成节点）</span>
      <button class="drawer-close" onclick={() => (activeOpen = false)}>✕</button>
    </div>
    <pre class="log-content">{snapshot.manifest.active_chain}</pre>
    <div class="health-row">
      {#if snapshot.manifest.chain_health.in_progress_count > 0}
        <span class="health-chip in-progress">🔄 {snapshot.manifest.chain_health.in_progress_count}</span>
      {/if}
      {#if snapshot.manifest.chain_health.pending_count > 0}
        <span class="health-chip pending">⏳ {snapshot.manifest.chain_health.pending_count}</span>
      {/if}
      {#if snapshot.manifest.chain_health.failed_count > 0}
        <span class="health-chip failed">❌ {snapshot.manifest.chain_health.failed_count}</span>
      {/if}
      {#if snapshot.manifest.chain_health.blocked_count > 0}
        <span class="health-chip blocked">🚫 {snapshot.manifest.chain_health.blocked_count}</span>
      {/if}
      <span class="health-chip success">✅ {snapshot.manifest.chain_health.success_count}</span>
    </div>
  </div>
{/if}

{#if snapOpen}
  <div class="drawer log-drawer">
    <div class="drawer-header">
      <span class="drawer-title">链快照（.chain/logs/）</span>
      <button class="drawer-close" onclick={() => (snapOpen = false)}>✕</button>
    </div>
    <div class="drawer-list">
      {#if snapLoading}
        <div class="issue-text">读取中…</div>
      {:else if snapshots.length === 0}
        <div class="issue-text">（无快照）——用工具栏「快照」按钮创建</div>
      {:else}
        {#each snapshots as s}
          <div class="snap-row">
            <span class="snap-tag">{s.tag}</span>
            <span class="snap-meta">{s.created_at.slice(0, 16)} · {s.node_count} 节点</span>
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}

<footer class="status-bar">
  <div class="left">
    {#if snapshot}
      <span>{snapshot.manifest.node_count} 节点 · {snapshot.manifest.edge_count} 边</span>
    {:else}
      <span class="muted">未选择目录</span>
    {/if}
    {#if guideVersion !== null}
      <span class="muted" title="本软件内嵌 AI 指南版本（v2.1 双指南，按工作区模式）">
        {mode === 'dev' ? '知识库' : '协议'}指南 v{guideVersion}
      </span>
    {/if}
  </div>
  <div class="right">
    {#if chainDir && snapshot}
      <button class="rescan-btn" onclick={toggleActive} title="只含未完成节点的紧凑树状摘要，AI 进场快速恢复认知">活跃链</button>
      <button class="rescan-btn" onclick={toggleSnaps} title="查看链状态快照列表">快照</button>
      <button class="rescan-btn" onclick={toggleLog} title="查看 .chain/PROCESS_LOG.md 试错流水账">过程日志</button>
    {/if}
    {#if onrescan && snapshot}
      <button class="rescan-btn" onclick={onrescan}>重新扫描</button>
    {/if}
    {#if snapshot}
      <button
        class="validation-btn"
        class:clickable={hasIssues}
        onclick={toggleDrawer}
      >
        {#if errorCount > 0}
          <span class="dot dot-error"></span>
          <span>{errorCount} 错误</span>
        {:else if warningCount > 0}
          <span class="dot dot-warn"></span>
          <span>{warningCount} 警告</span>
        {:else}
          <span class="dot dot-ok"></span>
          <span>校验通过</span>
        {/if}
      </button>
    {/if}
  </div>
</footer>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 20px;
    background: #111;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
    font-size: 11px;
    letter-spacing: 0.5px;
    color: rgba(255, 255, 255, 0.4);
    flex-shrink: 0;
  }
  .left, .right {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .muted { color: rgba(255, 255, 255, 0.25); }
  .rescan-btn {
    background: none;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.5);
    font-size: 10px;
    padding: 2px 10px;
    border-radius: 999px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .rescan-btn:hover { border-color: rgba(255, 255, 255, 0.25); color: rgba(255, 255, 255, 0.8); }
  .validation-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    color: inherit;
    font-size: inherit;
    letter-spacing: inherit;
    cursor: default;
    padding: 0;
  }
  .validation-btn.clickable { cursor: pointer; }
  .validation-btn.clickable:hover { color: rgba(255, 255, 255, 0.8); }
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot-ok { background: #34d399; }
  .dot-error { background: #f87171; }
  .dot-warn { background: #fbbf24; }
  .drawer {
    background: #111;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
    max-height: 200px;
    overflow-y: auto;
    flex-shrink: 0;
  }
  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  .drawer-title {
    font-size: 10px;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
  }
  .drawer-close {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.3);
    cursor: pointer;
    font-size: 12px;
    padding: 0 4px;
  }
  .drawer-close:hover { color: rgba(255, 255, 255, 0.7); }
  .drawer-list {
    padding: 4px 0;
  }
  .issue-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 20px;
  }
  .issue-text {
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.65);
  }
  /* 过程日志抽屉（v1.2） */
  .log-drawer {
    max-height: 280px;
  }
  .log-content {
    margin: 0;
    padding: 12px 20px;
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 11px;
    line-height: 1.7;
    color: rgba(255, 255, 255, 0.65);
    white-space: pre-wrap;
    word-break: break-all;
  }
  /* 活跃链健康度（v1.3） */
  .health-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 8px 20px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }
  .health-chip {
    font-size: 11px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.55);
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    padding: 2px 10px;
    border-radius: 999px;
  }
  .health-chip.in-progress { color: #22d3ee; }
  .health-chip.pending { color: rgba(255, 255, 255, 0.45); }
  .health-chip.failed { color: #f87171; }
  .health-chip.blocked { color: #fbbf24; }
  .health-chip.success { color: #34d399; }
  /* 快照列表（v1.3） */
  .snap-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 20px;
  }
  .snap-tag {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.75);
  }
  .snap-meta {
    font-size: 10px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.35);
  }
</style>
