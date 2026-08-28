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
  // App.svelte 在 add 后立即同步 fit + center(root)，首帧即"根节点居屏幕中央 + 全图可见"，
  // 消除"所有节点先堆在左上角 (0,0) 再跳到中央"的闪烁（v1.6 只修了 (0,0) 堆叠瞬间，未修首帧视口）。
  const R = 180 + n * 5;

  nodes.forEach((node, i) => {
    let x = 0;
    let y = 0;
    if (n > 1 && node.id !== rootId) {
      const ang = (i / (n - 1)) * Math.PI * 2;
      x = Math.cos(ang) * R;
      y = Math.sin(ang) * R;
    }
    elements.push({
      data: {
        id: node.id,
        label: displayLabel(node.type, node.title),
        nodeType: node.type,
        nodeStatus: node.status,
        chainParent: node.parent,  // 注意：不能用 `parent` 字段名——那是 cytoscape 保留字段（compound 复合节点），会把子节点渲染进父节点内部撑出巨型容器；chain 协议的父子关系由 edge 表达，这里仅保留信息备查
      },
      position: { x, y },
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
