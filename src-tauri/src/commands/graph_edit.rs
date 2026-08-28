//! 开发模式自由图谱编辑命令（v2.0）：
//! - `create_node`：新建节点文件（id 可省略自动生成）
//! - `delete_node`：删除节点文件（孤立化其子节点——自由图谱不校验悬空）
//! - `set_parent`：建立/断开链接（改写子节点 frontmatter 的 parent）
//! - `set_mode`：切换扫描模式并返回当前目录按新模式的快照
//! 以上编辑命令仅开发模式可用；分析模式的链结构由 AI 按协议维护。

use std::path::PathBuf;
use tauri::command;
use serde::{Deserialize, Serialize};
use crate::model::ScanMode;
use crate::model::chain::ChainSnapshot;
use crate::scanner::walker::scan_chain_dir_mode;

/// 新建节点入参
#[derive(Debug, Clone, Deserialize)]
pub struct CreateNodeInput {
    /// 可选；缺省自动生成 node-N
    pub id: Option<String>,
    pub title: String,
    /// goal/design/task/verification；缺省 task
    #[serde(default)]
    pub node_type: Option<String>,
    /// pending/in_progress/success/failed/blocked；缺省 pending
    #[serde(default)]
    pub status: Option<String>,
    /// 可选父节点 id（建立链接）；缺省 = 独立节点
    #[serde(default)]
    pub parent: Option<String>,
    /// v2.4 递进关系类型：contains（默认）/ solves / alternative
    #[serde(default)]
    pub rel: Option<String>,
}

fn normalize_rel(r: &Option<String>) -> &str {
    match r.as_deref() {
        Some("solves") => "solves",
        Some("alternative") => "alternative",
        _ => "contains",
    }
}

/// id 安全校验：只允许字母数字连字符下划线（防路径穿越/非法文件名）
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 自动生成不重复的 id：node-1、node-2、…
fn auto_id(nodes_dir: &std::path::Path) -> String {
    let mut n = 1;
    loop {
        let candidate = format!("node-{n}");
        if !nodes_dir.join(format!("{candidate}.md")).exists() {
            return candidate;
        }
        n += 1;
    }
}

fn normalize_type(t: &Option<String>) -> &str {
    match t.as_deref() {
        Some("goal") => "goal",
        Some("design") => "design",
        Some("task") => "task",
        Some("verification") => "verification",
        // v2.0 开发模式默认中性类型 note（知识库节点不好归入链协议四类型）
        _ => "note",
    }
}

fn normalize_status(s: &Option<String>) -> &str {
    match s.as_deref() {
        Some("pending") => "pending",
        Some("in_progress") => "in_progress",
        Some("success") => "success",
        Some("failed") => "failed",
        Some("blocked") => "blocked",
        // v2.0 开发模式默认无状态 none
        _ => "none",
    }
}

#[command]
pub fn create_node(dir: String, input: CreateNodeInput, mode: Option<String>) -> Result<ChainSnapshot, String> {
    let scan_mode = mode.as_deref().map(ScanMode::from_str).unwrap_or(ScanMode::Analysis);
    if !scan_mode.is_dev() {
        return Err("仅开发模式可自由新建节点（分析模式的链由 AI 按协议维护）".into());
    }
    let root = PathBuf::from(&dir);
    // v2.1 模式强绑定
    crate::commands::workspace::check_mode(&root, scan_mode)?;
    let nodes_dir = root.join(".chain").join("nodes");
    if !nodes_dir.is_dir() {
        return Err("nodes 目录不存在，请先初始化".into());
    }

    let id = match &input.id {
        Some(id) => {
            if !is_safe_id(id) {
                return Err("id 只允许字母/数字/连字符/下划线（如 node-1、算法笔记）".into());
            }
            id.clone()
        }
        None => auto_id(&nodes_dir),
    };
    let file = nodes_dir.join(format!("{id}.md"));
    if file.exists() {
        return Err(format!("节点 {id} 已存在"));
    }

    let now = crate::scanner::frontmatter::now_iso8601();
    let title = if input.title.trim().is_empty() { id.clone() } else { input.title.trim().to_string() };
    let node_type = normalize_type(&input.node_type);
    let status = normalize_status(&input.status);
    let parent_line = match &input.parent {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => "null".to_string(),
    };
    let rel = normalize_rel(&input.rel);

    let content = format!(
        "---\nid: {id}\ntype: {node_type}\ntitle: {title}\nparent: {parent_line}\nrel: {rel}\nstatus: {status}\ncreated: {now}\nupdated: {now}\nrevision: 1\ntags: []\n---\n\n# {title}\n"
    );
    std::fs::write(&file, content).map_err(|e| format!("写节点文件失败：{e}"))?;

    scan_chain_dir_mode(&root, scan_mode).map_err(|e| format!("重扫失败：{e}"))
}

#[command]
pub fn delete_node(dir: String, node_id: String, mode: Option<String>) -> Result<ChainSnapshot, String> {
    let scan_mode = mode.as_deref().map(ScanMode::from_str).unwrap_or(ScanMode::Analysis);
    if !scan_mode.is_dev() {
        return Err("仅开发模式可自由删除节点（分析模式的链由 AI 按协议维护）".into());
    }
    let root = PathBuf::from(&dir);
    // v2.1 模式强绑定
    crate::commands::workspace::check_mode(&root, scan_mode)?;
    let nodes_dir = root.join(".chain").join("nodes");
    if !is_safe_id(&node_id) {
        return Err("节点 id 非法".into());
    }
    let file = nodes_dir.join(format!("{node_id}.md"));
    if !file.exists() {
        return Err(format!("节点 {node_id} 不存在"));
    }
    std::fs::remove_file(&file).map_err(|e| format!("删除失败：{e}"))?;

    scan_chain_dir_mode(&root, scan_mode).map_err(|e| format!("重扫失败：{e}"))
}

#[command]
pub fn set_parent(
    dir: String,
    node_id: String,
    parent: Option<String>,
    mode: Option<String>,
    rel: Option<String>,
) -> Result<ChainSnapshot, String> {
    let scan_mode = mode.as_deref().map(ScanMode::from_str).unwrap_or(ScanMode::Analysis);
    if !scan_mode.is_dev() {
        return Err("仅开发模式可自由编辑链接（分析模式的链由 AI 按协议维护）".into());
    }
    let root = PathBuf::from(&dir);
    // v2.1 模式强绑定
    crate::commands::workspace::check_mode(&root, scan_mode)?;
    let nodes_dir = root.join(".chain").join("nodes");
    if !is_safe_id(&node_id) {
        return Err("节点 id 非法".into());
    }
    let file = nodes_dir.join(format!("{node_id}.md"));
    if !file.exists() {
        return Err(format!("节点 {node_id} 不存在"));
    }

    // 父节点（若指定）必须存在，避免生成永远无效的链接
    let parent = match parent {
        Some(p) if !p.trim().is_empty() => {
            if !nodes_dir.join(format!("{}.md", p.trim())).exists() {
                return Err(format!("父节点 {} 不存在", p.trim()));
            }
            Some(p.trim().to_string())
        }
        _ => None,
    };
    // v2.4 递进关系类型
    let rel = normalize_rel(&rel).to_string();

    let raw = std::fs::read_to_string(&file).map_err(|e| format!("读取失败：{e}"))?;
    // 开发模式宽松解析：无 frontmatter 时先补最小 frontmatter
    let (mut fm, body) = match crate::scanner::frontmatter::parse(&raw) {
        Ok(result) => result,
        Err(_) => {
            use serde_yaml::Value as YamlValue;
            let now = crate::scanner::frontmatter::now_iso8601();
            let mut m = serde_yaml::Mapping::new();
            m.insert(YamlValue::String("id".into()), YamlValue::String(node_id.clone()));
            m.insert(YamlValue::String("type".into()), YamlValue::String("note".into()));
            m.insert(YamlValue::String("status".into()), YamlValue::String("none".into()));
            m.insert(YamlValue::String("title".into()), YamlValue::String(node_id.clone()));
            m.insert(YamlValue::String("created".into()), YamlValue::String(now.clone()));
            m.insert(YamlValue::String("updated".into()), YamlValue::String(now));
            m.insert(YamlValue::String("revision".into()), YamlValue::Number(1u64.into()));
            m.insert(YamlValue::String("parent".into()), YamlValue::Null);
            (m, raw.clone())
        }
    };

    let fields = crate::model::UpdateFields {
        title: None,
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: Some(parent),
        rel: Some(rel),
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    let new_content = crate::scanner::frontmatter::serialize(&fm, &body)
        .map_err(|e| format!("序列化失败：{e}"))?;
    std::fs::write(&file, new_content).map_err(|e| format!("写回失败：{e}"))?;

    scan_chain_dir_mode(&root, scan_mode).map_err(|e| format!("重扫失败：{e}"))
}

#[derive(Debug, Clone, Serialize)]
pub struct DevEditResult {
    pub created: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        tmp
    }

    fn input(title: &str) -> CreateNodeInput {
        CreateNodeInput {
            id: None,
            title: title.to_string(),
            node_type: None,
            status: None,
            parent: None,
            rel: None,
        }
    }

    #[test]
    fn test_create_node_auto_id_and_defaults() {
        let tmp = setup();
        let dir = tmp.path().to_str().unwrap().to_string();
        let snap = create_node(dir.clone(), input("物理笔记"), Some("dev".into())).unwrap();
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].id, "node-1");
        assert_eq!(snap.nodes[0].title, "物理笔记");
        assert_eq!(snap.nodes[0].parent, None);
        assert!(snap.validation.valid);
        // 第二个自动递增
        let snap2 = create_node(dir, input("第二篇"), Some("dev".into())).unwrap();
        assert_eq!(snap2.nodes.len(), 2);
        assert!(snap2.nodes.iter().any(|n| n.id == "node-2"));
    }

    #[test]
    fn test_create_node_with_parent_link() {
        let tmp = setup();
        let dir = tmp.path().to_str().unwrap().to_string();
        create_node(dir.clone(), input("根节点"), Some("dev".into())).unwrap();
        let mut child = input("子节点");
        child.parent = Some("node-1".into());
        let snap = create_node(dir, child, Some("dev".into())).unwrap();
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].parent, "node-1");
        assert_eq!(snap.edges[0].child, "node-2");
    }

    #[test]
    fn test_create_node_rejects_analysis_mode() {
        let tmp = setup();
        let dir = tmp.path().to_str().unwrap().to_string();
        let res = create_node(dir, input("x"), None);
        assert!(res.is_err(), "分析模式应拒绝自由新建节点: {res:?}");
    }

    #[test]
    fn test_create_node_rejects_bad_id() {
        let tmp = setup();
        let dir = tmp.path().to_str().unwrap().to_string();
        let mut bad = input("x");
        bad.id = Some("../evil".into());
        assert!(create_node(dir, bad, Some("dev".into())).is_err(), "路径穿越 id 应被拒绝");
    }

    #[test]
    fn test_delete_node() {
        let tmp = setup();
        let dir = tmp.path().to_str().unwrap().to_string();
        create_node(dir.clone(), input("要删的"), Some("dev".into())).unwrap();
        let snap = delete_node(dir.clone(), "node-1".into(), Some("dev".into())).unwrap();
        assert_eq!(snap.nodes.len(), 0);
        assert!(!tmp.path().join(".chain/nodes/node-1.md").exists());
    }

    #[test]
    fn test_set_parent_connect_and_disconnect() {
        let tmp = setup();
        let dir = tmp.path().to_str().unwrap().to_string();
        create_node(dir.clone(), input("A"), Some("dev".into())).unwrap();
        create_node(dir.clone(), input("B"), Some("dev".into())).unwrap();

        // 建立链接 B → A
        let snap = set_parent(dir.clone(), "node-2".into(), Some("node-1".into()), Some("dev".into()), None).unwrap();
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].child, "node-2");

        // 断开链接
        let snap = set_parent(dir.clone(), "node-2".into(), None, Some("dev".into()), None).unwrap();
        assert_eq!(snap.edges.len(), 0);

        // 指向不存在的父节点应报错
        assert!(set_parent(dir, "node-2".into(), Some("ghost".into()), Some("dev".into()), None).is_err());
    }
}
