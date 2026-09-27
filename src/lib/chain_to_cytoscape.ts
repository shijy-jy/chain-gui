import type { ElementDefinition } from 'cytoscape';
import type { ChainSnapshot, NodeType } from './types';

// v1.7 图谱节点显示命名：「类型 · 标题」——曾用于画布标签。
// v3.3 起**图谱上不显示任何名称**（用户要求）：节点的身份靠类型配色 + 大小（度）+
// 悬停浮层（id · 类型）+ 点击右侧信息栏承载。这里保留两张表是因为图例与悬停提示仍在用。
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

export function chainToElements(
  snap: ChainSnapshot,
  opts?: { withEdges?: boolean; includeArchived?: boolean },
): ElementDefinition[] {
  const withEdges = opts?.withEdges ?? true;
  const includeArchived = opts?.includeArchived ?? false;
  const elements: ElementDefinition[] = [];
  const nodes = snap.nodes;
  const n = nodes.length;
  const rootId = snap.manifest.root;

  // v3.0：初始位置不再由本模块生成——位置统一由 lib/tree_layout.ts 的树布局算出并写入
  // （App.svelte 的 relayout 在 add 之后立即调用，并带 300ms 补间）。
  // 历史包袱说明：v1.7–v2.17 这里预写"根锚原点 + BFS 同心圆环"散点作为力导向起点，
  // 环半径随 n 线性膨胀（1500 节点时最深层半径 27034px、直径 5.4 万像素），
  // 是首帧"一团雾 + 爆炸"的直接原因之一；力导向整体已被确定性树布局取代，故整段删除。

  // v2.16 支链闭环：task 节点无验证子节点（且正文无「自验收」注明）→ 开环标记（琥珀虚线框）
  // 注意：要的是「有没有子节点」——即本节点是否出现在任何边的 parent 端（装 child 集合会误判所有非根节点为有子）
  const parentSet = new Set<string>();
  for (const e of snap.edges) parentSet.add(e.parent);

  nodes.forEach((node) => {
    // v3.3 不再生成 label 数据（图谱不显示名称，见文件头说明）
    const openLoop =
      node.type === 'task' && !parentSet.has(node.id) && !(node.body ?? '').includes('自验收');
    elements.push({
      data: {
        id: node.id,
        nodeType: node.type,
        nodeStatus: node.status,
        chainParent: node.parent,  // 注意：不能用 `parent` 字段名——那是 cytoscape 保留字段（compound 复合节点），会把子节点渲染进父节点内部撑出巨型容器；chain 协议的父子关系由 edge 表达，这里仅保留信息备查
        ...(node.code_map ? { codeMap: true } : {}),
        ...(openLoop ? { openLoop: true } : {}),
      },
    });
  });

  // v2.12 M-Code/归档视图开关：归档节点淡色虚线纳入画布（外围环，无边——归档不进活跃图）
  // v3.0：位置只给一个固定外围环作占位——归档节点不参与树布局的可见性裁剪（它们本来就不挂边），
  // 半径取常数而非随 n 膨胀的 R，避免大图上归档环被推到几万像素外
  if (includeArchived) {
    const archived = snap.archived ?? [];
    const archR = 900;
    archived.forEach((node, i) => {
      const ang =
        (i / Math.max(archived.length, 1)) * Math.PI * 2 - Math.PI / 2;
      elements.push({
        data: {
          id: node.id,
          nodeType: node.type,
          nodeStatus: node.status,
          chainParent: node.parent,
          archived: true,
          ...(node.code_map ? { codeMap: true } : {}),
        },
        position: {
          x: Math.cos(ang) * archR,
          y: Math.sin(ang) * archR,
        },
        style: {
          opacity: 0.45,
          'border-style': 'dashed',
        },
      });
    });
  }

  // v2.2 涟漪视图：开发模式可关闭连线渲染（联系改由亮度层级+波纹表达，连接数据仍存 snapshot.edges）
  if (!withEdges) return elements;

  // v2.15 大图：一次性 nodeById（替代每条边 O(n) find）+ 渐变按规模降级实线
  // （性能策略：>300 边跑逐边渐变纹理是平移缩放的大头——注释许久了这次真落地）
  const nodeById = new Map(nodes.map((nd) => [nd.id, nd]));
  const useGradient = snap.edges.length <= 300;
  for (const edge of snap.edges) {
    // 悬空边直接跳过（后端理论上已过滤；这里双保险——cytoscape cy.add 遇到
    // 不存在的端点会抛异常导致整图不渲染，绝不能把坏边喂给它）
    const src = nodeById.get(edge.parent);
    const tgt = nodeById.get(edge.child);
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
      style: useGradient
        ? {
            'line-fill': 'linear-gradient',
            'line-gradient-stop-colors': [srcColor, tgtColor],
            'line-gradient-stop-positions': ['0%', '100%'],
            'target-arrow-color': tgtColor,
          }
        : {
            'line-color': 'rgba(148,163,184,0.5)',
            'target-arrow-color': 'rgba(255,255,255,0.4)',
          },
    });
  }

  return elements;
}
