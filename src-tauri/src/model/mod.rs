pub mod node;
pub mod chain;
pub mod validation;

pub use chain::{ChainHealth, ProjectPersona, SnapshotMeta};
pub use node::FoldedInfo;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UpdateFields {
    pub title: Option<String>,
    pub status: Option<crate::model::node::NodeStatus>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub evidence: Option<Vec<String>>,
}
