<script lang="ts">
  import type { ScanMode, WorkspaceInfo } from '../lib/types';

  let {
    workspaces,
    mode,
    currentDir,
    busy,
    error,
    onSwitchMode,
    onOpen,
    onAdd,
    onRemove,
  }: {
    workspaces: WorkspaceInfo[];
    mode: ScanMode;
    currentDir: string | null;
    busy: boolean;
    error: string | null;
    onSwitchMode: (m: ScanMode) => void;
    onOpen: (ws: WorkspaceInfo) => void;
    onAdd: () => void;
    onRemove: (dir: string) => void;
  } = $props();

  // v2.1 左侧栏布局状态（模块级：组件重挂载仍保留）
  const panel = $state({ width: 240, collapsed: false });

  let layerWorkspaces = $derived(workspaces.filter((w) => w.mode === mode));
  let modeCounts = $derived({
    analysis: workspaces.filter((w) => w.mode === 'analysis').length,
    dev: workspaces.filter((w) => w.mode === 'dev').length,
  });

  function resizeWidth(e: PointerEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = panel.width;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const w = Math.min(Math.max(startW + (ev.clientX - startX), 180), 420);
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
</script>

{#if panel.collapsed}
  <aside class="ws-sidebar collapsed" aria-label="工作区侧栏（已折叠）">
    <button class="expand-btn" onclick={() => (panel.collapsed = false)} title="展开工作区栏">»</button>
  </aside>
{:else}
  <aside class="ws-sidebar" style:width="{panel.width}px" aria-label="工作区侧栏">
    <div class="width-handle" role="separator" aria-orientation="vertical" onpointerdown={resizeWidth} title="拖拽调整侧栏宽度"></div>

    <header>
      <span class="ws-title">工作区</span>
      <button class="collapse-btn" onclick={() => (panel.collapsed = true)} title="折叠工作区栏">«</button>
    </header>

    <!-- v2.1 两层：分析 / 开发 模式页签（切换后默认打开该层第一个工作区） -->
    <div class="mode-tabs" role="tablist" aria-label="工作区模式">
      <button type="button" class="mode-tab" class:active={mode === 'analysis'} role="tab"
              aria-selected={mode === 'analysis'} onclick={() => onSwitchMode('analysis')}>
        分析<span class="count">{modeCounts.analysis}</span>
      </button>
      <button type="button" class="mode-tab" class:active={mode === 'dev'} role="tab"
              aria-selected={mode === 'dev'} onclick={() => onSwitchMode('dev')}>
        开发<span class="count">{modeCounts.dev}</span>
      </button>
    </div>

    <div class="ws-list">
      {#if layerWorkspaces.length === 0}
        <div class="ws-empty">
          {mode === 'analysis' ? '还没有分析模式工作区' : '还没有开发模式工作区'}
          <p class="ws-empty-sub">点下方「＋ 添加工作区」<br />当前页签的模式即新工作区的模式</p>
        </div>
      {:else}
        {#each layerWorkspaces as ws (ws.path)}
          <div class="ws-row" class:active={ws.path === currentDir} role="button" tabindex="0"
               aria-label="打开工作区 {ws.name}"
               onclick={() => onOpen(ws)}
               onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onOpen(ws); } }}
               title={ws.path}>
            <span class="ws-name">{ws.name}</span>
            <span class="ws-path">{ws.path.length > 46 ? '…' + ws.path.slice(-45) : ws.path}</span>
            <button class="ws-remove" title="从列表移除（不删除磁盘文件）"
                    onclick={(e) => { e.stopPropagation(); onRemove(ws.path); }}>✕</button>
          </div>
        {/each}
      {/if}
    </div>

    <footer>
      <button class="ws-add" onclick={onAdd} disabled={busy}>
        {busy ? '处理中…' : '＋ 添加工作区'}
      </button>
      {#if error}
        <p class="ws-error">⚠ {error}</p>
      {/if}
      <p class="ws-tip">添加到当前页签的模式层；模式标签写入 .chain/.mode，随工程走</p>
    </footer>
  </aside>
{/if}

<style>
  .ws-sidebar {
    position: relative;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: #0f0f11;
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    min-width: 180px;
    max-width: 420px;
  }
  .ws-sidebar.collapsed {
    min-width: 44px;
    width: 44px;
    align-items: center;
  }
  .expand-btn, .collapse-btn {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.4);
    font-size: 14px;
    cursor: pointer;
    padding: 8px;
  }
  .expand-btn { margin-top: 8px; }
  .expand-btn:hover, .collapse-btn:hover { color: rgba(255, 255, 255, 0.9); }

  .width-handle {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 5px;
    cursor: col-resize;
    z-index: 2;
  }
  .width-handle:hover { background: rgba(255, 255, 255, 0.12); }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 12px 8px 16px;
    flex-shrink: 0;
  }
  .ws-title {
    font-size: 11px;
    letter-spacing: 2px;
    color: rgba(255, 255, 255, 0.5);
    font-weight: 500;
  }

  .mode-tabs {
    display: flex;
    margin: 0 12px 10px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    overflow: hidden;
    flex-shrink: 0;
  }
  .mode-tab {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 7px 0;
    font-size: 12px;
    font-family: inherit;
    color: rgba(255, 255, 255, 0.5);
    background: transparent;
    border: none;
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
  }
  .mode-tab + .mode-tab { border-left: 1px solid rgba(255, 255, 255, 0.12); }
  .mode-tab:hover { color: rgba(255, 255, 255, 0.9); }
  .mode-tab.active { background: rgba(255, 255, 255, 0.14); color: #fff; }
  .count {
    font-size: 10px;
    font-family: 'Consolas', monospace;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 999px;
    padding: 0 7px;
    line-height: 16px;
  }

  .ws-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 8px;
  }
  .ws-empty {
    padding: 18px 10px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.35);
    text-align: center;
  }
  .ws-empty-sub {
    margin: 8px 0 0;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.22);
    line-height: 1.6;
  }
  .ws-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 7px;
    cursor: pointer;
    transition: background 0.12s ease;
    position: relative;
  }
  .ws-row:hover { background: rgba(255, 255, 255, 0.06); }
  .ws-row.active { background: rgba(255, 255, 255, 0.11); }
  .ws-name {
    font-size: 12.5px;
    color: rgba(255, 255, 255, 0.88);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
  }
  .ws-path {
    font-size: 9.5px;
    font-family: 'Consolas', monospace;
    color: rgba(255, 255, 255, 0.3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .ws-remove {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    font-size: 9px;
    color: rgba(255, 255, 255, 0.25);
    background: none;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    opacity: 0;
    transition: all 0.12s ease;
  }
  .ws-row:hover .ws-remove { opacity: 1; }
  .ws-remove:hover { color: #f87171; background: rgba(248, 113, 113, 0.15); }

  footer {
    flex-shrink: 0;
    padding: 10px 12px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }
  .ws-add {
    width: 100%;
    padding: 8px 0;
    font-size: 12px;
    font-family: inherit;
    color: #7dd3fc;
    background: rgba(125, 211, 252, 0.08);
    border: 1px dashed rgba(125, 211, 252, 0.35);
    border-radius: 7px;
    cursor: pointer;
    transition: background 0.15s ease;
  }
  .ws-add:hover:not(:disabled) { background: rgba(125, 211, 252, 0.16); }
  .ws-add:disabled { opacity: 0.4; cursor: not-allowed; }
  .ws-error {
    margin: 8px 0 0;
    font-size: 10.5px;
    color: #f87171;
    line-height: 1.5;
    word-break: break-all;
  }
  .ws-tip {
    margin: 8px 0 0;
    font-size: 9.5px;
    color: rgba(255, 255, 255, 0.25);
    line-height: 1.6;
  }
</style>
