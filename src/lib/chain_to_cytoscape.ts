import type { ElementDefinition } from 'cytoscape';
import type { ChainSnapshot, NodeType } from './types';

// v1.7 图谱节点显示命名：「类型 · 标题」——id 是机器标识（文件名/交叉引用）不带语义，
// 标题才承载"这条链在干什么"的宏观作用。画布标签以 title 为主 + 类型前缀
// （强调前后逻辑关系：目标→设计→任务→验证）；id 移出画布标签，
// 点击节点在侧栏可见、悬停节点在浮层可见（见 App.svelte hover-tip）。
export const NODE_TYPE_LABEL: Record<NodeType, string> = {
  goal: '目标',
  design: '设计',
  task: '任务',
  verification: '验证',
  note: '笔记',
};

// v2.0 类型色（与 App.svelte 图例/节点配色一致；边渐变 = 源类型色 → 目标类型色）
export const NODE_TYPE_COLOR: Record<NodeType, string> = {
  goal: '#a78bfa',
  design: '#60a5fa',
  task: '#22d3ee',
  verification: '#34d399',
  note: '#94a3b8',
};

function displayLabel(type: NodeType, title: string): string {
  const max = 20;
  const t = title.length > max ? `${title.slice(0, max)}…` : title;
  // v2.0 开发模式中性类型 note：不加「笔记」前缀（知识库节点标题即显示名，类型可忽略）
  if (type === 'note') return t;
  return `${NODE_TYPE_LABEL[type]} · ${t}`;
}

export function chainToElements(snap: ChainSnapshot, opts?: { withEdges?: boolean }): ElementDefinition[] {
  const withEdges = opts?.withEdges ?? true;
  const elements: ElementDefinition[] = [];
  const nodes = snap.nodes;
  const n = nodes.length;
  const rootId = snap.manifest.root;

  // v1.7 初始散点预写入：根节点锚定原点，其余节点绕根均匀圆环。
  // v2.4 改为 BFS 分层同心圆环：同层按遍历顺序均布、层间半径递增——
  // 树/链结构的首帧即为无交叉布局，力模拟从低交叉起点收敛，
  // 配合 runForceLayout 的交叉惩罚与质心后处理，"重排后连线乱交"大幅减少。
  // App.svelte 在 add 后立即同步 fit + center(root)，首帧即"根节点居屏幕中央 + 全图可见"，
  // 消除"所有节点先堆在左上角 (0,0) 再跳到中央"的闪烁（v1.6 只修了 (0,0) 堆叠瞬间，未修首帧视口）。
  const R = 180 + n * 5;
  const ringGap = Math.max(120, R * 0.42);

  // 邻接表（无向；分析模式树与开发模式多父/多分量图通吃）
  const adj = new Map<string, string[]>();
  for (const nd of nodes) adj.set(nd.id, []);
  for (const e of snap.edges) {
    adj.get(e.parent)?.push(e.child);
    adj.get(e.child)?.push(e.parent);
  }

  // 连通分量拆分：从各根（优先 manifest.root）做 BFS，得到分量成员及其层深
  const components: { root: string; members: string[]; depth: Map<string, number> }[] = [];
  const visited = new Set<string>();
  const startIds: string[] = [];
  if (rootId && adj.has(rootId)) startIds.push(rootId);
  for (const nd of nodes) if (!startIds.includes(nd.id)) startIds.push(nd.id);
  for (const start of startIds) {
    if (visited.has(start)) continue;
    const depth = new Map<string, number>();
    const members: string[] = [];
    const queue: string[] = [start];
    visited.add(start);
    depth.set(start, 0);
    while (queue.length > 0) {
      const cur = queue.shift()!;
      members.push(cur);
      for (const nb of adj.get(cur) ?? []) {
        if (!visited.has(nb)) {
          visited.add(nb);
          depth.set(nb, (depth.get(cur) ?? 0) + 1);
          queue.push(nb);
        }
      }
    }
    components.push({ root: start, members, depth });
  }

  // 分量锚点：单分量 = 原点；多分量 = 大圆均布（各分量互不重叠）
  const positions = new Map<string, { x: number; y: number }>();
  components.forEach((comp, ci) => {
    let anchor = { x: 0, y: 0 };
    if (components.length > 1) {
      const rr = 200 + components.length * 60;
      const ang = (ci / components.length) * Math.PI * 2;
      anchor = { x: Math.cos(ang) * rr, y: Math.sin(ang) * rr };
    }
    // 每层一个同心圆环，同层按 BFS 遍历顺序均布（顺序稳定 → 布局可复现）
    const byDepth = new Map<number, string[]>();
    for (const id of comp.members) {
      const d = comp.depth.get(id) ?? 0;
      const arr = byDepth.get(d) ?? [];
      arr.push(id);
      byDepth.set(d, arr);
    }
    for (const [d, ids] of byDepth) {
      const radius = d === 0 ? 0 : R + (d - 1) * ringGap;
      ids.forEach((id, k) => {
        const ang = (k / Math.max(ids.length, 1)) * Math.PI * 2 - Math.PI / 2;
        positions.set(id, {
          x: anchor.x + Math.cos(ang) * radius,
          y: anchor.y + Math.sin(ang) * radius,
        });
      });
    }
  });

  nodes.forEach((node) => {
    const p = positions.get(node.id) ?? { x: 0, y: 0 };
    elements.push({
      data: {
        id: node.id,
        label: displayLabel(node.type, node.title),
        nodeType: node.type,
        nodeStatus: node.status,
        chainParent: node.parent,  // 注意：不能用 `parent` 字段名——那是 cytoscape 保留字段（compound 复合节点），会把子节点渲染进父节点内部撑出巨型容器；chain 协议的父子关系由 edge 表达，这里仅保留信息备查
      },
      position: p,
    });
  });

  // v2.2 涟漪视图：开发模式可关闭连线渲染（联系改由亮度层级+波纹表达，连接数据仍存 snapshot.edges）
  if (!withEdges) return elements;

  for (const edge of snap.edges) {
    // 悬空边直接跳过（后端理论上已过滤；这里双保险——cytoscape cy.add 遇到
    // 不存在的端点会抛异常导致整图不渲染，绝不能把坏边喂给它）
    const src = nodes.find((n) => n.id === edge.parent);
    const tgt = nodes.find((n) => n.id === edge.child);
    if (!src || !tgt) continue;
    // v2.0 边渐变（源类型色 → 目标类型色）：用「逐边内联样式 + 数组字面值」实现——
    // 关键坑：cytoscape 的 data() 映射不支持多值属性（line-gradient-stop-colors），
    // 解析器会跳过映射分支把 "data(...)" 当字面颜色 → null → 渲染崩溃（已实测踩坑，勿回退）
    const srcColor = NODE_TYPE_COLOR[src.type];
    const tgtColor = NODE_TYPE_COLOR[tgt.type];
    elements.push({
      data: {
        id: `${edge.parent}->${edge.child}`,
        source: edge.parent,
        target: edge.child,
        // v2.4 递进关系类型（驱动边线型选择器：contains 实线 / solves 虚线 / alternative 点线）
        rel: edge.rel ?? 'contains',
      },
      style: {
        'line-gradient-stop-colors': [srcColor, tgtColor],
        'line-gradient-stop-positions': ['0%', '100%'],
        'target-arrow-color': tgtColor,
      },
    });
  }

  return elements;
}
