//! 开发模式自由图谱编辑原语（v2.0，核心唯一写路径）：
//! - `create_node`：新建节点文件（id 可省略自动生成）
//! - `delete_node`：删除节点文件（孤立化其子节点——自由图谱不校验悬空）
//! - `set_parent`：建立/断开链接（改写子节点 frontmatter 的 parent）
//! - `update_node_fields`：GUI 侧字段更新（分析/开发双模式）
//!
//! 以上编辑仅开发模式可用（update_node_fields 除外）；分析模式的链结构由 AI 按协议维护。
//! 所有写入都走 core 的原子写与统一解析原语（宪法第 5 条：入口不得直写文件）。

use crate::model::chain::ChainSnapshot;
use crate::model::{ScanMode, UpdateFields};
use crate::ops::{atomic_write, parse_lenient};
use crate::profile::DEV;
use crate::scanner::{frontmatter, walker};
use crate::workspace::check_mode;
use serde::{Deserialize, Serialize};
use std::path::Path;

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

#[derive(Debug, Clone, Serialize)]
pub struct DevEditResult {
    pub created: bool,
}

/// rel 归一（开发模式宽容：词表外 → contains 默认值）
fn normalize_rel(r: &Option<String>) -> &str {
    match r.as_deref() {
        Some("solves") => "solves",
        Some("alternative") => "alternative",
        _ => "contains",
    }
}

/// id 安全校验：只允许字母数字连字符下划线（防路径穿越/非法文件名）
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 自动生成不重复的 id：node-1、node-2、…
pub fn auto_id(nodes_dir: &std::path::Path) -> String {
    let mut n = 1;
    loop {
        let candidate = format!("node-{n}");
        if !nodes_dir.join(format!("{candidate}.md")).exists() {
            return candidate;
        }
        n += 1;
    }
}

fn normalize_type(t: &Option<String>) -> String {
    match t.as_deref() {
        Some(s) if DEV.type_vocab.contains(&s) => s.to_string(),
        // v2.0 开发模式默认中性类型 note（知识库节点不好归入链协议四类型）
        _ => "note".to_string(),
    }
}

fn normalize_status(s: &Option<String>) -> String {
    match s.as_deref() {
        Some(v) if DEV.status_vocab.contains(&v) => v.to_string(),
        // v2.0 开发模式默认无状态 none
        _ => "none".to_string(),
    }
}

/// 新建节点文件（仅开发模式）。返回重扫后的快照。
pub fn create_node(
    root: &Path,
    input: &CreateNodeInput,
    mode: ScanMode,
) -> Result<ChainSnapshot, String> {
    if !mode.is_dev() {
        return Err("仅开发模式可自由新建节点（分析模式的链由 AI 按协议维护）".into());
    }
    // v2.1 模式强绑定
    check_mode(root, mode)?;
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

    let now = frontmatter::now_iso8601();
    let title = if input.title.trim().is_empty() {
        id.clone()
    } else {
        input.title.trim().to_string()
    };
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
    atomic_write(&file, &content).map_err(|e| format!("写节点文件失败：{e}"))?;

    walker::scan_chain_dir_mode(root, mode).map_err(|e| format!("重扫失败：{e}"))
}

/// 删除节点文件（仅开发模式）。返回重扫后的快照。
pub fn delete_node(root: &Path, node_id: &str, mode: ScanMode) -> Result<ChainSnapshot, String> {
    if !mode.is_dev() {
        return Err("仅开发模式可自由删除节点（分析模式的链由 AI 按协议维护）".into());
    }
    // v2.1 模式强绑定
    check_mode(root, mode)?;
    let nodes_dir = root.join(".chain").join("nodes");
    if !is_safe_id(node_id) {
        return Err("节点 id 非法".into());
    }
    let file = nodes_dir.join(format!("{node_id}.md"));
    if !file.exists() {
        return Err(format!("节点 {node_id} 不存在"));
    }
    std::fs::remove_file(&file).map_err(|e| format!("删除失败：{e}"))?;

    walker::scan_chain_dir_mode(root, mode).map_err(|e| format!("重扫失败：{e}"))
}

/// 建立/断开链接（仅开发模式）。parent=None 断开；rel 宽容归一。返回重扫后的快照。
pub fn set_parent(
    root: &Path,
    node_id: &str,
    parent: Option<String>,
    mode: ScanMode,
    rel: Option<String>,
) -> Result<ChainSnapshot, String> {
    if !mode.is_dev() {
        return Err("仅开发模式可自由编辑链接（分析模式的链由 AI 按协议维护）".into());
    }
    // v2.1 模式强绑定
    check_mode(root, mode)?;
    let nodes_dir = root.join(".chain").join("nodes");
    if !is_safe_id(node_id) {
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
    // 开发模式宽松解析：无 frontmatter 时先补最小 frontmatter（唯一写路径共用原语）
    let (mut fm, body) = parse_lenient(&raw, node_id)?;

    let fields = UpdateFields {
        title: None,
        status: None,
        body: None,
        tags: None,
        evidence: None,
        parent: Some(parent),
        rel: Some(rel),
    };
    crate::model::node::apply_update(&mut fm, &fields).map_err(|e| format!("应用更新失败：{e}"))?;
    let new_content = frontmatter::serialize(&fm, &body).map_err(|e| format!("序列化失败：{e}"))?;
    atomic_write(&file, &new_content).map_err(|e| format!("写回失败：{e}"))?;

    walker::scan_chain_dir_mode(root, mode).map_err(|e| format!("重扫失败：{e}"))
}

/// 修改指定节点的字段并写回磁盘（GUI update_node 核心逻辑）。
/// v2.0：mode 区分——分析模式严格（body 不能为空）；开发模式宽松（无内容要求，
/// 连 frontmatter 都没有的 .md 也能改：先补一个最小 frontmatter 再应用字段）。
/// 写回走原子写（v2.8 起：GUI 写路径与 MCP 写路径同守门）。
pub fn update_node_fields(
    root: &Path,
    node_id: &str,
    fields: &UpdateFields,
    mode: ScanMode,
) -> Result<ChainSnapshot, String> {
    // v2.1 模式强绑定
    check_mode(root, mode)?;
    let node_path = root
        .join(".chain")
        .join("nodes")
        .join(format!("{node_id}.md"));

    if !node_path.exists() {
        return Err(format!("节点文件不存在：{}", node_path.display()));
    }

    // body 前置校验：分析模式下空 body 直接拒绝（在读文件之前失败，错误来源直观）
    if !mode.is_dev() {
        if let Some(b) = &fields.body {
            if b.trim().is_empty() {
                return Err("body 不能为空".into());
            }
        }
    }

    // 1. 读原文件
    let raw = std::fs::read_to_string(&node_path).map_err(|e| format!("读取失败：{}", e))?;

    // 2. 解析 frontmatter（开发模式：无 frontmatter 时兜底为最小 frontmatter）
    let (mut fm, body) = match frontmatter::parse(&raw) {
        Ok(result) => result,
        Err(_) if mode.is_dev() => parse_lenient(&raw, node_id)?,
        Err(e) => return Err(format!("解析 frontmatter 失败：{}", e)),
    };

    // 3. 应用 fields
    crate::model::node::apply_update(&mut fm, fields)
        .map_err(|e| format!("应用更新失败：{}", e))?;

    // 4. body 替换
    let new_body = match &fields.body {
        Some(b) => b.clone(),
        None => body,
    };

    // 5. 写回文件（原子写）
    let new_content =
        frontmatter::serialize(&fm, &new_body).map_err(|e| format!("序列化失败：{}", e))?;
    atomic_write(&node_path, &new_content).map_err(|e| format!("写回失败：{}", e))?;

    // 6. 按当前模式重扫整个 chain，返回新 snapshot
    walker::scan_chain_dir_mode(root, mode).map_err(|e| format!("重扫失败：{}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::node::NodeStatus;
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
        let snap = create_node(tmp.path(), &input("物理笔记"), ScanMode::Dev).unwrap();
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].id, "node-1");
        assert_eq!(snap.nodes[0].title, "物理笔记");
        assert_eq!(snap.nodes[0].parent, None);
        assert!(snap.validation.valid);
        // 第二个自动递增
        let snap2 = create_node(tmp.path(), &input("第二篇"), ScanMode::Dev).unwrap();
        assert_eq!(snap2.nodes.len(), 2);
        assert!(snap2.nodes.iter().any(|n| n.id == "node-2"));
    }

    #[test]
    fn test_create_node_with_parent_link() {
        let tmp = setup();
        create_node(tmp.path(), &input("根节点"), ScanMode::Dev).unwrap();
        let mut child = input("子节点");
        child.parent = Some("node-1".into());
        let snap = create_node(tmp.path(), &child, ScanMode::Dev).unwrap();
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].parent, "node-1");
        assert_eq!(snap.edges[0].child, "node-2");
    }

    #[test]
    fn test_create_node_rejects_analysis_mode() {
        let tmp = setup();
        let res = create_node(tmp.path(), &input("x"), ScanMode::Analysis);
        assert!(res.is_err(), "分析模式应拒绝自由新建节点: {res:?}");
    }

    #[test]
    fn test_create_node_rejects_bad_id() {
        let tmp = setup();
        let mut bad = input("x");
        bad.id = Some("../evil".into());
        assert!(
            create_node(tmp.path(), &bad, ScanMode::Dev).is_err(),
            "路径穿越 id 应被拒绝"
        );
    }

    #[test]
    fn test_delete_node() {
        let tmp = setup();
        create_node(tmp.path(), &input("要删的"), ScanMode::Dev).unwrap();
        let snap = delete_node(tmp.path(), "node-1", ScanMode::Dev).unwrap();
        assert_eq!(snap.nodes.len(), 0);
        assert!(!tmp.path().join(".chain/nodes/node-1.md").exists());
    }

    #[test]
    fn test_set_parent_connect_and_disconnect() {
        let tmp = setup();
        create_node(tmp.path(), &input("A"), ScanMode::Dev).unwrap();
        create_node(tmp.path(), &input("B"), ScanMode::Dev).unwrap();

        // 建立链接 B → A
        let snap = set_parent(
            tmp.path(),
            "node-2",
            Some("node-1".into()),
            ScanMode::Dev,
            None,
        )
        .unwrap();
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.edges[0].child, "node-2");

        // 断开链接
        let snap = set_parent(tmp.path(), "node-2", None, ScanMode::Dev, None).unwrap();
        assert_eq!(snap.edges.len(), 0);

        // 指向不存在的父节点应报错
        assert!(set_parent(
            tmp.path(),
            "node-2",
            Some("ghost".into()),
            ScanMode::Dev,
            None
        )
        .is_err());
    }

    fn setup_test_chain() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();

        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
        ).unwrap();

        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: [old]\n---\n\n# 设计1\n",
        ).unwrap();

        tmp
    }

    #[test]
    fn test_update_title() {
        let tmp = setup_test_chain();

        let fields = UpdateFields {
            title: Some("新的目标标题".into()),
            status: None,
            body: None,
            tags: None,
            evidence: None,
            parent: None,
            rel: None,
        };
        let snap = update_node_fields(tmp.path(), "g-001", &fields, ScanMode::Analysis).unwrap();

        let g = snap.nodes.iter().find(|n| n.id == "g-001").unwrap();
        assert_eq!(g.title, "新的目标标题");
        assert_eq!(g.revision, 2);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/g-001.md")).unwrap();
        assert!(raw.contains("title: 新的目标标题"));
        assert!(raw.contains("revision: 2"));
    }

    #[test]
    fn test_update_status() {
        let tmp = setup_test_chain();

        let fields = UpdateFields {
            title: None,
            status: Some(NodeStatus::InProgress),
            body: None,
            tags: None,
            evidence: None,
            parent: None,
            rel: None,
        };
        let snap = update_node_fields(tmp.path(), "g-001", &fields, ScanMode::Analysis).unwrap();

        let g = snap.nodes.iter().find(|n| n.id == "g-001").unwrap();
        assert_eq!(g.status, NodeStatus::InProgress);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/g-001.md")).unwrap();
        assert!(raw.contains("status: in_progress"));
    }

    #[test]
    fn test_update_tags() {
        let tmp = setup_test_chain();

        let fields = UpdateFields {
            title: None,
            status: None,
            body: None,
            tags: Some(vec!["new1".into(), "new2".into()]),
            evidence: None,
            parent: None,
            rel: None,
        };
        let snap = update_node_fields(tmp.path(), "d-001", &fields, ScanMode::Analysis).unwrap();

        let d = snap.nodes.iter().find(|n| n.id == "d-001").unwrap();
        assert_eq!(d.tags, vec!["new1", "new2"]);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/d-001.md")).unwrap();
        assert!(raw.contains("new1"));
        assert!(raw.contains("new2"));
    }

    #[test]
    fn test_update_node_not_found() {
        let tmp = setup_test_chain();

        let fields = UpdateFields {
            title: Some("不存在".into()),
            status: None,
            body: None,
            tags: None,
            evidence: None,
            parent: None,
            rel: None,
        };
        let result = update_node_fields(tmp.path(), "nonexistent", &fields, ScanMode::Analysis);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("节点文件不存在"));
    }

    #[test]
    fn test_update_body_empty() {
        let tmp = setup_test_chain();

        let fields = UpdateFields {
            title: None,
            status: None,
            body: Some("".into()),
            tags: None,
            evidence: None,
            parent: None,
            rel: None,
        };
        let result = update_node_fields(tmp.path(), "g-001", &fields, ScanMode::Analysis);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("body 不能为空"));
    }

    #[test]
    fn test_update_evidence() {
        let tmp = setup_test_chain();

        let fields = UpdateFields {
            title: None,
            status: None,
            body: None,
            tags: None,
            evidence: Some(vec!["artifacts/d-001/架构图.png".into()]),
            parent: None,
            rel: None,
        };
        let snap = update_node_fields(tmp.path(), "d-001", &fields, ScanMode::Analysis).unwrap();

        let d = snap.nodes.iter().find(|n| n.id == "d-001").unwrap();
        assert_eq!(d.evidence, vec!["artifacts/d-001/架构图.png"]);

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/d-001.md")).unwrap();
        assert!(raw.contains("evidence:"));
        assert!(raw.contains("架构图.png"));
    }

    #[test]
    fn test_update_writes_valid_rfc3339_updated() {
        // 防止 now_iso8601 回归成垃圾字符串（历史 bug：曾输出 "1970-01-01T00:00:00+00:00 +1723511234s"）
        let tmp = setup_test_chain();

        let fields = UpdateFields {
            title: Some("触发 updated 更新".into()),
            status: None,
            body: None,
            tags: None,
            evidence: None,
            parent: None,
            rel: None,
        };
        update_node_fields(tmp.path(), "g-001", &fields, ScanMode::Analysis).unwrap();

        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/g-001.md")).unwrap();
        let updated_line = raw
            .lines()
            .find(|l| l.trim_start().starts_with("updated:"))
            .expect("写回的文件必须有 updated 字段");
        let ts = updated_line
            .split_once(':')
            .unwrap()
            .1
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        // RFC3339（+08:00 本地时区）：YYYY-MM-DDTHH:MM:SS+08:00，25 字符
        assert_eq!(ts.len(), 25, "updated 必须是 RFC3339 格式，实际：{}", ts);
        assert_eq!(&ts[10..11], "T");
        assert!(
            ts.ends_with("+08:00"),
            "updated 必须以 +08:00 结尾，实际：{}",
            ts
        );
        let year: i32 = ts[0..4].parse().expect("年份必须是数字");
        assert!((2026..3000).contains(&year), "年份不合理：{}", year);
    }

    #[test]
    fn test_atomic_write_leaves_no_tmp_residue() {
        let tmp = setup();
        create_node(tmp.path(), &input("残tmp"), ScanMode::Dev).unwrap();
        update_node_fields(
            tmp.path(),
            "node-1",
            &UpdateFields {
                title: Some("改名".into()),
                status: None,
                body: None,
                tags: None,
                evidence: None,
                parent: None,
                rel: None,
            },
            ScanMode::Dev,
        )
        .unwrap();
        let residue: Vec<_> = fs::read_dir(tmp.path().join(".chain").join("nodes"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(residue.is_empty(), "不应残留 .tmp 文件");
    }
}
