import type { ChainNode, NodeType } from './types';

// v2.19 阅读模式（人专用）：把「图结构」还原成「文件树」的纯逻辑层。
//
// 设计约束（与 ARCHITECTURE.md 一致）：
// - 纯派生：只读 ChainSnapshot（事实源的投影），不写任何文件、不发 IPC、不进 MCP——
//   树只在 GUI 内存里存在，AI 不可识别、不可使用（详见 ReaderMode.svelte 头部说明）。
// - 无副作用、可无头复算：输入 nodes + 根 id，输出稳定的树，顺序确定性（同数据同结果）。
//
// 树从哪来：链协议的父子关系由每个节点自己的 `parent` 字段承载（walker 据此派生 edges），
// 因此这里以 `parent` 为准构图——活跃节点与归档节点同一套规则，且天然是「单父树」。
// 开发模式允许环与悬空引用，本模块必须容错：环 → 提升为根并标记，悬空 parent → 视为根并标记。

/** 同层排序权重：读一份链时按「目标 → 设计 → 任务 → 验证 → 笔记」的叙事顺序 */
export const NODE_TYPE_ORDER: Record<NodeType, number> = {
  goal: 0,
  design: 1,
  task: 2,
  verification: 3,
  note: 4,
};

/** 树节点（一个节点在树中只出现一次——环与多父引用不会重复展开） */
export interface TreeEntry {
  node: ChainNode;
  /** 0 = 根层 */
  depth: number;
  children: TreeEntry[];
  /** 父节点引用存在但父节点不在集合里（悬空 parent：只读展示，不修数据） */
  brokenParent: string | null;
  /** 因环路被提升为根（父链成环，开发模式可能） */
  cycleBreak: boolean;
}

export interface NodeTree {
  roots: TreeEntry[];
  /** id → entry（构树后的唯一索引） */
  index: Map<string, TreeEntry>;
  /** id → 祖先链（根在前、不含自身） */
  ancestors: Map<string, TreeEntry[]>;
  stats: {
    total: number;
    roots: number;
    /** 悬空 parent 的节点数 */
    broken: number;
    /** 因环路被提升为根的节点数 */
    cycles: number;
  };
}

export interface BuildTreeOptions {
  /** 优先作为首个根的 id（链协议的 manifest.root） */
  preferredRoot?: string | null;
}

/** 同层排序：类型序 → 创建时间 → id（稳定、可复现） */
function compareSiblings(a: ChainNode, b: ChainNode): number {
  const ta = NODE_TYPE_ORDER[a.type] ?? 9;
  const tb = NODE_TYPE_ORDER[b.type] ?? 9;
  if (ta !== tb) return ta - tb;
  const ca = a.created ?? '';
  const cb = b.created ?? '';
  if (ca !== cb) return ca < cb ? -1 : 1;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/**
 * 图结构 → 文件树。
 * 复杂度 O(n log n)（同层排序），1500 节点实测量级 < 10ms。
 */
export function buildNodeTree(nodes: ChainNode[], opts: BuildTreeOptions = {}): NodeTree {
  const byId = new Map<string, ChainNode>();
  for (const n of nodes) byId.set(n.id, n);

  const index = new Map<string, TreeEntry>();
  const ancestors = new Map<string, TreeEntry[]>();
  const childrenOf = new Map<string, ChainNode[]>();
  const rootNodes: ChainNode[] = [];
  let broken = 0;

  for (const n of nodes) {
    const p = n.parent;
    // 自指（parent === id）当作环处理：进根候选，不挂到自己下面
    if (p && p !== n.id && byId.has(p)) {
      const arr = childrenOf.get(p);
      if (arr) arr.push(n);
      else childrenOf.set(p, [n]);
    } else {
      rootNodes.push(n);
      if (p && p !== n.id) broken += 1;
    }
  }

  const preferred = opts.preferredRoot ?? null;
  const orderRoots = (list: ChainNode[]) =>
    list.slice().sort((a, b) => {
      if (preferred) {
        if (a.id === preferred) return -1;
        if (b.id === preferred) return 1;
      }
      return compareSiblings(a, b);
    });

  let cycles = 0;

  // 迭代式 DFS（1500 节点深链不爆栈）：显式栈 + 祖先路径检测环
  const makeEntry = (node: ChainNode, depth: number, path: TreeEntry[], cycleBreak: boolean, brokenParent: string | null): TreeEntry => {
    const entry: TreeEntry = { node, depth, children: [], brokenParent, cycleBreak };
    index.set(node.id, entry);
    if (path.length > 0) ancestors.set(node.id, path.slice());
    return entry;
  };

  const buildFrom = (rootNode: ChainNode, cycleBreak: boolean, brokenParent: string | null): TreeEntry => {
    const rootEntry = makeEntry(rootNode, 0, [], cycleBreak, brokenParent);
    const stack: { entry: TreeEntry; kids: ChainNode[]; i: number; path: TreeEntry[] }[] = [
      { entry: rootEntry, kids: (childrenOf.get(rootNode.id) ?? []).slice().sort(compareSiblings), i: 0, path: [rootEntry] },
    ];
    while (stack.length > 0) {
      const top = stack[stack.length - 1];
      if (top.i >= top.kids.length) {
        stack.pop();
        continue;
      }
      const child = top.kids[top.i++];
      // 环：该子节点已在当前路径上（自指链）或已就位（多父/重复引用）→ 不重复展开
      if (index.has(child.id)) {
        cycles += 1;
        continue;
      }
      const childEntry = makeEntry(child, top.entry.depth + 1, top.path, false, null);
      top.entry.children.push(childEntry);
      const path = top.path.concat(childEntry);
      stack.push({
        entry: childEntry,
        kids: (childrenOf.get(child.id) ?? []).slice().sort(compareSiblings),
        i: 0,
        path,
      });
    }
    return rootEntry;
  };

  const roots: TreeEntry[] = [];
  for (const r of orderRoots(rootNodes)) {
    if (index.has(r.id)) continue;
    roots.push(buildFrom(r, false, r.parent && r.parent !== r.id && !byId.has(r.parent) ? r.parent : null));
  }
  // 兜底：不连到任何根的纯环分量（开发模式）→ 逐个提升为根
  for (const n of orderRoots(nodes)) {
    if (index.has(n.id)) continue;
    cycles += 1;
    roots.push(buildFrom(n, true, null));
  }

  return {
    roots,
    index,
    ancestors,
    stats: { total: nodes.length, roots: roots.length, broken, cycles },
  };
}

export interface FlatRow {
  entry: TreeEntry;
  depth: number;
}

/** 按展开集合压平成可见行（渲染层只画可见行——大图折叠态下 DOM 与可见行同阶） */
export function flattenVisible(tree: NodeTree, expanded: Set<string>): FlatRow[] {
  const rows: FlatRow[] = [];
  const stack: TreeEntry[] = [];
  for (let i = tree.roots.length - 1; i >= 0; i--) stack.push(tree.roots[i]);
  while (stack.length > 0) {
    const e = stack.pop() as TreeEntry;
    rows.push({ entry: e, depth: e.depth });
    if (e.children.length > 0 && expanded.has(e.node.id)) {
      for (let i = e.children.length - 1; i >= 0; i--) stack.push(e.children[i]);
    }
  }
  return rows;
}

/** 全书阅读顺序：忽略折叠的 DFS 前序（上一篇/下一篇沿这条线走） */
export function readingOrder(tree: NodeTree): ChainNode[] {
  const out: ChainNode[] = [];
  const stack: TreeEntry[] = [];
  for (let i = tree.roots.length - 1; i >= 0; i--) stack.push(tree.roots[i]);
  while (stack.length > 0) {
    const e = stack.pop() as TreeEntry;
    out.push(e.node);
    for (let i = e.children.length - 1; i >= 0; i--) stack.push(e.children[i]);
  }
  return out;
}

/** 打开阅读模式时的默认展开：所有根 + 目标节点的祖先链 + 目标节点自身 */
export function defaultExpanded(tree: NodeTree, focusId: string | null): Set<string> {
  const set = new Set<string>();
  for (const r of tree.roots) set.add(r.node.id);
  if (focusId) {
    for (const a of tree.ancestors.get(focusId) ?? []) set.add(a.node.id);
    if (tree.index.has(focusId)) set.add(focusId);
  }
  return set;
}

/** 祖先链（根 → … → 父），用于面包屑 */
export function breadcrumbOf(tree: NodeTree, id: string): ChainNode[] {
  const chain = tree.ancestors.get(id) ?? [];
  const self = tree.index.get(id);
  return self ? [...chain.map((e) => e.node), self.node] : chain.map((e) => e.node);
}

export type MatchKind = 'title' | 'id' | 'tag' | 'body';

export interface NodeMatch {
  node: ChainNode;
  kind: MatchKind;
  /** 正文命中时的上下文片段 */
  snippet?: string;
}

/**
 * 树内检索：标题 / id / 标签 / 正文。
 * 只读遍历，正文命中返回上下文片段；上限 80 条（够定位，不拖慢大图）。
 */
export function searchNodes(nodes: ChainNode[], query: string, limit = 80): NodeMatch[] {
  const q = query.trim().toLowerCase();
  if (q === '') return [];
  const out: NodeMatch[] = [];
  for (const n of nodes) {
    if (out.length >= limit) break;
    if (n.title.toLowerCase().includes(q)) {
      out.push({ node: n, kind: 'title' });
      continue;
    }
    if (n.id.toLowerCase().includes(q)) {
      out.push({ node: n, kind: 'id' });
      continue;
    }
    if (n.tags.some((t) => t.toLowerCase().includes(q))) {
      out.push({ node: n, kind: 'tag' });
      continue;
    }
    const idx = (n.body ?? '').toLowerCase().indexOf(q);
    if (idx >= 0) {
      const from = Math.max(0, idx - 24);
      const snippet = (n.body ?? '').slice(from, idx + q.length + 46).replace(/\s+/g, ' ').trim();
      out.push({ node: n, kind: 'body', snippet: `${from > 0 ? '…' : ''}${snippet}…` });
    }
  }
  return out;
}

/** 悬空 parent 的父 id 集合（只读展示用：提示"父节点缺失"） */
export function missingParents(tree: NodeTree): string[] {
  const miss = new Set<string>();
  for (const entry of tree.index.values()) {
    if (entry.brokenParent) miss.add(entry.brokenParent);
  }
  return [...miss];
}

/**
 * 未闭环任务判定（与 chain_to_cytoscape.ts 的图标记同规则）：
 * task 且没有任何子节点（本节点不在任何边的 parent 端）且正文无「自验收」。
 * 阅读模式下同步显示，读到任务时闭环状态一眼可见。
 */
export function isTaskOpenLoop(node: ChainNode, parentIds: Set<string>): boolean {
  return node.type === 'task' && !parentIds.has(node.id) && !(node.body ?? '').includes('自验收');
}

/** 全部 parent 端 id 集合（isTaskOpenLoop 的输入，O(E)） */
export function parentIdSet(nodes: ChainNode[]): Set<string> {
  const set = new Set<string>();
  for (const n of nodes) if (n.parent) set.add(n.parent);
  return set;
}
