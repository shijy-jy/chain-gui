//! 链折叠模块（v1.3）：将已完成子链折叠为摘要节点，压缩节点数量。
//! 参考 TencentDB Agent Memory 的"上下文卸载"理念：原始节点归档保留，
//! 摘要节点替换进活跃链，保持可追溯性。
//! 参考 MemGPT 的递归摘要：折叠时生成摘要正文，标注原始节点引用。
use std::fs;
use std::path::PathBuf;
use tauri::command;
use crate::model::chain::ChainSnapshot;
use crate::model::node::{FoldedInfo, Node, NodeStatus};
use crate::scanner::frontmatter::{now_iso8601, parse, serialize};
use crate::scanner::walker::scan_chain_dir;

const ARCHIVE_DIR: &str = "archive";

/// 折叠指定节点及其所有子孙节点为一个摘要节点。
/// 前提：子链中所有节点必须为 success 状态。
/// 折叠后：原始节点文件移至 .chain/archive/{fold_id}/，摘要节点原地替换。
#[command]
pub fn fold_chain(dir: String, node_id: String) -> Result<ChainSnapshot, String> {
    let root = PathBuf::from(&dir);
    let chain_dir = root.join(".chain");
    let nodes_dir = chain_dir.join("nodes");

    // 1. 扫描当前链，获取完整节点列表
    let snap = scan_chain_dir(&root).map_err(|e| format!("扫描失败：{e}"))?;

    // 2. 找到目标节点
    let target = snap.nodes.iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| format!("节点 {} 不存在", node_id))?;

    // 2b. 目标节点自身状态校验：failed/blocked 是协议红线（§4.4 失败定格保留），拒绝折叠。
    // 若允许折叠，第 7 步会把状态强改为 success，等于洗白失败记录。
    match target.status {
        NodeStatus::Failed => {
            return Err(format!(
                "节点 {} 状态为 failed，按协议必须定格保留，不能折叠。\n失败节点应派生子 goal 追查原因或修复后重验（见指南 §4.4）。",
                node_id
            ));
        }
        NodeStatus::Blocked => {
            return Err(format!(
                "节点 {} 状态为 blocked，不能折叠。\n先解除阻塞（改状态）或保留原状等待外部条件。",
                node_id
            ));
        }
        _ => {}
    }

    // 3. 收集子链中所有节点（BFS 沿 parent→child 边）
    let mut sub_chain_ids: Vec<String> = Vec::new();
    let mut queue: Vec<&str> = vec![&node_id];
    while let Some(current) = queue.pop() {
        sub_chain_ids.push(current.to_string());
        for edge in &snap.edges {
            if edge.parent == current {
                queue.push(&edge.child);
            }
        }
    }
    // 排除目标节点自身（它要被替换为摘要节点，不归档）
    let archive_ids: Vec<String> = sub_chain_ids.iter()
        .filter(|id| id.as_str() != node_id)
        .cloned()
        .collect();

    // 4. 校验：所有子节点必须是 success
    for id in &archive_ids {
        let node = snap.nodes.iter().find(|n| n.id == *id)
            .ok_or_else(|| format!("子节点 {} 不在图谱中", id))?;
        if node.status != NodeStatus::Success {
            return Err(format!(
                "节点 {} 状态为 {:?}，不是 success。折叠要求子链中所有节点均为 success。\n提示：失败节点应保留为历史记录，不应折叠。",
                id, node.status
            ));
        }
    }

    // 5. 生成摘要正文
    let summary = build_fold_summary(&snap, &target, &archive_ids);

    // 6. 归档原始节点（含目标节点自身——它的原始正文即将被摘要覆盖，必须备份）
    let fold_id = format!("fold_{}", node_id);
    let archive_dir = chain_dir.join(ARCHIVE_DIR).join(&fold_id);
    fs::create_dir_all(&archive_dir)
        .map_err(|e| format!("创建归档目录失败：{e}"))?;

    // 目标节点自身：复制原文件为 _self.md（rename 会破坏后续读写，用 copy 备份）
    let target_path = nodes_dir.join(format!("{}.md", node_id));
    let self_backup = archive_dir.join("_self.md");
    if target_path.exists() {
        fs::copy(&target_path, &self_backup)
            .map_err(|e| format!("备份目标节点失败：{e}"))?;
    }

    for id in &archive_ids {
        let src = nodes_dir.join(format!("{}.md", id));
        let dst = archive_dir.join(format!("{}.md", id));
        if src.exists() {
            fs::rename(&src, &dst)
                .map_err(|e| format!("归档节点 {} 失败：{}", id, e))?;
        }
    }

    // 7. 更新目标节点为摘要节点
    let raw = fs::read_to_string(&target_path)
        .map_err(|e| format!("读目标节点失败：{e}"))?;
    let (mut fm, _body) = parse(&raw)
        .map_err(|e| format!("解析目标节点 frontmatter 失败：{e}"))?;

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
    let folded_yaml = serde_yaml::to_value(&folded)
        .map_err(|e| format!("序列化 folded 信息失败：{e}"))?;
    fm.insert(
        serde_yaml::Value::String("folded".into()),
        folded_yaml,
    );
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

    let new_content = serialize(&fm, &summary)
        .map_err(|e| format!("序列化摘要节点失败：{e}"))?;
    fs::write(&target_path, new_content)
        .map_err(|e| format!("写摘要节点失败：{e}"))?;

    // 8. 重扫返回
    scan_chain_dir(&root).map_err(|e| format!("重扫失败：{e}"))
}

/// 生成折叠摘要正文，参考 TencentDB 的 JSONL 中间层 + MemGPT 的递归摘要
fn build_fold_summary(
    snap: &ChainSnapshot,
    target: &Node,
    archive_ids: &[String],
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("# {}（已折叠）", target.title));
    lines.push(String::new());
    lines.push(format!("> 折叠时间：{}", now_iso8601()));
    lines.push(format!("> 原始节点数：{}（含本节点共 {} 个）", archive_ids.len(), archive_ids.len() + 1));
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
            };
            // 取正文第一行作为摘要（UTF-8 安全截断，中文下 100 字节切中间会 panic）
            let first_line = node.body
                .lines()
                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .unwrap_or("(无正文)")
                .trim()
                .to_string();
            let truncated = crate::scanner::frontmatter::truncate_utf8(&first_line, 100);
            let summary = if truncated.len() < first_line.len() {
                format!("{}…", truncated)
            } else {
                truncated.to_string()
            };
            lines.push(format!("- {} **{}** `{}` → {}", type_label, node.title, id, summary));
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
        let dir = tmp.path().to_str().unwrap().to_string();

        let snap = fold_chain(dir.clone(), "d-001".into()).unwrap();

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
        let dir = tmp.path().to_str().unwrap().to_string();

        // 把 t-001 改成 pending
        let t_path = tmp.path().join(".chain/nodes/t-001.md");
        let raw = fs::read_to_string(&t_path).unwrap();
        let modified = raw.replace("status: success", "status: pending");
        fs::write(&t_path, modified).unwrap();

        let result = fold_chain(dir, "d-001".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("不是 success"));
    }

    #[test]
    fn test_fold_nonexistent_node() {
        let tmp = setup_deep_chain();
        let dir = tmp.path().to_str().unwrap().to_string();
        let result = fold_chain(dir, "x-999".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_fold_rejects_failed_target() {
        // 目标节点自身 failed：即使子节点全 success 也必须拒绝（§4.4 失败定格）
        let tmp = setup_deep_chain();
        let dir = tmp.path().to_str().unwrap().to_string();
        let d_path = tmp.path().join(".chain/nodes/d-001.md");
        let raw = fs::read_to_string(&d_path).unwrap();
        fs::write(&d_path, raw.replace("status: success", "status: failed")).unwrap();

        let result = fold_chain(dir, "d-001".into());
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
        let dir = tmp.path().to_str().unwrap().to_string();
        let d_path = tmp.path().join(".chain/nodes/d-001.md");
        let raw = fs::read_to_string(&d_path).unwrap();
        fs::write(&d_path, raw.replace("status: success", "status: blocked")).unwrap();

        let result = fold_chain(dir, "d-001".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("blocked"));
    }

    #[test]
    fn test_fold_backs_up_target_self() {
        let tmp = setup_deep_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        fold_chain(dir.clone(), "d-001".into()).unwrap();

        // 目标节点折叠前的原始内容必须备份在 archive/_self.md
        let self_backup = tmp.path().join(".chain/archive/fold_d-001/_self.md");
        assert!(self_backup.exists(), "_self.md 备份应存在");
        let content = fs::read_to_string(&self_backup).unwrap();
        assert!(content.contains("设计1正文"), "备份应含原始正文: {content}");
    }

    #[test]
    fn test_fold_utf8_long_first_line() {
        // 中文长正文（>100 字节首行）：摘要截断必须落在字符边界，不能 panic
        let tmp = TempDir::new().unwrap();
        let nodes_dir = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes_dir).unwrap();
        fs::write(
            nodes_dir.join("g-001.md"),
            "---\nid: g-001\ntype: goal\ntitle: 根目标\nparent: null\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 根目标\n",
        ).unwrap();
        fs::write(
            nodes_dir.join("d-001.md"),
            "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: success\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 设计1\n",
        ).unwrap();
        // 40 个汉字 = 120 字节首行，第 100 字节落在汉字中间
        fs::write(
            nodes_dir.join("t-001.md"),
            "---\nid: t-001\ntype: task\ntitle: 任务1\nparent: d-001\nstatus: success\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 任务1\n\n汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文\n",
        ).unwrap();

        let dir = tmp.path().to_str().unwrap().to_string();
        let snap = fold_chain(dir.clone(), "d-001".into()).unwrap();
        let d = snap.nodes.iter().find(|n| n.id == "d-001").unwrap();
        // 摘要行应存在且带省略号
        assert!(d.body.contains("汉字正文"), "摘要应含截断后的正文: {}", d.body);
        assert!(d.body.contains("…"), "超长首行应带省略号");
    }
}