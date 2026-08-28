// v2.2 涟漪视图（开发模式专属）：水波纹传播的核心纯逻辑。
// 联系强弱 = 与主节点的层级距离（BFS，无向——知识链接不分方向，直接相关=强）。
// 波深上限 6 层；不连通部分不进波内。

export interface RippleLayers {
  /** 主节点 id */
  source: string;
  /** 节点 id → 层级深度（0=主节点，1=直接相关，逐级 +1） */
  depth: Map<string, number>;
  /** 每层的节点 id 列表（按层分组，便于批量加类） */
  byDepth: string[][];
  /** 波前边：连接第 i 层与第 i-1 层的边 id，minDepth = 浅端层级（i-1） */
  edges: Map<string, number>;
}

export const RIPPLE_MAX_DEPTH = 6;

/** 亮度（opacity）按层级单调递减：点击的节点最亮，直接相关次之，逐级降低 */
export function rippleOpacity(depth: number): number {
  const table = [1.0, 0.85, 0.72, 0.6, 0.5, 0.42, 0.35];
  return depth < table.length ? table[depth] : 0.28;
}

/** 呼吸脉动幅度：层级越近越强 */
export function ripplePulseAmp(depth: number): number {
  const table = [0, 0.1, 0.07, 0.05, 0.035, 0.025, 0.02];
  return depth < table.length ? table[depth] : 0.015;
}

/** 脉动/脉冲频率（Hz）：层级越近越快 */
export function rippleFreq(depth: number): number {
  const table = [0, 2.2, 1.8, 1.5, 1.2, 1.0, 0.85];
  return depth < table.length ? table[depth] : 0.7;
}

/**
 * 以 source 为圆心做无向 BFS 分层。
 * @param adjacency 邻接表（节点 id → 相邻节点 id 列表，无向）
 * @returns 各层节点与波前边（edgeKey = `${a}->${b}` 由调用方按自己的边 id 规则匹配）
 */
export function computeRippleLayers(
  adjacency: Map<string, string[]>,
  source: string,
  maxDepth: number = RIPPLE_MAX_DEPTH,
): RippleLayers {
  const depth = new Map<string, number>();
  const byDepth: string[][] = [];
  depth.set(source, 0);
  byDepth.push([source]);

  const frontier: string[] = [source];
  for (let d = 1; d <= maxDepth; d++) {
    const next: string[] = [];
    const seen = new Set<string>(depth.keys());
    for (const id of frontier) {
      for (const nb of adjacency.get(id) ?? []) {
        if (!seen.has(nb)) {
          seen.add(nb);
          depth.set(nb, d);
          next.push(nb);
        }
      }
    }
    if (next.length === 0) break;
    byDepth.push(next);
    frontier.length = 0;
    frontier.push(...next);
  }

  // 波前边：两端都在波内，且层级差为 1；归属浅端层级
  const edges = new Map<string, number>();
  for (const [a, nbs] of adjacency) {
    const da = depth.get(a);
    if (da === undefined) continue;
    for (const b of nbs) {
      const db = depth.get(b);
      if (db === undefined) continue;
      if (Math.abs(da - db) === 1) {
        const shallow = Math.min(da, db);
        const key = a < b ? `${a}->${b}` : `${b}->${a}`;
        if (!edges.has(key) || (edges.get(key) ?? 99) > shallow) {
          edges.set(key, shallow);
        }
      }
    }
  }

  return { source, depth, byDepth, edges };
}
