<script lang="ts">
  // 三层重构 P3 · 对话阅读面（只读；人用，不进 __engramDebug、不注册 MCP 工具、不写文件）。
  // 数据源：get_dialogue（记忆层 dialogue_log 的结构化账本）→ 渲染期投影为 Markdown。
  // 不变量：不落盘第二格式（JSONL 是唯一事实）；本视图只读。
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { renderBody } from './body_render';

  interface Props {
    chainDir: string | null;
    onExit: () => void;
  }
  let { chainDir, onExit }: Props = $props();

  interface Head { session: string; model: string | null; guide: string; started: string }
  interface Record {
    line: number; kind: 'msg' | 'tool' | 'decision'; seq: number | null; session: string;
    ts: string; role: 'user' | 'assistant' | null; text: string; part: number; parts: number;
    tool_name: string | null; tool_args: string | null;
    decided: 'keep' | 'skip' | 'revise' | null; covers: [number, number] | null; nodes: string[];
  }
  interface Ledger {
    file: string; exists: boolean; heads: Head[]; records: Record[];
    malformed: { line: number; reason: string }[];
    unconsumed_from: number | null; last_covered_to: number | null;
  }

  let data = $state<Ledger | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let query = $state('');
  let sessionSel = $state<string>('all');

  onMount(async () => {
    try {
      data = await invoke<Ledger>('get_dialogue', { dir: chainDir });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  const sessions = $derived(
    (data?.heads ?? []).map((h) => h.session)
  );

  const filtered = $derived.by(() => {
    const rs = data?.records ?? [];
    const q = query.trim().toLowerCase();
    return rs.filter((r) => {
      if (sessionSel !== 'all' && r.session !== sessionSel) return false;
      if (!q) return true;
      return (
        (r.text ?? '').toLowerCase().includes(q) ||
        (r.tool_name ?? '').toLowerCase().includes(q)
      );
    });
  });

  // 同一逻辑消息（seq + part）合并为一条渲染，避免拆行碎片打断阅读
  const merged = $derived.by(() => {
    const out: { seq: number; session: string; ts: string; role: 'user' | 'assistant'; text: string }[] = [];
    for (const r of filtered) {
      if (r.kind !== 'msg') continue;
      const last = out[out.length - 1];
      if (last && last.seq === r.seq && r.part > 1) {
        last.text += r.text;
      } else if (r.role) {
        out.push({ seq: r.seq ?? 0, session: r.session, ts: r.ts, role: r.role, text: r.text });
      }
    }
    return out;
  });

  const modelOf = (s: string) => data?.heads.find((h) => h.session === s)?.model ?? null;
  const guideOf = (s: string) => data?.heads.find((h) => h.session === s)?.guide ?? '';

  const isUnconsumed = (seq: number | null) =>
    seq !== null && data?.unconsumed_from !== null && seq >= (data?.unconsumed_from ?? 0);

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onExit();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="dlg-mask">
  <div class="dlg">
    <header class="dlg-head">
      <span class="dlg-title">💬 对话 · 工作过程（只读）</span>
      <span class="dlg-meta">
        {#if data?.exists}
          {data.records.length} 条记录 · {sessions.length} 个会话
          · 未消费起点 {data.unconsumed_from ?? '—'}
        {:else}
          （本工作区还没有对话账本）
        {/if}
      </span>
      <button class="dlg-close" onclick={onExit} title="关闭（Esc）">✕</button>
    </header>

    {#if loading}
      <div class="dlg-empty">加载中…</div>
    {:else if error}
      <div class="dlg-empty err">{error}</div>
    {:else if data}
      <div class="dlg-body">
        <aside class="dlg-sessions">
          <div class="dlg-sess-title">会话</div>
          <button class="dlg-sess" class:sel={sessionSel === 'all'} onclick={() => (sessionSel = 'all')}>
            全部
          </button>
          {#each sessions as s (s)}
            <button class="dlg-sess" class:sel={sessionSel === s} onclick={() => (sessionSel = s)} title={guideOf(s)}>
              {s}<span class="dlg-sess-model">{modelOf(s) ?? ''}</span>
            </button>
          {/each}
        </aside>

        <main class="dlg-feed">
          <div class="dlg-search">
            <input bind:value={query} placeholder="检索对话（内容/工具名/决策理由）…" />
          </div>

          {#if data.malformed.length > 0}
            <div class="dlg-warn">
              ⚠ 账本有 {data.malformed.length} 条坏行（对话不可再生，修复前先备份）：
              {#each data.malformed.slice(0, 3) as m (m.line)}
                <div class="dlg-warn-line">L{m.line}：{m.reason.slice(0, 120)}</div>
              {/each}
            </div>
          {/if}

          {#each merged as m (m.seq + '-' + m.session)}
            <article class="dlg-msg" class:unconsumed={isUnconsumed(m.seq)}>
              <div class="dlg-msg-head">
                <span class="dlg-role {m.role}">{m.role === 'user' ? '你' : 'AI'}</span>
                <span class="dlg-sess-tag">{m.session}</span>
                <span class="dlg-ts">{m.ts}</span>
                {#if isUnconsumed(m.seq)}
                  <span class="dlg-new">未消费</span>
                {/if}
              </div>
              <div class="dlg-md">{@html renderBody(m.text)}</div>
            </article>
          {/each}

          {#each filtered.filter((r) => r.kind === 'tool') as r (r.line)}
            <div class="dlg-tool" class:unconsumed={isUnconsumed(r.seq)}>
              🛠 <b>{r.tool_name}</b>
              {#if r.tool_args}<code>{r.tool_args.slice(0, 120)}</code>{/if}
              {#if r.text}<span class="dlg-tool-result">{r.text.slice(0, 200)}</span>{/if}
              <span class="dlg-ts">{r.ts}</span>
            </div>
          {/each}

          {#each filtered.filter((r) => r.kind === 'decision') as r (r.line)}
            <div class="dlg-dec" class:unconsumed={isUnconsumed(r.seq)}>
              <span class="dlg-dec-badge {r.decided ?? ''}">
                {r.decided === 'keep' ? '保留' : r.decided === 'skip' ? '不保留' : r.decided === 'revise' ? '修订' : '决策'}
              </span>
              {#if r.covers}<span class="dlg-dec-covers">对话 seq {r.covers[0]}–{r.covers[1]}</span>{/if}
              {#if r.nodes.length > 0}<span class="dlg-dec-nodes">→ {r.nodes.join(', ')}</span>{/if}
              <span class="dlg-dec-reason">{r.text}</span>
              <span class="dlg-ts">{r.ts}</span>
            </div>
          {/each}

          {#if merged.length === 0 && filtered.length === 0}
            <div class="dlg-empty">没有匹配的记录</div>
          {/if}
        </main>
      </div>
    {/if}
  </div>
</div>

<style>
  .dlg-mask {
    position: fixed; inset: 0; z-index: 2600; background: rgba(2, 6, 18, 0.92);
    display: flex; align-items: stretch; justify-content: stretch;
  }
  .dlg { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .dlg-head {
    display: flex; align-items: center; gap: 12px; padding: 10px 16px;
    border-bottom: 1px solid rgba(148, 163, 184, 0.18); color: #e2e8f0;
  }
  .dlg-title { font-weight: 700; }
  .dlg-meta { color: #94a3b8; font-size: 12px; flex: 1; }
  .dlg-close {
    background: none; border: 1px solid rgba(148, 163, 184, 0.3); color: #cbd5e1;
    border-radius: 6px; padding: 2px 10px; cursor: pointer;
  }
  .dlg-body { flex: 1; display: flex; min-height: 0; }
  .dlg-sessions {
    width: 220px; border-right: 1px solid rgba(148, 163, 184, 0.18); padding: 10px;
    overflow-y: auto; display: flex; flex-direction: column; gap: 4px;
  }
  .dlg-sess-title { color: #64748b; font-size: 11px; margin-bottom: 4px; }
  .dlg-sess {
    text-align: left; background: none; border: 1px solid transparent; color: #cbd5e1;
    border-radius: 6px; padding: 5px 8px; cursor: pointer; font-size: 12px;
    display: flex; flex-direction: column; gap: 2px;
  }
  .dlg-sess.sel { background: rgba(96, 165, 250, 0.15); border-color: rgba(96, 165, 250, 0.4); }
  .dlg-sess-model { color: #64748b; font-size: 10px; }
  .dlg-feed { flex: 1; overflow-y: auto; padding: 14px 18px; }
  .dlg-search input {
    width: 100%; background: rgba(148, 163, 184, 0.08); border: 1px solid rgba(148, 163, 184, 0.25);
    border-radius: 8px; color: #e2e8f0; padding: 8px 12px; margin-bottom: 12px;
  }
  .dlg-msg {
    background: rgba(148, 163, 184, 0.05); border: 1px solid rgba(148, 163, 184, 0.12);
    border-radius: 10px; padding: 10px 14px; margin-bottom: 10px;
  }
  .dlg-msg.unconsumed, .dlg-tool.unconsumed, .dlg-dec.unconsumed {
    border-left: 3px solid #fbbf24;
  }
  .dlg-msg-head { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
  .dlg-role { font-weight: 700; font-size: 12px; padding: 1px 8px; border-radius: 10px; }
  .dlg-role.user { background: rgba(96, 165, 250, 0.2); color: #93c5fd; }
  .dlg-role.assistant { background: rgba(52, 211, 153, 0.18); color: #6ee7b7; }
  .dlg-sess-tag { color: #64748b; font-size: 11px; }
  .dlg-ts { color: #475569; font-size: 10px; }
  .dlg-new { color: #fbbf24; font-size: 10px; border: 1px solid rgba(251, 191, 36, 0.5); border-radius: 8px; padding: 0 6px; }
  .dlg-md { color: #cbd5e1; font-size: 13px; line-height: 1.7; }
  .dlg-tool {
    font-size: 12px; color: #94a3b8; background: rgba(148, 163, 184, 0.04);
    border: 1px solid rgba(148, 163, 184, 0.1); border-radius: 8px;
    padding: 6px 10px; margin-bottom: 6px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap;
  }
  .dlg-tool code { color: #7dd3fc; font-size: 11px; }
  .dlg-tool-result { color: #64748b; }
  .dlg-dec {
    font-size: 12px; color: #94a3b8; border-radius: 8px; border: 1px dashed rgba(148, 163, 184, 0.25);
    padding: 6px 10px; margin-bottom: 6px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap;
  }
  .dlg-dec-badge { font-weight: 700; padding: 0 8px; border-radius: 8px; font-size: 11px; }
  .dlg-dec-badge.keep { background: rgba(52, 211, 153, 0.18); color: #6ee7b7; }
  .dlg-dec-badge.skip { background: rgba(148, 163, 184, 0.18); color: #94a3b8; }
  .dlg-dec-badge.revise { background: rgba(251, 191, 36, 0.18); color: #fcd34d; }
  .dlg-dec-covers, .dlg-dec-nodes { color: #64748b; font-size: 11px; }
  .dlg-dec-reason { color: #cbd5e1; flex: 1; }
  .dlg-warn {
    background: rgba(251, 191, 36, 0.08); border: 1px solid rgba(251, 191, 36, 0.35);
    border-radius: 8px; color: #fcd34d; font-size: 12px; padding: 8px 12px; margin-bottom: 10px;
  }
  .dlg-warn-line { color: #d6b56b; font-size: 11px; margin-top: 3px; }
  .dlg-empty { padding: 40px; color: #64748b; text-align: center; }
  .dlg-empty.err { color: #f87171; }
</style>
