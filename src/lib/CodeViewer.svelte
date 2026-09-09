<script lang="ts">
  // v2.18 代码骨架新窗口阅读器：独立 Tauri 窗口（?view=code&ws=..&node=..），
  // 全页大字体展示骨架（Mermaid 图 + 接口清单 + 调用边），完整滚动——解决侧栏面板太小看不清。
  import { invoke } from '@tauri-apps/api/core';
  import { renderBody } from './body_render';

  let { ws, nodeId }: { ws: string; nodeId: string } = $props();

  let md = $state<string | null>(null);
  let err = $state<string | null>(null);
  let mermaidHtml = $state<string | null>(null);
  let codePath = $state('');

  $effect(() => {
    if (!ws || !nodeId) {
      err = '缺少参数（ws/node）';
      return;
    }
    md = null;
    err = null;
    mermaidHtml = null;
    invoke<string | null>('get_code_map', { dir: ws, nodeId })
      .then((m) => {
        if (!m) {
          err = '该节点没有代码骨架（未挂载 code_map 或骨架未生成）';
          return;
        }
        md = m;
        const mm = m.match(/```mermaid\n([\s\S]*?)\n```/);
        if (mm) {
          import('mermaid')
            .then((mod) => {
              mod.default.initialize({ startOnLoad: false, theme: 'dark', securityLevel: 'loose' });
              return mod.default.render(`codeview-mermaid-${Date.now()}`, mm[1].trim());
            })
            .then(({ svg }) => (mermaidHtml = svg))
            .catch(() => (mermaidHtml = null));
        }
      })
      .catch((e) => (err = String(e)));
  });

  // 正文去掉 mermaid 围栏（图已单独渲染），其余 Markdown 全量渲染
  let bodyHtml = $derived(md ? renderBody(md.replace(/```mermaid\n[\s\S]*?\n```\n?/, '')) : '');
</script>

<div class="codeview-page">
  <header class="cv-header">
    <div class="cv-title">
      <span class="cv-badge">M-Code</span>
      <h1>代码骨架</h1>
      <span class="cv-node">{nodeId}</span>
    </div>
    <span class="cv-ws" title={ws}>工作区：{ws}</span>
  </header>

  {#if err}
    <div class="cv-error">⚠ {err}</div>
  {:else if md === null}
    <div class="cv-loading">加载骨架中…</div>
  {:else}
    <main class="cv-main">
      {#if mermaidHtml}
        <section class="cv-section">
          <h2>调用关系图</h2>
          <div class="cv-mermaid">{@html mermaidHtml}</div>
        </section>
      {/if}
      <div class="cv-body">{@html bodyHtml}</div>
    </main>
  {/if}
</div>

<style>
  .codeview-page {
    min-height: 100vh;
    padding: 28px 36px 60px;
    box-sizing: border-box;
    background: #0d1117;
    color: rgba(255, 255, 255, 0.88);
    font-family: 'Segoe UI', 'Microsoft YaHei', system-ui, sans-serif;
  }
  .cv-header {
    display: flex;
    align-items: baseline;
    gap: 16px;
    justify-content: space-between;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    padding-bottom: 14px;
    margin-bottom: 22px;
  }
  .cv-title {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .cv-badge {
    font-size: 12px;
    color: #34d399;
    border: 1px solid rgba(52, 211, 153, 0.5);
    border-radius: 999px;
    padding: 2px 10px;
  }
  .cv-header h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
  }
  .cv-node {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.55);
    font-family: 'Consolas', monospace;
  }
  .cv-ws {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.45);
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cv-error,
  .cv-loading {
    margin-top: 60px;
    text-align: center;
    color: rgba(255, 255, 255, 0.6);
    font-size: 15px;
  }
  .cv-error {
    color: #f87171;
  }
  .cv-main {
    max-width: 1200px;
    margin: 0 auto;
  }
  .cv-section {
    margin-bottom: 24px;
  }
  .cv-section h2 {
    font-size: 15px;
    color: rgba(255, 255, 255, 0.7);
    margin: 0 0 10px;
  }
  .cv-mermaid {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 14px;
    overflow: auto;
  }
  .cv-mermaid :global(svg) {
    max-width: none;
    height: auto;
  }
  /* 骨架正文：大字号、清晰的代码块 */
  .cv-body :global(h1) {
    font-size: 20px;
  }
  .cv-body :global(h2) {
    font-size: 17px;
    color: rgba(255, 255, 255, 0.85);
    border-left: 3px solid #34d399;
    padding-left: 10px;
    margin-top: 28px;
  }
  .cv-body :global(blockquote) {
    color: rgba(255, 255, 255, 0.6);
    border-left: 3px solid rgba(255, 255, 255, 0.2);
    margin: 10px 0;
    padding-left: 12px;
  }
  .cv-body :global(li) {
    margin: 5px 0;
    line-height: 1.6;
  }
  .cv-body :global(pre) {
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 14px 16px;
    overflow-x: auto;
  }
  .cv-body :global(code) {
    font-family: 'Consolas', 'Cascadia Code', monospace;
    font-size: 13px;
    color: #9eceff;
  }
  .cv-body :global(p code),
  .cv-body :global(li code) {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    padding: 1px 5px;
  }
  .cv-body :global(a) {
    color: #7dd3fc;
  }
</style>
