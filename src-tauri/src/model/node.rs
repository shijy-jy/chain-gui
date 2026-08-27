use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Goal,
    Design,
    Task,
    Verification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    InProgress,
    Success,
    Failed,
    Blocked,
}

/// 折叠信息：当子链被折叠为摘要节点时记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldedInfo {
    /// 被折叠的原始节点 id 列表
    pub original_nodes: Vec<String>,
    /// 折叠时刻
    pub folded_at: String,
    /// 折叠前该子链的节点总数
    pub original_node_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub title: String,
    pub parent: Option<String>,
    pub status: NodeStatus,
    pub created: String,
    pub updated: String,
    pub revision: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub body: String,
    /// 折叠标记：存在时表示此节点是子链折叠后的摘要节点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folded: Option<FoldedInfo>,
}

impl Node {
    pub fn short_id(&self) -> &str {
        &self.id
    }
}

/// 把 UpdateFields 里的 Some 字段应用到 frontmatter map
/// 自动更新 updated 时间和 revision+1
pub fn apply_update(
    fm: &mut serde_yaml::Mapping,
    fields: &crate::model::UpdateFields,
) -> Result<(), String> {
    use serde_yaml::Value as YamlValue;

    if let Some(title) = &fields.title {
        fm.insert(
            YamlValue::String("title".into()),
            YamlValue::String(title.clone()),
        );
    }
    if let Some(status) = &fields.status {
        let status_str = serde_json::to_string(status)
            .map_err(|e| format!("status 序列化失败：{}", e))?
            .trim_matches('"')
            .to_string();
        fm.insert(
            YamlValue::String("status".into()),
            YamlValue::String(status_str),
        );
    }
    // body 不是 frontmatter 字段，不在此处理（空值校验在 update_node 顶层完成）
    if let Some(tags) = &fields.tags {
        fm.insert(
            YamlValue::String("tags".into()),
            YamlValue::Sequence(
                tags.iter()
                    .map(|t| YamlValue::String(t.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(evidence) = &fields.evidence {
        fm.insert(
            YamlValue::String("evidence".into()),
            YamlValue::Sequence(
                evidence.iter()
                    .map(|e| YamlValue::String(e.clone()))
                    .collect(),
            ),
        );
    }
    // v2.0：parent 链接编辑（Some(None) = 断开 → null）
    if let Some(parent) = &fields.parent {
        let value = match parent {
            Some(id) => YamlValue::String(id.clone()),
            None => YamlValue::Null,
        };
        fm.insert(YamlValue::String("parent".into()), value);
    }

    // 自增 revision
    let rev_key = YamlValue::String("revision".into());
    let new_rev = match fm.get(&rev_key) {
        Some(YamlValue::Number(n)) => n.as_u64().unwrap_or(0) + 1,
        _ => 1,
    };
    fm.insert(
        rev_key,
        YamlValue::Number(new_rev.into()),
    );

    // 更新 updated 时间
    let now = crate::scanner::frontmatter::now_iso8601();
    fm.insert(
        YamlValue::String("updated".into()),
        YamlValue::String(now),
    );

    Ok(())
}
