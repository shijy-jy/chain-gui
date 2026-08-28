pub mod node;
pub mod chain;
pub mod validation;

pub use chain::{ChainHealth, ProjectPersona, SnapshotMeta};
pub use node::FoldedInfo;

/// 软件工作模式（v2.0）：
/// - Analysis（分析模式）：严格 chain 协议校验，供 AI 与使用者按协议分析/验收图谱
/// - Dev（开发模式）：自由知识图谱——节点字段宽松（无必填）、允许多根/孤立节点/环，
///   开发者可自由增删节点与链接，不要求 AI 协议内容
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    Analysis,
    Dev,
}

impl ScanMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "dev" => ScanMode::Dev,
            _ => ScanMode::Analysis,
        }
    }
    pub fn is_dev(self) -> bool {
        matches!(self, ScanMode::Dev)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UpdateFields {
    pub title: Option<String>,
    pub status: Option<crate::model::node::NodeStatus>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub evidence: Option<Vec<String>>,
    /// v2.0：Some(Some(id)) = 设置父节点；Some(None) = 断开链接（parent: null）
    #[serde(default)]
    pub parent: Option<Option<String>>,
    /// v2.4：递进关系类型（contains/solves/alternative）
    #[serde(default)]
    pub rel: Option<String>,
}
