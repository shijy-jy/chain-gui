import type { NodeType } from './types';

// 图节点呈现常量（二维画布退役后仅剩的纯展示数据：类型配色 + 类型名）。
//
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

// v2.0 类型色（图例 / 三维球体配色 / 边渐变 = 源类型色 → 目标类型色）
export const NODE_TYPE_COLOR: Record<NodeType, string> = {
  goal: '#a78bfa',
  design: '#60a5fa',
  task: '#22d3ee',
  verification: '#34d399',
  note: '#94a3b8',
};

/**
 * 三维图「形态」档位（展开度预设）：
 *   layered = 紧凑（分支锥角 0.8，枝条抱拢）
 *   radial  = 舒展（锥角 1.35，树突张开）
 * 由 Graph3D.computeNeuron3D 解释；'auto'（标准 1.05）只是 App 侧的滑条取值，不属于本档位。
 */
export type LayoutMode = 'layered' | 'radial';
