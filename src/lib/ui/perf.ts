// v2.15 性能策略中枢（1500 节点承载力 + 未来扩展预留）：
// - 所有「按规模降级」的阈值集中在此表——未来新增降级项 = 加字段 + 接入点读策略，不撒魔法数字；
// - createFrameMonitor：rAF FPS/帧耗时监控（诊断浮层与自动化验证共用）；
// - fnv1a：图快照签名哈希（大图 $effect 不再拼巨型字符串）。

export interface PerfPolicy {
  /** 边渐变上限：超过此边数全部走实线（canvas 渐变纹理是大图平移缩放的大头） */
  gradientEdgeLimit: number;
  /** 标签最小渲染字号（cytoscape min-zoomed-font-size：缩到更小就不画标签，内建裁剪） */
  minZoomedFont: number;
  /** 涟漪最大层深（密集图 BFS 6 层≈全图，降深省类与环数） */
  rippleMaxDepth: number;
  /** 力导向最大迭代数（网格近似版；节点越多代数越少，保证布局预算不爆炸） */
  forceMaxIter: number;
  /** 呼吸样式更新频率（Hz）：慢正弦，低到 15Hz 视觉无差 */
  breathHz: number;
  /** 位置缓存重建节流（每 N 帧重建一次，平移/缩放期间省分配） */
  posCacheFrames: number;
}

interface Tier {
  maxNodes: number;
  p: PerfPolicy;
}

const TIERS: Tier[] = [
  {
    maxNodes: 400,
    p: { gradientEdgeLimit: 300, minZoomedFont: 6, rippleMaxDepth: 6, forceMaxIter: 400, breathHz: 30, posCacheFrames: 1 },
  },
  {
    maxNodes: 800,
    p: { gradientEdgeLimit: 300, minZoomedFont: 8, rippleMaxDepth: 5, forceMaxIter: 200, breathHz: 30, posCacheFrames: 1 },
  },
  {
    maxNodes: 1400,
    p: { gradientEdgeLimit: 0, minZoomedFont: 10, rippleMaxDepth: 4, forceMaxIter: 120, breathHz: 20, posCacheFrames: 2 },
  },
  {
    maxNodes: Infinity,
    p: { gradientEdgeLimit: 0, minZoomedFont: 12, rippleMaxDepth: 3, forceMaxIter: 80, breathHz: 15, posCacheFrames: 2 },
  },
];

/** 按节点数选档（边数已并入阈值字段；未来可按 edges 细分） */
export function perfPolicy(nodes: number): PerfPolicy {
  for (const t of TIERS) {
    if (nodes <= t.maxNodes) return t.p;
  }
  return TIERS[TIERS.length - 1].p;
}

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
