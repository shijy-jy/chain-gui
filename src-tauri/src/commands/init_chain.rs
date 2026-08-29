use std::fs;
use std::path::PathBuf;
use tauri::command;
use crate::commands::ai_guide::{AI_GUIDE, AI_GUIDE_VERSION, parse_guide_version};
use crate::model::chain::ChainSnapshot;
use crate::scanner::frontmatter::now_iso8601;
use crate::scanner::walker::scan_chain_dir;

/// 在 dir 下初始化 chain 工程：建 .chain/nodes/ + 写一个示例 goal 节点
/// + 写入 .chain/AI_GUIDE.md（AI 使用指南），然后重扫返回。
/// 幂等：已有 g-001.md 时不覆盖。
/// v1.2：AI_GUIDE.md 改为版本对比——盘上无版本标记或版本 < 内嵌版本时刷新
/// （旧版指南会丢掉新增的守则/协议，必须更新）；同版或更新则保留（尊重用户批注）。
#[command]
pub fn init_chain(dir: String) -> Result<ChainSnapshot, String> {
    let root = PathBuf::from(&dir);
    let nodes_dir = root.join(".chain").join("nodes");
    fs::create_dir_all(&nodes_dir).map_err(|e| format!("创建 .chain/nodes 失败：{e}"))?;

    let example = nodes_dir.join("g-001.md");
    if !example.exists() {
        let now = now_iso8601();
        let content = format!(
            "---\nid: g-001\ntype: goal\nstatus: pending\ntitle: 示例目标（改我）\ncreated: {now}\nupdated: {now}\nrevision: 1\ntags: []\nparent: null\n---\n\n这是初始化向导生成的示例节点，在侧栏编辑或直接用编辑器改这个文件。\n"
        );
        fs::write(&example, content).map_err(|e| format!("写示例节点失败：{e}"))?;
    }

    // AI 使用指南：版本对比刷新（v1.2）
    refresh_ai_guide_if_stale(&root)?;

    scan_chain_dir(&root).map_err(|e| e.to_string())
}

/// 盘上 AI_GUIDE.md 无版本标记或版本低于内嵌版本时，用内嵌指南刷新。
/// 返回 (是否刷新, 盘上版本描述)。
pub fn refresh_ai_guide_if_stale(root: &std::path::Path) -> Result<(bool, String), String> {
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
            Ok((true, "unmarked->".to_string() + &AI_GUIDE_VERSION.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_init_chain_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap().to_string();

        let snap = init_chain(dir).unwrap();

        // .chain/nodes/g-001.md 存在
        assert!(tmp.path().join(".chain").join("nodes").join("g-001.md").exists());
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
        let dir = tmp.path().to_str().unwrap().to_string();

        // 第一次 init
        init_chain(dir.clone()).unwrap();

        // 改掉 g-001.md 的 title
        let node_path = tmp.path().join(".chain").join("nodes").join("g-001.md");
        let original = fs::read_to_string(&node_path).unwrap();
        let modified = original.replace("示例目标（改我）", "我改过了");
        fs::write(&node_path, modified).unwrap();

        // 第二次 init（节点幂等，不覆盖）
        init_chain(dir).unwrap();

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
        fs::write(&guide_path, "<!-- CHAIN_GUIDE_VERSION: 3 -->\n我的批注版").unwrap();

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
}
