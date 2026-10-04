export type NodeType = 'goal' | 'design' | 'task' | 'verification' | 'note';
export type NodeStatus = 'pending' | 'in_progress' | 'success' | 'failed' | 'blocked' | 'none';

// v2.0 软件工作模式：分析（严格 chain 协议）/ 开发（自由知识图谱）
export type ScanMode = 'analysis' | 'dev';

// v2.1 工作区条目：path=文件夹路径、mode=该文件夹绑定的模式（.chain/.mode 标签）、name=显示名
export interface WorkspaceInfo {
  path: string;
  mode: ScanMode;
  name: string;
}

export interface FoldedInfo {
  original_nodes: string[];
  folded_at: string;
  original_node_count: number;
}

export interface ChainNode {
  id: string;
  type: NodeType;
  title: string;
  parent: string | null;
  /** v2.4 递进关系类型（开发模式）：contains / solves / alternative */
  rel?: string;
  status: NodeStatus;
  created: string;
  updated: string;
  revision: number;
  tags: string[];
  evidence: string[];
  body: string;
  folded?: FoldedInfo;
  /** v2.10 M7' 归档标记：默认不进图与检索 */
  archived?: boolean;
  archived_reason?: string;
  /** v2.11 M8' 蒸馏产物（derived:true，检索默认降权） */
  derived?: boolean;
  /** v2.11 M8' 冲突冻结（[待裁决]，拒绝写入） */
  frozen?: boolean;
  freeze_reason?: string;
  /** v2.12 M-Code 代码骨架挂载（源码相对路径） */
  code_map?: string;
  // ── P2-7 人机同源结构指标（后端 snapshot_view 下发；显示层**不自己重算**）──
  // 人看到的球径 = degree；AI 在工具响应里读到的 children_count / subtree_size / depth
  // 出自同一份 structure_index + subtree_sizes 算法 —— 两边是同一个数。
  /** 度数（关联边数，无向） */
  degree?: number;
  /** 深度（根 = 0；与 AI 的 depth 同源） */
  depth?: number;
  /** 直接子节点数（与 AI 的 children_count 同源） */
  children_count?: number;
  /** 子树规模（含自身；与 AI 的 subtree_size 同源） */
  subtree_size?: number;
}

export interface ChainEdge {
  parent: string;
  child: string;
  /** v2.4 关系类型：contains（默认）/ solves / alternative */
  rel: string;
}

export interface ChainHealth {
  blocked_count: number;
  failed_count: number;
  in_progress_count: number;
  pending_count: number;
  success_count: number;
  root_goal: string;
}

export interface ProjectPersona {
  domain: string;
  tech_stack: string[];
  coding_style: string;
  key_conventions: string[];
}

export interface ChainManifest {
  root: string;
  node_count: number;
  edge_count: number;
  generated_at: string;
  active_chain: string;
  chain_health: ChainHealth;
  project_persona?: ProjectPersona;
}

export interface ValidationReport {
  valid: boolean;
  errors: string[];
  warnings: string[];
}

export interface ChainSnapshot {
  nodes: ChainNode[];
  edges: ChainEdge[];
  /** v2.10 M7' 归档节点列表（与活跃图分离；GUI 归档视图开关按需纳入） */
  archived: ChainNode[];
  manifest: ChainManifest;
  validation: ValidationReport;
}

export interface SnapshotMeta {
  id: string;
  tag: string;
  created_at: string;
  node_count: number;
  edge_count: number;
}
