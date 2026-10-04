// 三维图谱时代的性能/工具模块：
// - createFrameMonitor：rAF FPS/帧耗时监控（诊断浮层与自动化验证共用）；
// - fnv1a：三维视图确定性微扰（枝条抖动）与快照签名哈希。
// 说明：2D cytoscape 时代的 PerfPolicy/TIERS（按规模降级表）已随 2D 机械删除——
// 3D 的分级策略集中在 Graph3D.svelte（bloom 仅 ≤1200 节点、力导向 tick 数按规模分档）。

/** 当前档位名（诊断浮层展示） */
export function perfTierName(nodes: number): string {
  if (nodes <= 400) return 's';
  if (nodes <= 800) return 'm';
  if (nodes <= 1400) return 'l';
  return 'xl';
}

/** FNV-1a 32 位哈希（图快照签名：避免 1500 节点 sort+join 巨型字符串与 GC 压力） */
export function fnv1a(str: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = (h * 0x01000193) >>> 0;
  }
  return h >>> 0;
}

export interface FrameStats {
  fps: number;
  avgMs: number;
  p95Ms: number;
  maxMs: number;
}

export interface FrameMonitor {
  /** 取最近窗口的统计（自上次 take 后持续采样） */
  take: () => FrameStats;
  reset: () => void;
  dispose: () => void;
}

/** rAF 帧监控：常驻采样最近 120 帧，take() 返回统计并重置（诊断浮层/自动化验证共用） */
export function createFrameMonitor(): FrameMonitor {
  let raf = 0;
  const deltas: number[] = [];
  let last = performance.now();
  const tick = (now: number) => {
    const d = now - last;
    last = now;
    if (d > 0 && d < 500) {
      deltas.push(d);
      if (deltas.length > 120) deltas.shift();
    }
    raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);
  return {
    take() {
      if (deltas.length === 0) return { fps: 0, avgMs: 0, p95Ms: 0, maxMs: 0 };
      const arr = [...deltas].sort((a, b) => a - b);
      const avg = arr.reduce((s, d) => s + d, 0) / arr.length;
      return {
        fps: Math.round((1000 / avg) * 10) / 10,
        avgMs: Math.round(avg * 100) / 100,
        p95Ms: Math.round(arr[Math.floor(arr.length * 0.95)] * 100) / 100,
        maxMs: Math.round(arr[arr.length - 1] * 100) / 100,
      };
    },
    reset() {
      deltas.length = 0;
      last = performance.now();
    },
    dispose() {
      cancelAnimationFrame(raf);
    },
  };
}
