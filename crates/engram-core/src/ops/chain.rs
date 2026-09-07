//! 链级操作（核心纯逻辑）：初始化 / 过程日志 / 快照 / 折叠。
//! 与节点编辑原语（node_edit）共同构成唯一写路径。

use crate::guide::{parse_guide_version, AI_GUIDE, AI_GUIDE_DEV, AI_GUIDE_VERSION};
use crate::model::chain::{ChainSnapshot, SnapshotMeta};
use crate::model::node::{FoldedInfo, Node, NodeStatus};
use crate::model::ScanMode;
use crate::scanner::frontmatter::{now_iso8601, parse, serialize, truncate_utf8};
use crate::scanner::walker::{scan_chain_dir, scan_chain_dir_mode};
use crate::workspace::{check_mode, write_mode_tag};
use std::fs;
use std::path::{Path, PathBuf};

// ── 初始化 ─────────────────────────────────────────────────

/// 在 root 下初始化 chain 工程：建 .chain/nodes/ + 写一个示例 goal 节点
/// + 写入 .chain/AI_GUIDE.md（AI 使用指南），然后重扫返回。
///
/// 幂等：已有 g-001.md 时不覆盖。
///
/// v1.2：AI_GUIDE.md 改为版本对比——盘上无版本标记或版本 < 内嵌版本时刷新
///   （旧版指南会丢掉新增的守则/协议，必须更新）；同版或更新则保留（尊重用户批注）。
///
/// v2.0：开发模式下不写链协议 AI_GUIDE.md（自由知识图谱），写知识库指南（缺省才写）。
pub fn init_chain(root: &Path, mode: ScanMode) -> Result<ChainSnapshot, String> {
    // v2.1 模式强绑定：已有标签且与所选模式不符 → 拒绝初始化
    check_mode(root, mode)?;
    let nodes_dir = root.join(".chain").join("nodes");
    fs::create_dir_all(&nodes_dir).map_err(|e| format!("创建 .chain/nodes 失败：{e}"))?;
    // v2.1 写模式标签（.chain/.mode），随工程走
    write_mode_tag(root, mode)?;

    let example = nodes_dir.join("g-001.md");
    if !example.exists() {
        let now = now_iso8601();
        let content = format!(
            "---\nid: g-001\ntype: goal\nstatus: pending\ntitle: 示例目标（改我）\ncreated: {now}\nupdated: {now}\nrevision: 1\ntags: []\nparent: null\n---\n\n这是初始化向导生成的示例节点，在侧栏编辑或直接用编辑器改这个文件。\n"
        );
        fs::write(&example, content).map_err(|e| format!("写示例节点失败：{e}"))?;
    }

    // AI 使用指南：分析模式版本对比刷新（v1.2）；开发模式写知识库指南（v2.1，缺省才写、不刷新）
    if !mode.is_dev() {
        refresh_ai_guide_if_stale(root)?;
    } else {
        let guide = root.join(".chain").join("AI_GUIDE.md");
        if !guide.exists() {
            fs::write(&guide, AI_GUIDE_DEV)
                .map_err(|e| format!("写开发模式 AI_GUIDE.md 失败：{e}"))?;
        }
    }

    scan_chain_dir_mode(root, mode).map_err(|e| e.to_string())
}

/// 盘上 AI_GUIDE.md 无版本标记或版本低于内嵌版本时，用内嵌指南刷新。
/// 返回 (是否刷新, 盘上版本描述)。
pub fn refresh_ai_guide_if_stale(root: &Path) -> Result<(bool, String), String> {
    let guide = root.join(".chain").join("AI_GUIDE.md");
    if !guide.exists() {
        fs::write(&guide, AI_GUIDE).map_err(|e| format!("写 AI_GUIDE.md 失败：{e}"))?;
        return Ok((true, "absent".into()));
    }
    let existing = fs::read_to_string(&guide).map_err(|e| format!("读 AI_GUIDE.md 失败：{e}"))?;
    match parse_guide_version(&existing) {
        Some(v) if v >= AI_GUIDE_VERSION => Ok((false, format!("v{v}"))),
        Some(v) => {
            fs::write(&guide, AI_GUIDE).map_err(|e| format!("刷新 AI_GUIDE.md 失败：{e}"))?;
            Ok((true, format!("v{v}->v{AI_GUIDE_VERSION}")))
        }
        None => {
            // 无版本标记：视为旧版（v1.2 之前的指南无标记），刷新
            fs::write(&guide, AI_GUIDE).map_err(|e| format!("刷新 AI_GUIDE.md 失败：{e}"))?;
            Ok((
                true,
                "unmarked->".to_string() + &AI_GUIDE_VERSION.to_string(),
            ))
        }
    }
}

// ── 过程日志 ───────────────────────────────────────────────

const LOG_NAME: &str = "PROCESS_LOG.md";

fn log_path(dir: &Path) -> PathBuf {
    dir.join(".chain").join(LOG_NAME)
}

/// 追加一条带时间戳的过程日志。
/// 幂等追加，不覆盖已有内容；日志文件不存在时自动创建（带表头）。
pub fn append_log(dir: &Path, text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("日志内容不能为空".into());
    }

    let path = log_path(dir);
    let ts = now_iso8601();

    if !path.exists() {
        let header = "# 过程日志\n\n> 试错流水账：环境坑 / 失败尝试 / 关键转折。按时间倒序追加，一行一条。\n> 结论仍以图谱节点为准，这里的记录是给后来者的铺路石。\n\n".to_string();
        fs::write(&path, header).map_err(|e| format!("创建过程日志失败：{e}"))?;
    }

    let mut content = fs::read_to_string(&path).map_err(|e| format!("读过程日志失败：{e}"))?;
    // 单行日志（内部换行转为 "；"）
    let one_line = text.replace('\n', "；");
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&format!("- {} {}\n", ts, one_line));
    fs::write(&path, content).map_err(|e| format!("写过程日志失败：{e}"))?;

    Ok(ts)
}

/// 读取过程日志全文（不存在返回空字符串）
pub fn get_process_log(dir: &Path) -> Result<String, String> {
    let path = log_path(dir);
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(&path).map_err(|e| format!("读过程日志失败：{e}"))
}

// ── 快照 ───────────────────────────────────────────────────

const LOGS_DIR: &str = "logs";
const INDEX_FILE: &str = "index.json";

fn logs_dir(dir: &Path) -> PathBuf {
    dir.join(".chain").join(LOGS_DIR)
}

fn index_path(dir: &Path) -> PathBuf {
    logs_dir(dir).join(INDEX_FILE)
}

/// 创建当前链状态的快照，保存为 `.chain/logs/{id}.json`，
/// 并更新 index.json。返回快照 id。
pub fn snapshot_chain(dir: &Path, tag: &str) -> Result<String, String> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Err("快照标签不能为空".into());
    }

    let snap = scan_chain_dir(dir).map_err(|e| format!("扫描失败：{e}"))?;

    let logs = logs_dir(dir);
    fs::create_dir_all(&logs).map_err(|e| format!("创建 logs 目录失败：{e}"))?;

    let ts = now_iso8601();
    // 同秒多次快照会碰撞覆盖：加毫秒保证唯一（now_iso8601 精度到秒）
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0);
    let id = format!("snap_{}_{:03}", ts.replace([':', '-', '+'], ""), millis);

    // 写快照 JSON
    let snap_path = logs.join(format!("{}.json", id));
    let snap_json =
        serde_json::to_string_pretty(&snap).map_err(|e| format!("序列化快照失败：{e}"))?;
    fs::write(&snap_path, snap_json).map_err(|e| format!("写快照文件失败：{e}"))?;

    // 更新 index
    let meta = SnapshotMeta {
        id: id.clone(),
        tag: tag.to_string(),
        created_at: ts,
        node_count: snap.manifest.node_count,
        edge_count: snap.manifest.edge_count,
    };
    let mut index: Vec<SnapshotMeta> = if index_path(dir).exists() {
        let raw = fs::read_to_string(index_path(dir)).map_err(|e| format!("读 index 失败：{e}"))?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        Vec::new()
    };
    index.insert(0, meta); // 最新的在前
    let index_json =
        serde_json::to_string_pretty(&index).map_err(|e| format!("序列化 index 失败：{e}"))?;
    fs::write(index_path(dir), index_json).map_err(|e| format!("写 index 失败：{e}"))?;

    Ok(id)
}

/// 列出所有快照元数据（按时间倒序）
pub fn list_snapshots(dir: &Path) -> Result<Vec<SnapshotMeta>, String> {
    let ip = index_path(dir);
    if !ip.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&ip).map_err(|e| format!("读 index 失败：{e}"))?;
    let index: Vec<SnapshotMeta> =
        serde_json::from_str(&raw).map_err(|e| format!("解析 index 失败：{e}"))?;
    Ok(index)
}

/// 读取指定快照的完整链状态
pub fn read_snapshot(dir: &Path, snap_id: &str) -> Result<ChainSnapshot, String> {
    let snap_path = logs_dir(dir).join(format!("{snap_id}.json"));
    if !snap_path.exists() {
        return Err(format!("快照 {snap_id} 不存在"));
    }
    let raw = fs::read_to_string(&snap_path).map_err(|e| format!("读快照失败：{e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析快照失败：{e}"))
}

// ── 折叠 ───────────────────────────────────────────────────

const ARCHIVE_DIR: &str = "archive";

/// 折叠指定节点及其所有子孙节点为一个摘要节点。
/// 前提：子链中所有节点必须为 success 状态。
/// 折叠后：原始节点文件移至 .chain/archive/{fold_id}/，摘要节点原地替换。
/// v2.0：仅分析模式可用——开发模式是自由图谱（可成环、多根），不存在"子链"语义。
pub fn fold_chain(dir: &Path, node_id: &str, mode: ScanMode) -> Result<ChainSnapshot, String> {
    if mode.is_dev() {
        return Err("开发模式不支持折叠：自由图谱可成环、多根，没有「子链」语义（折叠是分析模式的链协议功能）".into());
    }
    // v2.1 模式强绑定
    check_mode(dir, mode)?;
    let chain_dir = dir.join(".chain");
    let nodes_dir = chain_dir.join("nodes");

    // 1. 扫描当前链，获取完整节点列表
    let snap = scan_chain_dir_mode(dir, mode).map_err(|e| format!("扫描失败：{e}"))?;

    // 2. 找到目标节点
    let target = snap
        .nodes
        .iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| format!("节点 {node_id} 不存在"))?;

    // 2b. 目标节点自身状态校验：failed/blocked 是协议红线（§4.4 失败定格保留），拒绝折叠。
    // 若允许折叠，第 7 步会把状态强改为 success，等于洗白失败记录。
    match target.status {
        NodeStatus::Failed => {
            return Err(format!(
                "节点 {node_id} 状态为 failed，按协议必须定格保留，不能折叠。\n失败节点应派生子 goal 追查原因或修复后重验（见指南 §4.4）。",
            ));
        }
        NodeStatus::Blocked => {
            return Err(format!(
                "节点 {node_id} 状态为 blocked，不能折叠。\n先解除阻塞（改状态）或保留原状等待外部条件。",
            ));
        }
        _ => {}
    }

    // 3. 收集子链中所有节点（BFS 沿 parent→child 边）
    let mut sub_chain_ids: Vec<String> = Vec::new();
    let mut queue: Vec<&str> = vec![node_id];
    while let Some(current) = queue.pop() {
        sub_chain_ids.push(current.to_string());
        for edge in &snap.edges {
            if edge.parent == current {
                queue.push(&edge.child);
            }
        }
    }
    // 排除目标节点自身（它要被替换为摘要节点，不归档）
    let archive_ids: Vec<String> = sub_chain_ids
        .iter()
        .filter(|id| id.as_str() != node_id)
        .cloned()
        .collect();

    // 4. 校验：所有子节点必须是 success
    for id in &archive_ids {
        let node = snap
            .nodes
            .iter()
            .find(|n| n.id == *id)
            .ok_or_else(|| format!("子节点 {id} 不在图谱中"))?;
        if node.status != NodeStatus::Success {
            return Err(format!(
                "节点 {} 状态为 {:?}，不是 success。折叠要求子链中所有节点均为 success。\n提示：失败节点应保留为历史记录，不应折叠。",
                id, node.status
            ));
        }
    }

    // 5. 生成摘要正文
    let summary = build_fold_summary(&snap, target, &archive_ids);

    // 6. 归档原始节点（含目标节点自身——它的原始正文即将被摘要覆盖，必须备份）
    let fold_id = format!("fold_{node_id}");
    let archive_dir = chain_dir.join(ARCHIVE_DIR).join(&fold_id);
    fs::create_dir_all(&archive_dir).map_err(|e| format!("创建归档目录失败：{e}"))?;

    // 目标节点自身：复制原文件为 _self.md（rename 会破坏后续读写，用 copy 备份）
    let target_path = nodes_dir.join(format!("{node_id}.md"));
    let self_backup = archive_dir.join("_self.md");
    if target_path.exists() {
        fs::copy(&target_path, &self_backup).map_err(|e| format!("备份目标节点失败：{e}"))?;
    }

    for id in &archive_ids {
        let src = nodes_dir.join(format!("{id}.md"));
        let dst = archive_dir.join(format!("{id}.md"));
        if src.exists() {
            fs::rename(&src, &dst).map_err(|e| format!("归档节点 {id} 失败：{e}"))?;
        }
    }

    // 7. 更新目标节点为摘要节点
    let raw = fs::read_to_string(&target_path).map_err(|e| format!("读目标节点失败：{e}"))?;
    let (mut fm, _body) = parse(&raw).map_err(|e| format!("解析目标节点 frontmatter 失败：{e}"))?;

    // 摘要节点状态：只有 success 是"折叠完成"的合理终态。
    // failed/blocked 已在前面拒绝；pending/in_progress/success 折叠后统一为 success。
    fm.insert(
        serde_yaml::Value::String("status".into()),
        serde_yaml::Value::String("success".into()),
    );
    // 添加 folded 标记
    let folded = FoldedInfo {
        original_nodes: archive_ids.clone(),
        folded_at: now_iso8601(),
        original_node_count: archive_ids.len() + 1, // +1 包含目标节点自身
    };
    let folded_yaml =
        serde_yaml::to_value(&folded).map_err(|e| format!("序列化 folded 信息失败：{e}"))?;
    fm.insert(serde_yaml::Value::String("folded".into()), folded_yaml);
    // 更新 revision
    let rev_key = serde_yaml::Value::String("revision".into());
    let new_rev = match fm.get(&rev_key) {
        Some(serde_yaml::Value::Number(n)) => n.as_u64().unwrap_or(0) + 1,
        _ => 1,
    };
    fm.insert(rev_key, serde_yaml::Value::Number(new_rev.into()));
    // 更新 updated
    let now = now_iso8601();
    fm.insert(
        serde_yaml::Value::String("updated".into()),
        serde_yaml::Value::String(now),
    );

    let new_content = serialize(&fm, &summary).map_err(|e| format!("序列化摘要节点失败：{e}"))?;
    fs::write(&target_path, new_content).map_err(|e| format!("写摘要节点失败：{e}"))?;

    // 8. 重扫返回
    scan_chain_dir_mode(dir, mode).map_err(|e| format!("重扫失败：{e}"))
}

/// 生成折叠摘要正文，参考 TencentDB 的 JSONL 中间层 + MemGPT 的递归摘要
fn build_fold_summary(snap: &ChainSnapshot, target: &Node, archive_ids: &[String]) -> String {
    let mut lines = Vec::new();
    lines.push(format!("# {}（已折叠）", target.title));
    lines.push(String::new());
    lines.push(format!("> 折叠时间：{}", now_iso8601()));
    lines.push(format!(
        "> 原始节点数：{}（含本节点共 {} 个）",
        archive_ids.len(),
        archive_ids.len() + 1
    ));
    lines.push(format!("> 归档位置：`.chain/archive/fold_{}/`", target.id));
    lines.push(String::new());

    lines.push("## 子链摘要".to_string());
    lines.push(String::new());

    for id in archive_ids {
        if let Some(node) = snap.nodes.iter().find(|n| n.id == *id) {
            let type_label = match node.node_type {
                crate::model::node::NodeType::Goal => "🎯",
                crate::model::node::NodeType::Design => "📐",
                crate::model::node::NodeType::Task => "🔧",
                crate::model::node::NodeType::Verification => "🔍",
                crate::model::node::NodeType::Note => "🧩",
            };
            // 取正文第一行作为摘要（UTF-8 安全截断，中文下 100 字节切中间会 panic）
            let first_line = node
                .body
                .lines()
                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .unwrap_or("(无正文)")
                .trim()
                .to_string();
            let truncated = truncate_utf8(&first_line, 100);
            let summary = if truncated.len() < first_line.len() {
                format!("{}…", truncated)
            } else {
                truncated.to_string()
            };
            lines.push(format!(
                "- {} **{}** `{}` → {}",
                type_label, node.title, id, summary
            ));
        }
    }

    lines.push(String::new());
    lines.push("## 原始内容".to_string());
    lines.push(String::new());
    lines.push(format!(
        "原始节点文件已归档至 `.chain/archive/fold_{}/`，保留完整历史记录。",
        target.id
    ));
    lines.push("可通过文件系统直接查看，或使用 `unfold` 命令恢复（未来版本）。".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_chain() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 测试目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 测试目标\n",
        ).unwrap();
        tmp
    }

    // ── 初始化 ──

    #[test]
    fn test_init_chain_creates_structure() {
        let tmp = TempDir::new().unwrap();

        let snap = init_chain(tmp.path(), ScanMode::Analysis).unwrap();

        // .chain/nodes/g-001.md 存在
        assert!(tmp
            .path()
            .join(".chain")
            .join("nodes")
            .join("g-001.md")
            .exists());
        // .chain/AI_GUIDE.md 存在且为内嵌指南全文
        let guide_path = tmp.path().join(".chain").join("AI_GUIDE.md");
        assert!(guide_path.exists(), "init 应生成 AI_GUIDE.md");
        let guide = fs::read_to_string(&guide_path).unwrap();
        assert!(guide.contains("Chain Protocol"), "AI_GUIDE.md 应为指南全文");
        assert_eq!(guide, AI_GUIDE, "写盘内容应与内嵌资源完全一致");
        // 返回 snapshot 含 1 节点
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].id, "g-001");
        // 校验通过
        assert!(snap.validation.valid);
    }

    #[test]
    fn test_init_chain_idempotent_for_node() {
        let tmp = TempDir::new().unwrap();

        // 第一次 init
        init_chain(tmp.path(), ScanMode::Analysis).unwrap();

        // 改掉 g-001.md 的 title
        let node_path = tmp.path().join(".chain").join("nodes").join("g-001.md");
        let original = fs::read_to_string(&node_path).unwrap();
        let modified = original.replace("示例目标（改我）", "我改过了");
        fs::write(&node_path, modified).unwrap();

        // 第二次 init（节点幂等，不覆盖）
        init_chain(tmp.path(), ScanMode::Analysis).unwrap();

        let after = fs::read_to_string(&node_path).unwrap();
        assert!(after.contains("我改过了"));
        assert!(!after.contains("示例目标（改我）"));
    }

    #[test]
    fn test_guide_refresh_unmarked() {
        // v1.2 之前盘上指南无版本标记 → 必须刷新为新版
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain")).unwrap();
        let guide_path = root.join(".chain").join("AI_GUIDE.md");
        fs::write(&guide_path, "旧版无标记指南，8 条守则").unwrap();

        let (refreshed, desc) = refresh_ai_guide_if_stale(root).unwrap();
        assert!(refreshed, "无标记指南应刷新: {desc}");
        let after = fs::read_to_string(&guide_path).unwrap();
        assert_eq!(after, AI_GUIDE);
    }

    #[test]
    fn test_guide_refresh_older_version() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain")).unwrap();
        let guide_path = root.join(".chain").join("AI_GUIDE.md");
        fs::write(&guide_path, "<!-- CHAIN_GUIDE_VERSION: 1 -->\n旧版 v1").unwrap();

        let (refreshed, desc) = refresh_ai_guide_if_stale(root).unwrap();
        assert!(refreshed, "旧版本应刷新: {desc}");
        let after = fs::read_to_string(&guide_path).unwrap();
        assert_eq!(after, AI_GUIDE);
    }

    #[test]
    fn test_guide_keep_same_version() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain")).unwrap();
        let guide_path = root.join(".chain").join("AI_GUIDE.md");
        // 与常量同版：用 AI_GUIDE_VERSION 动态生成，版本号 bump 时测试自动跟随
        fs::write(
            &guide_path,
            format!("<!-- CHAIN_GUIDE_VERSION: {AI_GUIDE_VERSION} -->\n我的批注版"),
        )
        .unwrap();

        let (refreshed, _) = refresh_ai_guide_if_stale(root).unwrap();
        assert!(!refreshed, "同版本不应刷新（保留用户批注）");
        let after = fs::read_to_string(&guide_path).unwrap();
        assert!(after.contains("我的批注版"));
    }

    #[test]
    fn test_guide_keep_newer_version() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain")).unwrap();
        let guide_path = root.join(".chain").join("AI_GUIDE.md");
        fs::write(&guide_path, "<!-- CHAIN_GUIDE_VERSION: 99 -->\n更新的版本").unwrap();

        let (refreshed, _) = refresh_ai_guide_if_stale(root).unwrap();
        assert!(!refreshed, "更新版本不应被旧软件降级");
    }

    #[test]
    fn test_guide_refresh_absent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".chain")).unwrap();

        let (refreshed, _) = refresh_ai_guide_if_stale(root).unwrap();
        assert!(refreshed);
        let guide_path = root.join(".chain").join("AI_GUIDE.md");
        assert!(guide_path.exists());
        assert_eq!(fs::read_to_string(&guide_path).unwrap(), AI_GUIDE);
    }

    // ── 过程日志 ──

    #[test]
    fn test_append_creates_log_with_header() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain")).unwrap();

        append_log(tmp.path(), "环境坑：测试一条").unwrap();

        let content = fs::read_to_string(log_path(tmp.path())).unwrap();
        assert!(content.contains("# 过程日志"));
        assert!(content.contains("环境坑：测试一条"));
        assert!(content.contains("+08:00"), "应有时间戳: {}", content);
    }

    #[test]
    fn test_append_multiple_lines() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain")).unwrap();

        append_log(tmp.path(), "第一条").unwrap();
        append_log(tmp.path(), "第二条").unwrap();
        append_log(tmp.path(), "多行\n内容\n合并").unwrap();

        let content = fs::read_to_string(log_path(tmp.path())).unwrap();
        assert!(content.contains("第一条"));
        assert!(content.contains("第二条"));
        assert!(
            content.contains("多行；内容；合并"),
            "多行应合并为单行: {}",
            content
        );
        // 三条日志行
        let log_lines = content.lines().filter(|l| l.starts_with("- ")).count();
        assert_eq!(log_lines, 3);
    }

    #[test]
    fn test_append_empty_rejected() {
        let tmp = TempDir::new().unwrap();
        let result = append_log(tmp.path(), "   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_log_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let content = get_process_log(tmp.path()).unwrap();
        assert_eq!(content, "");
    }

    // ── 快照 ──

    #[test]
    fn test_snapshot_and_list() {
        let tmp = setup_chain();

        let id = snapshot_chain(tmp.path(), "手动快照").unwrap();
        assert!(id.starts_with("snap_"), "快照 id 应以 snap_ 开头: {id}");

        let list = list_snapshots(tmp.path()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tag, "手动快照");
        assert_eq!(list[0].node_count, 1);

        // 快照文件存在
        let snap_file = logs_dir(tmp.path()).join(format!("{}.json", id));
        assert!(snap_file.exists());
    }

    #[test]
    fn test_read_snapshot() {
        let tmp = setup_chain();

        let id = snapshot_chain(tmp.path(), "测试").unwrap();
        let snap = read_snapshot(tmp.path(), &id).unwrap();
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].id, "g-001");
    }

    #[test]
    fn test_snapshot_empty_tag_rejected() {
        let tmp = setup_chain();
        let result = snapshot_chain(tmp.path(), "   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_empty() {
        let tmp = TempDir::new().unwrap();
        let list = list_snapshots(tmp.path()).unwrap();
        assert!(list.is_empty());
    }

    // ── 折叠 ──

    fn setup_deep_chain() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();

        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 根目标\nparent: null\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 根目标\n\n根目标正文。\n",
        ).unwrap();
        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: success\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 设计1\n\n设计1正文。\n",
        ).unwrap();
        fs::write(
            nodes_dir.join("t-001.md"),
            "---\nid: t-001\ntype: task\ntitle: 任务1\nparent: d-001\nstatus: success\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 任务1\n\n任务1正文。\n",
        ).unwrap();
        tmp
    }

    #[test]
    fn test_fold_sub_chain() {
        let tmp = setup_deep_chain();

        let snap = fold_chain(tmp.path(), "d-001", ScanMode::Analysis).unwrap();

        // d-001 应变为 success + 有 folded 标记
        let d = snap.nodes.iter().find(|n| n.id == "d-001").unwrap();
        assert_eq!(d.status, NodeStatus::Success);
        assert!(d.folded.is_some(), "d-001 应有 folded 标记");
        assert_eq!(d.folded.as_ref().unwrap().original_node_count, 2);

        // t-001 应已归档
        let t_path = tmp.path().join(".chain/nodes/t-001.md");
        assert!(!t_path.exists(), "t-001 应已从 nodes/ 移走");

        // 归档文件应存在
        let archive_path = tmp.path().join(".chain/archive/fold_d-001/t-001.md");
        assert!(archive_path.exists(), "t-001 应在归档目录");

        // 根节点不变
        assert!(snap.nodes.iter().any(|n| n.id == "g-001"));
    }

    #[test]
    fn test_fold_rejects_non_success() {
        let tmp = setup_deep_chain();

        // 把 t-001 改成 pending
        let t_path = tmp.path().join(".chain/nodes/t-001.md");
        let raw = fs::read_to_string(&t_path).unwrap();
        let modified = raw.replace("status: success", "status: pending");
        fs::write(&t_path, modified).unwrap();

        let result = fold_chain(tmp.path(), "d-001", ScanMode::Analysis);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("不是 success"));
    }

    #[test]
    fn test_fold_nonexistent_node() {
        let tmp = setup_deep_chain();
        let result = fold_chain(tmp.path(), "x-999", ScanMode::Analysis);
        assert!(result.is_err());
    }

    #[test]
    fn test_fold_rejects_failed_target() {
        // 目标节点自身 failed：即使子节点全 success 也必须拒绝（§4.4 失败定格）
        let tmp = setup_deep_chain();
        let d_path = tmp.path().join(".chain/nodes/d-001.md");
        let raw = fs::read_to_string(&d_path).unwrap();
        fs::write(&d_path, raw.replace("status: success", "status: failed")).unwrap();

        let result = fold_chain(tmp.path(), "d-001", ScanMode::Analysis);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("failed"), "应拒绝 failed 目标: {err}");
        // 节点未被改动
        let after = fs::read_to_string(&d_path).unwrap();
        assert!(after.contains("status: failed"), "failed 节点应保持原样");
    }

    #[test]
    fn test_fold_rejects_blocked_target() {
        let tmp = setup_deep_chain();
        let d_path = tmp.path().join(".chain/nodes/d-001.md");
        let raw = fs::read_to_string(&d_path).unwrap();
        fs::write(&d_path, raw.replace("status: success", "status: blocked")).unwrap();

        let result = fold_chain(tmp.path(), "d-001", ScanMode::Analysis);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("blocked"));
    }

    #[test]
    fn test_fold_backs_up_target_self() {
        let tmp = setup_deep_chain();
        let snap = fold_chain(tmp.path(), "d-001", ScanMode::Analysis).unwrap();
        assert!(snap.nodes.iter().any(|n| n.id == "d-001"));
        assert!(
            tmp.path()
                .join(".chain/archive/fold_d-001/_self.md")
                .exists(),
            "目标节点应有 _self.md 备份"
        );
    }
}
