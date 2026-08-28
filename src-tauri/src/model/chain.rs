use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::model::validation::ValidationReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainHealth {
    pub blocked_count: usize,
    pub failed_count: usize,
    pub in_progress_count: usize,
    pub pending_count: usize,
    pub success_count: usize,
    pub root_goal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPersona {
    pub domain: String,
    pub tech_stack: Vec<String>,
    pub coding_style: String,
    pub key_conventions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub root: PathBuf,
    pub node_count: usize,
    pub edge_count: usize,
    pub generated_at: String,
    /// ≤200 token 紧凑树状摘要，只展示非 success 节点，供 AI 快速恢复全局认知
    pub active_chain: String,
    /// 各状态节点计数 + 根目标标题，一眼看清工程健康度
    pub chain_health: ChainHealth,
    /// 项目画像（可选）：从根 goal 和 AI_GUIDE.md 自动提取的领域/技术栈/编码风格
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_persona: Option<ProjectPersona>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub parent: String,
    pub child: String,
    /// v2.4 关系类型：contains（默认）/ solves / alternative
    #[serde(default = "default_edge_rel")]
    pub rel: String,
}

fn default_edge_rel() -> String {
    "contains".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSnapshot {
    pub nodes: Vec<crate::model::node::Node>,
    pub edges: Vec<Edge>,
    pub manifest: Manifest,
    pub validation: ValidationReport,
}

/// 快照元数据：存储于 .chain/logs/index.json，每条对应一个快照文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: String,
    pub tag: String,
    pub created_at: String,
    pub node_count: usize,
    pub edge_count: usize,
}
