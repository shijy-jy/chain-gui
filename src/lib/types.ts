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
  status: NodeStatus;
  created: string;
  updated: string;
  revision: number;
  tags: string[];
  evidence: string[];
  body: string;
  folded?: FoldedInfo;
}

export interface ChainEdge {
  parent: string;
  child: string;
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
