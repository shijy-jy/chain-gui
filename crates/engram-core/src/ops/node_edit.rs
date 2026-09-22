//! 开发模式自由图谱编辑原语（v2.0，核心唯一写路径）：
//! - `create_node`：新建节点文件（id 可省略自动生成）
//! - `delete_node`：删除节点文件（孤立化其子节点——自由图谱不校验悬空）
//! - `set_parent`：建立/断开链接（改写子节点 frontmatter 的 parent）
//! - `update_node_fields`：GUI 侧字段更新（分析/开发双模式）
//!
//! 两条写入通道（唯一写路径都在这里，入口 crate 只做适配）：
//! - **MCP/AI 通道**（create_node / delete_node / set_parent）：仅开发模式；分析模式一律拒绝，
//!   链结构由 AI 按协议直接维护节点文件；
//! - **人用通道 v2.20**（create_node_human / delete_node_human / set_parent_human）：GUI 文件树模式
//!   专用，分析模式也允许人编辑结构，但 core 内守协议护栏（新建必挂已存在父节点、禁删根、
//!   禁删还有子节点的节点、改链接禁断根禁成环）。
//! `update_node_fields` 双模式通用（内容字段：标题/状态/正文/标签/证据）。
//! 所有写入都走 core 的原子写与统一解析原语（宪法第 5 条：入口不得直写文件）。

use crate::model::chain::ChainSnapshot;
use crate::model::{ScanMode, UpdateFields};
use crate::ops::{atomic_write, parse_lenient};
use crate::profile::DEV;
use crate::scanner::{frontmatter, walker};
use crate::workspace::check_mode;
use serde::Deserialize;
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

/// rel 归一（开发模式宽容：词表唯一数据源 profile::REL_TYPES，词表外 → contains 默认值）
fn normalize_rel(r: &Option<String>) -> &str {
    match r.as_deref() {
        Some(v) if crate::profile::REL_TYPES.contains(&v) => v,
        _ => "contains",
    }
}

/// id 安全校验：拒绝路径穿越与非法文件名字符（`\ / : * ? " < > |`、控制字符、`.` 开头、空）。
/// v2.13 放宽：开发模式节点 id = 文件名，中文/空格/「·」等合法文件名此前被 ASCII 白名单
/// 误杀——MCP 工具读不到中文 id 节点（learning/story/water 等知识库全线中招）。
/// 正确防线是**黑名单路径危险字符**，而不是白名单字符集。
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| !c.is_control() && !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
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
    create_node_inner(root, input, mode, false)
}

/// v2.20 人用通道（GUI 文件树模式）：人在分析模式也能新建节点，但守协议护栏——
/// 必须挂到已存在的父节点下（严格单根树：不允许新增根）、类型/状态限本模式词表。
/// MCP 工具不走这里（仍 create_node → 分析模式一律拒绝），工具契约与 AI 行为零变化。
pub fn create_node_human(
    root: &Path,
    input: &CreateNodeInput,
    mode: ScanMode,
) -> Result<ChainSnapshot, String> {
    create_node_inner(root, input, mode, true)
}

fn create_node_inner(
    root: &Path,
    input: &CreateNodeInput,
    mode: ScanMode,
    human: bool,
) -> Result<ChainSnapshot, String> {
    if !mode.is_dev() && !human {
        return Err("仅开发模式可自由新建节点（分析模式的链由 AI 按协议维护）".into());
    }
    // v2.1 模式强绑定
    check_mode(root, mode)?;
    let nodes_dir = root.join(".chain").join("nodes");
    if !nodes_dir.is_dir() {
        return Err("nodes 目录不存在，请先初始化".into());
    }

    // ── v2.20 人用护栏（分析模式）：父节点必填且必须已存在；类型/状态限协议词表 ──
    if !mode.is_dev() && human {
        let snap = walker::scan_chain_dir_mode(root, mode).map_err(|e| format!("重扫失败：{e}"))?;
        let parent = input
            .parent
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty());
        let Some(parent) = parent else {
            return Err(
                "分析模式新建必须挂在父节点下（严格单根树：不允许新增根节点；根 goal 由初始化创建）"
                    .into(),
            );
        };
        if !snap.nodes.iter().any(|n| n.id == parent) {
            return Err(format!("父节点 {parent} 不存在——分析模式只允许挂到已有节点下"));
        }
        if let Some(t) = input.node_type.as_deref() {
            if !crate::profile::ANALYSIS_TYPES.contains(&t) {
                return Err(format!(
                    "类型 {t} 不属于分析模式协议词表（goal / design / task / verification）"
                ));
            }
        }
        if let Some(s) = input.status.as_deref() {
            if !crate::profile::ANALYSIS_STATUSES.contains(&s) {
                return Err(format!(
                    "状态 {s} 不属于分析模式协议词表（pending / in_progress / success / failed / blocked）"
                ));
            }
        }
    }

    let id = match &input.id {
        Some(id) => {
            if !is_safe_id(id) {
                return Err("id 非法（不能为空、超 64 字符、含路径字符或非法文件名字符）".into());
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
    // 分析模式缺省走协议默认（task / pending），开发模式保持 note / none 中性默认
    let node_type = if mode.is_dev() {
        normalize_type(&input.node_type)
    } else {
        match input.node_type.as_deref() {
            Some(t) if crate::profile::ANALYSIS_TYPES.contains(&t) => t.to_string(),
            _ => "task".to_string(),
        }
    };
    let status = if mode.is_dev() {
        normalize_status(&input.status)
    } else {
        match input.status.as_deref() {
            Some(s) if crate::profile::ANALYSIS_STATUSES.contains(&s) => s.to_string(),
            _ => "pending".to_string(),
        }
    };
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
    delete_node_inner(root, node_id, mode, false)
}

/// v2.20 人用通道：人在分析模式也能删节点，但守协议护栏——不能删根（链必须保留唯一根）、
/// 不能删还有子节点的节点（否则留下悬空分支）。
pub fn delete_node_human(
    root: &Path,
    node_id: &str,
    mode: ScanMode,
) -> Result<ChainSnapshot, String> {
    delete_node_inner(root, node_id, mode, true)
}

fn delete_node_inner(
    root: &Path,
    node_id: &str,
    mode: ScanMode,
    human: bool,
) -> Result<ChainSnapshot, String> {
    if !mode.is_dev() && !human {
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

    // ── v2.20 人用护栏（分析模式）：保根 + 不留悬空 ──
    if !mode.is_dev() && human {
        let snap = walker::scan_chain_dir_mode(root, mode).map_err(|e| format!("重扫失败：{e}"))?;
        let Some(node) = snap.nodes.iter().find(|n| n.id == node_id) else {
            return Err(format!("节点 {node_id} 不在当前链里"));
        };
        if node.parent.is_none() {
            return Err("根节点不能删除（链协议要求保留唯一根 goal）".into());
        }
        let children: Vec<&str> = snap
            .nodes
            .iter()
            .filter(|n| n.parent.as_deref() == Some(node_id))
            .map(|n| n.id.as_str())
            .collect();
        if !children.is_empty() {
            return Err(format!(
                "该节点还有 {} 个子节点（{}）——先把子节点改挂到别处或删除，链不能出现悬空分支",
                children.len(),
                children.join("、")
            ));
        }
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
    set_parent_inner(root, node_id, parent, mode, rel, false)
}

/// v2.20 人用通道：人在分析模式也能改链接，但守协议护栏——必须挂到已存在的父节点
/// （不允许断开成根）、不允许成环（新父节点不能在本节点的子树里）。
pub fn set_parent_human(
    root: &Path,
    node_id: &str,
    parent: Option<String>,
    mode: ScanMode,
    rel: Option<String>,
) -> Result<ChainSnapshot, String> {
    set_parent_inner(root, node_id, parent, mode, rel, true)
}

fn set_parent_inner(
    root: &Path,
    node_id: &str,
    parent: Option<String>,
    mode: ScanMode,
    rel: Option<String>,
    human: bool,
) -> Result<ChainSnapshot, String> {
    if !mode.is_dev() && !human {
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

    // ── v2.20 人用护栏（分析模式）：不许断根、不许成环 ──
    if !mode.is_dev() && human {
        let snap = walker::scan_chain_dir_mode(root, mode).map_err(|e| format!("重扫失败：{e}"))?;
        let new_parent = parent
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string());
        let Some(new_parent) = new_parent else {
            return Err("分析模式不允许断开链接（链必须保持单根树；请改挂到其它节点）".into());
        };
        if new_parent == node_id {
            return Err("不能把节点挂到自己下面".into());
        }
        if !snap.nodes.iter().any(|n| n.id == new_parent) {
            return Err(format!("父节点 {new_parent} 不存在——只允许挂到已有节点下"));
        }
        // 环检测：从新父节点沿 parent 向上回溯，若遇到本节点 → 成环
        let mut cur = Some(new_parent.clone());
        let mut hops = 0usize;
        while let Some(c) = cur {
            if c == node_id {
                return Err(
                    "这会形成环（新父节点在本节点自己的子树里）——链协议禁止环".into(),
                );
            }
            hops += 1;
            if hops > snap.nodes.len() + 1 {
                break; // 现存数据本就有环：不无限循环，交给校验器报告
            }
            cur = snap
                .nodes
                .iter()
                .find(|n| n.id == c)
                .and_then(|n| n.parent.clone());
        }
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
    // 开发模式宽松解析：无 frontmatter 时先补最小 frontmatter（唯一写路径共用原语）；
    // 分析模式严格解析（文件畸形要报错而不是猜）
    let (mut fm, body) = match frontmatter::parse(&raw) {
        Ok(result) => result,
        Err(_) if mode.is_dev() => parse_lenient(&raw, node_id)?,
        Err(e) => return Err(format!("解析 frontmatter 失败：{e}")),
    };

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
    use crate::model::node::NodeType;
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

    // ── v2.20 人用通道（GUI 文件树模式）：分析模式结构编辑的护栏 ────────────────
    // 契约要点：MCP 走的 create_node/delete_node/set_parent 一律不变（分析模式拒绝），
    // 只有 GUI 的 *_human 三件套允许人在分析模式编辑结构，且必须守住链协议。

    fn write_analysis_node(tmp: &TempDir, id: &str, ntype: &str, parent: Option<&str>) {
        let parent_line = match parent {
            Some(p) => p.to_string(),
            None => "null".to_string(),
        };
        let content = format!(
            "---\nid: {id}\ntype: {ntype}\ntitle: {id} 标题\nparent: {parent_line}\nrel: contains\nstatus: pending\ncreated: 2026-01-01T00:00:00+08:00\nupdated: 2026-01-01T00:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {id} 标题\n\n正文\n"
        );
        fs::write(
            tmp.path().join(".chain").join("nodes").join(format!("{id}.md")),
            content,
        )
        .unwrap();
    }

    #[test]
    fn test_create_node_human_analysis_requires_existing_parent() {
        let tmp = setup();
        write_analysis_node(&tmp, "g-001", "goal", None);
        // 没有父节点 → 拒绝（严格单根树不许新增根）
        let err = create_node_human(tmp.path(), &input("新任务"), ScanMode::Analysis).unwrap_err();
        assert!(err.contains("父节点"), "应提示必须挂父节点: {err}");
        // 父节点不存在 → 拒绝
        let mut orphan = input("新任务");
        orphan.parent = Some("d-404".into());
        assert!(
            create_node_human(tmp.path(), &orphan, ScanMode::Analysis).is_err(),
            "父节点不存在应被拒绝"
        );
        // MCP 通道契约不变：分析模式仍然一律拒绝
        assert!(create_node(tmp.path(), &input("x"), ScanMode::Analysis).is_err());
    }

    #[test]
    fn test_create_node_human_analysis_ok_with_protocol_vocab() {
        let tmp = setup();
        write_analysis_node(&tmp, "g-001", "goal", None);
        write_analysis_node(&tmp, "d-001", "design", Some("g-001"));
        let mut req = input("新任务");
        req.id = Some("t-001".into());
        req.parent = Some("d-001".into());
        req.node_type = Some("task".into());
        req.status = Some("pending".into());
        let snap = create_node_human(tmp.path(), &req, ScanMode::Analysis).unwrap();
        let created = snap.nodes.iter().find(|n| n.id == "t-001").expect("新节点应进链");
        assert_eq!(created.parent.as_deref(), Some("d-001"));
        assert_eq!(created.node_type, NodeType::Task);
        assert_eq!(created.status, NodeStatus::Pending);
        assert_eq!(snap.edges.len(), 2, "新节点应带一条挂载边");
        assert!(
            snap.validation.valid,
            "护栏保证下不应产生结构错误: {:?}",
            snap.validation.errors
        );
    }

    #[test]
    fn test_create_node_human_analysis_rejects_dev_only_vocab() {
        let tmp = setup();
        write_analysis_node(&tmp, "g-001", "goal", None);
        let mut note = input("笔记");
        note.parent = Some("g-001".into());
        note.node_type = Some("note".into());
        let err = create_node_human(tmp.path(), &note, ScanMode::Analysis).unwrap_err();
        assert!(err.contains("词表"), "note 不属于分析词表: {err}");

        let mut none_status = input("任务");
        none_status.parent = Some("g-001".into());
        none_status.status = Some("none".into());
        let err2 = create_node_human(tmp.path(), &none_status, ScanMode::Analysis).unwrap_err();
        assert!(err2.contains("词表"), "none 不属于分析词表: {err2}");
    }

    #[test]
    fn test_delete_node_human_analysis_guards() {
        let tmp = setup();
        write_analysis_node(&tmp, "g-001", "goal", None);
        write_analysis_node(&tmp, "d-001", "design", Some("g-001"));
        write_analysis_node(&tmp, "t-001", "task", Some("d-001"));
        // 根不能删
        let err = delete_node_human(tmp.path(), "g-001", ScanMode::Analysis).unwrap_err();
        assert!(err.contains("根节点"), "根应拒绝删除: {err}");
        // 还有子节点不能删
        let err2 = delete_node_human(tmp.path(), "d-001", ScanMode::Analysis).unwrap_err();
        assert!(err2.contains("子节点"), "有子节点应拒绝删除: {err2}");
        // 叶子可以删
        let snap = delete_node_human(tmp.path(), "t-001", ScanMode::Analysis).unwrap();
        assert_eq!(snap.nodes.len(), 2);
        // MCP 通道契约不变
        assert!(delete_node(tmp.path(), "d-001", ScanMode::Analysis).is_err());
    }

    #[test]
    fn test_set_parent_human_analysis_guards_and_cycle() {
        let tmp = setup();
        write_analysis_node(&tmp, "g-001", "goal", None);
        write_analysis_node(&tmp, "d-001", "design", Some("g-001"));
        write_analysis_node(&tmp, "t-001", "task", Some("d-001"));
        // 不允许断开（会新增第二个根）
        let err = set_parent_human(tmp.path(), "t-001", None, ScanMode::Analysis, None).unwrap_err();
        assert!(err.contains("断开"), "断开链接应被拒绝: {err}");
        // 成环：把 d-001 挂到自己的子树 t-001 下
        let err2 = set_parent_human(
            tmp.path(),
            "d-001",
            Some("t-001".into()),
            ScanMode::Analysis,
            None,
        )
        .unwrap_err();
        assert!(err2.contains("环"), "成环应被拒绝: {err2}");
        // 父节点不存在
        assert!(
            set_parent_human(tmp.path(), "t-001", Some("nope".into()), ScanMode::Analysis, None)
                .is_err()
        );
        // 正常改挂：t-001 移到 g-001 下（rel=solves）
        let snap = set_parent_human(
            tmp.path(),
            "t-001",
            Some("g-001".into()),
            ScanMode::Analysis,
            Some("solves".into()),
        )
        .unwrap();
        let moved = snap.nodes.iter().find(|x| x.id == "t-001").unwrap();
        assert_eq!(moved.parent.as_deref(), Some("g-001"));
        assert_eq!(moved.rel.as_deref(), Some("solves"));
        assert!(snap.validation.valid, "合法改挂后校验应通过");
        // MCP 通道契约不变
        assert!(set_parent(tmp.path(), "t-001", Some("d-001".into()), ScanMode::Analysis, None).is_err());
    }

    #[test]
    fn test_create_node_human_dev_keeps_permissive_defaults() {
        let tmp = setup();
        let snap = create_node_human(tmp.path(), &input("自由笔记"), ScanMode::Dev).unwrap();
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].node_type, NodeType::Note, "开发模式仍默认中性 note");
        assert_eq!(snap.nodes[0].status, NodeStatus::None, "开发模式仍默认无状态");
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
        let mut bad2 = input("x");
        bad2.id = Some("a/b".into());
        assert!(
            create_node(tmp.path(), &bad2, ScanMode::Dev).is_err(),
            "含路径分隔符应被拒绝"
        );
    }

    #[test]
    fn test_is_safe_id_allows_unicode_and_spaces() {
        // v2.13 放宽：开发模式节点 id = 文件名，中文/空格合法
        assert!(is_safe_id("方案 · SIR重采样"));
        assert!(is_safe_id("从渲染一张图到实时路径追踪"));
        assert!(is_safe_id("node-1"));
        assert!(!is_safe_id(""), "空 id 拒绝");
        assert!(!is_safe_id("../evil"), "路径穿越拒绝");
        assert!(!is_safe_id("a/b"), "斜杠拒绝");
        assert!(!is_safe_id("a\\b"), "反斜杠拒绝");
        assert!(!is_safe_id("a:b"), "冒号拒绝");
        assert!(!is_safe_id(".hidden"), "点开头拒绝");
        assert!(!is_safe_id(&"x".repeat(65)), "超长拒绝");
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
