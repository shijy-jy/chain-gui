//! 链快照模块（v1.3）：`.chain/logs/` 目录下的定时快照与 diff。
//! 快照记录完整链状态（节点+边+manifest），支持受控回溯。
use std::fs;
use std::path::PathBuf;
use tauri::command;
use crate::model::chain::{ChainSnapshot, SnapshotMeta};
use crate::scanner::frontmatter::now_iso8601;
use crate::scanner::walker::scan_chain_dir;

const LOGS_DIR: &str = "logs";
const INDEX_FILE: &str = "index.json";

fn logs_dir(dir: &str) -> PathBuf {
    PathBuf::from(dir).join(".chain").join(LOGS_DIR)
}

fn index_path(dir: &str) -> PathBuf {
    logs_dir(dir).join(INDEX_FILE)
}

/// 创建当前链状态的快照，保存为 `.chain/logs/{id}.json`，
/// 并更新 index.json。返回快照 id。
#[command]
pub fn snapshot_chain(dir: String, tag: String) -> Result<String, String> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Err("快照标签不能为空".into());
    }

    let root = PathBuf::from(&dir);
    let snap = scan_chain_dir(&root).map_err(|e| format!("扫描失败：{e}"))?;

    let logs = logs_dir(&dir);
    fs::create_dir_all(&logs).map_err(|e| format!("创建 logs 目录失败：{e}"))?;

    let ts = now_iso8601();
    // 同秒多次快照会碰撞覆盖：加毫秒保证唯一（now_iso8601 精度到秒）
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0);
    let id = format!(
        "snap_{}_{:03}",
        ts.replace(':', "").replace('-', "").replace('+', ""),
        millis
    );

    // 写快照 JSON
    let snap_path = logs.join(format!("{}.json", id));
    let snap_json = serde_json::to_string_pretty(&snap)
        .map_err(|e| format!("序列化快照失败：{e}"))?;
    fs::write(&snap_path, snap_json).map_err(|e| format!("写快照文件失败：{e}"))?;

    // 更新 index
    let meta = SnapshotMeta {
        id: id.clone(),
        tag: tag.to_string(),
        created_at: ts,
        node_count: snap.manifest.node_count,
        edge_count: snap.manifest.edge_count,
    };
    let mut index: Vec<SnapshotMeta> = if index_path(&dir).exists() {
        let raw = fs::read_to_string(index_path(&dir))
            .map_err(|e| format!("读 index 失败：{e}"))?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        Vec::new()
    };
    index.insert(0, meta); // 最新的在前
    let index_json = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化 index 失败：{e}"))?;
    fs::write(index_path(&dir), index_json)
        .map_err(|e| format!("写 index 失败：{e}"))?;

    Ok(id)
}

/// 列出所有快照元数据（按时间倒序）
#[command]
pub fn list_snapshots(dir: String) -> Result<Vec<SnapshotMeta>, String> {
    let ip = index_path(&dir);
    if !ip.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&ip).map_err(|e| format!("读 index 失败：{e}"))?;
    let index: Vec<SnapshotMeta> = serde_json::from_str(&raw)
        .map_err(|e| format!("解析 index 失败：{e}"))?;
    Ok(index)
}

/// 读取指定快照的完整链状态
#[command]
pub fn read_snapshot(dir: String, snap_id: String) -> Result<ChainSnapshot, String> {
    let snap_path = logs_dir(&dir).join(format!("{}.json", snap_id));
    if !snap_path.exists() {
        return Err(format!("快照 {} 不存在", snap_id));
    }
    let raw = fs::read_to_string(&snap_path)
        .map_err(|e| format!("读快照失败：{e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析快照失败：{e}"))
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

    #[test]
    fn test_snapshot_and_list() {
        let tmp = setup_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let id = snapshot_chain(dir.clone(), "手动快照".into()).unwrap();
        assert!(id.starts_with("snap_"), "快照 id 应以 snap_ 开头: {id}");

        let list = list_snapshots(dir.clone()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tag, "手动快照");
        assert_eq!(list[0].node_count, 1);

        // 快照文件存在
        let snap_file = logs_dir(&dir).join(format!("{}.json", id));
        assert!(snap_file.exists());
    }

    #[test]
    fn test_read_snapshot() {
        let tmp = setup_chain();
        let dir = tmp.path().to_str().unwrap().to_string();

        let id = snapshot_chain(dir.clone(), "测试".into()).unwrap();
        let snap = read_snapshot(dir.clone(), id).unwrap();
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].id, "g-001");
    }

    #[test]
    fn test_snapshot_empty_tag_rejected() {
        let tmp = setup_chain();
        let dir = tmp.path().to_str().unwrap().to_string();
        let result = snapshot_chain(dir, "   ".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_list_empty() {
        let tmp = TempDir::new().unwrap();
        let list = list_snapshots(tmp.path().to_str().unwrap().to_string()).unwrap();
        assert!(list.is_empty());
    }
}