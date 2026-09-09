<script lang="ts">
  // v2.15 性能浮层：FPS/帧耗时实时监控 + 规模分档展示。
  // 数据来自 lib/ui/perf.ts 的 rAF 帧监控；工具栏 ⚡ 开关。
  import { onMount } from 'svelte';
  import { createFrameMonitor } from '../lib/ui/perf';

  let { nodes = 0, edges = 0, tier = 's' }: {
    nodes?: number;
    edges?: number;
    tier?: string;
  } = $props();

  let stats = $state({ fps: 0, avgMs: 0, p95Ms: 0, maxMs: 0 });
  let timer: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    const mon = createFrameMonitor();
    timer = setInterval(() => {
      stats = mon.take();
    }, 600);
    return () => {
      clearInterval(timer);
      mon.dispose();
    };
  });
</script>

<div class="perf-overlay" aria-label="性能浮层">
  <span
    class="po-fps"
    class:good={stats.fps >= 50}
    class:warn={stats.fps >= 30 && stats.fps < 50}
    class:bad={stats.fps > 0 && stats.fps < 30}
  >
    {stats.fps > 0 ? stats.fps : '…'} fps
  </span>
  <span class="po-row">帧 {stats.avgMs}ms · p95 {stats.p95Ms}ms · 峰 {stats.maxMs}ms</span>
  <span class="po-row">节点 {nodes} · 边 {edges} · 性能档 {tier}</span>
</div>

<style>
  .perf-overlay {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: rgba(10, 12, 18, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 10px;
    padding: 8px 12px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.75);
    pointer-events: none;
    backdrop-filter: blur(6px);
  }
  .po-fps {
    font-size: 15px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.92);
  }
  .po-fps.good { color: #34d399; }
  .po-fps.warn { color: #fbbf24; }
  .po-fps.bad { color: #f87171; }
  .po-row { opacity: 0.85; }
</style>
